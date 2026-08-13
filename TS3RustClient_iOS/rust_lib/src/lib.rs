//! ts3-rust — C FFI wrapper around [ReSpeak/tsclientlib](https://github.com/ReSpeak/tsclientlib).
//!
//! tsclientlib connections are not `Send`, so all connections live inside a
//! single worker thread that owns the Tokio runtime. The exported C functions
//! marshal commands to that thread and block for the reply.

use std::collections::HashMap;
use std::ffi::{c_char, c_int, CStr, CString};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use futures::StreamExt;
use tsclientlib::prelude::*;
use tsclientlib::{Connection, DisconnectOptions, Identity, MessageTarget, StreamItem};
use tsproto_packets::packets::{AudioData, CodecType, OutAudio};

// ============================================================================
// Logging
// ============================================================================

type LogCallback = extern "C" fn(*const c_char);

static LOG_CALLBACK: Mutex<Option<LogCallback>> = Mutex::new(None);
static LOGGER_INIT: Mutex<bool> = Mutex::new(false);

fn forward_log(line: &str) {
    let callback = *LOG_CALLBACK.lock().unwrap();
    if let Some(callback) = callback {
        if let Ok(c) = CString::new(line) {
            callback(c.as_ptr());
        }
    }
}

struct CallbackWriter;

impl std::io::Write for CallbackWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(s) = std::str::from_utf8(buf) {
            let trimmed = s.trim_end_matches('\n');
            if !trimmed.is_empty() {
                forward_log(trimmed);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct CallbackMakeWriter;

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CallbackMakeWriter {
    type Writer = CallbackWriter;

    fn make_writer(&self) -> Self::Writer {
        CallbackWriter
    }
}

/// Initialize the Rust logging system. Safe to call multiple times.
#[no_mangle]
pub extern "C" fn init_logger() {
    let mut guard = LOGGER_INIT.lock().unwrap();
    if *guard {
        return;
    }
    *guard = true;
    drop(guard);

    tracing_subscriber::fmt()
        .with_writer(CallbackMakeWriter)
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .ok();

    // Bridge `log`-crate messages (used by some dependencies) into tracing.
    let _ = tracing_log::LogTracer::init();
}

/// Register a callback that receives every Rust log line as a C string.
///
/// # Safety
/// `callback` must be a valid function pointer. It may be called from the
/// Rust worker thread; the Swift side must not touch the UI from it directly.
#[no_mangle]
pub unsafe extern "C" fn ts3_set_log_callback(callback: extern "C" fn(*const c_char)) {
    *LOG_CALLBACK.lock().unwrap() = Some(callback);
}

// ============================================================================
// Audio & talk callbacks
// ============================================================================

/// Receives decoded audio frames: (connection_id, user_id, codec, data, len).
/// `data` is only valid for the duration of the call.
type Ts3AudioCallback = extern "C" fn(u64, u16, u8, *const u8, usize);

/// Receives talk status changes: (connection_id, user_id, talking).
type Ts3TalkCallback = extern "C" fn(u64, u16, c_int);

static AUDIO_CALLBACK: Mutex<Option<Ts3AudioCallback>> = Mutex::new(None);
static TALK_CALLBACK: Mutex<Option<Ts3TalkCallback>> = Mutex::new(None);

/// Register the callback that receives incoming voice frames.
///
/// # Safety
/// The callback may be invoked from the Rust worker thread. `data`/`len` are
/// only valid during the call; copy them out before returning.
#[no_mangle]
pub unsafe extern "C" fn ts3_set_audio_callback(callback: Ts3AudioCallback) {
    *AUDIO_CALLBACK.lock().unwrap() = Some(callback);
}

/// Register the callback that receives talk status changes (1 = talking).
///
/// # Safety
/// The callback may be invoked from the Rust worker thread.
#[no_mangle]
pub unsafe extern "C" fn ts3_set_talk_callback(callback: Ts3TalkCallback) {
    *TALK_CALLBACK.lock().unwrap() = Some(callback);
}

fn forward_audio(connection_id: u64, user_id: u16, codec: u8, data: &[u8]) {
    let callback = *AUDIO_CALLBACK.lock().unwrap();
    if let Some(callback) = callback {
        callback(connection_id, user_id, codec, data.as_ptr(), data.len());
    }
}

fn forward_talk(connection_id: u64, user_id: u16, talking: c_int) {
    let callback = *TALK_CALLBACK.lock().unwrap();
    if let Some(callback) = callback {
        callback(connection_id, user_id, talking);
    }
}

// ============================================================================
// Worker thread
// ============================================================================

enum Command {
    Connect {
        server: String,
        port: u16,
        nickname: String,
        password: Option<String>,
        reply: mpsc::Sender<Result<u64, String>>,
    },
    Disconnect {
        id: u64,
        reply: mpsc::Sender<Result<(), String>>,
    },
    SendMessage {
        id: u64,
        message: String,
        reply: mpsc::Sender<Result<(), String>>,
    },
    SendAudio {
        id: u64,
        data: Vec<u8>,
        codec: u8,
    },
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn worker(rx: mpsc::Receiver<Command>) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            forward_log(&format!("[ts3-rust] failed to create runtime: {e}"));
            return;
        }
    };

