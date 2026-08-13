//! C FFI bindings of tslib for embedding into iOS apps (Swift interop).
//!
//! The underlying [`Client`] is not `Send`/`Sync`, so each client handle owns
//! a dedicated worker thread with its own Tokio runtime. Events are converted
//! to JSON strings and pushed into a queue that Swift polls.

use std::collections::VecDeque;
use std::ffi::{c_char, c_int, c_uchar, CStr, CString};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use tslib_core::state::Channel;
use tslib_core::events::MessageTarget;
use tslib_core::{AudioCodec, Client, ClientConfig, ConnectionState, Event, Identity};

// ============================================================================
// Error codes & opaque handles
// ============================================================================

/// Error codes returned by the C API.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsLibError {
    Ok = 0,
    InvalidArgument = 1,
    ConnectionFailed = 2,
    NotConnected = 3,
    Timeout = 4,
    PermissionDenied = 5,
    ChannelError = 6,
    IdentityError = 7,
    InternalError = 99,
}

/// Opaque handle to a TeamSpeak identity.
#[repr(C)]
pub struct TsLibIdentity {
    _private: [u8; 0],
}

/// Opaque handle to a TeamSpeak client connection.
#[repr(C)]
pub struct TsClient {
    _private: [u8; 0],
}

/// Opaque handle to an Opus encoder/decoder pair.
#[repr(C)]
pub struct TsOpus {
    _private: [u8; 0],
}

// ============================================================================
// Identity
// ============================================================================

