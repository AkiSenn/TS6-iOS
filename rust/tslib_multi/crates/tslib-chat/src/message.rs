//! Chat message types

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Target for a chat message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageTarget {
    /// Server-wide message
    Server,
    /// Channel message
    Channel(u64),
    /// Private message to a user
    Private(u16),
}

/// A chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Message ID (local)
    pub id: u64,
    /// Sender client ID
    pub sender_id: u16,
    /// Sender nickname
    pub sender_name: String,
    /// Sender unique ID
    pub sender_uid: Option<String>,
    /// Message target
    pub target: MessageTarget,
    /// Raw message content (may contain BBCode)
    pub content: String,
    /// Timestamp
    pub timestamp: SystemTime,
    /// Is this message from us
    pub is_own: bool,
}

impl ChatMessage {
    /// Create a new chat message
    pub fn new(
        sender_id: u16,
        sender_name: impl Into<String>,
        target: MessageTarget,
        content: impl Into<String>,
    ) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

        Self {
            id: COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            sender_id,
            sender_name: sender_name.into(),
            sender_uid: None,
            target,
            content: content.into(),
            timestamp: SystemTime::now(),
            is_own: false,
        }
    }

    /// Check if this is a private message
    pub fn is_private(&self) -> bool {
        matches!(self.target, MessageTarget::Private(_))
    }

    /// Check if this is a channel message
    pub fn is_channel(&self) -> bool {
        matches!(self.target, MessageTarget::Channel(_))
    }

    /// Check if this is a server message
    pub fn is_server(&self) -> bool {
        matches!(self.target, MessageTarget::Server)
    }

    /// Get plain text content (strip BBCode)
    pub fn plain_text(&self) -> String {
        super::bbcode::strip_bbcode(&self.content)
    }
}

/// Builder for creating messages to send
#[derive(Debug, Clone)]
pub struct MessageBuilder {
    content: String,
    target: Option<MessageTarget>,
}

impl MessageBuilder {
    /// Create a new message builder
    pub fn new() -> Self {
        Self {
            content: String::new(),
            target: None,
        }
    }

    /// Set the message content
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    /// Add text to the message
    pub fn text(mut self, text: impl AsRef<str>) -> Self {
        self.content.push_str(text.as_ref());
        self
    }

    /// Add bold text
    pub fn bold(mut self, text: impl AsRef<str>) -> Self {
        self.content.push_str("[b]");
        self.content.push_str(text.as_ref());
        self.content.push_str("[/b]");
        self
    }

    /// Add italic text
    pub fn italic(mut self, text: impl AsRef<str>) -> Self {
        self.content.push_str("[i]");
        self.content.push_str(text.as_ref());
        self.content.push_str("[/i]");
        self
    }

    /// Add underlined text
    pub fn underline(mut self, text: impl AsRef<str>) -> Self {
        self.content.push_str("[u]");
        self.content.push_str(text.as_ref());
        self.content.push_str("[/u]");
        self
    }

    /// Add colored text
    pub fn color(mut self, color: impl AsRef<str>, text: impl AsRef<str>) -> Self {
        self.content.push_str("[color=");
        self.content.push_str(color.as_ref());
        self.content.push_str("]");
        self.content.push_str(text.as_ref());
        self.content.push_str("[/color]");
        self
    }

    /// Add a URL
    pub fn url(mut self, url: impl AsRef<str>, text: Option<&str>) -> Self {
        self.content.push_str("[url=");
        self.content.push_str(url.as_ref());
        self.content.push_str("]");
        self.content.push_str(text.unwrap_or(url.as_ref()));
        self.content.push_str("[/url]");
        self
    }

    /// Add a newline
    pub fn newline(mut self) -> Self {
        self.content.push('\n');
        self
    }

    /// Set target to server
    pub fn to_server(mut self) -> Self {
        self.target = Some(MessageTarget::Server);
        self
    }

    /// Set target to channel
    pub fn to_channel(mut self, channel_id: u64) -> Self {
        self.target = Some(MessageTarget::Channel(channel_id));
        self
    }

