## Why

Android rooting is essential for power users and gamers who need root access for game modding, performance tuning, and advanced customization. Nux needs a seamless root management workflow that handles boot.img patching and root mode switching without requiring users to manually juggle image files or use external tools. The three dominant root managers (Magisk, KernelSU, APatch) must all be supported to cover the full user base.

## What Changes

- New `nux-core::root` module for boot image management and root mode lifecycle
- Store stock `boot.img` per instance at `~/.local/share/nux/instances/<name>/boot.img`
- Store patched variants alongside: `boot_magisk.img`, `boot_kernelsu.img`, `boot_apatch.img`
- Patching workflow orchestrated via ADB bridge:
  1. Install root manager APK into the VM
  2. Push stock boot.img to the VM
  3. User (or automation script) patches inside Android
  4. Pull patched boot.img back to host
  5. Store patched image and update instance config
- Root mode switching: select which boot.img crosvm loads on next VM start (requires restart)
- Unroot: revert to stock boot.img
- Instance config gains `root_mode` field (`none`, `magisk`, `kernelsu`, `apatch`)

## Non-goals

- Building custom kernels — Nux ships a prebuilt kernel
- Xposed framework support — out of scope for root management
- Root hiding / SafetyNet bypass — that's the root manager app's responsibility
- Automated patching without user interaction for v1 (CLI automation is stretch goal)

## Capabilities

### New Capabilities
- `boot-image-management`: Storage, retrieval, and lifecycle of stock and patched boot.img files per instance
- `root-patching-workflow`: Orchestration of the ADB-based patching flow (APK install, push/pull boot images, config update)
- `root-mode-switching`: Selecting active root mode, passing correct boot.img to crosvm, and unrooting

### Modified Capabilities
<!-- No existing spec-level requirements are changing. The module depends on adb-bridge and config-system but does not modify their specs. -->

## Impact

- **nux-core**: New `root` module with public API consumed by nux-ui
- **nux-ui**: Settings panel needs root manager selection UI (separate change)
- **Dependencies**: Requires `adb-bridge` for VM communication and `config-system` for persisting root mode
- **crosvm integration**: Boot image path passed via crosvm CLI args — no crosvm code changes needed
- **File system**: New image files stored per-instance (~50-100 MB each)