/// Create a new identity.
///
/// # Safety
/// Returns a pointer that must be freed with `tslib_identity_free`.
#[no_mangle]
pub unsafe extern "C" fn tslib_identity_create() -> *mut TsLibIdentity {
    match Identity::create() {
        Ok(identity) => Box::into_raw(Box::new(identity)) as *mut TsLibIdentity,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Load an identity from a file.
///
/// # Safety
/// - `path` must be a valid null-terminated UTF-8 string.
/// - Returns a pointer that must be freed with `tslib_identity_free`.
#[no_mangle]
pub unsafe extern "C" fn tslib_identity_load(path: *const c_char) -> *mut TsLibIdentity {
    if path.is_null() {
        return std::ptr::null_mut();
    }
    let path = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    match Identity::load(path) {
        Ok(identity) => Box::into_raw(Box::new(identity)) as *mut TsLibIdentity,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Import an identity from its exported string form.
///
/// # Safety
/// - `data` must be a valid null-terminated UTF-8 string.
/// - Returns a pointer that must be freed with `tslib_identity_free`.
#[no_mangle]
pub unsafe extern "C" fn tslib_identity_import_string(data: *const c_char) -> *mut TsLibIdentity {
    if data.is_null() {
        return std::ptr::null_mut();
    }
    let data = match CStr::from_ptr(data).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    match Identity::from_string(data) {
        Ok(identity) => Box::into_raw(Box::new(identity)) as *mut TsLibIdentity,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Export an identity to its string form.
///
/// # Safety
/// - `identity` must be a valid pointer from `tslib_identity_create` or load.
/// - Returns a string that must be freed with `tslib_string_free`.
#[no_mangle]
pub unsafe extern "C" fn tslib_identity_export_string(
    identity: *const TsLibIdentity,
) -> *mut c_char {
    if identity.is_null() {
        return std::ptr::null_mut();
    }
    let identity = &*(identity as *const Identity);
    match identity.export_string() {
        Ok(s) => CString::new(s)
            .map(|c| c.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Save an identity to a file.
///
/// # Safety
/// - `identity` must be a valid pointer from `tslib_identity_create` or load.
/// - `path` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn tslib_identity_save(
    identity: *const TsLibIdentity,
    path: *const c_char,
) -> TsLibError {
    if identity.is_null() || path.is_null() {
        return TsLibError::InvalidArgument;
    }
    let identity = &*(identity as *const Identity);
    let path = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return TsLibError::InvalidArgument,
    };
    match identity.save(path) {
        Ok(()) => TsLibError::Ok,
        Err(_) => TsLibError::IdentityError,
    }
}

/// Get the unique ID of an identity.
///
/// # Safety
/// - `identity` must be a valid pointer.
/// - Returns a string that must be freed with `tslib_string_free`.
#[no_mangle]
pub unsafe extern "C" fn tslib_identity_unique_id(
    identity: *const TsLibIdentity,
) -> *mut c_char {
    if identity.is_null() {
        return std::ptr::null_mut();
    }
    let identity = &*(identity as *const Identity);
    match CString::new(identity.unique_id()) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get the security level of an identity.
#[no_mangle]
pub unsafe extern "C" fn tslib_identity_security_level(
    identity: *const TsLibIdentity,
) -> c_int {
    if identity.is_null() {
        return -1;
    }
    let identity = &*(identity as *const Identity);
    identity.security_level() as c_int
}

/// Free an identity.
///
/// # Safety
/// `identity` must be a valid pointer from `tslib_identity_create` or load.
#[no_mangle]
pub unsafe extern "C" fn tslib_identity_free(identity: *mut TsLibIdentity) {
    if !identity.is_null() {
        drop(Box::from_raw(identity as *mut Identity));
    }
}

// ============================================================================
// Client
// ============================================================================

/// Commands marshalled to the worker thread.
enum ClientCmd {
    SendAudio {
        data: Vec<u8>,
        codec: u8,
        reply: mpsc::Sender<Result<(), String>>,
    },
    SendAudioAsync {
        data: Vec<u8>,
        codec: u8,
    },
    SetInputMuted {
        muted: bool,
        reply: mpsc::Sender<Result<(), String>>,
    },
    MoveToChannel {
        channel_id: u64,
        password: Option<String>,
        reply: mpsc::Sender<Result<(), String>>,
    },
    SendServerMessage {
        message: String,
        reply: mpsc::Sender<Result<(), String>>,
    },
    SendChannelMessage {
        message: String,
        reply: mpsc::Sender<Result<(), String>>,
    },
    SendPrivateMessage {
        target: u16,
        message: String,
        reply: mpsc::Sender<Result<(), String>>,
    },
    Snapshot {
        reply: mpsc::Sender<Result<String, String>>,
    },
    Disconnect {
        reply: mpsc::Sender<Result<(), String>>,
    },
    Shutdown,
}

/// Internal client handle (private — never exposed to C).
struct InnerTsClient {
    cmd_tx: mpsc::Sender<ClientCmd>,
    events: Arc<Mutex<VecDeque<String>>>,
    state: Arc<AtomicI32>,
    join: Option<std::thread::JoinHandle<()>>,
}

fn push_event(events: &Arc<Mutex<VecDeque<String>>>, json: String) {
    if let Ok(mut queue) = events.lock() {
        queue.push_back(json);
    }
}

fn state_code(state: ConnectionState) -> i32 {
    match state {
        ConnectionState::Disconnected => 0,
        ConnectionState::Connecting => 1,
        ConnectionState::Connected => 2,
        ConnectionState::Initializing => 3,
        ConnectionState::Reconnecting => 4,
    }
}

fn state_str(state: ConnectionState) -> &'static str {
    match state {
        ConnectionState::Disconnected => "disconnected",
        ConnectionState::Connecting => "connecting",
        ConnectionState::Connected => "connected",
        ConnectionState::Initializing => "initializing",
        ConnectionState::Reconnecting => "reconnecting",
    }
}

fn error_json(message: &str) -> String {
    serde_json::json!({"type": "error", "message": message}).to_string()
}

fn disconnect_json(reason: &str) -> String {
    serde_json::json!({"type": "disconnected", "reason": reason}).to_string()
}

fn event_to_json(event: &Event) -> String {
    let value = match event {
        Event::Connected {
            server_name,
            welcome_message,
        } => serde_json::json!({
            "type": "connected",
            "server_name": server_name,
            "welcome_message": welcome_message,
        }),
        Event::Disconnected { reason } | Event::ConnectionLost { reason } => {
            serde_json::json!({"type": "disconnected", "reason": reason})
        }
        Event::ConnectionStateChanged { new_state, .. } => serde_json::json!({
            "type": "state",
            "state": state_str(*new_state),
            "code": state_code(*new_state),
        }),
        Event::TalkStatusStart { user_id } => {
            serde_json::json!({"type": "talk", "user_id": user_id, "talking": true})
        }
        Event::TalkStatusStop { user_id } => {
            serde_json::json!({"type": "talk", "user_id": user_id, "talking": false})
        }
        Event::TextMessage {
            sender_id,
            sender_name,
            message,
            target,
        } => serde_json::json!({
            "type": "text",
            "sender_id": sender_id,
            "sender_name": sender_name,
            "message": message,
            "target": match target {
                MessageTarget::Server => "server",
                MessageTarget::Channel => "channel",
                MessageTarget::Private => "private",
            },
        }),
        Event::Poked {
            poker_id,
            poker_name,
            message,
        } => serde_json::json!({
            "type": "poked",
            "poker_id": poker_id,
            "poker_name": poker_name,
            "message": message,
        }),
        Event::AudioReceived {
            user_id,
            codec,
            data,
        } => serde_json::json!({
            "type": "audio",
            "user_id": user_id,
            "codec": codec.id(),
            "data": base64::engine::general_purpose::STANDARD.encode(data),
        }),
        Event::CommandError { error_id, message } => serde_json::json!({
            "type": "command_error",
            "error_id": error_id,
            "message": message,
        }),
        _ => serde_json::json!({"type": "updated"}),
    };
    value.to_string()
}

fn snapshot_json(client: &Client) -> String {
    let state = client.server_state();
    let channels: Vec<&Channel> = state.channels.values().collect();
    let users = client.users();
    let value = serde_json::json!({
        "state": state_str(client.state()),
        "code": state_code(client.state()),
        "client_id": client.client_id(),
        "channel_id": client.channel_id(),
        "server": {
            "name": state.server.name,
            "uid": state.server.uid,
            "welcome_message": state.server.welcome_message,
            "platform": state.server.platform,
            "version": state.server.version,
            "clients_online": state.server.clients_online,
            "max_clients": state.server.max_clients,
        },
        "channels": channels,
        "users": users,
    });
    value.to_string()
}

fn client_worker(
    config: ClientConfig,
    events: Arc<Mutex<VecDeque<String>>>,
    state: Arc<AtomicI32>,
    rx: mpsc::Receiver<ClientCmd>,
) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            push_event(&events, error_json(&format!("tokio runtime: {e}")));
            state.store(0, Ordering::SeqCst);
            return;
        }
    };

    let mut client = match runtime.block_on(async { Client::connect(config) }) {
        Ok(client) => client,
        Err(e) => {
            push_event(&events, error_json(&e.to_string()));
            state.store(0, Ordering::SeqCst);
            return;
        }
    };

    state.store(1, Ordering::SeqCst); // connecting

    let wait_result = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(20), client.wait_connected()).await
    });

    match wait_result {
        Ok(Ok(())) => {
            state.store(2, Ordering::SeqCst); // connected
            let server_name = client.server_state().server.name.clone();
            push_event(
                &events,
                serde_json::json!({
                    "type": "connected",
                    "server_name": server_name,
                    "welcome_message": null,
                })
                .to_string(),
            );
            push_event(&events, serde_json::json!({"type": "updated"}).to_string());
        }
        Ok(Err(e)) => {
            state.store(0, Ordering::SeqCst);
            push_event(&events, error_json(&e.to_string()));
            return;
        }
        Err(_) => {
            state.store(0, Ordering::SeqCst);
            push_event(&events, error_json("Connection timeout"));
            return;
        }
    }

    loop {
        // 1. Process network events.
        match runtime.block_on(async { client.process_events().await }) {
            Ok(events_out) => {
                for event in events_out {
                    push_event(&events, event_to_json(&event));
                }
            }
            Err(e) => {
                state.store(0, Ordering::SeqCst);
                push_event(&events, error_json(&e.to_string()));
                push_event(&events, disconnect_json(&e.to_string()));
                break;
            }
        }

        // 2. Handle pending commands.
        let mut shutdown = false;
        loop {
            let cmd = match rx.try_recv() {
                Ok(cmd) => cmd,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    shutdown = true;
                    break;
                }
            };
            match cmd {
                ClientCmd::SendAudio { data, codec, reply } => {
                    let codec = AudioCodec::from_id(codec).unwrap_or(AudioCodec::OpusVoice);
                    let _ = reply.send(client.send_audio(&data, codec).map_err(|e| e.to_string()));
                }
                ClientCmd::SendAudioAsync { data, codec } => {
                    let codec = AudioCodec::from_id(codec).unwrap_or(AudioCodec::OpusVoice);
                    let _ = client.send_audio(&data, codec);
                }
                ClientCmd::SetInputMuted { muted, reply } => {
                    let _ = reply.send(client.set_input_muted(muted).map_err(|e| e.to_string()));
                }
                ClientCmd::MoveToChannel {
                    channel_id,
                    password,
                    reply,
                } => {
                    let _ = reply.send(
                        client
                            .move_to_channel_with_password(channel_id, password)
                            .map_err(|e| e.to_string()),
                    );
                }
                ClientCmd::SendServerMessage { message, reply } => {
                    let _ = reply.send(client.send_server_message(message).map_err(|e| e.to_string()));
                }
                ClientCmd::SendChannelMessage { message, reply } => {
                    let _ =
                        reply.send(client.send_channel_message(message).map_err(|e| e.to_string()));
                }
                ClientCmd::SendPrivateMessage {
                    target,
                    message,
                    reply,
                } => {
                    let _ = reply.send(
                        client
                            .send_private_message(target, message)
                            .map_err(|e| e.to_string()),
                    );
                }
                ClientCmd::Snapshot { reply } => {
                    let _ = reply.send(Ok(snapshot_json(&client)));
                }
                ClientCmd::Disconnect { reply } => {
                    let _ = reply.send(client.disconnect().map_err(|e| e.to_string()));
                    shutdown = true;
                }
                ClientCmd::Shutdown => {
                    shutdown = true;
                }
            }
            if shutdown {
                break;
            }
        }

        if shutdown {
            // Give the runtime a moment to flush the disconnect packet and
            // collect the final events.
            let deadline = Instant::now() + Duration::from_secs(1);
            while Instant::now() < deadline {
                if let Ok(events_out) = runtime.block_on(async { client.process_events().await }) {
                    for event in events_out {
                        push_event(&events, event_to_json(&event));
                    }
                }
                if !client.is_connected() {
                    break;
                }
            }
            break;
        }
    }
}

