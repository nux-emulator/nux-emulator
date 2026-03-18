//! Patching workflow orchestration.
//!
//! The [`RootManager`] struct coordinates the full root patching lifecycle:
//! install manager APK → push stock boot image → (user patches) → pull patched image → update config.

use super::apk;
use super::error::{RootError, RootResult};
use super::store::BootImageStore;
use super::switching;
use crate::config::{NuxConfig, RootMode};
use std::path::{Path, PathBuf};

/// Trait abstracting ADB operations needed by the root manager.
///
/// This allows testing with a mock and decouples the root module from the
/// concrete ADB bridge implementation.
#[allow(async_fn_in_trait)]
pub trait AdbBridge {
    /// Install an APK file into the running VM.
    ///
    /// # Errors
    ///
    /// Returns an error if the ADB install command fails.
    async fn install_apk(&self, apk_path: &Path) -> Result<(), String>;

    /// Push a file from the host to a path inside the VM.
    ///
    /// # Errors
    ///
    /// Returns an error if the ADB push command fails.
    async fn push_file(&self, host_path: &Path, vm_path: &str) -> Result<(), String>;

    /// Pull a file from the VM to a path on the host.
    ///
    /// # Errors
    ///
    /// Returns an error if the ADB pull command fails.
    async fn pull_file(&self, vm_path: &str, host_path: &Path) -> Result<(), String>;
}

/// Orchestrates root patching workflows.
///
/// Holds references to the ADB bridge, boot image store, and a resources
/// directory where root manager APKs are located.
pub struct RootManager<A> {
    adb: A,
    store: BootImageStore,
    resources_dir: PathBuf,
}

impl<A: AdbBridge> RootManager<A> {
    /// Create a new root manager.
    #[must_use]
    pub fn new(adb: A, store: BootImageStore, resources_dir: PathBuf) -> Self {
        Self {
            adb,
            store,
            resources_dir,
        }
    }

    /// Get a reference to the boot image store.
    #[must_use]
    pub fn store(&self) -> &BootImageStore {
        &self.store
    }

    /// Install the root manager APK for the given mode into the VM.
    ///
    /// # Errors
    ///
    /// Returns `RootError::Adb` if the install fails.
    /// Returns `RootError::WorkflowAborted` if the mode is `None`.
    pub async fn install_manager_apk(&self, mode: RootMode) -> RootResult<()> {
        let apk_path =
            apk::apk_path(&self.resources_dir, mode).ok_or_else(|| RootError::WorkflowAborted {
                step: "install_apk".to_owned(),
                cause: "cannot install APK for RootMode::None".to_owned(),
            })?;

        self.adb
            .install_apk(&apk_path)
            .await
            .map_err(|detail| RootError::Adb {
                operation: "install".to_owned(),
                detail,
            })
    }

    /// Push the stock boot image to the VM at `/sdcard/boot.img`.
    ///
    /// # Errors
    ///
    /// Returns `RootError::StockImageMissing` if the stock image doesn't exist.
    /// Returns `RootError::Adb` if the push fails.
    pub async fn push_stock_image(&self) -> RootResult<()> {
        let stock_path = self.store.resolve(RootMode::None)?;

        self.adb
            .push_file(&stock_path, apk::VM_STOCK_BOOT_PATH)
            .await
            .map_err(|detail| RootError::Adb {
                operation: "push".to_owned(),
                detail,
            })
    }

    /// Pull the patched boot image from the VM and store it locally.
    ///
    /// # Errors
    ///
    /// Returns `RootError::Adb` if the pull fails.
    /// Returns `RootError::Io` if storing the image fails.
    pub async fn pull_patched_image(&self, mode: RootMode) -> RootResult<()> {
        let vm_path = apk::vm_patched_output_path(mode);
        if vm_path.is_empty() {
            return Err(RootError::WorkflowAborted {
                step: "pull_patched".to_owned(),
                cause: "cannot pull patched image for RootMode::None".to_owned(),
            });
        }

        // Pull to a temporary location first, then move into the store
        let tmp_path = self.store.instance_dir().join(".patched_tmp.img");

        self.adb
            .pull_file(vm_path, &tmp_path)
            .await
            .map_err(|detail| RootError::Adb {
                operation: "pull".to_owned(),
                detail,
            })?;

        self.store.store_patched_image(mode, &tmp_path)?;

        // Clean up temp file
        let _ = std::fs::remove_file(&tmp_path);

        Ok(())
    }

