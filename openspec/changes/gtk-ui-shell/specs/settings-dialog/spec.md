## ADDED Requirements

### Requirement: Settings dialog is an AdwPreferencesWindow
The settings dialog SHALL be an `AdwPreferencesWindow` opened via the `win.open-settings` action. It SHALL be modal relative to the main window.

#### Scenario: Open settings
- **WHEN** the user activates the settings action (toolbar button or keyboard shortcut)
- **THEN** an `AdwPreferencesWindow` SHALL be presented as a modal dialog

#### Scenario: Close settings
- **WHEN** the user closes the settings dialog
- **THEN** any changed settings SHALL have already been persisted to the config file

### Requirement: Performance settings page
The Performance page SHALL contain controls for CPU core count, RAM allocation, display resolution, and DPI.

#### Scenario: Adjust CPU cores
- **WHEN** the user changes the CPU cores spin row (range: 1 to host core count)
- **THEN** the config SHALL be updated and a restart-required banner SHALL be displayed

#### Scenario: Adjust RAM
- **WHEN** the user changes the RAM combo row (preset values: 2GB, 4GB, 6GB, 8GB)
- **THEN** the config SHALL be updated and a restart-required banner SHALL be displayed

#### Scenario: Adjust DPI
- **WHEN** the user changes the DPI spin row (range: 120–640, step 20)
- **THEN** the config SHALL be updated and a restart-required banner SHALL be displayed

### Requirement: Root settings page
The Root page SHALL allow the user to select a root mode and view boot.img patching status.

#### Scenario: Select root mode
- **WHEN** the user selects a root mode from the combo row (None, Magisk, KernelSU, APatch)
- **THEN** the config SHALL be updated with the selected root mode

#### Scenario: View boot.img status
- **WHEN** the Root page is displayed
- **THEN** an action row SHALL show the current boot.img patch status (Unpatched, Patched with X, Error)

### Requirement: Google Services settings page
The Google Services page SHALL allow the user to select between MicroG, GApps, or None.

#### Scenario: Select Google Services mode
- **WHEN** the user selects a Google Services option from the combo row
- **THEN** the config SHALL be updated and a restart-required banner SHALL be displayed if the VM is running

### Requirement: Device settings page
The Device page SHALL allow the user to configure device model spoofing and default orientation.

#### Scenario: Set device model
- **WHEN** the user selects a device model from the combo row (e.g., Pixel 9, Samsung S24, Custom)
- **THEN** the config SHALL be updated with the selected device model fingerprint

#### Scenario: Set default orientation
- **WHEN** the user selects an orientation (Portrait, Landscape) from the combo row
- **THEN** the config SHALL be updated with the selected default orientation

### Requirement: Display settings page
The Display page SHALL allow the user to select resolution presets or enter a custom resolution.

#### Scenario: Select resolution preset
- **WHEN** the user selects a resolution preset (e.g., 1080x1920, 1440x2560, 720x1280)
- **THEN** the config SHALL be updated with the selected resolution

#### Scenario: Enter custom resolution
- **WHEN** the user selects "Custom" and enters width and height values
- **THEN** the config SHALL be updated with the custom resolution values, validated to be within 320x480 minimum and 3840x2160 maximum

### Requirement: About page
The About page SHALL display application version, build info, license (GPLv3), and links to the project repository and issue tracker.

#### Scenario: View about information
- **WHEN** the user navigates to the About page
- **THEN** the page SHALL display the application name, version string, license, and clickable project links

### Requirement: Restart-required banner
When a setting change requires a VM restart to take effect, an `AdwBanner` SHALL be displayed at the top of the settings window indicating that a restart is needed.

#### Scenario: Banner appears on restart-required change
- **WHEN** the user changes a setting that requires VM restart (CPU, RAM, DPI, resolution, Google Services)
- **THEN** an `AdwBanner` SHALL appear with the message "VM restart required for changes to take effect"

#### Scenario: Banner dismissed after restart
- **WHEN** the VM is restarted after a restart-required change
- **THEN** the restart-required banner SHALL be hidden
