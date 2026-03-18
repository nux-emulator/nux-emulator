## ADDED Requirements

### Requirement: Fullscreen toggle
The system SHALL provide a fullscreen toggle that switches the GTK window between windowed and fullscreen modes using GTK4's native fullscreen API (`GtkWindow::fullscreen()` / `GtkWindow::unfullscreen()`).

#### Scenario: Enter fullscreen
- **WHEN** the user triggers the fullscreen toggle while in windowed mode
- **THEN** the system SHALL call `GtkWindow::fullscreen()`, hide UI chrome (toolbar, status bar), and let the display widget fill the entire screen

#### Scenario: Exit fullscreen
- **WHEN** the user triggers the fullscreen toggle while in fullscreen mode
- **THEN** the system SHALL call `GtkWindow::unfullscreen()`, restore UI chrome, and return the display widget to its windowed layout

#### Scenario: Fullscreen via keyboard shortcut
- **WHEN** the user presses F11 (or the configured fullscreen keybinding)
- **THEN** the system SHALL toggle fullscreen state

### Requirement: Fullscreen scaling adaptation
The system SHALL adapt frame scaling when entering or exiting fullscreen. The display widget SHALL recalculate its scaling to fill the fullscreen area while preserving the configured scaling mode and aspect ratio.

#### Scenario: Scaling updates on fullscreen enter
- **WHEN** the window enters fullscreen mode
- **THEN** the system SHALL recalculate frame scaling for the full monitor resolution while preserving the active scaling mode (contain or integer) and aspect ratio

#### Scenario: Scaling updates on fullscreen exit
- **WHEN** the window exits fullscreen mode
- **THEN** the system SHALL recalculate frame scaling for the restored window size

### Requirement: No frame drop during fullscreen transition
The system SHALL continue presenting frames without interruption during fullscreen transitions. There SHALL be no visible black flash or frame gap when entering or exiting fullscreen.

#### Scenario: Continuous frame display during enter
- **WHEN** the window transitions from windowed to fullscreen
- **THEN** the system SHALL continue presenting the latest frame throughout the transition with no visible interruption

#### Scenario: Continuous frame display during exit
- **WHEN** the window transitions from fullscreen to windowed
- **THEN** the system SHALL continue presenting the latest frame throughout the transition with no visible interruption