    runtime.block_on(async move {
        let mut connections: HashMap<u64, Connection> = HashMap::new();
        let mut audio_sequences: HashMap<u64, u16> = HashMap::new();
        let mut talking: HashMap<(u64, u16), Instant> = HashMap::new();

        loop {
            // Drive all connections so tsclientlib can process packets.
            let ids: Vec<u64> = connections.keys().copied().collect();
            for id in ids {
                if let Some(con) = connections.get_mut(&id) {
                    if let Ok(Some(item)) =
                        tokio::time::timeout(Duration::from_millis(1), con.events().next()).await
                    {
                        if let Ok(item) = item {
                            process_item(id, item, &mut talking);
                        }
                    }
                }
            }

            // Emit talk-stop for users who went silent.
            let now = Instant::now();
            talking.retain(|&key, last| {
                if now.duration_since(*last) > Duration::from_millis(300) {
                    forward_talk(key.0, key.1, 0);
                    false
                } else {
                    true
                }
            });

            // Handle pending commands.
            match rx.try_recv() {
                Ok(cmd) => handle_command(cmd, &mut connections, &mut audio_sequences).await,
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => break,
            }

            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        forward_log("[ts3-rust] worker stopped");
    });
}

async fn handle_command(
    cmd: Command,
    connections: &mut HashMap<u64, Connection>,
    audio_sequences: &mut HashMap<u64, u16>,
) {
    match cmd {
        Command::Connect {
            server,
            port,
            nickname,
            password,
            reply,
        } => {
            let address = format!("{server}:{port}");
            let mut options = Connection::build(address)
                .name(nickname)
                .identity(Identity::create());
            if let Some(password) = password {
                options = options.password(password);
            }

            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            match options.connect() {
                Ok(mut con) => {
                    // Wait for the first BookEvents, which means we are connected.
                    let connected = tokio::time::timeout(Duration::from_secs(15), async {
                        loop {
                            match con.events().next().await {
                                Some(Ok(StreamItem::BookEvents(_))) => break true,
                                Some(Ok(_)) => continue,
                                Some(Err(_)) | None => break false,
                            }
                        }
                    })
                    .await;

                    match connected {
                        Ok(true) => {
                            connections.insert(id, con);
                            let _ = reply.send(Ok(id));
                        }
                        Ok(false) => {
                            let _ = reply.send(Err("服务器拒绝了连接或连接已关闭".to_string()));
                        }
                        Err(_) => {
                            let _ = reply.send(Err("连接超时".to_string()));
                        }
                    }
                }
                Err(e) => {
                    let _ = reply.send(Err(e.to_string()));
                }
            }
        }

        Command::Disconnect { id, reply } => {
            let result = match connections.get_mut(&id) {
                Some(con) => con
                    .disconnect(DisconnectOptions::new().message("leaving"))
                    .map_err(|e| e.to_string()),
                None => Err("连接不存在".to_string()),
            };
            let _ = reply.send(result);
            connections.remove(&id);
        }

        Command::SendMessage { id, message, reply } => {
            let result = (|| -> Result<(), String> {
                let con = connections
                    .get_mut(&id)
                    .ok_or_else(|| "连接不存在".to_string())?;
                let state = con.get_state().map_err(|e| e.to_string())?;
                state
                    .send_message(MessageTarget::Channel, &message)
                    .send(con)
                    .map_err(|e| e.to_string())
            })();
            let _ = reply.send(result);
        }

        Command::SendAudio { id, data, codec } => {
            if let Some(con) = connections.get_mut(&id) {
                let seq = audio_sequences.entry(id).or_insert(0);
                let codec = match codec {
                    4 => CodecType::OpusVoice,
                    5 => CodecType::OpusMusic,
                    _ => CodecType::OpusVoice,
                };
                let audio_data = AudioData::C2S {
                    id: *seq,
                    codec,
                    data: &data,
                };
                *seq = seq.wrapping_add(1);
                let packet = OutAudio::new(&audio_data);
                let _ = con.send_audio(packet);
            }
        }
    }
}

fn process_item(
    connection_id: u64,
    item: StreamItem,
    talking: &mut HashMap<(u64, u16), Instant>,
) {
    match item {
        StreamItem::Audio(audio) => {
            let audio_data = audio.data().data();
            match audio_data {
                AudioData::S2C { from, codec, data, .. }
                | AudioData::S2CWhisper { from, codec, data, .. } => {
                    let key = (connection_id, *from);
                    if !talking.contains_key(&key) {
                        forward_talk(connection_id, *from, 1);
                    }
                    talking.insert(key, Instant::now());
                    forward_audio(connection_id, *from, *codec as u8, data);
                }
                _ => {}
            }
        }
        _ => {}
    }
}

static WORKER: OnceLock<mpsc::Sender<Command>> = OnceLock::new();

fn worker_tx() -> &'static mpsc::Sender<Command> {
    WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Command>();
        std::thread::Builder::new()
            .name("ts3-worker".to_string())
            .spawn(move || worker(rx))
            .expect("failed to spawn ts3 worker thread");
        tx
    })
}

