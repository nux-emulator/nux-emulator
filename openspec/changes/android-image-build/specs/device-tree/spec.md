## ADDED Requirements

### Requirement: Device tree directory structure
The device tree SHALL reside at `device/nux/emulator/` within the AOSP source tree and follow standard AOSP device tree conventions.

#### Scenario: Directory layout exists
- **WHEN** the repository is cloned and AOSP source is initialized
- **THEN** `device/nux/emulator/` SHALL contain `BoardConfig.mk`, `device.mk`, `AndroidProducts.mk`, and `vendorsetup.sh`

### Requirement: BoardConfig defines virtual hardware
`BoardConfig.mk` SHALL declare the target architecture as `x86_64`, define partition sizes for system/vendor/userdata, and specify the virtual hardware configuration (no physical SoC, virtio-based peripherals).

#### Scenario: Architecture and partitions configured
- **WHEN** `BoardConfig.mk` is parsed by the AOSP build system
- **THEN** `TARGET_ARCH` SHALL be `x86_64`, `TARGET_ARCH_VARIANT` SHALL be `x86_64`, and partition sizes SHALL be defined for `BOARD_SYSTEMIMAGE_PARTITION_SIZE`, `BOARD_VENDORIMAGE_PARTITION_SIZE`, and `BOARD_USERDATAIMAGE_PARTITION_SIZE`

#### Scenario: Filesystem types configured
- **WHEN** the build system reads partition configuration
- **THEN** system and vendor partitions SHALL use `ext4` filesystem, and the fstab SHALL reference virtio-blk devices

### Requirement: Lunch target available
The device tree SHALL register a lunch target `nux_emulator-userdebug` via `AndroidProducts.mk` and `vendorsetup.sh`.

#### Scenario: Lunch target selectable
- **WHEN** a developer runs `source build/envsetup.sh && lunch nux_emulator-userdebug`
- **THEN** the build environment SHALL be configured for the Nux virtual device without errors

### Requirement: Hardware feature declarations
The device tree SHALL declare hardware features appropriate for a virtual device via `frameworks/native/data/etc/` XML files copied into the system image.

#### Scenario: Virtual device features declared
- **WHEN** the system image is built
- **THEN** the image SHALL include feature declarations for `android.hardware.ethernet`, `android.software.vulkan.deqp.level`, and `android.hardware.vulkan.level` while excluding features like `android.hardware.camera` and `android.hardware.bluetooth`

### Requirement: fstab and init scripts
The device tree SHALL provide an `fstab.nux` file mapping virtio-blk devices to Android partitions and init `.rc` scripts for device-specific initialization.

#### Scenario: fstab mounts partitions correctly
- **WHEN** Android boots using the generated images
- **THEN** `/system`, `/vendor`, and `/data` SHALL be mounted from virtio-blk devices as defined in `fstab.nux`

#### Scenario: Init scripts execute
- **WHEN** Android init processes device-specific rc files
- **THEN** Nux-specific services and property settings SHALL be applied during boot
