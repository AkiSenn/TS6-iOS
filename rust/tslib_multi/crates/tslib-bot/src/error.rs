//! Bot error types

use thiserror::Error;

/// Result type for bot operations
pub type Result<T> = std::result::Result<T, BotError>;

/// Bot-related errors
#[derive(Error, Debug)]
pub enum BotError {
    /// Core library error
    #[error("Core error: {0}")]
    Core(#[from] tslib_core::error::Error),

    /// Command error
    #[error("Command error: {0}")]
    Command(String),

    /// Plugin error
    #[error("Plugin error: {0}")]
    Plugin(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Invalid argument
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Command not found
    #[error("Command not found: {0}")]
    CommandNotFound(String),

    /// Too many arguments
    #[error("Too many arguments")]
    TooManyArguments,

    /// Not enough arguments
    #[error("Not enough arguments: expected {expected}, got {got}")]
    NotEnoughArguments { expected: usize, got: usize },
}