/// Connect to a TeamSpeak server.
///
/// # Safety
/// - All string parameters must be valid null-terminated UTF-8 strings or NULL.
/// - `identity` must be a valid pointer (ownership is NOT transferred).
/// - Returns a pointer that must be freed with `tslib_client_free`.
#[no_mangle]
pub unsafe extern "C" fn tslib_client_connect(
    address: *const c_char,
    identity: *const TsLibIdentity,
    nickname: *const c_char,
    server_password: *const c_char,
    channel: *const c_char,
    channel_password: *const c_char,
) -> *mut TsClient {
    if address.is_null() || identity.is_null() || nickname.is_null() {
        return std::ptr::null_mut();
    }

    let address = match CStr::from_ptr(address).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return std::ptr::null_mut(),
    };
    let nickname = match CStr::from_ptr(nickname).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return std::ptr::null_mut(),
    };
    let identity = &*(identity as *const Identity);

    let mut builder = ClientConfig::builder()
        .address(address)
        .identity(identity.clone())
        .nickname(nickname);

    if !server_password.is_null() {
        if let Ok(s) = CStr::from_ptr(server_password).to_str() {
            builder = builder.password(s.to_string());
        }
    }
    if !channel.is_null() {
        if let Ok(s) = CStr::from_ptr(channel).to_str() {
            builder = builder.channel(s.to_string());
        }
    }
    if !channel_password.is_null() {
        if let Ok(s) = CStr::from_ptr(channel_password).to_str() {
            builder = builder.channel_password(s.to_string());
        }
    }

    let config = match builder.build() {
        Ok(config) => config,
        Err(_) => return std::ptr::null_mut(),
    };

    let events = Arc::new(Mutex::new(VecDeque::new()));
    let state = Arc::new(AtomicI32::new(1));
    let (cmd_tx, cmd_rx) = mpsc::channel();

    let worker_events = events.clone();
    let worker_state = state.clone();
    let join = match std::thread::Builder::new()
        .name("tslib-client".to_string())
        .spawn(move || client_worker(config, worker_events, worker_state, cmd_rx))
    {
        Ok(handle) => handle,
        Err(_) => return std::ptr::null_mut(),
    };

    Box::into_raw(Box::new(InnerTsClient {
        cmd_tx,
        events,
        state,
        join: Some(join),
    })) as *mut TsClient
}

