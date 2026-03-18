//! Network configuration types.

use serde::{Deserialize, Serialize};

/// Network backend selection.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkBackend {
    /// TAP device attached to a bridge (requires root setup).
    #[default]
    Tap,
    /// passt userspace networking (no root required).
    Passt,
    /// Automatically select: TAP if available, else passt.
    Auto,
}

/// Network configuration for a Nux VM instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkVmConfig {
    /// Preferred backend (auto, tap, or passt).
    pub backend: NetworkBackend,
    /// Bridge interface name for TAP mode.
    pub bridge_name: String,
    /// Static guest IP address in TAP mode.
    pub guest_ip: String,
    /// Host-side ADB port for port forwarding.
    pub adb_port: u16,
    /// Guest-side ADB port.
    pub guest_adb_port: u16,
    /// Whether networking is enabled at all.
    pub enabled: bool,
}

impl Default for NetworkVmConfig {
    fn default() -> Self {
        Self {
            backend: NetworkBackend::Auto,
            bridge_name: "nux-br0".to_owned(),
            guest_ip: "192.168.100.2".to_owned(),
            adb_port: 5555,
            guest_adb_port: 5555,
            enabled: true,
        }
    }
}
