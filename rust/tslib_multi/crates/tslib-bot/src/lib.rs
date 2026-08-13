//! # tslib-bot
//!
//! Bot framework for tslib providing:
//! - Command system with prefix support
//! - Plugin architecture
//! - Permission system
//! - Auto-reconnection
//!
//! ## Example
//!
//! ```rust,no_run
//! use tslib_bot::{Bot, BotConfig, Command, CommandContext};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = BotConfig::builder()
//!     .address("localhost")
//!     .nickname("MyBot")
//!     .command_prefix("!")
//!     .build()?;
//!
//! let mut bot = Bot::new(config).await?;
//!
//! bot.command("ping", |ctx| async move {
//!     ctx.reply("Pong!").await
//! });
//!
//! bot.run().await?;
//! # Ok(())
//! # }
//! ```

pub mod bot;
pub mod command;
pub mod config;
pub mod error;
pub mod plugin;

// Re-exports
pub use bot::Bot;
pub use command::{Command, CommandContext, CommandHandler};
pub use config::BotConfig;
pub use error::{BotError, Result};
pub use plugin::{Plugin, PluginManager};