unsafe fn exec(
    client: *mut TsClient,
    make: impl FnOnce(mpsc::Sender<Result<(), String>>) -> ClientCmd,
) -> TsLibError {
    if client.is_null() {
        return TsLibError::InvalidArgument;
    }
    let inner = &*(client as *const InnerTsClient);
    let (tx, rx) = mpsc::channel();
    if inner.cmd_tx.send(make(tx)).is_err() {
        return TsLibError::NotConnected;
    }
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => TsLibError::Ok,
        Ok(Err(_)) => TsLibError::InternalError,
        Err(_) => TsLibError::Timeout,
    }
}

/// Send an encoded audio frame to the server.
///
/// # Safety
/// `data` must point to `len` valid bytes.
#[no_mangle]
pub unsafe extern "C" fn tslib_client_send_audio(
    client: *mut TsClient,
    data: *const c_uchar,
    len: usize,
    codec: c_int,
) -> TsLibError {
    if data.is_null() {
        return TsLibError::InvalidArgument;
    }
    let bytes = std::slice::from_raw_parts(data, len).to_vec();
    exec(client, move |reply| ClientCmd::SendAudio {
        data: bytes,
        codec: codec as u8,
        reply,
    })
}

/// Send an encoded audio frame without waiting for the worker thread to
/// process it. Safe to call from real-time audio threads.
///
/// # Safety
/// `data` must point to `len` valid bytes.
#[no_mangle]
pub unsafe extern "C" fn tslib_client_send_audio_async(
    client: *mut TsClient,
    data: *const c_uchar,
    len: usize,
    codec: c_int,
) -> TsLibError {
    if client.is_null() || data.is_null() {
        return TsLibError::InvalidArgument;
    }
    let bytes = std::slice::from_raw_parts(data, len).to_vec();
    let inner = &*(client as *const InnerTsClient);
    if inner
        .cmd_tx
        .send(ClientCmd::SendAudioAsync {
            data: bytes,
            codec: codec as u8,
        })
        .is_err()
    {
        return TsLibError::NotConnected;
    }
    TsLibError::Ok
}

