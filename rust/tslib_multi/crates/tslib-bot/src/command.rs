//! Command system

use crate::error::{BotError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Command context passed to handlers
pub struct CommandContext {
    /// Sender client ID
    pub sender_id: u16,
    /// Sender name
    pub sender_name: String,
    /// Sender unique ID
    pub sender_uid: Option<String>,
    /// Command name
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// Raw message
    pub raw_message: String,
    /// Is private message
    pub is_private: bool,
    /// Reply function
    reply_fn: Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync>,
}

impl CommandContext {
    /// Create a new command context
    pub fn new(
        sender_id: u16,
        sender_name: String,
        sender_uid: Option<String>,
        command: String,
        args: Vec<String>,
        raw_message: String,
        is_private: bool,
        reply_fn: impl Fn(String) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync + 'static,
    ) -> Self {
        Self {
            sender_id,
            sender_name,
            sender_uid,
            command,
            args,
            raw_message,
            is_private,
            reply_fn: Arc::new(reply_fn),
        }
    }

    /// Reply to the command
    pub async fn reply(&self, message: impl Into<String>) -> Result<()> {
        (self.reply_fn)(message.into()).await
    }

    /// Get argument at index
    pub fn arg(&self, index: usize) -> Option<&str> {
        self.args.get(index).map(|s| s.as_str())
    }

    /// Get argument or default
    pub fn arg_or<'a>(&'a self, index: usize, default: &'a str) -> &'a str {
        self.args.get(index).map(|s| s.as_str()).unwrap_or(default)
    }

    /// Get all arguments as a single string
    pub fn args_string(&self) -> String {
        self.args.join(" ")
    }

    /// Get arguments from index onwards as a single string
    pub fn args_from(&self, index: usize) -> String {
        self.args[index..].join(" ")
    }

    /// Check if has enough arguments
    pub fn require_args(&self, count: usize) -> Result<()> {
        if self.args.len() < count {
            Err(BotError::NotEnoughArguments {
                expected: count,
                got: self.args.len(),
            })
        } else {
            Ok(())
        }
    }
}

/// Command handler trait
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Handle the command
    async fn handle(&self, ctx: CommandContext) -> Result<()>;

    /// Get command help text
    fn help(&self) -> Option<&str> {
        None
    }

    /// Get command usage
    fn usage(&self) -> Option<&str> {
        None
    }

    /// Check if user can use this command
    fn can_use(&self, _ctx: &CommandContext) -> bool {
        true
    }
}

/// A registered command
pub struct Command {
    /// Command name
    pub name: String,
    /// Command aliases
    pub aliases: Vec<String>,
    /// Command handler
    pub handler: Arc<dyn CommandHandler>,
    /// Help text
    pub help: Option<String>,
    /// Usage text
    pub usage: Option<String>,
    /// Is owner-only
    pub owner_only: bool,
    /// Is hidden from help
    pub hidden: bool,
}

impl Command {
    /// Create a new command
    pub fn new(name: impl Into<String>, handler: impl CommandHandler + 'static) -> Self {
        Self {
            name: name.into(),
            aliases: Vec::new(),
            handler: Arc::new(handler),
            help: None,
            usage: None,
            owner_only: false,
            hidden: false,
        }
    }

    /// Add an alias
    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    /// Set help text
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Set usage text
    pub fn usage(mut self, usage: impl Into<String>) -> Self {
        self.usage = Some(usage.into());
        self
    }

    /// Mark as owner-only
    pub fn owner_only(mut self) -> Self {
        self.owner_only = true;
        self
    }

    /// Mark as hidden
    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }
}

/// Command registry
pub struct CommandRegistry {
    commands: HashMap<String, Arc<Command>>,
    prefix: String,
}

impl CommandRegistry {
    /// Create a new command registry
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            commands: HashMap::new(),
            prefix: prefix.into(),
        }
    }

    /// Register a command
    pub fn register(&mut self, command: Command) {
        let command = Arc::new(command);

        // Register main name
        self.commands.insert(command.name.clone(), command.clone());

        // Register aliases
        for alias in &command.aliases {
            self.commands.insert(alias.clone(), command.clone());
        }
    }

    /// Get a command by name
    pub fn get(&self, name: &str) -> Option<Arc<Command>> {
        self.commands.get(name).cloned()
    }

    /// Parse a message and return command + context if valid
    pub fn parse<'a>(&self, message: &'a str) -> Option<(&'a str, Vec<String>)> {
        if !message.starts_with(&self.prefix) {
            return None;
        }

        let without_prefix = &message[self.prefix.len()..];
        let mut parts = without_prefix.split_whitespace();

        let command = parts.next()?;
        let args: Vec<String> = parts.map(String::from).collect();

        Some((command, args))
    }

    /// Get all commands (excluding aliases)
    pub fn all_commands(&self) -> Vec<Arc<Command>> {
        let mut seen = std::collections::HashSet::new();
        let mut commands = Vec::new();

        for cmd in self.commands.values() {
            if seen.insert(&cmd.name) {
                commands.push(cmd.clone());
            }
        }

        commands.sort_by(|a, b| a.name.cmp(&b.name));
        commands
    }

    /// Get visible commands for help
    pub fn visible_commands(&self) -> Vec<Arc<Command>> {
        self.all_commands()
            .into_iter()
            .filter(|c| !c.hidden)
            .collect()
    }
}

/// Simple function-based command handler
pub struct FnHandler<F> {
    handler: F,
    help: Option<String>,
}

