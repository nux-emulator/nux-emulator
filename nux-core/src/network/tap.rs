//! TAP device setup for crosvm networking.

use super::config::NetworkVmConfig;
use std::ffi::OsString;

/// Build crosvm CLI arguments for TAP networking.
///
/// Appends `--net tap-name=<tap_name>` where the tap name is derived from
/// the bridge name. crosvm creates the TAP device itself when given a name.
pub fn build_tap_args(config: &NetworkVmConfig) -> Vec<OsString> {
    let tap_name = tap_device_name(&config.bridge_name);
    vec!["--net".into(), format!("tap-name={tap_name}").into()]
}

/// Derive a TAP device name from the bridge name.
///
/// Convention: replace `br` suffix with `tap`, e.g. `nux-br0` → `nux-tap0`.
/// If the name doesn't follow the convention, append `-tap`.
fn tap_device_name(bridge_name: &str) -> String {
    if let Some(prefix) = bridge_name.strip_prefix("nux-br") {
        format!("nux-tap{prefix}")
    } else {
        format!("{bridge_name}-tap")
    }
}

/// Get the guest IP address for direct ADB connection in TAP mode.
pub fn guest_adb_address(config: &NetworkVmConfig) -> String {
    format!("{}:{}", config.guest_ip, config.guest_adb_port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_name_from_default_bridge() {
        assert_eq!(tap_device_name("nux-br0"), "nux-tap0");
    }

    #[test]
    fn tap_name_from_custom_bridge() {
        assert_eq!(tap_device_name("custom-bridge"), "custom-bridge-tap");
    }

    #[test]
    fn tap_args_contain_net_flag() {
        let config = NetworkVmConfig::default();
        let args = build_tap_args(&config);
        let strings: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(strings[0], "--net");
        assert_eq!(strings[1], "tap-name=nux-tap0");
    }

    #[test]
    fn guest_adb_address_default() {
        let config = NetworkVmConfig::default();
        assert_eq!(guest_adb_address(&config), "192.168.100.2:5555");
    }
}
