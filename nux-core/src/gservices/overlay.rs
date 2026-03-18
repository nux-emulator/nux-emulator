//! Overlay filesystem management for provider switching.
//!
//! Each instance has a writable overlay directory. Provider switches
//! modify this overlay rather than the shared base system image.

use crate::gservices::types::{GServicesError, GServicesResult};
use std::path::{Path, PathBuf};

/// Return the overlay directory for an instance.
///
/// Resolves to `$XDG_DATA_HOME/nux/instances/<name>/overlay/`.
///
/// # Panics
///
/// Panics if the home directory cannot be determined.
pub fn instance_overlay_dir(instance_name: &str) -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| {
        let home = dirs::home_dir().expect("cannot determine home directory");
        home.join(".local").join("share")
    });
    base.join("nux")
        .join("instances")
        .join(instance_name)
        .join("overlay")
}

/// Return the backup path for an overlay directory.
fn backup_path(overlay_dir: &Path) -> PathBuf {
    overlay_dir.with_extension("overlay.backup")
}

/// Back up the current overlay before modification.
///
/// Copies the overlay directory to `<overlay>.backup`. If a backup
/// already exists it is removed first.
///
/// # Errors
///
/// Returns `GServicesError::OverlayError` on I/O failures.
pub async fn backup_overlay(overlay_dir: &Path) -> GServicesResult<()> {
    let backup = backup_path(overlay_dir);

    if backup.exists() {
        tokio::fs::remove_dir_all(&backup).await.map_err(|e| {
            GServicesError::OverlayError(format!(
                "failed to remove old backup {}: {e}",
                backup.display()
            ))
        })?;
    }

    if overlay_dir.exists() {
        copy_dir_recursive(overlay_dir, &backup)
            .await
            .map_err(|e| GServicesError::OverlayError(format!("backup failed: {e}")))?;
    }

    Ok(())
}

/// Restore the overlay from its backup.
///
/// # Errors
///
/// Returns `GServicesError::OverlayError` if the backup doesn't exist or
/// restoration fails.
pub async fn restore_overlay(overlay_dir: &Path) -> GServicesResult<()> {
    let backup = backup_path(overlay_dir);

    if !backup.exists() {
        return Err(GServicesError::OverlayError(
            "no backup found to restore".to_owned(),
        ));
    }

    if overlay_dir.exists() {
        tokio::fs::remove_dir_all(overlay_dir).await.map_err(|e| {
            GServicesError::OverlayError(format!("failed to remove overlay for restore: {e}"))
        })?;
    }

    tokio::fs::rename(&backup, overlay_dir)
        .await
        .map_err(|e| GServicesError::OverlayError(format!("restore failed: {e}")))?;

    Ok(())
}

/// Apply a `GApps` package to the instance overlay.
///
/// Creates the overlay directory structure and writes a marker file
/// indicating `GApps` is active. In a full implementation this would
/// extract the zip contents into the overlay's system partition.
///
/// # Errors
///
/// Returns `GServicesError::OverlayError` on I/O failures.
pub async fn apply_gapps_overlay(overlay_dir: &Path, _gapps_zip: &Path) -> GServicesResult<()> {
    let system_dir = overlay_dir.join("system").join("app");
    tokio::fs::create_dir_all(&system_dir)
        .await
        .map_err(|e| GServicesError::OverlayError(format!("failed to create overlay dirs: {e}")))?;

    // Write provider marker
    let marker = overlay_dir.join(".gservices_provider");
    tokio::fs::write(&marker, "gapps").await.map_err(|e| {
        GServicesError::OverlayError(format!("failed to write provider marker: {e}"))
    })?;

    log::info!("applied GApps overlay to {}", overlay_dir.display());
    Ok(())
}

/// Reset the overlay to base state (`MicroG` pre-installed in base image).
///
/// Removes the overlay directory entirely so the base image shows through.
///
/// # Errors
///
/// Returns `GServicesError::OverlayError` on I/O failures.
pub async fn reset_overlay_to_base(overlay_dir: &Path) -> GServicesResult<()> {
    if overlay_dir.exists() {
        tokio::fs::remove_dir_all(overlay_dir)
            .await
            .map_err(|e| GServicesError::OverlayError(format!("failed to reset overlay: {e}")))?;
    }

    log::info!("reset overlay to base at {}", overlay_dir.display());
    Ok(())
}

