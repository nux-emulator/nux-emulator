//! TAP bridge detection and validation.

use super::error::{NetworkError, NetworkResult};
use std::path::Path;

/// Check whether a network bridge interface exists on the host.
///
/// Reads from `/sys/class/net/<name>/bridge/` to determine if the interface
/// is a bridge.
///
/// # Errors
///
/// Returns `NetworkError::BridgeNotFound` if the interface does not exist,
/// or `NetworkError::BridgeMisconfigured` if it exists but is not a bridge.
pub fn bridge_exists(name: &str) -> NetworkResult<bool> {
    let iface_path = format!("/sys/class/net/{name}");
    if !Path::new(&iface_path).exists() {
        return Ok(false);
    }

    let bridge_path = format!("/sys/class/net/{name}/bridge");
    if !Path::new(&bridge_path).exists() {
        return Err(NetworkError::BridgeMisconfigured {
            name: name.to_owned(),
            detail: "interface exists but is not a bridge".to_owned(),
        });
    }

    Ok(true)
}

/// Validate that a bridge has an IP address configured (operstate is up).
///
/// # Errors
///
/// Returns `NetworkError::BridgeMisconfigured` if the bridge is not in an
/// operational state.
pub fn validate_bridge(name: &str) -> NetworkResult<()> {
    let operstate_path = format!("/sys/class/net/{name}/operstate");
    let state = std::fs::read_to_string(&operstate_path).map_err(|e| {
        NetworkError::BridgeMisconfigured {
            name: name.to_owned(),
            detail: format!("cannot read operstate: {e}"),
        }
    })?;

    let state = state.trim();
    // "up" or "unknown" are both acceptable (unknown is common for bridges with no carrier)
    if state != "up" && state != "unknown" {
        return Err(NetworkError::BridgeMisconfigured {
            name: name.to_owned(),
            detail: format!("bridge operstate is '{state}', expected 'up' or 'unknown'"),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonexistent_bridge_returns_false() {
        assert!(!bridge_exists("nux-test-nonexistent-br99").unwrap());
    }

    #[test]
    fn loopback_is_not_a_bridge() {
        // lo exists on all Linux systems but is not a bridge
        let result = bridge_exists("lo");
        match result {
            Ok(false) => panic!("lo should exist but not be a bridge"),
            Err(NetworkError::BridgeMisconfigured { .. }) => {} // expected
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
