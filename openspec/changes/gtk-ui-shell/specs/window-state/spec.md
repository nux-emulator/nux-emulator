## ADDED Requirements

### Requirement: Window geometry persistence
The application SHALL save the main window's width, height, and maximized state to the Nux TOML config file when the window is closed, and restore these values on next launch.

#### Scenario: Save window state on close
- **WHEN** the user closes the main window
- **THEN** the current window width, height, and maximized state SHALL be written to the `[ui.window]` section of the config file

#### Scenario: Restore window state on launch
- **WHEN** the application launches and a saved window state exists in the config
- **THEN** the main window SHALL be created with the saved width, height, and maximized state

#### Scenario: First launch with no saved state
- **WHEN** the application launches and no saved window state exists
- **THEN** the main window SHALL use default dimensions (1024x768, not maximized)

### Requirement: Window state respects display bounds
The restored window geometry SHALL be validated against the current display dimensions to avoid off-screen placement.

#### Scenario: Saved size exceeds current display
- **WHEN** the saved window dimensions exceed the current display's available area
- **THEN** the window SHALL be clamped to fit within the display and centered

#### Scenario: Display configuration changed between sessions
- **WHEN** the saved window position would place the window off-screen (e.g., monitor removed)
- **THEN** the window SHALL be repositioned to the center of the primary display

### Requirement: Fullscreen state is not persisted
The fullscreen state SHALL NOT be saved or restored. The application SHALL always launch in windowed mode.

#### Scenario: Close while fullscreen
- **WHEN** the user closes the application while in fullscreen mode
- **THEN** the pre-fullscreen window dimensions SHALL be saved (not the fullscreen dimensions)

#### Scenario: Launch after fullscreen close
- **WHEN** the application launches after being closed in fullscreen
- **THEN** the window SHALL open in windowed mode with the pre-fullscreen dimensions