    /// Run the full patching workflow for a given root mode.
    ///
    /// Steps:
    /// 1. Install the root manager APK
    /// 2. Push the stock boot image to the VM
    /// 3. Pull the patched boot image back from the VM
    /// 4. Update the instance config
    ///
    /// If any step fails, the config is left unchanged.
    ///
    /// Note: In v1, between steps 2 and 3 the user must manually open the
    /// root manager app in Android and perform the patching. The caller is
    /// responsible for waiting until the user signals completion before
    /// calling this method (or calling the individual steps separately).
    ///
    /// # Errors
    ///
    /// Returns `RootError::WorkflowAborted` with the failing step name.
    pub async fn patch(&self, mode: RootMode, config: &mut NuxConfig) -> RootResult<()> {
        // Step 1: Install APK
        self.install_manager_apk(mode)
            .await
            .map_err(|e| RootError::WorkflowAborted {
                step: "install_apk".to_owned(),
                cause: e.to_string(),
            })?;

        // Step 2: Push stock image
        self.push_stock_image()
            .await
            .map_err(|e| RootError::WorkflowAborted {
                step: "push_stock_image".to_owned(),
                cause: e.to_string(),
            })?;

        // Step 3: Pull patched image (caller must ensure user has patched first)
        self.pull_patched_image(mode)
            .await
            .map_err(|e| RootError::WorkflowAborted {
                step: "pull_patched_image".to_owned(),
                cause: e.to_string(),
            })?;

        // Step 4: Update config only after all file operations succeed
        switching::set_root_mode(config, mode, &self.store)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    /// Records which ADB operations were called and can be configured to fail.
    #[derive(Clone, Default)]
    struct MockAdb {
        calls: Arc<Mutex<Vec<String>>>,
        fail_install: Arc<Mutex<bool>>,
        fail_push: Arc<Mutex<bool>>,
        fail_pull: Arc<Mutex<bool>>,
    }

    impl MockAdb {
        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }

        fn set_fail_install(&self, fail: bool) {
            *self.fail_install.lock().unwrap() = fail;
        }

        fn set_fail_push(&self, fail: bool) {
            *self.fail_push.lock().unwrap() = fail;
        }

        fn set_fail_pull(&self, fail: bool) {
            *self.fail_pull.lock().unwrap() = fail;
        }
    }