impl<F, Fut> FnHandler<F>
where
    F: Fn(CommandContext) -> Fut + Send + Sync,
    Fut: Future<Output = Result<()>> + Send,
{
    /// Create a new function handler
    pub fn new(handler: F) -> Self {
        Self {
            handler,
            help: None,
        }
    }

    /// Set help text
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

#[async_trait]
impl<F, Fut> CommandHandler for FnHandler<F>
where
    F: Fn(CommandContext) -> Fut + Send + Sync,
    Fut: Future<Output = Result<()>> + Send,
{
    async fn handle(&self, ctx: CommandContext) -> Result<()> {
        (self.handler)(ctx).await
    }

    fn help(&self) -> Option<&str> {
        self.help.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(command: &str, args: Vec<&str>) -> CommandContext {
        CommandContext::new(
            1,
            "Alice".to_string(),
            Some("uid=abc".to_string()),
            command.to_string(),
            args.into_iter().map(String::from).collect(),
            format!("!{}", command),
            true,
            |_msg| Box::pin(async { Ok(()) }),
        )
    }

    fn dummy_handler() -> impl CommandHandler {
        FnHandler::new(|_ctx| async { Ok(()) })
    }

    // --- CommandContext tests ---

    #[test]
    fn arg_returns_correct_index() {
        let ctx = make_ctx("test", vec!["a", "b", "c"]);
        assert_eq!(ctx.arg(0), Some("a"));
        assert_eq!(ctx.arg(1), Some("b"));
        assert_eq!(ctx.arg(2), Some("c"));
        assert_eq!(ctx.arg(3), None);
    }

    #[test]
    fn arg_or_returns_default() {
        let ctx = make_ctx("test", vec!["a"]);
        assert_eq!(ctx.arg_or(0, "x"), "a");
        assert_eq!(ctx.arg_or(5, "default"), "default");
    }

    #[test]
    fn args_string_joins() {
        let ctx = make_ctx("test", vec!["hello", "world"]);
        assert_eq!(ctx.args_string(), "hello world");
    }

    #[test]
    fn args_string_empty() {
        let ctx = make_ctx("test", vec![]);
        assert_eq!(ctx.args_string(), "");
    }

    #[test]
    fn args_from_index() {
        let ctx = make_ctx("test", vec!["a", "b", "c"]);
        assert_eq!(ctx.args_from(1), "b c");
    }

    #[test]
    fn require_args_ok() {
        let ctx = make_ctx("test", vec!["a", "b"]);
        assert!(ctx.require_args(2).is_ok());
        assert!(ctx.require_args(1).is_ok());
    }

    #[test]
    fn require_args_fail() {
        let ctx = make_ctx("test", vec!["a"]);
        assert!(ctx.require_args(2).is_err());
    }

    // --- CommandRegistry tests ---

    #[test]
    fn parse_with_prefix() {
        let reg = CommandRegistry::new("!");
        let result = reg.parse("!help arg1 arg2");
        assert!(result.is_some());
        let (cmd, args) = result.unwrap();
        assert_eq!(cmd, "help");
        assert_eq!(args, vec!["arg1", "arg2"]);
    }

    #[test]
    fn parse_no_prefix_returns_none() {
        let reg = CommandRegistry::new("!");
        assert!(reg.parse("help").is_none());
    }

    #[test]
    fn parse_command_no_args() {
        let reg = CommandRegistry::new("!");
        let (cmd, args) = reg.parse("!ping").unwrap();
        assert_eq!(cmd, "ping");
        assert!(args.is_empty());
    }

    #[test]
    fn parse_multi_char_prefix() {
        let reg = CommandRegistry::new("!!");
        let (cmd, args) = reg.parse("!!help me").unwrap();
        assert_eq!(cmd, "help");
        assert_eq!(args, vec!["me"]);
    }

    #[test]
    fn register_and_get() {
        let mut reg = CommandRegistry::new("!");
        reg.register(Command::new("ping", dummy_handler()));
        assert!(reg.get("ping").is_some());
        assert!(reg.get("nope").is_none());
    }

    #[test]
    fn register_with_alias() {
        let mut reg = CommandRegistry::new("!");
        reg.register(Command::new("help", dummy_handler()).alias("h").alias("?"));
        assert!(reg.get("help").is_some());
        assert!(reg.get("h").is_some());
        assert!(reg.get("?").is_some());
    }

    #[test]
    fn all_commands_deduplicates_aliases() {
        let mut reg = CommandRegistry::new("!");
        reg.register(Command::new("help", dummy_handler()).alias("h"));
        let all = reg.all_commands();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "help");
    }

    #[test]
    fn visible_commands_excludes_hidden() {
        let mut reg = CommandRegistry::new("!");
        reg.register(Command::new("help", dummy_handler()));
        reg.register(Command::new("secret", dummy_handler()).hidden());
        let visible = reg.visible_commands();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].name, "help");
    }

    #[test]
    fn all_commands_sorted_by_name() {
        let mut reg = CommandRegistry::new("!");
        reg.register(Command::new("zzz", dummy_handler()));
        reg.register(Command::new("aaa", dummy_handler()));
        let all = reg.all_commands();
        assert_eq!(all[0].name, "aaa");
        assert_eq!(all[1].name, "zzz");
    }

    // --- Command builder tests ---

    #[test]
    fn command_builder_flags() {
        let cmd = Command::new("test", dummy_handler())
            .help("Help text")
            .usage("!test <arg>")
            .owner_only()
            .hidden();
        assert_eq!(cmd.help.as_deref(), Some("Help text"));
        assert_eq!(cmd.usage.as_deref(), Some("!test <arg>"));
        assert!(cmd.owner_only);
        assert!(cmd.hidden);
    }

    #[test]
    fn command_default_flags() {
        let cmd = Command::new("test", dummy_handler());
        assert!(!cmd.owner_only);
        assert!(!cmd.hidden);
        assert!(cmd.help.is_none());
        assert!(cmd.usage.is_none());
        assert!(cmd.aliases.is_empty());
    }
}
