use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::error::audio_to_py_err;
use tslib_audio::codec::{Decoder, Encoder};

/// Audio configuration.
#[pyclass(name = "AudioConfig", frozen, from_py_object)]
#[derive(Clone)]
pub struct PyAudioConfig {
    pub(crate) inner: tslib_audio::AudioConfig,
}

#[pymethods]
impl PyAudioConfig {
    /// Create a default audio configuration (voice-optimized).
    #[new]
    fn new() -> Self {
        Self {
            inner: tslib_audio::AudioConfig::default(),
        }
    }

    /// Create a configuration optimized for music.
    #[staticmethod]
    fn music() -> Self {
        Self {
            inner: tslib_audio::AudioConfig::music(),
        }
    }

    /// Create a configuration optimized for low latency.
    #[staticmethod]
    fn low_latency() -> Self {
        Self {
            inner: tslib_audio::AudioConfig::low_latency(),
        }
    }

    #[getter]
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate
    }

    #[getter]
    fn channels(&self) -> u16 {
        self.inner.channels
    }

    #[getter]
    fn bitrate(&self) -> u32 {
        self.inner.bitrate
    }

    #[getter]
    fn frame_size_ms(&self) -> u32 {
        self.inner.frame_size_ms
    }

    /// Frame size in samples.
    #[getter]
    fn frame_size_samples(&self) -> usize {
        self.inner.frame_size_samples()
    }

    fn __repr__(&self) -> String {
        format!(
            "AudioConfig(rate={}, ch={}, bitrate={})",
            self.inner.sample_rate, self.inner.channels, self.inner.bitrate
        )
    }
}

/// Opus encoder/decoder pair.
///
/// Example::
///
///     codec = OpusCodec()
///     encoded = codec.encode(pcm_bytes)
///     decoded = codec.decode(encoded)
#[pyclass(name = "OpusCodec", unsendable)]
pub struct PyOpusCodec {
    encoder: tslib_audio::codec::OpusEncoder,
    decoder: tslib_audio::codec::OpusDecoder,
    config: tslib_audio::AudioConfig,
}

#[pymethods]
impl PyOpusCodec {
    /// Create a new Opus codec with the given configuration.
    ///
    /// If no config is provided, defaults are used.
    #[new]
    #[pyo3(signature = (config=None))]
    fn new(config: Option<PyAudioConfig>) -> PyResult<Self> {
        let audio_config = config
            .map(|c| c.inner)
            .unwrap_or_default();

        let codec =
            tslib_audio::OpusCodec::new(audio_config.clone()).map_err(audio_to_py_err)?;
        let encoder = codec.create_encoder().map_err(audio_to_py_err)?;
        let decoder = codec.create_decoder().map_err(audio_to_py_err)?;

        Ok(Self {
            encoder,
            decoder,
            config: audio_config,
        })
    }

    /// Encode PCM samples (16-bit LE bytes) to Opus.
    ///
    /// Args:
    ///     pcm: Raw PCM data as bytes (little-endian i16 samples).
    ///
    /// Returns:
    ///     Encoded Opus packet as bytes.
    fn encode<'py>(&mut self, py: Python<'py>, pcm: &[u8]) -> PyResult<Bound<'py, PyBytes>> {
        // Convert bytes to i16 samples
        if pcm.len() % 2 != 0 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "PCM data must have even length (16-bit samples)",
            ));
        }

        let samples: Vec<i16> = pcm
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();

        let mut output = vec![0u8; 4000]; // Max Opus packet size
        let len = self
            .encoder
            .encode(&samples, &mut output)
            .map_err(audio_to_py_err)?;

        Ok(PyBytes::new(py, &output[..len]))
    }

    /// Decode an Opus packet to PCM samples.
    ///
    /// Args:
    ///     data: Opus-encoded data as bytes.
    ///
    /// Returns:
    ///     Decoded PCM data as bytes (little-endian i16 samples).
    fn decode<'py>(&mut self, py: Python<'py>, data: &[u8]) -> PyResult<Bound<'py, PyBytes>> {
        let frame_samples =
            self.config.frame_size_samples() * self.config.channels as usize;
        let mut output = vec![0i16; frame_samples];

        let len = self
            .decoder
            .decode(data, &mut output)
            .map_err(audio_to_py_err)?;

        // Convert i16 samples back to bytes
        let bytes: Vec<u8> = output[..len]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();

        Ok(PyBytes::new(py, &bytes))
    }

    fn __repr__(&self) -> String {
        format!(
            "OpusCodec(rate={}, ch={})",
            self.config.sample_rate, self.config.channels
        )
    }
}
