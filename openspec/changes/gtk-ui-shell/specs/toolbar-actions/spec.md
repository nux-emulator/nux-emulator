## ADDED Requirements

### Requirement: Screenshot action
The toolbar SHALL include a screenshot button that captures the current Android display frame.

#### Scenario: Take screenshot
- **WHEN** the user clicks the screenshot toolbar button or activates `win.screenshot`
- **THEN** the current display frame SHALL be saved as a PNG file to the user's default screenshot directory (e.g., `~/Pictures/Nux/`)
- **THEN** an `AdwToast` SHALL confirm the save with the file path

#### Scenario: Screenshot with VM stopped
- **WHEN** the user activates the screenshot action while the VM is not running
- **THEN** the action SHALL be disabled (button grayed out)

### Requirement: Volume control actions
The toolbar SHALL include volume up and volume down buttons that send volume key events to the Android VM.

#### Scenario: Volume up
- **WHEN** the user clicks the volume up toolbar button or activates `win.volume-up`
- **THEN** a `VOLUME_UP` key event SHALL be sent to the VM via the input system

#### Scenario: Volume down
- **WHEN** the user clicks the volume down toolbar button or activates `win.volume-down`
- **THEN** a `VOLUME_DOWN` key event SHALL be sent to the VM via the input system

### Requirement: Shake device action
The toolbar SHALL include a shake button that simulates an accelerometer shake event in the Android VM.

#### Scenario: Shake device
- **WHEN** the user clicks the shake toolbar button or activates `win.shake`
- **THEN** an accelerometer shake sequence SHALL be sent to the VM via the input system

### Requirement: Rotate action
The toolbar SHALL include a rotate button that toggles the Android display between portrait and landscape orientation.

#### Scenario: Rotate from portrait to landscape
- **WHEN** the user clicks the rotate button while the VM is in portrait mode
- **THEN** the VM display SHALL rotate to landscape orientation and the display widget SHALL resize accordingly

#### Scenario: Rotate from landscape to portrait
- **WHEN** the user clicks the rotate button while the VM is in landscape mode
- **THEN** the VM display SHALL rotate to portrait orientation and the display widget SHALL resize accordingly

### Requirement: Install APK action
The toolbar SHALL include an install APK button that opens a file chooser dialog for selecting an APK file.

#### Scenario: Install APK via file chooser
- **WHEN** the user clicks the install APK toolbar button or activates `win.install-apk`
- **THEN** a `GtkFileDialog` SHALL open filtered to `.apk` files
- **WHEN** the user selects an APK file
- **THEN** the APK SHALL be installed via the ADB bridge and an `AdwToast` SHALL report success or failure

#### Scenario: Cancel APK file chooser
- **WHEN** the user opens the file chooser and cancels without selecting a file
- **THEN** no action SHALL be taken

### Requirement: Toggle keymap overlay action
The toolbar SHALL include a keymap overlay toggle button that shows or hides keymap hints on the display.

#### Scenario: Toggle keymap overlay
- **WHEN** the user clicks the keymap overlay toolbar button
- **THEN** the `win.toggle-keymap-overlay` action SHALL be activated, toggling the overlay visibility

### Requirement: Settings action
The toolbar SHALL include a settings button that opens the settings dialog.

#### Scenario: Open settings from toolbar
- **WHEN** the user clicks the settings toolbar button
- **THEN** the `win.open-settings` action SHALL be activated, presenting the settings dialog

### Requirement: Fullscreen action
The toolbar SHALL include a fullscreen toggle button.

#### Scenario: Toggle fullscreen from toolbar
- **WHEN** the user clicks the fullscreen toolbar button
- **THEN** the `win.toggle-fullscreen` action SHALL be activated, toggling fullscreen mode

### Requirement: Toolbar actions disabled when VM is stopped
Actions that require a running VM (screenshot, volume, shake, rotate) SHALL be disabled when the VM is not running.

#### Scenario: Actions disabled on VM stop
- **WHEN** the VM state transitions to Stopped
- **THEN** the screenshot, volume up, volume down, shake, and rotate toolbar buttons SHALL be insensitive (grayed out)

#### Scenario: Actions enabled on VM start
- **WHEN** the VM state transitions to Running
- **THEN** the screenshot, volume up, volume down, shake, and rotate toolbar buttons SHALL become sensitive (clickable)