    /// Set target to private message
    pub fn to_user(mut self, user_id: u16) -> Self {
        self.target = Some(MessageTarget::Private(user_id));
        self
    }

    /// Build the message content
    pub fn build(self) -> String {
        self.content
    }

    /// Get the target
    pub fn target(&self) -> Option<MessageTarget> {
        self.target
    }
}

impl Default for MessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_ids_auto_increment() {
        let m1 = ChatMessage::new(1, "A", MessageTarget::Server, "hi");
        let m2 = ChatMessage::new(1, "A", MessageTarget::Server, "ho");
        assert!(m2.id > m1.id);
    }

    #[test]
    fn is_private() {
        let m = ChatMessage::new(1, "A", MessageTarget::Private(2), "hi");
        assert!(m.is_private());
        assert!(!m.is_channel());
        assert!(!m.is_server());
    }

    #[test]
    fn is_channel() {
        let m = ChatMessage::new(1, "A", MessageTarget::Channel(5), "hi");
        assert!(m.is_channel());
        assert!(!m.is_private());
        assert!(!m.is_server());
    }

    #[test]
    fn is_server() {
        let m = ChatMessage::new(1, "A", MessageTarget::Server, "hi");
        assert!(m.is_server());
        assert!(!m.is_private());
        assert!(!m.is_channel());
    }

    #[test]
    fn plain_text_strips_bbcode() {
        let m = ChatMessage::new(1, "A", MessageTarget::Server, "[b]hello[/b]");
        assert_eq!(m.plain_text(), "hello");
    }

    #[test]
    fn builder_content() {
        let msg = MessageBuilder::new().content("hello").build();
        assert_eq!(msg, "hello");
    }

    #[test]
    fn builder_text_appends() {
        let msg = MessageBuilder::new().text("a").text("b").build();
        assert_eq!(msg, "ab");
    }

    #[test]
    fn builder_bold() {
        let msg = MessageBuilder::new().bold("x").build();
        assert_eq!(msg, "[b]x[/b]");
    }

    #[test]
    fn builder_italic() {
        let msg = MessageBuilder::new().italic("x").build();
        assert_eq!(msg, "[i]x[/i]");
    }

    #[test]
    fn builder_underline() {
        let msg = MessageBuilder::new().underline("x").build();
        assert_eq!(msg, "[u]x[/u]");
    }

    #[test]
    fn builder_color() {
        let msg = MessageBuilder::new().color("red", "x").build();
        assert_eq!(msg, "[color=red]x[/color]");
    }

    #[test]
    fn builder_url_with_text() {
        let msg = MessageBuilder::new().url("http://ex.com", Some("click")).build();
        assert_eq!(msg, "[url=http://ex.com]click[/url]");
    }

    #[test]
    fn builder_url_without_text() {
        let msg = MessageBuilder::new().url("http://ex.com", None).build();
        assert_eq!(msg, "[url=http://ex.com]http://ex.com[/url]");
    }

    #[test]
    fn builder_newline() {
        let msg = MessageBuilder::new().text("a").newline().text("b").build();
        assert_eq!(msg, "a\nb");
    }

    #[test]
    fn builder_chaining() {
        let msg = MessageBuilder::new()
            .bold("hi")
            .text(" ")
            .italic("world")
            .build();
        assert_eq!(msg, "[b]hi[/b] [i]world[/i]");
    }

    #[test]
    fn builder_targets() {
        let b = MessageBuilder::new().to_server();
        assert_eq!(b.target(), Some(MessageTarget::Server));

        let b = MessageBuilder::new().to_channel(42);
        assert_eq!(b.target(), Some(MessageTarget::Channel(42)));

        let b = MessageBuilder::new().to_user(7);
        assert_eq!(b.target(), Some(MessageTarget::Private(7)));
    }

    #[test]
    fn builder_default_has_no_target() {
        let b = MessageBuilder::new();
        assert_eq!(b.target(), None);
    }
}
