//! Error types for root management operations.

use crate::config::RootMode;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during root management operations.
#[derive(Debug, Error)]
pub enum RootError {
    /// The requested boot image file does not exist on disk.
    #[error("boot image not found: {0}")]
    ImageNotFound(PathBuf),

    /// The boot image file exists but is empty (zero bytes).
    #[error("boot image is empty or corrupt: {0}")]
    ImageEmpty(PathBuf),

    /// A patched boot image is required but has not been created yet.
    #[error("patched boot image for {0:?} does not exist; run the patching workflow first")]
    PatchedImageMissing(RootMode),

    /// The stock boot image is missing from the instance directory.
    #[error("stock boot image not found in instance directory: {0}")]
    StockImageMissing(PathBuf),

    /// An ADB operation failed during the patching workflow.
    #[error("ADB {operation} failed: {detail}")]
    Adb {
        /// Which ADB operation failed (install, push, pull).
        operation: String,
        /// Error detail from the ADB layer.
        detail: String,
    },

    /// The patching workflow was aborted because a step failed.
    #[error("patching workflow aborted at step '{step}': {cause}")]
    WorkflowAborted {
        /// The step that failed.
        step: String,
        /// The underlying cause.
        cause: String,
    },

    /// I/O error during file operations.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for root management operations.
pub type RootResult<T> = Result<T, RootError>;
