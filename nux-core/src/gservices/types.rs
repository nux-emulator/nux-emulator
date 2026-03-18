//! Types and errors for the Google Services manager.

use crate::config::{GAppsSource, GoogleServicesProvider};
use thiserror::Error;

/// Errors from Google Services operations.
#[derive(Debug, Error)]
pub enum GServicesError {
    /// The VM must be stopped before switching providers.
    #[error("VM must be stopped before switching providers")]
    VmRunning,

    /// Already on the requested provider.
    #[error("already using provider {0:?}")]
    AlreadyActive(GoogleServicesProvider),

    /// ADB is not connected.
    #[error("ADB is not connected")]
    AdbUnavailable,

    /// `GApps` download failed.
    #[error("GApps download failed: {0}")]
    DownloadFailed(String),

    /// SHA-256 hash mismatch after download.
    #[error("integrity check failed: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    /// Overlay filesystem operation failed.
    #[error("overlay operation failed: {0}")]
    OverlayError(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// ADB command failed.
    #[error("ADB command failed: {0}")]
    AdbError(String),

    /// HTTP error from reqwest.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

/// Result alias for Google Services operations.
pub type GServicesResult<T> = Result<T, GServicesError>;

/// Whether a status reading is from a live ADB query or cached config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Detected live from the running guest via ADB.
    Live,
    /// Read from persisted config (ADB was unavailable).
    Cached,
}

/// Current Google Services status for an instance.
#[derive(Debug, Clone)]
pub struct GoogleServicesStatus {
    /// The active provider.
    pub provider: GoogleServicesProvider,
    /// Version string of the active provider, if known.
    pub version: Option<String>,
    /// Whether this was detected live or read from cache.
    pub freshness: Freshness,
    /// Whether a VM restart is needed to apply a pending switch.
    pub restart_required: bool,
}

/// Metadata about a `GApps` package available for download.
#[derive(Debug, Clone)]
pub struct GAppsPackageInfo {
    /// Download URL.
    pub url: String,
    /// Expected SHA-256 hex digest.
    pub sha256: String,
    /// Which source this package is from.
    pub source: GAppsSource,
    /// Filename for caching.
    pub filename: String,
}

/// Trait abstracting ADB shell access for testability.
///
/// The real `AdbClient` is one implementor; tests provide mocks.
/// Uses a synchronous return of a boxed future for dyn-compatibility.
pub trait AdbShell {
    /// Run a shell command on the guest, returning stdout.
    fn shell_exec(
        &mut self,
        command: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + '_>>;

    /// Whether ADB is currently connected.
    fn is_connected(&self) -> bool;
}