// ============================================================================
// C API
// ============================================================================

/// Connect to a TeamSpeak 3 server and return a connection id.
///
/// Returns `0` on failure (invalid arguments, timeout, or server rejection).
/// Blocks until connected (up to ~15s).
///
/// # Safety
/// All string parameters must be valid null-terminated UTF-8 strings; the
/// optional `password` may be NULL.
#[no_mangle]
pub unsafe extern "C" fn ts3_connect(
    server: *const c_char,
    port: u16,
    nickname: *const c_char,
    password: *const c_char,
) -> u64 {
    if server.is_null() || nickname.is_null() {
        return 0;
    }

    let server = match CStr::from_ptr(server).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return 0,
    };
    let nickname = match CStr::from_ptr(nickname).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return 0,
    };
    let password = if password.is_null() {
        None
    } else {
        CStr::from_ptr(password).to_str().ok().map(|s| s.to_string())
    };

    let (tx, rx) = mpsc::channel();
    if worker_tx()
        .send(Command::Connect {
            server,
            port,
            nickname,
            password,
            reply: tx,
        })
        .is_err()
    {
        return 0;
    }

    match rx.recv_timeout(Duration::from_secs(20)) {
        Ok(Ok(id)) => id,
        _ => 0,
    }
}

/// Disconnect from the server and release the connection.
#[no_mangle]
pub unsafe extern "C" fn ts3_disconnect(connection_id: u64) {
    let (tx, rx) = mpsc::channel();
    if worker_tx()
        .send(Command::Disconnect {
            id: connection_id,
            reply: tx,
        })
        .is_ok()
    {
        let _ = rx.recv_timeout(Duration::from_secs(2));
    }
}

/// Send a text message to the current channel.
///
/// Returns `0` on success, `-1` on failure.
///
/// # Safety
/// `message` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn ts3_send_message(connection_id: u64, message: *const c_char) -> i32 {
    if message.is_null() {
        return -1;
    }
    let message = match CStr::from_ptr(message).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };

    let (tx, rx) = mpsc::channel();
    if worker_tx()
        .send(Command::SendMessage {
            id: connection_id,
            message,
            reply: tx,
        })
        .is_err()
    {
        return -1;
    }

    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => 0,
        _ => -1,
    }
}

/// Send one encoded audio frame to the server (non-blocking, fire-and-forget).
///
/// `codec` uses TeamSpeak protocol ids: 4 = Opus voice, 5 = Opus music.
/// Returns `0` on accepted, `-1` on invalid input or no worker.
///
/// # Safety
/// `data` must point to `len` valid bytes.
#[no_mangle]
pub unsafe extern "C" fn ts3_send_audio(
    connection_id: u64,
    data: *const u8,
    len: usize,
    codec: c_int,
) -> i32 {
    if data.is_null() {
        return -1;
    }
    let bytes = std::slice::from_raw_parts(data, len).to_vec();
    if worker_tx()
        .send(Command::SendAudio {
            id: connection_id,
            data: bytes,
            codec: codec as u8,
        })
        .is_err()
    {
        return -1;
    }
    0
}

// ============================================================================
// Opus codec (for Swift-side capture/playback)
// ============================================================================

/// Opaque handle to an Opus encoder/decoder pair.
#[repr(C)]
pub struct Ts3Opus {
    _private: [u8; 0],
}

struct InnerOpus {
    encoder: opus::Encoder,
    decoder: opus::Decoder,
    channels: usize,
    frame_size: usize,
}