    impl AdbBridge for MockAdb {
        async fn install_apk(&self, apk_path: &Path) -> Result<(), String> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("install:{}", apk_path.display()));
            if *self.fail_install.lock().unwrap() {
                return Err("install failed".to_owned());
            }
            Ok(())
        }

        async fn push_file(&self, host_path: &Path, vm_path: &str) -> Result<(), String> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("push:{}:{vm_path}", host_path.display()));
            if *self.fail_push.lock().unwrap() {
                return Err("push failed".to_owned());
            }
            Ok(())
        }

        async fn pull_file(&self, vm_path: &str, host_path: &Path) -> Result<(), String> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("pull:{vm_path}:{}", host_path.display()));
            if *self.fail_pull.lock().unwrap() {
                return Err("pull failed".to_owned());
            }
            // Write fake patched content so store_patched_image succeeds
            if let Some(parent) = host_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(host_path, b"PATCHED_DATA").unwrap();
            Ok(())
        }
    }

    fn setup() -> (TempDir, TempDir, MockAdb) {
        let instance_dir = TempDir::new().unwrap();
        let resources_dir = TempDir::new().unwrap();
        let adb = MockAdb::default();
        (instance_dir, resources_dir, adb)
    }

    fn make_manager(
        adb: MockAdb,
        instance_dir: &Path,
        resources_dir: &Path,
    ) -> RootManager<MockAdb> {
        let store = BootImageStore::new(instance_dir.to_path_buf());
        RootManager::new(adb, store, resources_dir.to_path_buf())
    }

    fn write_stock(store: &BootImageStore) {
        std::fs::create_dir_all(store.instance_dir()).unwrap();
        std::fs::write(store.stock_image_path(), b"ANDROID!").unwrap();
    }

    #[tokio::test]
    async fn install_manager_apk_calls_adb() {
        let (inst, res, adb) = setup();
        let mgr = make_manager(adb.clone(), inst.path(), res.path());

        mgr.install_manager_apk(RootMode::Magisk).await.unwrap();

        let calls = adb.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].contains("install"));
        assert!(calls[0].contains("Magisk.apk"));
    }

    #[tokio::test]
    async fn install_manager_apk_none_errors() {
        let (inst, res, adb) = setup();
        let mgr = make_manager(adb, inst.path(), res.path());

        let err = mgr.install_manager_apk(RootMode::None).await.unwrap_err();
        assert!(matches!(err, RootError::WorkflowAborted { .. }));
    }

    #[tokio::test]
    async fn push_stock_image_calls_adb() {
        let (inst, res, adb) = setup();
        let mgr = make_manager(adb.clone(), inst.path(), res.path());
        write_stock(mgr.store());

        mgr.push_stock_image().await.unwrap();

        let calls = adb.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].contains("push"));
        assert!(calls[0].contains("/sdcard/boot.img"));
    }

    #[tokio::test]
    async fn push_stock_image_missing_errors() {
        let (inst, res, adb) = setup();
        let mgr = make_manager(adb, inst.path(), res.path());

        let err = mgr.push_stock_image().await.unwrap_err();
        assert!(matches!(err, RootError::StockImageMissing(_)));
    }

    #[tokio::test]
    async fn pull_patched_image_stores_file() {
        let (inst, res, adb) = setup();
        let mgr = make_manager(adb, inst.path(), res.path());
        write_stock(mgr.store());

        mgr.pull_patched_image(RootMode::Magisk).await.unwrap();

        assert!(mgr.store().has_patched_image(RootMode::Magisk));
    }

    #[tokio::test]
    async fn full_patch_workflow_success() {
        let (inst, res, adb) = setup();
        let mgr = make_manager(adb.clone(), inst.path(), res.path());
        write_stock(mgr.store());

        let mut config = NuxConfig::default();
        assert_eq!(config.root.mode, RootMode::None);

        mgr.patch(RootMode::Magisk, &mut config).await.unwrap();

        assert_eq!(config.root.mode, RootMode::Magisk);
        assert!(mgr.store().has_patched_image(RootMode::Magisk));

        let calls = adb.calls();
        assert_eq!(calls.len(), 3); // install, push, pull
    }

    #[tokio::test]
    async fn workflow_install_failure_leaves_config_unchanged() {
        let (inst, res, adb) = setup();
        adb.set_fail_install(true);
        let mgr = make_manager(adb, inst.path(), res.path());
        write_stock(mgr.store());

        let mut config = NuxConfig::default();
        let err = mgr.patch(RootMode::Magisk, &mut config).await.unwrap_err();

        assert!(matches!(err, RootError::WorkflowAborted { .. }));
        assert_eq!(config.root.mode, RootMode::None);
    }

    #[tokio::test]
    async fn workflow_push_failure_leaves_config_unchanged() {
        let (inst, res, adb) = setup();
        adb.set_fail_push(true);
        let mgr = make_manager(adb, inst.path(), res.path());
        write_stock(mgr.store());

        let mut config = NuxConfig::default();
        let err = mgr.patch(RootMode::Magisk, &mut config).await.unwrap_err();

        assert!(matches!(err, RootError::WorkflowAborted { .. }));
        assert_eq!(config.root.mode, RootMode::None);
    }

    #[tokio::test]
    async fn workflow_pull_failure_leaves_config_unchanged() {
        let (inst, res, adb) = setup();
        adb.set_fail_pull(true);
        let mgr = make_manager(adb, inst.path(), res.path());
        write_stock(mgr.store());

        let mut config = NuxConfig::default();
        let err = mgr.patch(RootMode::Magisk, &mut config).await.unwrap_err();

        assert!(matches!(err, RootError::WorkflowAborted { .. }));
        assert_eq!(config.root.mode, RootMode::None);
    }
}
