//! Bot configuration

use crate::error::{BotError, Result};
use tslib_core::identity::Identity;
use std::time::Duration;

/// Bot configuration builder
#[derive(Debug, Clone, Default)]
pub struct BotConfigBuilder {
    address: Option<String>,
    identity: Option<Identity>,
    nickname: Option<String>,
    password: Option<String>,
    channel: Option<String>,
    channel_password: Option<String>,
    command_prefix: String,
    owners: Vec<String>,
    auto_reconnect: bool,
    reconnect_delay: Duration,
}

impl BotConfigBuilder {
    /// Set the server address
    pub fn address(mut self, address: impl Into<String>) -> Self {
        self.address = Some(address.into());
        self
    }

    /// Set the identity
    pub fn identity(mut self, identity: Identity) -> Self {
        self.identity = Some(identity);
        self
    }

    /// Set the nickname
    pub fn nickname(mut self, nickname: impl Into<String>) -> Self {
        self.nickname = Some(nickname.into());
        self
    }

    /// Set the server password
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// Set the default channel
    pub fn channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = Some(channel.into());
        self
    }

    /// Set the channel password
    pub fn channel_password(mut self, password: impl Into<String>) -> Self {
        self.channel_password = Some(password.into());
        self
    }

    /// Set the command prefix
    pub fn command_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.command_prefix = prefix.into();
        self
    }

    /// Add a bot owner (by unique ID)
    pub fn owner(mut self, uid: impl Into<String>) -> Self {
        self.owners.push(uid.into());
        self
    }

    /// Add multiple bot owners
    pub fn owners(mut self, uids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.owners.extend(uids.into_iter().map(Into::into));
        self
    }

    /// Enable/disable auto-reconnect
    pub fn auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }

    /// Set reconnect delay
    pub fn reconnect_delay(mut self, delay: Duration) -> Self {
        self.reconnect_delay = delay;
        self
    }

    /// Build the configuration
    pub fn build(self) -> Result<BotConfig> {
        let address = self
            .address
            .ok_or_else(|| BotError::Config("Server address is required".into()))?;

        let identity = self.identity.unwrap_or_else(|| {
            Identity::create().expect("Failed to create identity")
        });

        let nickname = self.nickname.unwrap_or_else(|| "TsLibBot".to_string());

        Ok(BotConfig {
            address,
            identity,
            nickname,
            password: self.password,
            channel: self.channel,
            channel_password: self.channel_password,
            command_prefix: if self.command_prefix.is_empty() {
                "!".to_string()
            } else {
                self.command_prefix
            },
            owners: self.owners,
            auto_reconnect: self.auto_reconnect,
            reconnect_delay: if self.reconnect_delay == Duration::ZERO {
                Duration::from_secs(5)
            } else {
                self.reconnect_delay
            },
        })
    }
}

/// Bot configuration
#[derive(Debug, Clone)]
pub struct BotConfig {
    /// Server address
    pub address: String,
    /// Bot identity
    pub identity: Identity,
    /// Bot nickname
    pub nickname: String,
    /// Server password
    pub password: Option<String>,
    /// Default channel
    pub channel: Option<String>,
    /// Channel password
    pub channel_password: Option<String>,
    /// Command prefix
    pub command_prefix: String,
    /// Bot owners (unique IDs)
    pub owners: Vec<String>,
    /// Auto-reconnect on disconnect
    pub auto_reconnect: bool,
    /// Reconnect delay
    pub reconnect_delay: Duration,
}

impl BotConfig {
    /// Create a new configuration builder
    pub fn builder() -> BotConfigBuilder {
        BotConfigBuilder::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_fails_without_address() {
        let result = BotConfigBuilder::default().build();
        assert!(result.is_err());
    }

    #[test]
    fn build_succeeds_with_address() {
        let config = BotConfigBuilder::default()
            .address("localhost")
            .build()
            .unwrap();
        assert_eq!(config.address, "localhost");
    }

    #[test]
    fn default_nickname() {
        let config = BotConfigBuilder::default()
            .address("localhost")
            .build()
            .unwrap();
        assert_eq!(config.nickname, "TsLibBot");
    }

    #[test]
    fn custom_nickname() {
        let config = BotConfigBuilder::default()
            .address("localhost")
            .nickname("MyBot")
            .build()
            .unwrap();
        assert_eq!(config.nickname, "MyBot");
    }

    #[test]
    fn default_command_prefix() {
        let config = BotConfigBuilder::default()
            .address("localhost")
            .build()
            .unwrap();
        assert_eq!(config.command_prefix, "!");
    }

    #[test]
    fn custom_command_prefix() {
        let config = BotConfigBuilder::default()
            .address("localhost")
            .command_prefix(".")
            .build()
            .unwrap();
        assert_eq!(config.command_prefix, ".");
    }

    #[test]
    fn default_reconnect_delay() {
        let config = BotConfigBuilder::default()
            .address("localhost")
            .build()
            .unwrap();
        assert_eq!(config.reconnect_delay, Duration::from_secs(5));
    }

    #[test]
    fn zero_reconnect_delay_becomes_5s() {
        let config = BotConfigBuilder::default()
            .address("localhost")
            .reconnect_delay(Duration::ZERO)
            .build()
            .unwrap();
        assert_eq!(config.reconnect_delay, Duration::from_secs(5));
    }

    #[test]
    fn custom_reconnect_delay() {
        let config = BotConfigBuilder::default()
            .address("localhost")
            .reconnect_delay(Duration::from_secs(10))
            .build()
            .unwrap();
        assert_eq!(config.reconnect_delay, Duration::from_secs(10));
    }

    #[test]
    fn single_owner() {
        let config = BotConfigBuilder::default()
            .address("localhost")
            .owner("uid1")
            .build()
            .unwrap();
        assert_eq!(config.owners, vec!["uid1"]);
    }

    #[test]
    fn multiple_owners() {
        let config = BotConfigBuilder::default()
            .address("localhost")
            .owner("uid1")
            .owners(vec!["uid2", "uid3"])
            .build()
            .unwrap();
        assert_eq!(config.owners, vec!["uid1", "uid2", "uid3"]);
    }

    #[test]
    fn identity_auto_created() {
        let config = BotConfigBuilder::default()
            .address("localhost")
            .build()
            .unwrap();
        // Identity was auto-created, so it should exist
        assert!(!config.identity.unique_id().is_empty());
    }
}