/// Apply a removal overlay that disables both `MicroG` and `GApps` (`None` mode).
///
/// Creates an overlay with marker files that instruct the guest init
/// to skip loading any Google Services packages.
///
/// # Errors
///
/// Returns `GServicesError::OverlayError` on I/O failures.
pub async fn apply_removal_overlay(overlay_dir: &Path) -> GServicesResult<()> {
    tokio::fs::create_dir_all(overlay_dir)
        .await
        .map_err(|e| GServicesError::OverlayError(format!("failed to create overlay dir: {e}")))?;

    let marker = overlay_dir.join(".gservices_provider");
    tokio::fs::write(&marker, "none").await.map_err(|e| {
        GServicesError::OverlayError(format!("failed to write removal marker: {e}"))
    })?;

    log::info!("applied removal overlay to {}", overlay_dir.display());
    Ok(())
}

/// Recursively copy a directory tree.
async fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(dst).await?;
    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type().await?.is_dir() {
            Box::pin(copy_dir_recursive(&src_path, &dst_path)).await?;
        } else {
            tokio::fs::copy(&src_path, &dst_path).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_dir_path_format() {
        let dir = instance_overlay_dir("gaming");
        assert!(dir.ends_with("nux/instances/gaming/overlay"));
    }

    #[test]
    fn backup_path_format() {
        let overlay = PathBuf::from("/data/nux/instances/test/overlay");
        let backup = backup_path(&overlay);
        assert_eq!(
            backup,
            PathBuf::from("/data/nux/instances/test/overlay.overlay.backup")
        );
    }

    #[tokio::test]
    async fn backup_and_restore_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let overlay = dir.path().join("overlay");
        tokio::fs::create_dir_all(&overlay).await.unwrap();
        tokio::fs::write(overlay.join("marker"), "test")
            .await
            .unwrap();

        // Backup
        backup_overlay(&overlay).await.unwrap();
        let backup = backup_path(&overlay);
        assert!(backup.exists());

        // Modify overlay
        tokio::fs::write(overlay.join("marker"), "modified")
            .await
            .unwrap();

        // Restore
        restore_overlay(&overlay).await.unwrap();
        let content = tokio::fs::read_to_string(overlay.join("marker"))
            .await
            .unwrap();
        assert_eq!(content, "test");
    }

    #[tokio::test]
    async fn restore_fails_without_backup() {
        let dir = tempfile::tempdir().unwrap();
        let overlay = dir.path().join("overlay");
        let result = restore_overlay(&overlay).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn apply_gapps_creates_marker() {
        let dir = tempfile::tempdir().unwrap();
        let overlay = dir.path().join("overlay");
        let fake_zip = dir.path().join("fake.zip");
        tokio::fs::write(&fake_zip, b"not a real zip")
            .await
            .unwrap();

        apply_gapps_overlay(&overlay, &fake_zip).await.unwrap();

        let marker = tokio::fs::read_to_string(overlay.join(".gservices_provider"))
            .await
            .unwrap();
        assert_eq!(marker, "gapps");
        assert!(overlay.join("system").join("app").exists());
    }

    #[tokio::test]
    async fn reset_overlay_removes_dir() {
        let dir = tempfile::tempdir().unwrap();
        let overlay = dir.path().join("overlay");
        tokio::fs::create_dir_all(&overlay).await.unwrap();
        tokio::fs::write(overlay.join("file"), "data")
            .await
            .unwrap();

        reset_overlay_to_base(&overlay).await.unwrap();
        assert!(!overlay.exists());
    }

    #[tokio::test]
    async fn reset_overlay_noop_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let overlay = dir.path().join("nonexistent");
        // Should not error
        reset_overlay_to_base(&overlay).await.unwrap();
    }

    #[tokio::test]
    async fn apply_removal_creates_none_marker() {
        let dir = tempfile::tempdir().unwrap();
        let overlay = dir.path().join("overlay");

        apply_removal_overlay(&overlay).await.unwrap();

        let marker = tokio::fs::read_to_string(overlay.join(".gservices_provider"))
            .await
            .unwrap();
        assert_eq!(marker, "none");
    }
}
