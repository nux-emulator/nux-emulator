//! Shared types for the ADB bridge module.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during ADB operations.
#[derive(Debug, Error)]
pub enum AdbError {
    /// The guest refused the TCP connection (adbd not listening).
    #[error("connection refused: adbd is not listening on {0}")]
    ConnectionRefused(String),

    /// An operation timed out.
    #[error("ADB operation timed out: {0}")]
    Timeout(String),

    /// ADB protocol-level error (unexpected message, bad checksum, etc.).
    #[error("ADB protocol error: {0}")]
    ProtocolError(String),

    /// The guest returned an error (e.g. shell command failed).
    #[error("guest error: {0}")]
    GuestError(String),

    /// The client is not connected to the guest.
    #[error("not connected to guest — call connect() first")]
    NotConnected,

    /// I/O error wrapper.
    #[error("ADB I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The host file was not found (e.g. APK path doesn't exist).
    #[error("host file not found: {0}")]
    FileNotFound(PathBuf),

    /// Sync protocol error during file transfer.
    #[error("sync protocol error: {0}")]
    SyncError(String),
}

/// Result type alias for ADB operations.
pub type AdbResult<T> = Result<T, AdbError>;

/// Connection state of the ADB client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected to the guest.
    Disconnected,
    /// Attempting to connect.
    Connecting,
    /// Connected and ready for operations.
    Connected,
    /// Connection failed with an error message.
    Error(String),
}

/// Preferred transport mechanism for ADB communication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportKind {
    /// TCP over the virtual network (default).
    Tcp,
    /// Virtio-serial direct channel.
    VirtioSerial,
}

/// Information about an installed Android package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInfo {
    /// Fully-qualified package name (e.g. `com.example.app`).
    pub package_name: String,
}

/// Information about the connected Android device (guest).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeviceInfo {
    /// Android version (e.g. "16").
    pub android_version: Option<String>,
    /// SDK level (e.g. "36").
    pub sdk_level: Option<String>,
    /// Device model string.
    pub model: Option<String>,
    /// Supported CPU ABIs (e.g. "x86_64,arm64-v8a").
    pub cpu_abi_list: Option<String>,
}

/// Configuration for the ADB client.
#[derive(Debug, Clone)]
pub struct AdbConfig {
    /// Guest IP address for TCP transport.
    pub guest_ip: String,
    /// Guest ADB port for TCP transport.
    pub guest_port: u16,
    /// Path to the virtio-serial device for fallback transport.
    pub virtio_serial_path: PathBuf,
    /// Preferred transport kind.
    pub preferred_transport: TransportKind,
    /// Connection timeout in milliseconds.
    pub connect_timeout_ms: u64,
    /// Default command timeout in milliseconds.
    pub command_timeout_ms: u64,
}

impl Default for AdbConfig {
    fn default() -> Self {
        Self {
            guest_ip: "127.0.0.1".to_owned(),
            guest_port: 5555,
            virtio_serial_path: PathBuf::from("/dev/virtio-ports/adb"),
            preferred_transport: TransportKind::Tcp,
            connect_timeout_ms: 5000,
            command_timeout_ms: 30_000,
        }
    }
}
