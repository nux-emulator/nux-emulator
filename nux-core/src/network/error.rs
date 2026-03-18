//! Network error types.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during network setup and management.
#[derive(Debug, Error)]
pub enum NetworkError {
    /// TAP bridge not found on the host.
    #[error("bridge interface '{0}' not found — run `sudo scripts/setup-network.sh` to create it")]
    BridgeNotFound(String),

    /// passt binary not found on `$PATH`.
    #[error(
        "passt binary not found on $PATH — install passt or run `sudo scripts/setup-network.sh` for TAP networking"
    )]
    PasstNotFound,

    /// No network backend is available (neither TAP nor passt).
    #[error(
        "no network backend available: {reason}. Either run `sudo scripts/setup-network.sh` \
         to set up TAP networking, or install passt for userspace networking"
    )]
    NoBackendAvailable { reason: String },

    /// Failed to spawn the passt process.
    #[error("failed to spawn passt: {0}")]
    PasstSpawnFailed(String),

    /// passt process exited unexpectedly.
    #[error("passt exited unexpectedly: exit code {exit_code}, stderr: {stderr}")]
    PasstCrashed { exit_code: i32, stderr: String },

    /// TAP device creation failed.
    #[error("failed to create TAP device: {0}")]
    TapCreationFailed(String),

    /// Bridge configuration is invalid or incomplete.
    #[error("bridge '{name}' exists but is misconfigured: {detail}")]
    BridgeMisconfigured { name: String, detail: String },

    /// Socket path error.
    #[error("passt socket error: {0}")]
    SocketError(String),

    /// I/O error wrapper.
    #[error("network I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The passt socket path does not exist after spawning.
    #[error("passt socket not found at {0} after startup")]
    PasstSocketNotFound(PathBuf),
}

/// Result type alias for network operations.
pub type NetworkResult<T> = Result<T, NetworkError>;
