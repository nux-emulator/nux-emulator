//! Network backend selection logic.

use super::bridge;
use super::config::{NetworkBackend, NetworkVmConfig};
use super::error::{NetworkError, NetworkResult};
use super::passt;
use std::ffi::OsString;

/// The resolved network backend after detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedBackend {
    /// TAP device attached to a bridge.
    Tap {
        /// The bridge interface name.
        bridge_name: String,
    },
    /// passt userspace networking.
    Passt,
    /// Networking is disabled.
    Disabled,
}

/// Result of backend selection, including crosvm args and ADB address.
#[derive(Debug)]
pub struct NetworkSetup {
    /// The resolved backend.
    pub backend: ResolvedBackend,
    /// Additional crosvm CLI arguments for networking.
    pub crosvm_args: Vec<OsString>,
    /// The ADB connection address (host:port).
    pub adb_address: String,
}

/// Select the best available network backend based on config and host state.
///
/// Selection logic:
/// - If networking is disabled, returns `Disabled`.
/// - `Auto`: try TAP first (bridge must exist), fall back to passt, else error.
/// - `Tap`: require the bridge, error if missing.
/// - `Passt`: require the passt binary, error if missing.
///
/// # Errors
///
/// Returns `NetworkError::NoBackendAvailable` if no backend can be used,
/// `NetworkError::BridgeNotFound` if TAP is explicitly requested but the bridge
/// is missing, or `NetworkError::PasstNotFound` if passt is explicitly requested
/// but not installed.
pub fn select_backend(config: &NetworkVmConfig) -> NetworkResult<NetworkSetup> {
    if !config.enabled {
        return Ok(NetworkSetup {
            backend: ResolvedBackend::Disabled,
            crosvm_args: Vec::new(),
            adb_address: String::new(),
        });
    }

    match config.backend {
        NetworkBackend::Auto => select_auto(config),
        NetworkBackend::Tap => select_tap(config),
        NetworkBackend::Passt => select_passt(config),
    }
}

fn select_auto(config: &NetworkVmConfig) -> NetworkResult<NetworkSetup> {
    // Try TAP first
    match bridge::bridge_exists(&config.bridge_name) {
        Ok(true) => {
            log::info!(
                "TAP backend selected: bridge '{}' found",
                config.bridge_name
            );
            return Ok(make_tap_setup(config));
        }
        Ok(false) => {
            log::info!(
                "bridge '{}' not found, trying passt fallback",
                config.bridge_name
            );
        }
        Err(e) => {
            log::warn!("bridge detection error: {e}, trying passt fallback");
        }
    }

    // Try passt
    if passt::passt_available() {
        log::info!("passt backend selected as fallback");
        return Ok(make_passt_setup(config));
    }

    Err(NetworkError::NoBackendAvailable {
        reason: format!(
            "bridge '{}' not found and passt binary not on $PATH",
            config.bridge_name
        ),
    })
}

fn select_tap(config: &NetworkVmConfig) -> NetworkResult<NetworkSetup> {
    if bridge::bridge_exists(&config.bridge_name)? {
        Ok(make_tap_setup(config))
    } else {
        Err(NetworkError::BridgeNotFound(config.bridge_name.clone()))
    }
}

fn select_passt(config: &NetworkVmConfig) -> NetworkResult<NetworkSetup> {
    if passt::passt_available() {
        Ok(make_passt_setup(config))
    } else {
        Err(NetworkError::PasstNotFound)
    }
}

fn make_tap_setup(config: &NetworkVmConfig) -> NetworkSetup {
    use super::tap;
    NetworkSetup {
        backend: ResolvedBackend::Tap {
            bridge_name: config.bridge_name.clone(),
        },
        crosvm_args: tap::build_tap_args(config),
        adb_address: tap::guest_adb_address(config),
    }
}

fn make_passt_setup(config: &NetworkVmConfig) -> NetworkSetup {
    let socket_path = passt::default_socket_path();
    NetworkSetup {
        backend: ResolvedBackend::Passt,
        crosvm_args: passt::build_passt_args(&socket_path),
        adb_address: passt::passt_adb_address(config),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_networking_returns_disabled() {
        let config = NetworkVmConfig {
            enabled: false,
            ..NetworkVmConfig::default()
        };
        let setup = select_backend(&config).unwrap();
        assert_eq!(setup.backend, ResolvedBackend::Disabled);
        assert!(setup.crosvm_args.is_empty());
    }

    #[test]
    fn auto_with_no_bridge_no_passt_returns_error() {
        let config = NetworkVmConfig {
            backend: NetworkBackend::Auto,
            bridge_name: "nux-test-nonexistent-br99".to_owned(),
            ..NetworkVmConfig::default()
        };
        // This will fail unless passt is installed, which is the expected test env
        let result = select_backend(&config);
        // Either passt is found (Ok with Passt backend) or error
        match result {
            Ok(setup) => assert_eq!(setup.backend, ResolvedBackend::Passt),
            Err(NetworkError::NoBackendAvailable { .. }) => {} // expected in CI
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn explicit_tap_with_no_bridge_returns_error() {
        let config = NetworkVmConfig {
            backend: NetworkBackend::Tap,
            bridge_name: "nux-test-nonexistent-br99".to_owned(),
            ..NetworkVmConfig::default()
        };
        let result = select_backend(&config);
        assert!(result.is_err());
    }

    #[test]
    fn explicit_passt_when_unavailable() {
        // This test depends on whether passt is installed
        let config = NetworkVmConfig {
            backend: NetworkBackend::Passt,
            ..NetworkVmConfig::default()
        };
        let result = select_backend(&config);
        // Either Ok (passt installed) or PasstNotFound
        match result {
            Ok(setup) => assert_eq!(setup.backend, ResolvedBackend::Passt),
            Err(NetworkError::PasstNotFound) => {}
            Err(e) => panic!("unexpected error: {e}"),
        }
    }
}
