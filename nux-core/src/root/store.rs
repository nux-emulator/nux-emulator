//! Boot image storage and retrieval.
//!
//! Manages stock and patched boot image files within an instance directory.
//! Each instance stores its boot images at:
//! - `boot.img` — stock (unmodified)
//! - `boot_magisk.img` — Magisk-patched
//! - `boot_kernelsu.img` — KernelSU-patched
//! - `boot_apatch.img` — APatch-patched

use super::error::{RootError, RootResult};
use crate::config::RootMode;
use std::path::{Path, PathBuf};

/// File name for the stock boot image.
const STOCK_IMAGE_NAME: &str = "boot.img";

/// Manages boot image files for a single instance.
#[derive(Debug, Clone)]
pub struct BootImageStore {
    /// Instance directory (e.g. `~/.local/share/nux/instances/<name>/`).
    instance_dir: PathBuf,
}

impl BootImageStore {
    /// Create a new store rooted at the given instance directory.
    #[must_use]
    pub fn new(instance_dir: PathBuf) -> Self {
        Self { instance_dir }
    }

    /// Return the instance directory path.
    #[must_use]
    pub fn instance_dir(&self) -> &Path {
        &self.instance_dir
    }

    /// Return the file name for a given root mode's boot image.
    #[must_use]
    pub fn image_filename(mode: RootMode) -> &'static str {
        match mode {
            RootMode::None => STOCK_IMAGE_NAME,
            RootMode::Magisk => "boot_magisk.img",
            RootMode::Kernelsu => "boot_kernelsu.img",
            RootMode::Apatch => "boot_apatch.img",
        }
    }

    /// Return the full path for a given root mode's boot image.
    #[must_use]
    pub fn image_path(&self, mode: RootMode) -> PathBuf {
        self.instance_dir.join(Self::image_filename(mode))
    }

    /// Return the path to the stock boot image.
    #[must_use]
    pub fn stock_image_path(&self) -> PathBuf {
        self.image_path(RootMode::None)
    }

    /// Copy a stock boot image into the instance directory.
    ///
    /// # Errors
    ///
    /// Returns `RootError::Io` if the copy fails.
    pub fn store_stock_image(&self, source: &Path) -> RootResult<()> {
        std::fs::create_dir_all(&self.instance_dir)?;
        std::fs::copy(source, self.stock_image_path())?;
        Ok(())
    }

    /// Store a patched boot image for the given root mode.
    ///
    /// # Errors
    ///
    /// Returns `RootError::Io` if the copy fails.
    ///
    /// # Panics
    ///
    /// Panics if `mode` is `RootMode::None` (stock images use `store_stock_image`).
    pub fn store_patched_image(&self, mode: RootMode, source: &Path) -> RootResult<()> {
        assert!(
            mode != RootMode::None,
            "use store_stock_image for stock images"
        );
        std::fs::create_dir_all(&self.instance_dir)?;
        std::fs::copy(source, self.image_path(mode))?;
        Ok(())
    }

    /// Resolve the boot image path for a given root mode.
    ///
    /// Validates that the file exists and is non-empty.
    ///
    /// # Errors
    ///
    /// Returns `RootError::ImageNotFound` if the file does not exist.
    /// Returns `RootError::ImageEmpty` if the file is zero bytes.
    /// Returns `RootError::PatchedImageMissing` for a missing patched variant.
    pub fn resolve(&self, mode: RootMode) -> RootResult<PathBuf> {
        let path = self.image_path(mode);

        if !path.exists() {
            return if mode == RootMode::None {
                Err(RootError::StockImageMissing(path))
            } else {
                Err(RootError::PatchedImageMissing(mode))
            };
        }

        let metadata = std::fs::metadata(&path)?;
        if metadata.len() == 0 {
            return Err(RootError::ImageEmpty(path));
        }

        Ok(path)
    }

    /// Check whether a patched image exists for the given mode.
    #[must_use]
    pub fn has_patched_image(&self, mode: RootMode) -> bool {
        if mode == RootMode::None {
            return true;
        }
        let path = self.image_path(mode);
        path.exists() && path.metadata().is_ok_and(|m| m.len() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, BootImageStore) {
        let dir = TempDir::new().unwrap();
        let store = BootImageStore::new(dir.path().to_path_buf());
        (dir, store)
    }

    fn write_image(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn image_filenames() {
        assert_eq!(BootImageStore::image_filename(RootMode::None), "boot.img");
        assert_eq!(
            BootImageStore::image_filename(RootMode::Magisk),
            "boot_magisk.img"
        );
        assert_eq!(
            BootImageStore::image_filename(RootMode::Kernelsu),
            "boot_kernelsu.img"
        );
        assert_eq!(
            BootImageStore::image_filename(RootMode::Apatch),
            "boot_apatch.img"
        );
    }

    #[test]
    fn store_and_resolve_stock_image() {
        let (_dir, store) = setup();
        let src = _dir.path().join("source_boot.img");
        write_image(&src, b"ANDROID!");

        store.store_stock_image(&src).unwrap();
        let resolved = store.resolve(RootMode::None).unwrap();
        assert_eq!(resolved, store.stock_image_path());
        assert_eq!(std::fs::read(&resolved).unwrap(), b"ANDROID!");
    }

    #[test]
    fn store_and_resolve_patched_image() {
        let (_dir, store) = setup();
        let src = _dir.path().join("patched.img");
        write_image(&src, b"PATCHED_MAGISK");

        store.store_patched_image(RootMode::Magisk, &src).unwrap();
        let resolved = store.resolve(RootMode::Magisk).unwrap();
        assert_eq!(resolved, store.image_path(RootMode::Magisk));
        assert_eq!(std::fs::read(&resolved).unwrap(), b"PATCHED_MAGISK");
    }

    #[test]
    fn resolve_missing_patched_image_errors() {
        let (_dir, store) = setup();
        // Stock exists but no patched image
        write_image(&store.stock_image_path(), b"ANDROID!");

        let err = store.resolve(RootMode::Apatch).unwrap_err();
        assert!(matches!(
            err,
            RootError::PatchedImageMissing(RootMode::Apatch)
        ));
    }

    #[test]
    fn resolve_missing_stock_image_errors() {
        let (_dir, store) = setup();
        let err = store.resolve(RootMode::None).unwrap_err();
        assert!(matches!(err, RootError::StockImageMissing(_)));
    }

    #[test]
    fn resolve_empty_image_errors() {
        let (_dir, store) = setup();
        write_image(&store.stock_image_path(), b"");

        let err = store.resolve(RootMode::None).unwrap_err();
        assert!(matches!(err, RootError::ImageEmpty(_)));
    }

    #[test]
    fn has_patched_image_checks() {
        let (_dir, store) = setup();
        assert!(store.has_patched_image(RootMode::None)); // None always true
        assert!(!store.has_patched_image(RootMode::Magisk));

        write_image(&store.image_path(RootMode::Magisk), b"PATCHED");
        assert!(store.has_patched_image(RootMode::Magisk));
    }

    #[test]
    fn has_patched_image_false_for_empty() {
        let (_dir, store) = setup();
        write_image(&store.image_path(RootMode::Kernelsu), b"");
        assert!(!store.has_patched_image(RootMode::Kernelsu));
    }

    #[test]
    #[should_panic(expected = "use store_stock_image")]
    fn store_patched_panics_for_none_mode() {
        let (_dir, store) = setup();
        let src = _dir.path().join("boot.img");
        write_image(&src, b"ANDROID!");
        let _ = store.store_patched_image(RootMode::None, &src);
    }
}