/// Notify the server whether our microphone is muted.
#[no_mangle]
pub unsafe extern "C" fn tslib_client_set_input_muted(
    client: *mut TsClient,
    muted: c_int,
) -> TsLibError {
    exec(client, move |reply| ClientCmd::SetInputMuted {
        muted: muted != 0,
        reply,
    })
}

/// Move to a channel (optionally password protected).
///
/// # Safety
/// `password` must be a valid null-terminated UTF-8 string or NULL.
#[no_mangle]
pub unsafe extern "C" fn tslib_client_move_to_channel(
    client: *mut TsClient,
    channel_id: u64,
    password: *const c_char,
) -> TsLibError {
    let password = if password.is_null() {
        None
    } else {
        CStr::from_ptr(password).to_str().ok().map(|s| s.to_string())
    };
    exec(client, move |reply| ClientCmd::MoveToChannel {
        channel_id,
        password,
        reply,
    })
}

/// Send a message to the whole server.
///
/// # Safety
/// `message` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn tslib_client_send_server_message(
    client: *mut TsClient,
    message: *const c_char,
) -> TsLibError {
    if message.is_null() {
        return TsLibError::InvalidArgument;
    }
    let message = match CStr::from_ptr(message).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return TsLibError::InvalidArgument,
    };
    exec(client, move |reply| ClientCmd::SendServerMessage { message, reply })
}

