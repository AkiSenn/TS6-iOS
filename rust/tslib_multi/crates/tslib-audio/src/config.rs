//! Audio configuration

use serde::{Deserialize, Serialize};

/// Audio configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Sample rate (usually 48000 for Opus)
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo)
    pub channels: u16,
    /// Frame size in milliseconds (20ms is standard for TS3)
    pub frame_size_ms: u32,
    /// Opus bitrate (usually 32000-96000)
    pub bitrate: u32,
    /// Opus application mode
    pub opus_application: OpusApplication,
    /// Voice activity detection enabled
    pub vad_enabled: bool,
    /// VAD threshold (0.0-1.0)
    pub vad_threshold: f32,
    /// Automatic gain control
    pub agc_enabled: bool,
    /// Target gain for AGC (dB)
    pub agc_target: f32,
    /// Noise suppression enabled
    pub noise_suppression: bool,
    /// Echo cancellation enabled (planned)
    pub echo_cancellation: bool,
    /// Input device name (None for default)
    pub input_device: Option<String>,
    /// Output device name (None for default)
    pub output_device: Option<String>,
    /// Input volume (0.0-2.0, 1.0 = normal)
    pub input_volume: f32,
    /// Output volume (0.0-2.0, 1.0 = normal)
    pub output_volume: f32,
    /// Playback buffer size in milliseconds
    pub playback_buffer_ms: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 1,
            frame_size_ms: 20,
            bitrate: 64000,
            opus_application: OpusApplication::Voip,
            vad_enabled: true,
            vad_threshold: 0.3,
            agc_enabled: true,
            agc_target: -18.0,
            noise_suppression: true,
            echo_cancellation: false, // Not implemented yet
            input_device: None,
            output_device: None,
            input_volume: 1.0,
            output_volume: 1.0,
            playback_buffer_ms: 60,
        }
    }
}

impl AudioConfig {
    /// Create a config optimized for music
    pub fn music() -> Self {
        Self {
            channels: 2,
            bitrate: 96000,
            opus_application: OpusApplication::Audio,
            vad_enabled: false,
            noise_suppression: false,
            ..Default::default()
        }
    }

    /// Create a config optimized for low latency
    pub fn low_latency() -> Self {
        Self {
            frame_size_ms: 10,
            playback_buffer_ms: 40,
            ..Default::default()
        }
    }

    /// Get the frame size in samples
    pub fn frame_size_samples(&self) -> usize {
        (self.sample_rate * self.frame_size_ms / 1000) as usize
    }

    /// Get the playback buffer size in samples
    pub fn playback_buffer_samples(&self) -> usize {
        (self.sample_rate * self.playback_buffer_ms / 1000) as usize
    }
}

/// Opus application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpusApplication {
    /// Optimized for voice
    Voip,
    /// Optimized for music/audio
    Audio,
    /// Restricted low delay mode
    LowDelay,
}

impl OpusApplication {
    /// Convert to opus library constant
    pub fn to_opus_const(&self) -> i32 {
        match self {
            Self::Voip => 2048,     // OPUS_APPLICATION_VOIP
            Self::Audio => 2049,    // OPUS_APPLICATION_AUDIO
            Self::LowDelay => 2051, // OPUS_APPLICATION_RESTRICTED_LOWDELAY
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let cfg = AudioConfig::default();
        assert_eq!(cfg.sample_rate, 48000);
        assert_eq!(cfg.channels, 1);
        assert_eq!(cfg.frame_size_ms, 20);
        assert_eq!(cfg.bitrate, 64000);
        assert_eq!(cfg.opus_application, OpusApplication::Voip);
        assert!(cfg.vad_enabled);
        assert!(cfg.noise_suppression);
        assert!(!cfg.echo_cancellation);
        assert_eq!(cfg.input_volume, 1.0);
        assert_eq!(cfg.output_volume, 1.0);
    }

    #[test]
    fn music_preset() {
        let cfg = AudioConfig::music();
        assert_eq!(cfg.channels, 2);
        assert_eq!(cfg.bitrate, 96000);
        assert_eq!(cfg.opus_application, OpusApplication::Audio);
        assert!(!cfg.vad_enabled);
        assert!(!cfg.noise_suppression);
        // inherited defaults
        assert_eq!(cfg.sample_rate, 48000);
    }

    #[test]
    fn low_latency_preset() {
        let cfg = AudioConfig::low_latency();
        assert_eq!(cfg.frame_size_ms, 10);
        assert_eq!(cfg.playback_buffer_ms, 40);
        // inherited defaults
        assert_eq!(cfg.sample_rate, 48000);
        assert_eq!(cfg.channels, 1);
    }

    #[test]
    fn frame_size_samples_default() {
        let cfg = AudioConfig::default();
        // 48000 * 20 / 1000 = 960
        assert_eq!(cfg.frame_size_samples(), 960);
    }

    #[test]
    fn frame_size_samples_low_latency() {
        let cfg = AudioConfig::low_latency();
        // 48000 * 10 / 1000 = 480
        assert_eq!(cfg.frame_size_samples(), 480);
    }

    #[test]
    fn playback_buffer_samples() {
        let cfg = AudioConfig::default();
        // 48000 * 60 / 1000 = 2880
        assert_eq!(cfg.playback_buffer_samples(), 2880);
    }

    #[test]
    fn opus_application_constants() {
        assert_eq!(OpusApplication::Voip.to_opus_const(), 2048);
        assert_eq!(OpusApplication::Audio.to_opus_const(), 2049);
        assert_eq!(OpusApplication::LowDelay.to_opus_const(), 2051);
    }
}
