//! # tslib-chat
//!
//! Chat module for tslib providing:
//! - Text message handling
//! - BBCode parsing and rendering
//! - Chat history management
//! - Message filtering

pub mod bbcode;
pub mod history;
pub mod message;

// Re-exports
pub use bbcode::{BBCodeParser, BBCodeRenderer};
pub use history::ChatHistory;
pub use message::{ChatMessage, MessageTarget};