/// Send a message to the current channel.
///
/// # Safety
/// `message` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn tslib_client_send_channel_message(
    client: *mut TsClient,
    message: *const c_char,
) -> TsLibError {
    if message.is_null() {
        return TsLibError::InvalidArgument;
    }
    let message = match CStr::from_ptr(message).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return TsLibError::InvalidArgument,
    };
    exec(client, move |reply| ClientCmd::SendChannelMessage { message, reply })
}

/// Send a private message to a user.
///
/// # Safety
/// `message` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn tslib_client_send_private_message(
    client: *mut TsClient,
    target: u16,
    message: *const c_char,
) -> TsLibError {
    if message.is_null() {
        return TsLibError::InvalidArgument;
    }
    let message = match CStr::from_ptr(message).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return TsLibError::InvalidArgument,
    };
    exec(client, move |reply| ClientCmd::SendPrivateMessage {
        target,
        message,
        reply,
    })
}

/// Disconnect from the server (the handle stays valid until `tslib_client_free`).
#[no_mangle]
pub unsafe extern "C" fn tslib_client_disconnect(client: *mut TsClient) -> TsLibError {
    if client.is_null() {
        return TsLibError::InvalidArgument;
    }
    let inner = &*(client as *const InnerTsClient);
    let (tx, rx) = mpsc::channel();
    if inner.cmd_tx.send(ClientCmd::Disconnect { reply: tx }).is_err() {
        return TsLibError::NotConnected;
    }
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => TsLibError::Ok,
        Ok(Err(_)) => TsLibError::InternalError,
        Err(_) => TsLibError::Timeout,
    }
}

/// Get the current connection state (0..4).
#[no_mangle]
pub unsafe extern "C" fn tslib_client_state(client: *const TsClient) -> c_int {
    if client.is_null() {
        return 0;
    }
    let inner = &*(client as *const InnerTsClient);
    inner.state.load(Ordering::SeqCst)
}

/// Pop the next event (JSON string) from the event queue.
///
/// Returns NULL when the queue is empty. The returned string must be freed
/// with `tslib_string_free`.
#[no_mangle]
pub unsafe extern "C" fn tslib_client_poll_event(client: *const TsClient) -> *mut c_char {
    if client.is_null() {
        return std::ptr::null_mut();
    }
    let inner = &*(client as *const InnerTsClient);
    let mut queue = match inner.events.lock() {
        Ok(q) => q,
        Err(_) => return std::ptr::null_mut(),
    };
    match queue.pop_front() {
        Some(json) => CString::new(json)
            .map(|c| c.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        None => std::ptr::null_mut(),
    }
}

/// Get a full JSON snapshot of the current server state (channels, users, ...).
///
/// The returned string must be freed with `tslib_string_free`.
#[no_mangle]
pub unsafe extern "C" fn tslib_client_snapshot(client: *const TsClient) -> *mut c_char {
    if client.is_null() {
        return std::ptr::null_mut();
    }
    let inner = &*(client as *const InnerTsClient);
    let (tx, rx) = mpsc::channel();
    if inner.cmd_tx.send(ClientCmd::Snapshot { reply: tx }).is_err() {
        return std::ptr::null_mut();
    }
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(json)) => CString::new(json)
            .map(|c| c.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        _ => std::ptr::null_mut(),
    }
}

/// Disconnect and free the client handle (joins the worker thread).
///
/// # Safety
/// `client` must be a valid pointer from `tslib_client_connect`.
#[no_mangle]
pub unsafe extern "C" fn tslib_client_free(client: *mut TsClient) {
    if client.is_null() {
        return;
    }
    let mut inner = Box::from_raw(client as *mut InnerTsClient);
    let _ = inner.cmd_tx.send(ClientCmd::Shutdown);
    if let Some(join) = inner.join.take() {
        let _ = join.join();
    }
}

/// Free a string returned by this library.
///
/// # Safety
/// `s` must be a pointer returned by a `tslib_*` string function.
#[no_mangle]
pub unsafe extern "C" fn tslib_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

// ============================================================================
// Opus codec
// ============================================================================

/// Internal Opus codec (private — never exposed to C).
struct InnerOpus {
    encoder: opus::Encoder,
    decoder: opus::Decoder,
    channels: usize,
    frame_size: usize,
}

