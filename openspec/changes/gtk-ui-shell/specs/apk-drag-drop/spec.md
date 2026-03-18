## ADDED Requirements

### Requirement: Drag-and-drop APK installation
The main window SHALL accept file drops via a `GtkDropTarget` controller. When one or more `.apk` files are dropped, the application SHALL install them via the ADB bridge.

#### Scenario: Single APK drop
- **WHEN** the user drags and drops a single `.apk` file onto the main window
- **THEN** the application SHALL install the APK via the ADB bridge and display an `AdwToast` with the result

#### Scenario: Multiple APK drop
- **WHEN** the user drags and drops multiple `.apk` files onto the main window
- **THEN** the application SHALL install each APK sequentially and display an `AdwToast` summarizing the results

#### Scenario: Non-APK file drop
- **WHEN** the user drags and drops a file that is not an `.apk` onto the main window
- **THEN** the application SHALL ignore the file and display an `AdwToast` stating "Only APK files can be installed"

#### Scenario: Drop while VM is stopped
- **WHEN** the user drops an APK file while the VM is not running
- **THEN** the application SHALL display an `AdwToast` stating "VM must be running to install APKs"

### Requirement: Drop visual feedback
The main window SHALL provide visual feedback during drag-over to indicate that APK drop is accepted.

#### Scenario: Drag-over visual cue
- **WHEN** the user drags a file over the main window
- **THEN** the display area SHALL show a visual drop indicator (e.g., overlay with "Drop APK to install" text and an icon)

#### Scenario: Drag-leave clears visual cue
- **WHEN** the user drags a file away from the main window without dropping
- **THEN** the drop indicator overlay SHALL be hidden

### Requirement: APK install progress indication
During APK installation, the application SHALL indicate that an install is in progress.

#### Scenario: Install in progress
- **WHEN** an APK installation is in progress
- **THEN** the application SHALL display a progress indicator (e.g., spinner in an `AdwToast` or toolbar)

#### Scenario: Install completes successfully
- **WHEN** the APK installation completes successfully
- **THEN** an `AdwToast` SHALL display "APK installed successfully: <package-name>"

#### Scenario: Install fails
- **WHEN** the APK installation fails
- **THEN** an `AdwToast` SHALL display the error message from the ADB bridge
