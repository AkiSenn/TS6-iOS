//! # tslib-channel
//!
//! Channel module for tslib providing:
//! - Channel tree navigation
//! - Channel creation and management
//! - Channel subscriptions
//! - File browser

pub mod manager;
pub mod tree;

// Re-exports
pub use manager::ChannelManager;
pub use tree::ChannelTree;
