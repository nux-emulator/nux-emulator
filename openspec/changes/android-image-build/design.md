## Context

Nux Emulator runs Android 16 (x86_64) inside crosvm with KVM acceleration and gfxstream GPU passthrough. The emulator application (nux-core/nux-ui) expects a set of pre-built Android images (`boot.img`, `system.img`, `vendor.img`, `userdata.img`) that are configured for virtualized execution. Currently no build system exists to produce these images reproducibly.

The Android image is built from AOSP 16 source using the standard Soong/Make build system, extended with a custom device tree, kernel config, and pre-integrated components (gfxstream HALs, libndk_translation, MicroG). This lives in a separate repository (`nux-android-image`) since AOSP's build system is fundamentally incompatible with Cargo workspaces.

Stakeholders: Nux developers (need reproducible images for testing), release pipeline (needs versioned artifacts), end users building from source.

## Goals / Non-Goals

**Goals:**
- Reproducible, scripted AOSP 16 image builds from a single entry point
- Properly configured virtio hardware abstraction for crosvm/KVM
- GPU acceleration via gfxstream guest HALs working out of the box
- ARM app compatibility via libndk_translation
- MicroG pre-installed as a privileged system app
- Versioned, checksummed image artifacts suitable for release distribution
- CI pipeline for automated builds on push/tag

**Non-Goals:**
- CTS/VTS certification
- GApps bundling (user-flashable separately)
- Multi-arch builds (ARM images, x86 32-bit)
- OTA update mechanism
- Emulator application integration (nux-core consumes images, doesn't build them)

## Decisions

### 1. Separate repository (`nux-android-image`)

**Choice**: Standalone repo with its own build system, not a subdirectory of the emulator workspace.

**Rationale**: AOSP builds use Soong/Make and require a specific directory layout (`device/`, `vendor/`, `external/`). Mixing this with a Cargo workspace creates confusion and bloats the emulator repo (~300 GB AOSP tree). The emulator repo references image artifacts by version, not source.

**Alternatives considered**:
- Git submodule inside emulator repo — rejected due to AOSP tree size and unrelated build systems.
- Mono-repo with separate build roots — adds complexity with no benefit.

### 2. Device tree structure: `device/nux/emulator/`

**Choice**: Standard AOSP device tree layout under `device/nux/emulator/` with `BoardConfig.mk`, product makefiles, and overlay directories.

**Rationale**: Following AOSP conventions means `lunch nux_emulator-userdebug` works as expected. Developers familiar with AOSP can navigate the tree without learning custom patterns.

### 3. Kernel: android-common with defconfig overlay

**Choice**: Use Google's `android-common` kernel branch with a Nux-specific defconfig fragment enabling virtio drivers.

**Rationale**: `android-common` tracks upstream LTS with Android-specific patches (binder, ashmem). A defconfig fragment on top is the standard way to enable board-specific drivers without forking the kernel. Virtio drivers (gpu, input, net, snd, blk) are all upstream and just need `CONFIG_*=y`.

**Alternatives considered**:
- Custom kernel fork — unnecessary maintenance burden; virtio support is already upstream.
- GKI (Generic Kernel Image) with modules — viable but adds complexity for a virtual device where built-in drivers are simpler.

### 4. gfxstream HALs as prebuilt vendor modules

**Choice**: Build gfxstream gralloc and hwcomposer from source as part of the AOSP build, placed in the vendor partition.

**Rationale**: gfxstream HALs are the matching guest-side component to the gfxstream host renderer in crosvm. Building from source (from the gfxstream repo) ensures version compatibility. Vendor partition is the correct location per Treble architecture.

### 5. libndk_translation as a prebuilt

**Choice**: Include libndk_translation as a prebuilt binary package extracted from a known-good source (e.g., Waydroid or Google's official release).

**Rationale**: libndk_translation is not open-source and cannot be built from AOSP. Prebuilt integration is the standard approach used by all Android x86 projects. Placed in `/system/lib64/arm64/` and `/system/lib/arm/` with the appropriate `build.prop` flags.

### 6. MicroG integration via `PRODUCT_PACKAGES`

**Choice**: Include MicroG (GmsCore, GsfProxy, FakeStore) as privileged system apps via product makefiles, with signature spoofing patch applied.

**Rationale**: MicroG requires signature spoofing to impersonate Google Play Services. The patch is small (framework-level) and well-established. Including via `PRODUCT_PACKAGES` ensures it's part of the build rather than a post-install step.

### 7. Build entry point: `build.sh` wrapper script

**Choice**: A single `build.sh` script that handles: repo init/sync, lunch target selection, image build, versioning, and checksum generation.

**Rationale**: AOSP's build system requires specific environment setup (`envsetup.sh`, `lunch`). A wrapper script encapsulates this and adds Nux-specific steps (version stamping, checksum generation, artifact collection). CI and developers use the same script.

### 8. Versioning: CalVer + build number

**Choice**: `YYYY.MM.patch` format (e.g., `2026.03.1`) embedded in `build.prop` and used for artifact naming.

**Rationale**: CalVer communicates freshness (which AOSP security patch level is included). Build number allows multiple releases per month. Matches the emulator's own versioning expectations.

## Risks / Trade-offs

- **Large build times (~1-2 hours)** → CI caching of ccache and prebuilt artifacts; incremental builds for development.
- **AOSP source size (~100 GB download, ~300 GB with build)** → CI runners need large disks; document minimum requirements clearly.
- **libndk_translation is a binary blob with unclear licensing** → Document the licensing situation; make it an optional build flag (`WITH_ARM_TRANSLATION=true`) so builds can exclude it.
- **Signature spoofing patch may break on AOSP updates** → Pin to specific AOSP tag; patch is small and well-maintained by the MicroG community.
- **gfxstream HAL API changes between versions** → Pin gfxstream version to match the host-side version used by crosvm in the emulator.
- **CI resource costs** → Builds are expensive; trigger only on tags and manual dispatch, not every push. Use self-hosted runners if GitHub-hosted runners lack disk space.

## Open Questions

1. Should the kernel be built as part of the AOSP tree (in-tree) or as a separate prebuilt? In-tree is simpler but slower; prebuilt allows independent kernel CI.
2. Exact libndk_translation version and source — Waydroid's packaged version vs. extracting from a Pixel image?
3. Should we support `userdebug` and `user` build variants, or only `userdebug` for v1?
4. How will the emulator application download/update images — baked into releases, or fetched on first run?
