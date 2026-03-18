## ADDED Requirements

### Requirement: Kernel based on android-common
The kernel SHALL be based on Google's `android-common` branch matching the AOSP 16 release, providing LTS stability and Android-specific patches (binder, ashmem).

#### Scenario: Kernel source branch
- **WHEN** the kernel source is checked out for building
- **THEN** it SHALL use the `android-common` branch corresponding to the AOSP 16 target kernel version

### Requirement: Virtio drivers enabled
The kernel defconfig SHALL enable all virtio drivers required for crosvm operation as built-in (`=y`), not modules.

#### Scenario: Required virtio configs present
- **WHEN** the kernel defconfig is applied
- **THEN** the following configs SHALL be set to `y`: `CONFIG_VIRTIO_GPU`, `CONFIG_VIRTIO_INPUT`, `CONFIG_VIRTIO_NET`, `CONFIG_VIRTIO_SND`, `CONFIG_VIRTIO_BLK`, `CONFIG_VIRTIO_PCI`, and `CONFIG_VIRTIO_MMIO`

#### Scenario: Kernel boots in crosvm
- **WHEN** the built kernel is loaded by crosvm with KVM acceleration
- **THEN** the kernel SHALL boot successfully and detect all virtio devices without missing driver errors

### Requirement: KVM guest support enabled
The kernel SHALL have KVM guest support enabled for paravirtualized operation.

#### Scenario: KVM guest configs present
- **WHEN** the kernel defconfig is applied
- **THEN** `CONFIG_KVM_GUEST` and `CONFIG_PARAVIRT` SHALL be set to `y`

### Requirement: Defconfig fragment overlay
Nux-specific kernel configuration SHALL be maintained as a defconfig fragment that overlays on top of the base `android-common` config, rather than forking the entire defconfig.

#### Scenario: Fragment applies cleanly
- **WHEN** the Nux defconfig fragment is merged with the base android-common defconfig
- **THEN** the resulting `.config` SHALL contain all Nux-specific settings without conflicts or warnings
