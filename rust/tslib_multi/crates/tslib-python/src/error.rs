use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

/// Helper to convert `tslib_core::Error` into `PyErr`.
pub fn to_py_err(err: tslib_core::Error) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

/// Helper to convert `tslib_audio::AudioError` into `PyErr`.
pub fn audio_to_py_err(err: tslib_audio::AudioError) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}