/// Create an Opus encoder/decoder pair. Free with `ts3_opus_destroy`.
#[no_mangle]
pub unsafe extern "C" fn ts3_opus_create(
    sample_rate: c_int,
    channels: c_int,
    bitrate: c_int,
    frame_size_ms: c_int,
) -> *mut Ts3Opus {
    let sample_rate = if sample_rate <= 0 { 48000 } else { sample_rate as u32 };
    let channels = if channels <= 0 { 1 } else { channels };
    let bitrate = if bitrate <= 0 { 48000 } else { bitrate };
    let frame_size_ms = if frame_size_ms <= 0 { 20 } else { frame_size_ms };
    let frame_size = (sample_rate as usize * frame_size_ms as usize) / 1000;

    let opus_channels = match channels {
        1 => opus::Channels::Mono,
        2 => opus::Channels::Stereo,
        _ => return std::ptr::null_mut(),
    };

    let mut encoder = match opus::Encoder::new(sample_rate, opus_channels, opus::Application::Voip)
    {
        Ok(e) => e,
        Err(_) => return std::ptr::null_mut(),
    };
    let _ = encoder.set_bitrate(opus::Bitrate::Bits(bitrate));
    let _ = encoder.set_inband_fec(true);
    let _ = encoder.set_packet_loss_perc(5);

    let decoder = match opus::Decoder::new(sample_rate, opus_channels) {
        Ok(d) => d,
        Err(_) => return std::ptr::null_mut(),
    };

    Box::into_raw(Box::new(InnerOpus {
        encoder,
        decoder,
        channels: channels as usize,
        frame_size,
    })) as *mut Ts3Opus
}

/// Encode one frame of 16-bit PCM into Opus.
///
/// Returns the number of bytes written, or a negative error code.
///
/// # Safety
/// - `pcm` must point to `pcm_len` valid i16 samples (at least one frame).
/// - `out` must point to `out_cap` valid bytes.
#[no_mangle]
pub unsafe extern "C" fn ts3_opus_encode(
    codec: *mut Ts3Opus,
    pcm: *const i16,
    pcm_len: usize,
    out: *mut u8,
    out_cap: usize,
) -> c_int {
    if codec.is_null() || pcm.is_null() || out.is_null() {
        return -1;
    }
    let inner = &mut *(codec as *mut InnerOpus);
    let samples = std::slice::from_raw_parts(pcm, pcm_len);
    if samples.len() < inner.frame_size * inner.channels {
        return -1;
    }
    let output = std::slice::from_raw_parts_mut(out, out_cap);
    match inner
        .encoder
        .encode(&samples[..inner.frame_size * inner.channels], output)
    {
        Ok(len) => len as c_int,
        Err(_) => -1,
    }
}

/// Decode one Opus packet into 16-bit PCM samples.
///
/// Returns the number of samples written, or a negative error code.
///
/// # Safety
/// - `data` must point to `len` valid bytes.
/// - `out` must point to `out_cap` valid i16 samples.
#[no_mangle]
pub unsafe extern "C" fn ts3_opus_decode(
    codec: *mut Ts3Opus,
    data: *const u8,
    len: usize,
    out: *mut i16,
    out_cap: usize,
) -> c_int {
    if codec.is_null() || data.is_null() || out.is_null() {
        return -1;
    }
    let inner = &mut *(codec as *mut InnerOpus);
    let input = std::slice::from_raw_parts(data, len);
    let output = std::slice::from_raw_parts_mut(out, out_cap);
    match inner.decoder.decode(input, output, false) {
        Ok(samples) => (samples * inner.channels) as c_int,
        Err(_) => -1,
    }
}

/// Free an Opus codec pair.
///
/// # Safety
/// `codec` must be a valid pointer from `ts3_opus_create`.
#[no_mangle]
pub unsafe extern "C" fn ts3_opus_destroy(codec: *mut Ts3Opus) {
    if !codec.is_null() {
        drop(Box::from_raw(codec as *mut InnerOpus));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn log_callback_forwarding() {
        static CALLED: AtomicBool = AtomicBool::new(false);

        extern "C" fn callback(_message: *const c_char) {
            CALLED.store(true, Ordering::SeqCst);
        }

        unsafe {
            ts3_set_log_callback(callback);
        }
        forward_log("hello from test");
        assert!(CALLED.load(Ordering::SeqCst));
    }

    #[test]
    fn opus_roundtrip_via_ffi() {
        unsafe {
            let codec = ts3_opus_create(48000, 1, 48000, 20);
            assert!(!codec.is_null());

            let pcm: Vec<i16> = (0..960).map(|i| ((i % 1000) - 500) as i16).collect();
            let mut encoded = vec![0u8; 2048];
            let n = ts3_opus_encode(
                codec,
                pcm.as_ptr(),
                pcm.len(),
                encoded.as_mut_ptr(),
                encoded.len(),
            );
            assert!(n > 0, "encode failed: {n}");

            let mut decoded = vec![0i16; 960];
            let m = ts3_opus_decode(
                codec,
                encoded.as_ptr(),
                n as usize,
                decoded.as_mut_ptr(),
                decoded.len(),
            );
            assert!(m > 0, "decode failed: {m}");

            ts3_opus_destroy(codec);
        }
    }
}
