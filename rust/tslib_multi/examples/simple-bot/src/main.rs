//! Simple TeamSpeak bot example
//!
//! This example demonstrates how to create a basic bot that:
//! - Connects to a TeamSpeak server
//! - Responds to commands
//! - Handles events

use anyhow::Result;
use clap::Parser;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use tslib_bot::{Bot, BotConfig};
use tslib_core::Identity;

/// Simple TeamSpeak Bot
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Server address (host:port)
    #[arg(short, long)]
    server: String,

    /// Bot nickname
    #[arg(short, long, default_value = "TsLibBot")]
    nickname: String,

    /// Server password (optional)
    #[arg(short, long)]
    password: Option<String>,

    /// Identity file path (creates new if not exists)
    #[arg(short, long, default_value = "identity.json")]
    identity: String,

    /// Command prefix
    #[arg(long, default_value = "!")]
    prefix: String,

    /// Bot owner unique ID (can be specified multiple times)
    #[arg(long)]
    owner: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Parse arguments
    let args = Args::parse();

    // Load or create identity
    let identity = if std::path::Path::new(&args.identity).exists() {
        info!("Loading identity from {}", args.identity);
        Identity::load(&args.identity)?
    } else {
        info!("Creating new identity");
        let identity = Identity::create()?;
        identity.save(&args.identity)?;
        info!("Identity saved to {}", args.identity);
        identity
    };

    info!("Identity UID: {}", identity.unique_id());
    info!("Security level: {}", identity.security_level());

    // Build bot configuration
    let mut config_builder = BotConfig::builder()
        .address(&args.server)
        .identity(identity)
        .nickname(&args.nickname)
        .command_prefix(&args.prefix)
        .auto_reconnect(true);

    if let Some(password) = args.password {
        config_builder = config_builder.password(password);
    }

    for owner in args.owner {
        config_builder = config_builder.owner(owner);
    }

    let config = config_builder.build()?;

    // Create bot
    let mut bot = Bot::new(config).await?;

    // Register custom commands
    register_commands(&bot).await;

    info!("Starting bot...");

    // Run the bot
    bot.run().await?;

    Ok(())
}

async fn register_commands(bot: &Bot) {
    // Echo command
    bot.command("echo", |ctx| async move {
        let message = ctx.args_string();
        if message.is_empty() {
            ctx.reply("Usage: !echo <message>").await
        } else {
            ctx.reply(message).await
        }
    })
    .await;

    // Say command (formatted)
    bot.command("say", |ctx| async move {
        let message = ctx.args_string();
        if message.is_empty() {
            ctx.reply("Usage: !say <message>").await
        } else {
            ctx.reply(format!("[b]Bot says:[/b] {}", message)).await
        }
    })
    .await;

    // Info command
    bot.command("info", |ctx| async move {
        ctx.reply(format!(
            "[b]TsLib Bot[/b]\n\
             Version: {}\n\
             Commands: Use {}help for a list of commands",
            env!("CARGO_PKG_VERSION"),
            "!" // TODO: Get actual prefix
        ))
        .await
    })
    .await;

    // Uptime command
    bot.command("uptime", |ctx| async move {
        // TODO: Track actual uptime
        ctx.reply("Bot uptime: Just started!").await
    })
    .await;

    // Dice roll command
    bot.command("roll", |ctx| async move {
        use std::time::{SystemTime, UNIX_EPOCH};

        let sides: u32 = ctx
            .arg(0)
            .and_then(|s| s.parse().ok())
            .unwrap_or(6);

        if sides < 2 {
            return ctx.reply("Dice must have at least 2 sides!").await;
        }

        // Simple random using time
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let roll = (nanos % sides) + 1;

        ctx.reply(format!("🎲 Rolling d{}... [b]{}[/b]!", sides, roll))
            .await
    })
    .await;

    info!("Registered custom commands: echo, say, info, uptime, roll");
}
