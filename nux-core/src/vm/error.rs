//! VM error types for crosvm integration.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during VM operations.
#[derive(Debug, Error)]
pub enum VmError {
    /// `/dev/kvm` is not available on this system.
    #[error("KVM is not available: {0}. Enable KVM in your kernel configuration.")]
    KvmNotAvailable(String),

    /// User lacks permissions to access `/dev/kvm`.
    #[error("KVM permission denied: add your user to the `kvm` group")]
    KvmPermissionDenied,

    /// KVM API version is not the expected stable version (12).
    #[error("unsupported KVM API version: expected 12, got {0}")]
    KvmUnsupportedVersion(i32),

    /// A required KVM extension is missing.
    #[error("missing required KVM extension(s): {0}")]
    MissingExtension(String),

    /// CPU lacks hardware virtualization support.
    #[error("CPU feature missing: {0}")]
    CpuFeatureMissing(String),

    /// crosvm binary not found at expected path.
    #[error("crosvm binary not found at {0}")]
    CrosvmNotFound(PathBuf),

    /// crosvm process failed to start.
    #[error("crosvm failed to start: exit code {exit_code}, stderr: {stderr}")]
    CrosvmStartFailed { exit_code: i32, stderr: String },

    /// crosvm process crashed unexpectedly.
    #[error("crosvm crashed: exit code {exit_code}, stderr: {stderr}")]
    CrosvmCrashed { exit_code: i32, stderr: String },

    /// VM configuration validation failed.
    #[error("config validation failed: {0}")]
    ConfigValidation(String),

    /// Control socket error.
    #[error("control socket error: {0}")]
    ControlSocket(String),

    /// Operation timed out.
    #[error("operation timed out: {0}")]
    Timeout(String),

    /// Failed to send signal to process.
    #[error("process signal error: {0}")]
    ProcessSignal(String),

    /// Invalid state transition.
    #[error("invalid state transition: cannot {operation} while in state {state}")]
    InvalidStateTransition { state: String, operation: String },

    /// I/O error wrapper.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for VM operations.
pub type VmResult<T> = Result<T, VmError>;
