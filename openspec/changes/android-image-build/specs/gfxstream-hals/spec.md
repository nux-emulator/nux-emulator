## ADDED Requirements

### Requirement: Gfxstream gralloc HAL included
The build SHALL include the gfxstream gralloc HAL as a vendor module, providing GPU buffer allocation through virtio-gpu.

#### Scenario: Gralloc HAL present in vendor image
- **WHEN** the vendor image is built
- **THEN** the gfxstream gralloc HAL shared library SHALL be present at the expected path under `/vendor/lib64/hw/`

#### Scenario: Gralloc allocates buffers
- **WHEN** Android's SurfaceFlinger requests a graphics buffer allocation
- **THEN** the gfxstream gralloc SHALL allocate the buffer via virtio-gpu without falling back to software allocation

### Requirement: Gfxstream hwcomposer HAL included
The build SHALL include the gfxstream hwcomposer (HWC) HAL as a vendor module, providing display composition through virtio-gpu.

#### Scenario: HWC HAL present in vendor image
- **WHEN** the vendor image is built
- **THEN** the gfxstream hwcomposer HAL shared library SHALL be present at the expected path under `/vendor/lib64/hw/`

#### Scenario: Display composition works
- **WHEN** Android boots with the gfxstream hwcomposer
- **THEN** SurfaceFlinger SHALL use the gfxstream HWC for display composition and the display SHALL render correctly

### Requirement: Gfxstream built from source
The gfxstream guest HALs SHALL be built from source as part of the AOSP build, not included as opaque prebuilts.

#### Scenario: Source build integration
- **WHEN** the AOSP build is executed
- **THEN** gfxstream gralloc and hwcomposer SHALL be compiled from source via Android.bp/Android.mk files included in the build tree

### Requirement: Gfxstream version pinned
The gfxstream guest HAL version SHALL be pinned to match the host-side gfxstream renderer version used by crosvm in the Nux emulator.

#### Scenario: Version compatibility
- **WHEN** the built Android image is run with the Nux emulator's crosvm
- **THEN** the guest gfxstream HALs and host gfxstream renderer SHALL be protocol-compatible with no version mismatch errors
