## ADDED Requirements

### Requirement: Execute shell command
The system SHALL execute a shell command on the Android guest via the ADB shell protocol and return the command output.

#### Scenario: Successful command execution
- **WHEN** the caller provides a shell command string
- **THEN** the system SHALL execute the command on the guest and return stdout and the exit code

#### Scenario: Command execution failure
- **WHEN** the shell command returns a non-zero exit code
- **THEN** the system SHALL return the exit code along with both stdout and stderr content

#### Scenario: Command timeout
- **WHEN** a shell command does not complete within a caller-specified timeout
- **THEN** the system SHALL abort the command and return a timeout error

### Requirement: Screenshot capture
The system SHALL capture a screenshot from the Android guest via `screencap` and return the image data.

#### Scenario: Successful screenshot
- **WHEN** the caller requests a screenshot
- **THEN** the system SHALL execute `screencap -p` on the guest and return the PNG image data as bytes

#### Scenario: Screenshot when display is off
- **WHEN** the caller requests a screenshot but the guest display is off
- **THEN** the system SHALL return an error indicating the display is unavailable

### Requirement: Device info queries
The system SHALL query device properties from the Android guest, including Android version, API level, device model, and supported ABIs.

#### Scenario: Query device info
- **WHEN** the caller requests device information
- **THEN** the system SHALL return a `DeviceInfo` struct containing: Android version (`ro.build.version.release`), API level (`ro.build.version.sdk`), device model (`ro.product.model`), and supported ABIs (`ro.product.cpu.abilist`)

#### Scenario: Property not available
- **WHEN** a specific device property is not set on the guest
- **THEN** the system SHALL return `None` for that field in the `DeviceInfo` struct rather than failing the entire query

### Requirement: Input injection fallback
The system SHALL support injecting input events via `adb shell input` as a fallback when direct input routing is unavailable.

#### Scenario: Inject tap event
- **WHEN** the caller requests a tap at coordinates (x, y)
- **THEN** the system SHALL execute `input tap <x> <y>` on the guest

#### Scenario: Inject text input
- **WHEN** the caller provides a text string to input
- **THEN** the system SHALL execute `input text <escaped_string>` on the guest with properly escaped special characters

#### Scenario: Inject key event
- **WHEN** the caller provides an Android keycode
- **THEN** the system SHALL execute `input keyevent <keycode>` on the guest

### Requirement: Screen resolution query
The system SHALL query the current screen resolution of the Android guest.

#### Scenario: Query resolution
- **WHEN** the caller requests the screen resolution
- **THEN** the system SHALL execute `wm size` on the guest and return the width and height in pixels

#### Scenario: Resolution with override
- **WHEN** the guest has a display size override set
- **THEN** the system SHALL return the override resolution, not the physical resolution
