## Why

Nux Emulator needs a reproducible Android 16 system image tailored for virtualized gaming on Linux. Stock AOSP images lack the virtio drivers, gfxstream GPU HALs, ARM binary translation, and pre-configuration needed for a seamless out-of-box experience with crosvm/KVM. Without a dedicated build system, every contributor must manually assemble these pieces — slowing development and making releases non-reproducible.

## What Changes

- Create an AOSP 16 device tree (`device/nux/emulator/`) defining the Nux virtual device with virtio-based hardware abstraction (GPU, input, network, sound, block).
- Integrate a kernel config based on `android-common` with all required virtio drivers enabled.
- Include gfxstream guest HALs (gralloc + hwcomposer) for GPU-accelerated rendering through virtio-gpu.
- Integrate `libndk_translation` for ARM → x86_64 binary translation so ARM-only games and apps run correctly.
- Bundle MicroG as the default Google Services replacement (GApps optionally flashable).
- Pre-configure the image: skip setup wizard, enable ADB by default, apply emulator-optimized default settings.
- Provide build scripts that produce versioned, checksummed output artifacts: `boot.img`, `system.img`, `vendor.img`, `userdata.img`.
- Add documentation for building from source.

## Non-goals

- The Nux emulator application itself (nux-core, nux-ui, crosvm management) — this change is purely about the Android image.
- GApps integration — MicroG is the default; GApps is a user-side optional flash.
- Upstream AOSP contributions or CTS compliance.
- Multi-architecture image builds (x86_64 only for v1).

## Capabilities

### New Capabilities
- `device-tree`: AOSP device tree for the Nux virtual device — BoardConfig, product makefiles, fstab, init scripts, and hardware feature declarations.
- `kernel-config`: Android-common kernel configuration with virtio drivers (virtio-gpu, virtio-input, virtio-net, virtio-snd, virtio-blk) and KVM guest support.
- `gfxstream-hals`: Gfxstream guest gralloc and hwcomposer HAL integration for GPU-accelerated rendering via virtio-gpu.
- `arm-translation`: libndk_translation integration for transparent ARM → x86_64 binary translation.
- `microg-integration`: MicroG bundled as a privileged system app replacing Google Services.
- `image-preconfig`: Default device configuration — setup wizard skip, ADB enabled, emulator-optimized settings.
- `build-pipeline`: Build scripts and CI pipeline producing versioned, checksummed boot/system/vendor/userdata images.

### Modified Capabilities
<!-- None — this is a new standalone build system with no existing specs. -->

## Impact

- **New repository/subdirectory**: Likely `nux-android-image` with its own AOSP-based build system (Soong/Make).
- **Dependencies**: AOSP 16 source tree, android-common kernel, gfxstream, libndk_translation, MicroG packages.
- **CI**: New GitHub Actions workflows (or separate CI) for image builds; large build times (~1-2 hours on capable hardware).
- **Release pipeline**: Nux emulator releases will pull pre-built images from this pipeline's output artifacts.
- **Disk/storage**: AOSP source + build output requires ~200-300 GB; CI runners need adequate resources.
