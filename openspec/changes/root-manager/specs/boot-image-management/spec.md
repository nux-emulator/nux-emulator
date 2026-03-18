## ADDED Requirements

### Requirement: Stock boot image storage
The system SHALL store a stock `boot.img` file at `~/.local/share/nux/instances/<name>/boot.img` for each instance. This file MUST be preserved as the original unmodified boot image and SHALL NOT be overwritten by patching operations.

#### Scenario: Stock boot image is stored on instance creation
- **WHEN** a new instance is created with an Android image
- **THEN** the system copies the stock boot.img from the Android image into the instance directory at `~/.local/share/nux/instances/<name>/boot.img`

#### Scenario: Stock boot image is never overwritten
- **WHEN** a patching operation completes
- **THEN** the stock `boot.img` file remains unchanged and byte-identical to the original

### Requirement: Patched boot image storage
The system SHALL store patched boot image variants alongside the stock image using the naming convention `boot_<manager>.img` where `<manager>` is one of `magisk`, `kernelsu`, or `apatch`.

#### Scenario: Magisk patched image is stored
- **WHEN** a Magisk-patched boot image is pulled from the VM
- **THEN** the system stores it at `~/.local/share/nux/instances/<name>/boot_magisk.img`

#### Scenario: KernelSU patched image is stored
- **WHEN** a KernelSU-patched boot image is pulled from the VM
- **THEN** the system stores it at `~/.local/share/nux/instances/<name>/boot_kernelsu.img`

#### Scenario: APatch patched image is stored
- **WHEN** an APatch-patched boot image is pulled from the VM
- **THEN** the system stores it at `~/.local/share/nux/instances/<name>/boot_apatch.img`

### Requirement: Boot image retrieval by root mode
The system SHALL resolve a `RootMode` value to the corresponding boot image file path. If the requested patched image does not exist, the system MUST return an error.

#### Scenario: Resolve stock boot image
- **WHEN** the root mode is `none`
- **THEN** the system returns the path to `boot.img`

#### Scenario: Resolve patched boot image
- **WHEN** the root mode is `magisk`, `kernelsu`, or `apatch`
- **THEN** the system returns the path to the corresponding `boot_<manager>.img` file

#### Scenario: Missing patched image
- **WHEN** the root mode references a patched variant that does not exist on disk
- **THEN** the system returns an error indicating the patched image is missing

### Requirement: Boot image integrity validation
The system SHALL verify that a boot image file exists and is non-empty before returning its path for VM launch.

#### Scenario: Valid boot image
- **WHEN** the resolved boot image file exists and has size greater than zero
- **THEN** the system returns the path successfully

#### Scenario: Corrupt or empty boot image
- **WHEN** the resolved boot image file is empty (zero bytes)
- **THEN** the system returns an error indicating the image is corrupt
