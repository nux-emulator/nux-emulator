## ADDED Requirements

### Requirement: Root mode enum
The system SHALL define a `RootMode` enum with variants `None`, `Magisk`, `KernelSu`, and `APatch`. This enum MUST be serializable to/from TOML as lowercase strings (`none`, `magisk`, `kernelsu`, `apatch`).

#### Scenario: Serialize root mode to config
- **WHEN** the root mode is `Magisk`
- **THEN** it serializes to the string `"magisk"` in TOML

#### Scenario: Deserialize root mode from config
- **WHEN** the config contains `root.mode = "kernelsu"`
- **THEN** it deserializes to `RootMode::KernelSu`

#### Scenario: Default root mode
- **WHEN** no `root.mode` is present in the config
- **THEN** the system defaults to `RootMode::None`

### Requirement: Root mode switching
The system SHALL allow changing the active root mode by updating the instance config. Switching root mode MUST NOT require re-patching if the target patched boot image already exists on disk.

#### Scenario: Switch from none to magisk
- **WHEN** the user sets root mode to `magisk` and `boot_magisk.img` exists
- **THEN** the system updates `root.mode` to `magisk` in the instance config

#### Scenario: Switch to unavailable patched image
- **WHEN** the user sets root mode to `apatch` but `boot_apatch.img` does not exist
- **THEN** the system returns an error indicating the patched image must be created first

#### Scenario: Switch between root managers
- **WHEN** the user switches from `magisk` to `kernelsu` and `boot_kernelsu.img` exists
- **THEN** the system updates `root.mode` to `kernelsu` in the instance config

### Requirement: Unroot instance
The system SHALL support unrooting by setting the root mode back to `none`. Unrooting MUST NOT delete any patched boot images so the user can re-enable root later without re-patching.

#### Scenario: Unroot preserves patched images
- **WHEN** the user unroots an instance that was using Magisk
- **THEN** the system sets `root.mode` to `none` and `boot_magisk.img` remains on disk

#### Scenario: Re-root after unroot
- **WHEN** the user sets root mode back to `magisk` after previously unrooting
- **THEN** the system uses the existing `boot_magisk.img` without requiring re-patching

### Requirement: Active boot image for VM launch
The system SHALL resolve the current root mode to a boot image path that the VM launcher passes to crosvm. The VM MUST be restarted for a root mode change to take effect.

#### Scenario: Launch with stock boot image
- **WHEN** the VM starts and root mode is `none`
- **THEN** crosvm receives the path to `boot.img`

#### Scenario: Launch with patched boot image
- **WHEN** the VM starts and root mode is `magisk`
- **THEN** crosvm receives the path to `boot_magisk.img`

#### Scenario: Root mode change requires restart
- **WHEN** the root mode is changed while the VM is running
- **THEN** the system indicates that a VM restart is required for the change to take effect
