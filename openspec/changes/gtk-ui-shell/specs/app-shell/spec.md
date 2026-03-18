## ADDED Requirements

### Requirement: Application entry point uses AdwApplication
The `nux-ui` binary SHALL bootstrap via `adw::Application` with a unique application ID (`com.nuxemu.Nux`). The application SHALL initialize GTK4 and libadwaita, register application-level actions, and present the main window on activation.

#### Scenario: Normal application launch
- **WHEN** the user launches `nux-ui` with no arguments
- **THEN** the application SHALL create and present the main window

#### Scenario: Launch with APK file argument
- **WHEN** the user launches `nux-ui` with an APK file path as a CLI argument
- **THEN** the application SHALL present the main window and initiate APK installation for the provided file

### Requirement: Single-instance enforcement
The application SHALL enforce single-instance semantics via `GApplication` D-Bus activation. A second launch SHALL activate the existing window instead of creating a new process.

#### Scenario: Second instance launch
- **WHEN** a second `nux-ui` process is started while one is already running
- **THEN** the existing window SHALL be raised and focused, and the second process SHALL exit

#### Scenario: Second instance with APK argument
- **WHEN** a second `nux-ui` process is started with an APK file argument while one is already running
- **THEN** the existing instance SHALL receive the file and initiate APK installation

### Requirement: Application menu and keyboard shortcuts
The application SHALL register global keyboard shortcuts via `gtk::ShortcutController` and expose an application menu with standard entries.

#### Scenario: Quit shortcut
- **WHEN** the user presses `Ctrl+Q`
- **THEN** the application SHALL initiate graceful shutdown

#### Scenario: Fullscreen shortcut
- **WHEN** the user presses `Ctrl+F` or `F11`
- **THEN** the main window SHALL toggle fullscreen mode

#### Scenario: About dialog
- **WHEN** the user activates the "About" menu entry
- **THEN** an `AdwAboutDialog` SHALL be presented showing application name, version, license (GPLv3), and project links

### Requirement: Graceful shutdown
The application SHALL perform graceful shutdown when closed, ensuring pending operations (e.g., APK install) are completed or cancelled and window state is persisted.

#### Scenario: Close during idle
- **WHEN** the user closes the application while no operations are in progress
- **THEN** the application SHALL save window state and exit

#### Scenario: Close during APK install
- **WHEN** the user closes the application while an APK install is in progress
- **THEN** the application SHALL display a confirmation dialog before exiting
