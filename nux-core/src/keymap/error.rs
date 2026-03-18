//! Keymap error types.

use thiserror::Error;

/// Errors that can occur during keymap operations.
#[derive(Debug, Error)]
pub enum KeymapError {
    /// TOML parsing or deserialization failed.
    #[error("keymap parse error: {0}")]
    ParseError(String),

    /// Semantic validation failed.
    #[error("keymap validation error: {0}")]
    ValidationError(String),

    /// File I/O error.
    #[error("keymap I/O error: {0}")]
    IoError(#[from] std::io::Error),
}
