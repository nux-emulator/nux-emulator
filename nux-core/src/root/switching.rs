//! Root mode switching and active boot image resolution.

use super::error::RootResult;
use super::store::BootImageStore;
use crate::config::{NuxConfig, RootMode};
use std::path::PathBuf;

/// Set the root mode in the instance config.
///
/// Validates that the target patched boot image exists before updating the config.
/// For `RootMode::None`, no patched image check is needed.
///
/// # Errors
///
/// Returns `RootError::PatchedImageMissing` if the target mode's boot image
/// does not exist on disk.
/// Returns `RootError::ImageEmpty` if the image exists but is zero bytes.
pub fn set_root_mode(
    config: &mut NuxConfig,
    mode: RootMode,
    store: &BootImageStore,
) -> RootResult<()> {
    // Validate the target image exists (resolve checks existence + non-empty)
    if mode != RootMode::None {
        store.resolve(mode)?;
    }
    config.root.mode = mode;
    Ok(())
}

/// Unroot an instance by setting the root mode to `None`.
///
/// Does not delete any patched boot images so the user can re-enable root later.
///
/// # Errors
///
/// This function is infallible but returns `Result` for API consistency.
pub fn unroot(config: &mut NuxConfig) -> RootResult<()> {
    config.root.mode = RootMode::None;
    Ok(())
}

/// Resolve the active boot image path for the current root mode.
///
/// Called by the VM launcher at crosvm spawn time to determine which
/// boot image to pass as the `--boot` argument.
///
/// # Errors
///
/// Returns an error if the resolved boot image does not exist or is empty.
pub fn active_boot_image_path(config: &NuxConfig, store: &BootImageStore) -> RootResult<PathBuf> {
    store.resolve(config.root.mode)
}

/// Check whether the root mode has changed since the last VM start.
///
/// Compares the current config mode against a previously recorded mode.
/// Returns `true` if they differ, indicating a VM restart is needed.
#[must_use]
pub fn restart_required(current_mode: RootMode, last_booted_mode: RootMode) -> bool {
    current_mode != last_booted_mode
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::root::RootError;
    use tempfile::TempDir;

    fn setup() -> (TempDir, BootImageStore, NuxConfig) {
        let dir = TempDir::new().unwrap();
        let store = BootImageStore::new(dir.path().to_path_buf());
        let config = NuxConfig::default();
        (dir, store, config)
    }

    fn write_image(path: &PathBuf, content: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn switch_to_magisk_with_image() {
        let (_dir, store, mut config) = setup();
        write_image(&store.stock_image_path(), b"ANDROID!");
        write_image(&store.image_path(RootMode::Magisk), b"PATCHED_MAGISK");

        set_root_mode(&mut config, RootMode::Magisk, &store).unwrap();
        assert_eq!(config.root.mode, RootMode::Magisk);
    }

    #[test]
    fn switch_to_unavailable_image_errors() {
        let (_dir, store, mut config) = setup();
        write_image(&store.stock_image_path(), b"ANDROID!");

        let err = set_root_mode(&mut config, RootMode::Apatch, &store).unwrap_err();
        assert!(matches!(
            err,
            RootError::PatchedImageMissing(RootMode::Apatch)
        ));
        // Config unchanged
        assert_eq!(config.root.mode, RootMode::None);
    }

    #[test]
    fn switch_between_root_managers() {
        let (_dir, store, mut config) = setup();
        write_image(&store.stock_image_path(), b"ANDROID!");
        write_image(&store.image_path(RootMode::Magisk), b"PATCHED_MAGISK");
        write_image(&store.image_path(RootMode::Kernelsu), b"PATCHED_KSU");

        set_root_mode(&mut config, RootMode::Magisk, &store).unwrap();
        assert_eq!(config.root.mode, RootMode::Magisk);

        set_root_mode(&mut config, RootMode::Kernelsu, &store).unwrap();
        assert_eq!(config.root.mode, RootMode::Kernelsu);
    }

    #[test]
    fn unroot_preserves_patched_images() {
        let (_dir, store, mut config) = setup();
        write_image(&store.stock_image_path(), b"ANDROID!");
        write_image(&store.image_path(RootMode::Magisk), b"PATCHED_MAGISK");

        set_root_mode(&mut config, RootMode::Magisk, &store).unwrap();
        unroot(&mut config).unwrap();

        assert_eq!(config.root.mode, RootMode::None);
        // Patched image still on disk
        assert!(store.image_path(RootMode::Magisk).exists());
    }

    #[test]
    fn reroot_after_unroot() {
        let (_dir, store, mut config) = setup();
        write_image(&store.stock_image_path(), b"ANDROID!");
        write_image(&store.image_path(RootMode::Magisk), b"PATCHED_MAGISK");

        set_root_mode(&mut config, RootMode::Magisk, &store).unwrap();
        unroot(&mut config).unwrap();
        set_root_mode(&mut config, RootMode::Magisk, &store).unwrap();

        assert_eq!(config.root.mode, RootMode::Magisk);
    }

    #[test]
    fn active_boot_image_stock() {
        let (_dir, store, config) = setup();
        write_image(&store.stock_image_path(), b"ANDROID!");

        let path = active_boot_image_path(&config, &store).unwrap();
        assert_eq!(path, store.stock_image_path());
    }

    #[test]
    fn active_boot_image_patched() {
        let (_dir, store, mut config) = setup();
        write_image(&store.stock_image_path(), b"ANDROID!");
        write_image(&store.image_path(RootMode::Magisk), b"PATCHED_MAGISK");

        set_root_mode(&mut config, RootMode::Magisk, &store).unwrap();
        let path = active_boot_image_path(&config, &store).unwrap();
        assert_eq!(path, store.image_path(RootMode::Magisk));
    }

    #[test]
    fn restart_required_detects_change() {
        assert!(restart_required(RootMode::Magisk, RootMode::None));
        assert!(restart_required(RootMode::None, RootMode::Magisk));
        assert!(!restart_required(RootMode::Magisk, RootMode::Magisk));
        assert!(!restart_required(RootMode::None, RootMode::None));
    }
}
