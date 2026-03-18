//! APK resource paths and VM-side patched output paths for each root manager.

use crate::config::RootMode;
use std::path::{Path, PathBuf};

/// Known VM-side paths where each root manager writes its patched boot image.
///
/// These are the paths inside the Android VM that the patching apps output to.
#[must_use]
pub fn vm_patched_output_path(mode: RootMode) -> &'static str {
    match mode {
        RootMode::Magisk => "/sdcard/Download/magisk_patched.img",
        RootMode::Kernelsu => "/sdcard/Download/kernelsu_patched.img",
        RootMode::Apatch => "/sdcard/Download/apatch_patched.img",
        RootMode::None => "",
    }
}

/// The VM-side path where the stock boot image is pushed for patching.
pub const VM_STOCK_BOOT_PATH: &str = "/sdcard/boot.img";

/// APK file names for each root manager (bundled in the resources directory).
#[must_use]
pub fn apk_filename(mode: RootMode) -> &'static str {
    match mode {
        RootMode::Magisk => "Magisk.apk",
        RootMode::Kernelsu => "KernelSU.apk",
        RootMode::Apatch => "APatch.apk",
        RootMode::None => "",
    }
}

/// Resolve the host-side path to a root manager APK.
///
/// Looks for APKs in the given resources directory.
///
/// # Errors
///
/// Returns `None` if `mode` is `RootMode::None`.
#[must_use]
pub fn apk_path(resources_dir: &Path, mode: RootMode) -> Option<PathBuf> {
    if mode == RootMode::None {
        return None;
    }
    Some(resources_dir.join("apks").join(apk_filename(mode)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_patched_paths_are_nonempty_for_managers() {
        assert!(!vm_patched_output_path(RootMode::Magisk).is_empty());
        assert!(!vm_patched_output_path(RootMode::Kernelsu).is_empty());
        assert!(!vm_patched_output_path(RootMode::Apatch).is_empty());
    }

    #[test]
    fn vm_patched_path_empty_for_none() {
        assert!(vm_patched_output_path(RootMode::None).is_empty());
    }

    #[test]
    fn apk_filenames_correct() {
        assert_eq!(apk_filename(RootMode::Magisk), "Magisk.apk");
        assert_eq!(apk_filename(RootMode::Kernelsu), "KernelSU.apk");
        assert_eq!(apk_filename(RootMode::Apatch), "APatch.apk");
    }

    #[test]
    fn apk_path_resolves_for_managers() {
        let dir = Path::new("/usr/share/nux");
        assert_eq!(
            apk_path(dir, RootMode::Magisk).unwrap(),
            PathBuf::from("/usr/share/nux/apks/Magisk.apk")
        );
        assert_eq!(
            apk_path(dir, RootMode::Kernelsu).unwrap(),
            PathBuf::from("/usr/share/nux/apks/KernelSU.apk")
        );
        assert_eq!(
            apk_path(dir, RootMode::Apatch).unwrap(),
            PathBuf::from("/usr/share/nux/apks/APatch.apk")
        );
    }

    #[test]
    fn apk_path_none_for_none_mode() {
        let dir = Path::new("/usr/share/nux");
        assert!(apk_path(dir, RootMode::None).is_none());
    }
}
