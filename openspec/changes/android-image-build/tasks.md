## 1. Repository & Build Skeleton

- [ ] 1.1 Create `nux-android-image` repository with README, LICENSE (GPLv3), `.gitignore` for AOSP build artifacts, and top-level directory structure (`device/`, `vendor/`, `kernel/`, `scripts/`)
- [ ] 1.2 Create `scripts/build.sh` entry point script with argument parsing (version, build variant, flags like `WITH_ARM_TRANSLATION`), AOSP `repo init`/`sync` invocation, `envsetup.sh` sourcing, and `lunch` target selection — stub out the actual build step. Verify: script runs, prints usage, and exits cleanly with `--help`
- [ ] 1.3 Add repo manifest (`default.xml`) or manifest overlay that pins AOSP 16 tag and includes Nux-specific repos (device tree, kernel, gfxstream, MicroG prebuilts). Verify: `repo init -m` accepts the manifest without errors

## 2. Device Tree (`device/nux/emulator/`)

- [ ] 2.1 Create `AndroidProducts.mk` and `vendorsetup.sh` registering the `nux_emulator-userdebug` lunch target. Verify: `lunch nux_emulator-userdebug` succeeds after `source build/envsetup.sh`
- [ ] 2.2 Create `BoardConfig.mk` with `TARGET_ARCH := x86_64`, partition sizes (system, vendor, userdata), filesystem types (ext4), and virtio-blk device references. Verify: build system parses BoardConfig without errors
- [ ] 2.3 Create `device.mk` with `PRODUCT_PACKAGES`, `PRODUCT_COPY_FILES`, and `PRODUCT_PROPERTY_OVERRIDES` stubs. Include hardware feature XML declarations (ethernet, vulkan present; camera, bluetooth absent). Verify: `make nux_emulator-userdebug` starts without missing-file errors
- [ ] 2.4 Create `fstab.nux` mapping virtio-blk devices to `/system`, `/vendor`, `/data` partitions. Create init `.rc` files for Nux-specific boot-time property settings. Verify: files pass `androidmk` / syntax validation

## 3. Kernel Configuration

- [ ] 3.1 Create Nux defconfig fragment (`kernel/nux_emulator_defconfig`) enabling `CONFIG_VIRTIO_GPU`, `CONFIG_VIRTIO_INPUT`, `CONFIG_VIRTIO_NET`, `CONFIG_VIRTIO_SND`, `CONFIG_VIRTIO_BLK`, `CONFIG_VIRTIO_PCI`, `CONFIG_VIRTIO_MMIO`, `CONFIG_KVM_GUEST`, `CONFIG_PARAVIRT` as built-in (`=y`). Verify: fragment applies cleanly on top of `android-common` base defconfig with `scripts/kconfig/merge_config.sh`
- [ ] 3.2 Integrate kernel build into the device tree (`BoardConfig.mk` kernel config references) so `make bootimage` produces `boot.img` with the Nux kernel. Verify: `boot.img` is generated and contains a kernel with virtio symbols (`grep VIRTIO /proc/config.gz` after boot, or `extract-ikconfig`)

## 4. Gfxstream Guest HALs

- [ ] 4.1 Add gfxstream source to the build tree (via repo manifest or `external/gfxstream/`) with `Android.bp` files for gralloc and hwcomposer HAL compilation. Verify: `make libgfxstream_gralloc` and `make libgfxstream_hwcomposer` compile without errors
- [ ] 4.2 Wire gfxstream HALs into `device.mk` via `PRODUCT_PACKAGES` so they land in `/vendor/lib64/hw/`. Pin gfxstream version in manifest to match host-side crosvm gfxstream. Verify: built vendor image contains gralloc and hwcomposer `.so` files at expected paths

## 5. ARM Translation (libndk_translation)

- [ ] 5.1 Create prebuilt module directory (`vendor/nux/arm-translation/`) with `Android.mk` that copies libndk_translation libraries to `/system/lib/arm/` and `/system/lib64/arm64/`. Implement `WITH_ARM_TRANSLATION` build flag gating inclusion. Verify: build with flag=true includes libraries; build with flag=false excludes them
- [ ] 5.2 Add `build.prop` properties (`ro.dalvik.vm.native.bridge=libndk_translation.so`, `ro.enable.native.bridge.exec=1`) conditionally via `PRODUCT_PROPERTY_OVERRIDES` in `device.mk`. Verify: properties present in built `build.prop` when enabled, absent when disabled

## 6. MicroG Integration

- [ ] 6.1 Create prebuilt module directory (`vendor/nux/microg/`) with `Android.mk` for GmsCore, GsfProxy, and FakeStore as privileged system apps under `/system/priv-app/`. Add to `PRODUCT_PACKAGES` in `device.mk`. Verify: APKs present in built system image at correct paths
- [ ] 6.2 Apply signature spoofing patch to `frameworks/base`. Create a patch file and integrate it into the build script's source-preparation step. Verify: patch applies cleanly; `android.permission.FAKE_PACKAGE_SIGNATURE` permission exists in framework
- [ ] 6.3 Create default-permissions XML (`etc/default-permissions/microg-permissions.xml`) pre-granting location, network, and accounts permissions to MicroG components. Add to `PRODUCT_COPY_FILES`. Verify: XML present in system image; MicroG has permissions on first boot without user prompts

## 7. Image Pre-configuration

- [ ] 7.1 Add setup wizard skip properties (`ro.setupwizard.mode=DISABLED`) and ADB properties (`ro.adb.secure=0`, `persist.sys.usb.config=adb`, `ro.debuggable=1`) to `PRODUCT_PROPERTY_OVERRIDES`. Verify: properties present in `build.prop`; first boot skips wizard; ADB connects immediately
- [ ] 7.2 Add emulator-optimized defaults: animation scales set to `0.5`, screen timeout ≥30 minutes, stay-awake-while-charging enabled, developer options pre-enabled. Implement via settings database overlay or init script. Verify: settings applied on first boot without manual configuration

## 8. Build Pipeline & Versioning

- [ ] 8.1 Implement version stamping in `build.sh`: accept `--version YYYY.MM.patch` argument, inject into `ro.nux.image.version` build property, and use in output artifact filenames (e.g., `nux-system-2026.03.1.img`). Verify: built images have correct version in `build.prop` and filenames
- [ ] 8.2 Implement artifact collection and SHA-256 checksum generation in `build.sh`: copy `boot.img`, `system.img`, `vendor.img`, `userdata.img` to output directory with versioned names; generate `SHA256SUMS` file. Verify: `sha256sum -c SHA256SUMS` passes
- [ ] 8.3 Create GitHub Actions workflow (`.github/workflows/build-image.yml`) triggered on version tags (`v*`) and `workflow_dispatch`. Workflow installs AOSP prerequisites, runs `build.sh`, and uploads images + checksums as release artifacts. Verify: workflow YAML passes `actionlint`; manual trigger starts a build

## 9. Documentation

- [ ] 9.1 Write `README.md` covering: project overview, system requirements (disk, RAM, OS, packages), step-by-step build instructions, available build flags (`WITH_ARM_TRANSLATION`, version, variant), output artifacts description, and troubleshooting common issues. Verify: a developer can follow the README to produce a working image build
