## Context

Nux currently boots Android VMs using a stock boot.img passed to crosvm. There is no mechanism to swap boot images, manage root manager APKs, or orchestrate the patching workflow. The ADB bridge (from the `adb-bridge` change) provides the communication layer needed to push/pull files and install APKs into the running VM. The config system (from `config-system`) provides per-instance TOML persistence.

Root management is a core differentiator for a gaming emulator — users expect one-click root setup comparable to what Magisk Manager provides on physical devices, but adapted to the emulator's host-guest architecture where we control the boot image directly.

## Goals / Non-Goals

**Goals:**
- Provide a clean Rust API in `nux-core::root` for managing boot images and root state
- Support all three major root managers: Magisk, KernelSU, APatch
- Orchestrate the full patching lifecycle over ADB without manual file management
- Persist root mode in instance config so it survives restarts
- Make root mode switching a config change + VM restart (no re-patching)

**Non-Goals:**
- Custom kernel compilation or kernel module management
- Root hiding, SafetyNet/Play Integrity bypass (root manager app's job)
- Xposed or LSPosed framework integration
- Multi-instance root state sharing
- Fully automated patching without user interaction in v1

## Decisions

### 1. Boot image storage layout — flat per-instance directory

Store all boot images in the instance directory (`~/.local/share/nux/instances/<name>/`):
- `boot.img` — stock, always preserved
- `boot_magisk.img`, `boot_kernelsu.img`, `boot_apatch.img` — patched variants

**Why:** Simple, discoverable, no separate image registry. Each instance is self-contained. Alternative considered: a shared image cache across instances — rejected because different instances may run different Android versions with incompatible boot images.

### 2. Root mode as an enum in instance config

```toml
[root]
mode = "none"  # none | magisk | kernelsu | apatch
```

**Why:** Explicit, type-safe in Rust (`RootMode` enum with serde). The mode determines which boot image file crosvm receives at launch. Alternative: storing the active boot image path directly — rejected because it's fragile and doesn't encode intent.

### 3. ADB-based patching orchestration

The patching flow uses the ADB bridge to:
1. Install the root manager APK (`adb install`)
2. Push stock boot.img to a known path in the VM (`adb push`)
3. Signal the user to open the manager and patch (v1: manual; stretch: CLI automation)
4. Pull the patched image back (`adb pull` from a known output path)

**Why:** This mirrors how patching works on real devices and keeps Nux out of the patching internals. Each root manager has its own patching logic that evolves independently. Alternative: running patching tools on the host — rejected because Magisk/KernelSU/APatch are Android binaries that expect an Android environment.

### 4. Module structure — single `nux-core::root` module

Public API surface:
- `RootMode` enum
- `RootManager` struct — owns boot image paths, exposes patching workflow methods
- `BootImageStore` — handles file I/O for boot images
- Methods: `install_manager()`, `push_stock_image()`, `pull_patched_image()`, `set_root_mode()`, `unroot()`, `active_boot_image_path()`

**Why:** Keeps root logic cohesive in one module. The struct takes an `AdbBridge` reference and `InstanceConfig` reference as dependencies via constructor injection. Alternative: spreading across multiple modules — rejected for a feature this focused.

### 5. crosvm boot image selection — CLI arg at spawn time

The VM launcher reads `config.root.mode`, resolves it to a boot image path via `BootImageStore`, and passes it to crosvm's `--initrd` or disk argument. No runtime hot-swap; changing root mode requires VM restart.

**Why:** crosvm doesn't support hot-swapping boot images. This is the simplest correct approach.

## Risks / Trade-offs

- **Patched image compatibility** — A patched boot.img from one Android version won't work with another. → Mitigation: validate boot image headers before use; warn user if Android image is updated.
- **Large file storage** — Each patched image is ~50-100 MB. Three variants = ~300 MB per instance. → Mitigation: acceptable for desktop; users can delete unused variants.
- **ADB bridge reliability** — Push/pull of large files over ADB can fail. → Mitigation: checksum verification after pull; retry logic in the workflow.
- **Root manager APK versioning** — Bundling APKs means they go stale. → Mitigation: v1 bundles known-good versions; future: check for updates on first use.
- **Manual patching step in v1** — User must open the manager app and tap "patch". → Mitigation: clear in-app guidance; stretch goal adds CLI automation for Magisk.

## Open Questions

- Should we bundle root manager APKs in the Nux package, or download them on demand? Bundling is simpler but increases package size.
- What is the canonical path inside the Android VM for push/pull of boot images? Needs testing with each root manager.
