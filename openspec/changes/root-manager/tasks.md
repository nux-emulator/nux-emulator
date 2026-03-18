## 1. Module Setup and Types

- [x] 1.1 Create `nux-core/src/root.rs` module file and register it in `nux-core/src/lib.rs`
- [x] 1.2 Define `RootMode` enum (`None`, `Magisk`, `KernelSu`, `APatch`) with serde serialization to lowercase strings and `Default` impl returning `None`
- [x] 1.3 Define `RootConfig` struct with `mode: RootMode` field, add `[root]` section to instance config schema
- [x] 1.4 Write unit tests for `RootMode` serialization/deserialization round-trips and default value

## 2. Boot Image Storage

- [x] 2.1 Implement `BootImageStore` struct that takes an instance directory path and exposes methods for resolving boot image paths by `RootMode`
- [x] 2.2 Implement `store_stock_image(&self, source: &Path)` to copy a stock boot.img into the instance directory
- [x] 2.3 Implement `store_patched_image(&self, mode: RootMode, source: &Path)` to store a patched variant with the correct naming convention (`boot_magisk.img`, etc.)
- [x] 2.4 Implement `resolve(&self, mode: RootMode) -> Result<PathBuf>` that returns the boot image path, validating the file exists and is non-empty
- [x] 2.5 Write unit tests: resolve stock image, resolve patched image, error on missing patched image, error on empty/corrupt image

## 3. Root Mode Switching

- [x] 3.1 Implement `set_root_mode(config: &mut InstanceConfig, mode: RootMode, store: &BootImageStore) -> Result<()>` that validates the target image exists before updating config
- [x] 3.2 Implement `unroot(config: &mut InstanceConfig) -> Result<()>` that sets mode to `None` without deleting patched images
- [x] 3.3 Implement `active_boot_image_path(config: &InstanceConfig, store: &BootImageStore) -> Result<PathBuf>` for the VM launcher to call at crosvm spawn time
- [x] 3.4 Write unit tests: switch modes, switch to unavailable image errors, unroot preserves files, re-root after unroot

## 4. Patching Workflow Orchestration

- [x] 4.1 Implement `RootManager` struct that holds references to `AdbBridge`, `BootImageStore`, and `InstanceConfig`
- [x] 4.2 Implement `install_manager_apk(&self, mode: RootMode) -> Result<()>` that calls ADB install with the correct APK for the selected manager
- [x] 4.3 Implement `push_stock_image(&self) -> Result<()>` that pushes stock boot.img to `/sdcard/boot.img` in the VM via ADB
- [x] 4.4 Implement `pull_patched_image(&self, mode: RootMode) -> Result<()>` that pulls the patched image from the VM and stores it via `BootImageStore`
- [x] 4.5 Implement `patch(&self, mode: RootMode) -> Result<()>` that orchestrates the full sequence: install APK → push stock image → return control for user patching → pull patched image → update config
- [x] 4.6 Add error handling: each step reports which phase failed, config is not updated if any step fails
- [x] 4.7 Write integration tests with a mock `AdbBridge`: successful full workflow, failure at each step leaves config unchanged

## 5. VM Launcher Integration

- [x] 5.1 Update the crosvm launch code to read `config.root.mode` and resolve the boot image path via `BootImageStore`
- [x] 5.2 Pass the resolved boot image path to crosvm CLI arguments
- [x] 5.3 Add a "restart required" check: if root mode changed since last VM start, signal the caller that a restart is needed
- [x] 5.4 Write integration test: VM launcher selects correct boot image for each root mode

## 6. APK Bundling and Paths

- [x] 6.1 Define known VM-side paths for each root manager's patched output (e.g., Magisk outputs to `/sdcard/Download/magisk_patched-*.img`)
- [x] 6.2 Bundle root manager APKs as resources or define a download-on-demand strategy with version pinning
- [x] 6.3 Write a test that verifies APK resource paths resolve correctly for all three managers
