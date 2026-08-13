//! Identity management for TeamSpeak 3
//!
//! TeamSpeak identities are based on ECDH (Elliptic Curve Diffie-Hellman)
//! and include a security level that must meet server requirements.

use crate::error::{IdentityError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

// Re-export tsclientlib's identity for internal use
pub use tsclientlib::Identity as TsIdentity;

/// A TeamSpeak 3 identity wrapper
///
/// Wraps tsclientlib's Identity with additional convenience methods.
#[derive(Clone)]
pub struct Identity {
    /// The underlying tsclientlib identity
    inner: TsIdentity,
    /// Optional nickname associated with this identity
    nickname: Option<String>,
}

impl Identity {
    /// Create a new random identity
    ///
    /// The identity starts at security level 0 and should be improved
    /// before connecting to servers with security requirements.
    pub fn create() -> Result<Self> {
        let inner = TsIdentity::create();
        Ok(Self {
            inner,
            nickname: None,
        })
    }

    /// Create an identity from the underlying tsclientlib identity
    pub fn from_ts_identity(inner: TsIdentity) -> Self {
        Self {
            inner,
            nickname: None,
        }
    }

    /// Get the underlying tsclientlib identity
    pub fn as_ts_identity(&self) -> &TsIdentity {
        &self.inner
    }

    /// Get a clone of the underlying tsclientlib identity
    pub fn to_ts_identity(&self) -> TsIdentity {
        self.inner.clone()
    }

    /// Load an identity from a file
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| IdentityError::ImportFailed(e.to_string()))?;

        Self::from_string(&content)
    }

    /// Save the identity to a file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = self.export_string()?;
        std::fs::write(path.as_ref(), content)
            .map_err(|e| IdentityError::ExportFailed(e.to_string()))?;
        Ok(())
    }

    /// Import an identity from a base64-encoded string
    pub fn from_string(data: &str) -> Result<Self> {
        // Try to parse as our JSON format first
        if let Ok(stored) = serde_json::from_str::<StoredIdentity>(data) {
            let inner = TsIdentity::new_from_str(&stored.key)
                .map_err(|e| IdentityError::ImportFailed(e.to_string()))?;
            return Ok(Self {
                inner,
                nickname: stored.nickname,
            });
        }

        // Try to parse as raw tsclientlib format
        let inner = TsIdentity::new_from_str(data.trim())
            .map_err(|e| IdentityError::ImportFailed(e.to_string()))?;

        Ok(Self {
            inner,
            nickname: None,
        })
    }

    /// Export the identity to a string
    pub fn export_string(&self) -> Result<String> {
        // Export the key in tsclientlib format
        let key = self.export_key_string();
        let stored = StoredIdentity {
            key,
            nickname: self.nickname.clone(),
        };
        serde_json::to_string_pretty(&stored)
            .map_err(|e| IdentityError::ExportFailed(e.to_string()).into())
    }

    /// Export just the key string in tsclientlib format
    fn export_key_string(&self) -> String {
        // Format: counterVbase64(private_key_bytes)
        use base64::Engine;
        let key_bytes = self.inner.key().to_short();
        let counter = self.inner.counter();
        let key_b64 = base64::engine::general_purpose::STANDARD.encode(&key_bytes);
        format!("{}V{}", counter, key_b64)
    }

    /// Get the unique identifier (public key hash)
    pub fn unique_id(&self) -> String {
        self.inner.key().to_pub().get_uid()
    }

    /// Get the current security level
    pub fn security_level(&self) -> u8 {
        self.inner.level()
    }

    /// Get the key counter (offset)
    pub fn key_offset(&self) -> u64 {
        self.inner.counter()
    }

    /// Get the associated nickname, if any
    pub fn nickname(&self) -> Option<&str> {
        self.nickname.as_deref()
    }

    /// Set the nickname for this identity
    pub fn set_nickname(&mut self, nickname: impl Into<String>) {
        self.nickname = Some(nickname.into());
    }

    /// Improve the identity's security level synchronously
    ///
    /// This is a CPU-intensive operation that finds a key offset
    /// which produces a hash with enough leading zeros.
    ///
    /// # Arguments
    /// * `target_level` - The desired security level (8-32 typically)
    pub fn improve(&mut self, target_level: u8) -> Result<()> {
        if target_level <= self.inner.level() {
            return Ok(());
        }

        self.inner.upgrade_level(target_level);
        Ok(())
    }

    /// Improve the identity asynchronously
    ///
    /// Runs the CPU-intensive work in a blocking task pool.
    pub async fn improve_async(&mut self, target_level: u8) -> Result<()> {
        if target_level <= self.inner.level() {
            return Ok(());
        }

        let mut inner = self.inner.clone();

        let result = tokio::task::spawn_blocking(move || {
            inner.upgrade_level(target_level);
            inner
        })
        .await
        .map_err(|e| IdentityError::ImproveFailed(e.to_string()))?;

        self.inner = result;
        Ok(())
    }
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("unique_id", &self.unique_id())
            .field("security_level", &self.security_level())
            .field("nickname", &self.nickname)
            .finish_non_exhaustive()
    }
}

/// Stored identity format for serialization
#[derive(Debug, Serialize, Deserialize)]
struct StoredIdentity {
    /// The identity key in tsclientlib format
    key: String,
    /// Optional nickname
    #[serde(skip_serializing_if = "Option::is_none")]
    nickname: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_identity() {
        let identity = Identity::create().unwrap();
        assert!(!identity.unique_id().is_empty());
        assert!(identity.security_level() < 100);
    }

    #[test]
    fn test_serialize_identity() {
        let identity = Identity::create().unwrap();
        let json = identity.export_string().unwrap();
        let loaded = Identity::from_string(&json).unwrap();
        assert_eq!(identity.unique_id(), loaded.unique_id());
    }
}
