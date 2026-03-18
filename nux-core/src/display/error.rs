//! Display pipeline error types.

use thiserror::Error;

/// Errors that can occur in the display pipeline.
#[derive(Debug, Error)]
pub enum DisplayError {
    /// Display configuration validation failed.
    #[error("display config validation failed: {0}")]
    ConfigValidation(String),

    /// Dmabuf import failed (driver or kernel doesn't support it).
    #[error("dmabuf import failed: {0}")]
    DmabufImportFailed(String),

    /// Shared memory mapping failed.
    #[error("shared memory map failed: {0}")]
    ShmMapFailed(String),

    /// The frame delivery channel was closed unexpectedly.
    #[error("frame channel closed")]
    ChannelClosed,

    /// The capture backend is not yet initialized.
    #[error("capture backend not initialized")]
    NotInitialized,

    /// I/O error wrapper.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for display pipeline operations.
pub type DisplayResult<T> = Result<T, DisplayError>;
