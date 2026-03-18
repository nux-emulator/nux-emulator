## ADDED Requirements

### Requirement: Install APK from file path
The system SHALL install an APK file onto the Android guest given a host file path. The APK SHALL be pushed to the guest and installed via the ADB install protocol.

#### Scenario: Successful APK install
- **WHEN** the caller provides a valid path to an APK file on the host filesystem
- **THEN** the system SHALL push the APK to the guest, invoke package installation, and return success with the installed package name

#### Scenario: APK file not found
- **WHEN** the caller provides a path that does not exist or is not readable
- **THEN** the system SHALL return an error indicating the file was not found, without contacting the guest

#### Scenario: Installation failure on guest
- **WHEN** the APK is pushed but the guest package manager rejects it (e.g., invalid APK, incompatible ABI)
- **THEN** the system SHALL return an error containing the failure reason from the guest package manager

#### Scenario: Install with progress reporting
- **WHEN** an APK is being pushed and installed
- **THEN** the system SHALL report progress (bytes transferred / total bytes) via a progress callback or channel

### Requirement: Uninstall app by package name
The system SHALL uninstall an application from the Android guest given its package name.

#### Scenario: Successful uninstall
- **WHEN** the caller provides a valid installed package name
- **THEN** the system SHALL uninstall the package and return success

#### Scenario: Package not installed
- **WHEN** the caller provides a package name that is not installed on the guest
- **THEN** the system SHALL return an error indicating the package was not found

### Requirement: List installed packages
The system SHALL retrieve the list of installed packages from the Android guest.

#### Scenario: List all packages
- **WHEN** the caller requests the installed package list
- **THEN** the system SHALL return a list of `PackageInfo` structs containing at minimum the package name for each installed app

#### Scenario: ADB not connected
- **WHEN** the caller requests the package list but ADB is not connected
- **THEN** the system SHALL return an error indicating ADB is not connected

### Requirement: Launch app by package name
The system SHALL launch an application on the Android guest given its package name, using the package's default launch intent.

#### Scenario: Successful launch
- **WHEN** the caller provides a package name that has a launchable activity
- **THEN** the system SHALL start the app's main activity on the guest

#### Scenario: No launchable activity
- **WHEN** the caller provides a package name that has no default launch intent
- **THEN** the system SHALL return an error indicating no launchable activity was found
