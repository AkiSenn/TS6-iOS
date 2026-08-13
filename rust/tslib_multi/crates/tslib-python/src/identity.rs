use pyo3::prelude::*;

use crate::error::to_py_err;

/// A TeamSpeak 3 identity.
///
/// Identities are based on ECDH and include a security level that must
/// meet server requirements.
#[pyclass(name = "Identity")]
pub struct PyIdentity {
    pub(crate) inner: tslib_core::Identity,
}

#[pymethods]
impl PyIdentity {
    /// Create a new random identity.
    #[new]
    fn new() -> PyResult<Self> {
        let inner = tslib_core::Identity::create().map_err(to_py_err)?;
        Ok(Self { inner })
    }

    /// Load an identity from a file.
    #[staticmethod]
    fn load(path: String) -> PyResult<Self> {
        let inner = tslib_core::Identity::load(&path).map_err(to_py_err)?;
        Ok(Self { inner })
    }

    /// Save the identity to a file.
    fn save(&self, path: String) -> PyResult<()> {
        self.inner.save(&path).map_err(to_py_err)
    }

    /// Import an identity from a string.
    #[staticmethod]
    fn from_string(data: String) -> PyResult<Self> {
        let inner = tslib_core::Identity::from_string(&data).map_err(to_py_err)?;
        Ok(Self { inner })
    }

    /// Export the identity as a string.
    fn export_string(&self) -> PyResult<String> {
        self.inner.export_string().map_err(to_py_err)
    }

    /// The unique identifier (public key hash).
    #[getter]
    fn unique_id(&self) -> String {
        self.inner.unique_id()
    }

    /// The current security level.
    #[getter]
    fn security_level(&self) -> u8 {
        self.inner.security_level()
    }

    /// The associated nickname, if any.
    #[getter]
    fn nickname(&self) -> Option<String> {
        self.inner.nickname().map(String::from)
    }

    /// Set the nickname for this identity.
    #[setter]
    fn set_nickname(&mut self, value: String) {
        self.inner.set_nickname(value);
    }

    /// Improve the identity's security level.
    ///
    /// This is CPU-intensive and may block for a while.
    fn improve(&mut self, target_level: u8) -> PyResult<()> {
        self.inner.improve(target_level).map_err(to_py_err)
    }

    fn __repr__(&self) -> String {
        format!(
            "Identity(uid={}, level={})",
            self.inner.unique_id(),
            self.inner.security_level()
        )
    }
}
