//! Provider switching state machine.
//!
//! Validates preconditions, dispatches to the correct transition,
//! and updates config after each switch.

use crate::config::{GAppsSource, GoogleServicesConfig, GoogleServicesProvider};
use crate::gservices::download::{download_gapps, resolve_package_info, verify_hash};
use crate::gservices::overlay::{
    apply_gapps_overlay, apply_removal_overlay, backup_overlay, instance_overlay_dir,
    reset_overlay_to_base, restore_overlay,
};
use crate::gservices::types::{GServicesError, GServicesResult};
use std::path::Path;

/// Whether the VM is currently running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmState {
    Running,
    Stopped,
}

/// Result of a successful provider switch.
#[derive(Debug, Clone)]
pub struct SwitchResult {
    /// The new provider after switching.
    pub new_provider: GoogleServicesProvider,
    /// Whether a VM restart is required.
    pub restart_required: bool,
}

/// Switch the Google Services provider for an instance.
///
/// Validates that the VM is stopped, then dispatches to the appropriate
/// transition. On success, returns a `SwitchResult` indicating the new
/// provider and that a restart is required.
///
/// The caller is responsible for persisting the updated config.
///
/// # Errors
///
/// Returns `GServicesError::VmRunning` if the VM is not stopped.
/// Returns `GServicesError::AlreadyActive` if already on the target provider.
/// Returns other errors on download, overlay, or I/O failures.
pub async fn switch_provider(
    instance_name: &str,
    current: GoogleServicesProvider,
    target: GoogleServicesProvider,
    gapps_source: GAppsSource,
    vm_state: VmState,
) -> GServicesResult<SwitchResult> {
    // Precondition: VM must be stopped
    if vm_state == VmState::Running {
        return Err(GServicesError::VmRunning);
    }

    // No-op if already on target
    if current == target {
        return Err(GServicesError::AlreadyActive(target));
    }

    let overlay_dir = instance_overlay_dir(instance_name);

    // Back up overlay before any modification
    backup_overlay(&overlay_dir).await?;

    let result = perform_transition(&overlay_dir, current, target, gapps_source).await;

    // Rollback on failure
    if result.is_err() {
        log::warn!("provider switch failed, restoring overlay backup");
        if let Err(restore_err) = restore_overlay(&overlay_dir).await {
            log::error!("overlay restore also failed: {restore_err}");
        }
        return result;
    }

    result
}

/// Perform the actual transition between providers.
async fn perform_transition(
    overlay_dir: &Path,
    _current: GoogleServicesProvider,
    target: GoogleServicesProvider,
    gapps_source: GAppsSource,
) -> GServicesResult<SwitchResult> {
    match target {
        GoogleServicesProvider::Gapps => transition_to_gapps(overlay_dir, gapps_source).await,
        GoogleServicesProvider::Microg => transition_to_microg(overlay_dir).await,
        GoogleServicesProvider::None => transition_to_none(overlay_dir).await,
    }
}

/// Transition to `GApps`: download, verify, apply overlay.
async fn transition_to_gapps(
    overlay_dir: &Path,
    source: GAppsSource,
) -> GServicesResult<SwitchResult> {
    let info = resolve_package_info(source);

    // Download (or use cache)
    #[allow(clippy::cast_precision_loss)]
    let zip_path = download_gapps(&info, |downloaded, total| {
        if total > 0 {
            log::info!(
                "downloading GApps: {:.1}%",
                (downloaded as f64 / total as f64) * 100.0
            );
        }
    })
    .await?;

    // Verify integrity
    verify_hash(&zip_path, &info.sha256).await?;

    // Reset overlay first, then apply GApps
    reset_overlay_to_base(overlay_dir).await?;
    apply_gapps_overlay(overlay_dir, &zip_path).await?;

    Ok(SwitchResult {
        new_provider: GoogleServicesProvider::Gapps,
        restart_required: true,
    })
}

/// Transition to `MicroG`: reset overlay to base image state.
async fn transition_to_microg(overlay_dir: &Path) -> GServicesResult<SwitchResult> {
    reset_overlay_to_base(overlay_dir).await?;

    Ok(SwitchResult {
        new_provider: GoogleServicesProvider::Microg,
        restart_required: true,
    })
}

/// Transition to None: apply removal overlay.
async fn transition_to_none(overlay_dir: &Path) -> GServicesResult<SwitchResult> {
    reset_overlay_to_base(overlay_dir).await?;
    apply_removal_overlay(overlay_dir).await?;

    Ok(SwitchResult {
        new_provider: GoogleServicesProvider::None,
        restart_required: true,
    })
}

/// Build an updated `GoogleServicesConfig` after a successful switch.
pub fn updated_config(
    current: &GoogleServicesConfig,
    result: &SwitchResult,
) -> GoogleServicesConfig {
    GoogleServicesConfig {
        provider: result.new_provider,
        // Version unknown until next live detection
        provider_version: None,
        gapps_source: current.gapps_source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reject_switch_while_vm_running() {
        let result = switch_provider(
            "test",
            GoogleServicesProvider::Microg,
            GoogleServicesProvider::Gapps,
            GAppsSource::Opengapps,
            VmState::Running,
        )
        .await;
        assert!(matches!(result, Err(GServicesError::VmRunning)));
    }

    #[tokio::test]
    async fn reject_switch_to_same_provider() {
        let result = switch_provider(
            "test",
            GoogleServicesProvider::Microg,
            GoogleServicesProvider::Microg,
            GAppsSource::Opengapps,
            VmState::Stopped,
        )
        .await;
        assert!(matches!(
            result,
            Err(GServicesError::AlreadyActive(
                GoogleServicesProvider::Microg
            ))
        ));
    }

    #[tokio::test]
    async fn transition_to_microg_resets_overlay() {
        let dir = tempfile::tempdir().unwrap();
        let overlay = dir.path().join("overlay");
        tokio::fs::create_dir_all(&overlay).await.unwrap();
        tokio::fs::write(overlay.join("marker"), "gapps")
            .await
            .unwrap();

        let result = transition_to_microg(&overlay).await.unwrap();
        assert_eq!(result.new_provider, GoogleServicesProvider::Microg);
        assert!(result.restart_required);
        assert!(!overlay.exists());
    }

    #[tokio::test]
    async fn transition_to_none_creates_removal_marker() {
        let dir = tempfile::tempdir().unwrap();
        let overlay = dir.path().join("overlay");

        let result = transition_to_none(&overlay).await.unwrap();
        assert_eq!(result.new_provider, GoogleServicesProvider::None);
        assert!(result.restart_required);

        let marker = tokio::fs::read_to_string(overlay.join(".gservices_provider"))
            .await
            .unwrap();
        assert_eq!(marker, "none");
    }

    #[test]
    fn updated_config_clears_version() {
        let current = GoogleServicesConfig {
            provider: GoogleServicesProvider::Microg,
            provider_version: Some("1.0".to_owned()),
            gapps_source: GAppsSource::Mindthegapps,
        };
        let result = SwitchResult {
            new_provider: GoogleServicesProvider::Gapps,
            restart_required: true,
        };
        let new_cfg = updated_config(&current, &result);
        assert_eq!(new_cfg.provider, GoogleServicesProvider::Gapps);
        assert_eq!(new_cfg.provider_version, None);
        assert_eq!(new_cfg.gapps_source, GAppsSource::Mindthegapps);
    }
}
