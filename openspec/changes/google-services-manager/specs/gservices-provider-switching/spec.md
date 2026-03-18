## ADDED Requirements

### Requirement: Provider mode enum
The system SHALL define a `GoogleServicesProvider` enum with variants `MicroG`, `GApps`, and `None` representing the three supported provider modes.

#### Scenario: All variants representable
- **WHEN** a provider mode is set to any of MicroG, GApps, or None
- **THEN** the enum correctly represents and serializes that variant

### Requirement: Switch from MicroG to GApps
The system SHALL support switching from MicroG to GApps by downloading a GApps package, verifying its SHA-256 hash, applying it to the instance's system image overlay, updating the config, and signaling that a VM restart is required.

#### Scenario: Successful MicroG to GApps switch
- **WHEN** the current provider is MicroG and the user requests a switch to GApps and the VM is stopped
- **THEN** the system downloads the GApps package, verifies integrity, applies the overlay, updates `GoogleServicesConfig` to `GApps`, and sets a restart-required flag

#### Scenario: GApps switch rejected while VM is running
- **WHEN** the user requests a switch to GApps and the VM is running
- **THEN** the system returns an error indicating the VM must be stopped before switching providers

### Requirement: Switch from GApps to MicroG
The system SHALL support switching from GApps back to MicroG by resetting the instance's system image overlay to the base state (which includes MicroG), updating the config, and signaling a VM restart.

#### Scenario: Successful GApps to MicroG switch
- **WHEN** the current provider is GApps and the user requests a switch to MicroG and the VM is stopped
- **THEN** the system resets the overlay to base state, updates `GoogleServicesConfig` to `MicroG`, and sets a restart-required flag

### Requirement: Switch to None mode
The system SHALL support switching to None mode from either MicroG or GApps by applying an overlay that removes all Google Services packages, updating the config, and signaling a VM restart.

#### Scenario: Switch from MicroG to None
- **WHEN** the current provider is MicroG and the user requests a switch to None and the VM is stopped
- **THEN** the system applies a removal overlay, updates `GoogleServicesConfig` to `None`, and sets a restart-required flag

#### Scenario: Switch from GApps to None
- **WHEN** the current provider is GApps and the user requests a switch to None and the VM is stopped
- **THEN** the system resets the GApps overlay and applies a removal overlay, updates config to `None`, and sets a restart-required flag

### Requirement: Switch from None to MicroG or GApps
The system SHALL support switching from None to either MicroG (by resetting overlay to base) or GApps (by downloading and applying the GApps package).

#### Scenario: Switch from None to MicroG
- **WHEN** the current provider is None and the user requests a switch to MicroG and the VM is stopped
- **THEN** the system resets the overlay to base state, updates config to `MicroG`, and sets a restart-required flag

#### Scenario: Switch from None to GApps
- **WHEN** the current provider is None and the user requests a switch to GApps and the VM is stopped
- **THEN** the system downloads GApps (if not cached), verifies integrity, applies the overlay, updates config to `GApps`, and sets a restart-required flag

### Requirement: GApps package download with integrity verification
The system SHALL download GApps packages (OpenGApps or MindTheGapps) from configured URLs and verify the SHA-256 hash before applying. Downloaded packages SHALL be cached under `$XDG_DATA_HOME/nux/cache/gapps/`.

#### Scenario: Successful download and verification
- **WHEN** a GApps download is initiated and the download completes
- **THEN** the system verifies the SHA-256 hash matches the expected value and stores the package in the cache directory

#### Scenario: Hash verification failure
- **WHEN** a GApps package is downloaded but the SHA-256 hash does not match
- **THEN** the system deletes the corrupted download and returns an error indicating integrity verification failed

#### Scenario: Use cached package
- **WHEN** a GApps switch is requested and a verified package already exists in the cache
- **THEN** the system uses the cached package without re-downloading

### Requirement: Overlay backup and rollback
The system SHALL back up the current instance overlay before applying provider changes. If the apply operation fails, the system SHALL restore the backup automatically.

#### Scenario: Successful apply with backup
- **WHEN** a provider switch modifies the overlay
- **THEN** the system creates a backup of the overlay before modification

#### Scenario: Rollback on failure
- **WHEN** an overlay modification fails mid-operation
- **THEN** the system restores the overlay from the backup and returns an error

### Requirement: VM stopped precondition
The system SHALL reject any provider switch operation if the VM is currently running, returning an error that instructs the user to stop the VM first.

#### Scenario: Reject switch while VM running
- **WHEN** any provider switch is requested and the VM is in a running state
- **THEN** the system returns an error with message indicating the VM must be stopped