/// Create an Opus encoder/decoder pair.
///
/// Returns NULL on failure. Free with `tslib_opus_destroy`.
#[no_mangle]
pub unsafe extern "C" fn tslib_opus_create(
    sample_rate: c_int,
    channels: c_int,
    bitrate: c_int,
    frame_size_ms: c_int,
) -> *mut TsOpus {
    let sample_rate = if sample_rate <= 0 { 48000 } else { sample_rate };
    let channels = if channels <= 0 { 1 } else { channels };
    let bitrate = if bitrate <= 0 { 48000 } else { bitrate };
    let frame_size_ms = if frame_size_ms <= 0 { 20 } else { frame_size_ms };
    let frame_size = (sample_rate as usize * frame_size_ms as usize) / 1000;

    let opus_channels = match channels {
        1 => opus::Channels::Mono,
        2 => opus::Channels::Stereo,
        _ => return std::ptr::null_mut(),
    };

    let mut encoder = match opus::Encoder::new(
        sample_rate as u32,
        opus_channels,
        opus::Application::Voip,
    ) {
        Ok(e) => e,
        Err(_) => return std::ptr::null_mut(),
    };
    let _ = encoder.set_bitrate(opus::Bitrate::Bits(bitrate));
    let _ = encoder.set_inband_fec(true);
    let _ = encoder.set_packet_loss_perc(5);

    let decoder = match opus::Decoder::new(sample_rate as u32, opus_channels) {
        Ok(d) => d,
        Err(_) => return std::ptr::null_mut(),
    };

    Box::into_raw(Box::new(InnerOpus {
        encoder,
        decoder,
        channels: channels as usize,
        frame_size,
    })) as *mut TsOpus
}

/// Encode one frame of 16-bit PCM samples into Opus.
///
/// # Safety
/// - `pcm` must point to `pcm_len` valid i16 samples (at least one frame).
/// - `out` must point to `out_cap` valid bytes.
/// Returns the number of bytes written, or a negative error code.
#[no_mangle]
pub unsafe extern "C" fn tslib_opus_encode(
    codec: *mut TsOpus,
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
/// # Safety
/// - `data` must point to `len` valid bytes.
/// - `out` must point to `out_cap` valid i16 samples.
/// Returns the number of samples written, or a negative error code.
#[no_mangle]
pub unsafe extern "C" fn tslib_opus_decode(
    codec: *mut TsOpus,
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
/// `codec` must be a valid pointer from `tslib_opus_create`.
#[no_mangle]
pub unsafe extern "C" fn tslib_opus_destroy(codec: *mut TsOpus) {
    if !codec.is_null() {
        drop(Box::from_raw(codec as *mut InnerOpus));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_roundtrip_via_ffi() {
        unsafe {
            let codec = tslib_opus_create(48000, 1, 48000, 20);
            assert!(!codec.is_null());

            let pcm: Vec<i16> = (0..960).map(|i| ((i % 1000) - 500) as i16).collect();
            let mut encoded = vec![0u8; 2048];
            let n = tslib_opus_encode(
                codec,
                pcm.as_ptr(),
                pcm.len(),
                encoded.as_mut_ptr(),
                encoded.len(),
            );
            assert!(n > 0, "encode failed: {n}");

            let mut decoded = vec![0i16; 960];
            let m = tslib_opus_decode(
                codec,
                encoded.as_ptr(),
                n as usize,
                decoded.as_mut_ptr(),
                decoded.len(),
            );
            assert!(m > 0, "decode failed: {m}");

            tslib_opus_destroy(codec);
        }
    }

    #[test]
    fn connect_failure_emits_error_event() {
        unsafe {
            let identity = tslib_identity_create();
            assert!(!identity.is_null());

            let address = CString::new("127.0.0.1:1").unwrap();
            let nickname = CString::new("ffi-test").unwrap();
            let client = tslib_client_connect(
                address.as_ptr(),
                identity,
                nickname.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            );
            assert!(!client.is_null());

            std::thread::sleep(Duration::from_millis(800));

            let ptr = tslib_client_poll_event(client);
            assert!(!ptr.is_null(), "expected an error event from failed connect");
            tslib_string_free(ptr);

            tslib_client_free(client);
            tslib_identity_free(identity);
        }
    }
}
