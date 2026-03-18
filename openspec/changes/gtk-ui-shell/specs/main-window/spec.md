## ADDED Requirements

### Requirement: Main window is an AdwApplicationWindow
The main window SHALL be an `AdwApplicationWindow` containing an `AdwHeaderBar`, a central display area, and a sidebar toolbar arranged in a horizontal layout.

#### Scenario: Window creation on activation
- **WHEN** the application is activated
- **THEN** an `AdwApplicationWindow` SHALL be created with a minimum size of 800x600 pixels

### Requirement: Header bar displays VM status and FPS
The `AdwHeaderBar` SHALL display a VM status indicator (Stopped, Starting, Running, Paused) on the left and an optional FPS counter on the right.

#### Scenario: VM status updates
- **WHEN** the VM state changes (e.g., from Starting to Running)
- **THEN** the header bar status label SHALL update to reflect the current state

#### Scenario: FPS counter toggle
- **WHEN** the user activates the `app.toggle-fps` action
- **THEN** the FPS counter label in the header bar SHALL toggle between visible and hidden

#### Scenario: FPS counter display
- **WHEN** the FPS counter is visible and the VM is running
- **THEN** the header bar SHALL display the current frames-per-second value, updated at least once per second

### Requirement: Central display area renders Android screen
The central area SHALL contain a `GtkOverlay` wrapping a `GtkGLArea` (or dmabuf widget) that renders the Android display surface. The display widget SHALL maintain the Android surface aspect ratio, centering the content with black letterbox bars when the aspect ratio does not match the window.

#### Scenario: Display rendering while VM is running
- **WHEN** the VM is running and producing frames
- **THEN** the display widget SHALL render frames from the display pipeline at the native refresh rate

#### Scenario: Aspect ratio preservation on resize
- **WHEN** the user resizes the window to a different aspect ratio than the Android surface
- **THEN** the display widget SHALL maintain the Android surface aspect ratio with black letterbox bars

#### Scenario: Display area when VM is stopped
- **WHEN** the VM is not running
- **THEN** the display area SHALL show a placeholder (e.g., Nux logo or "Start VM" prompt)

### Requirement: Keymap overlay on display
A `GtkOverlay` layer SHALL exist on top of the display widget for rendering keymap hint labels. The overlay SHALL be togglable via the `win.toggle-keymap-overlay` action.

#### Scenario: Keymap overlay toggle on
- **WHEN** the user activates `win.toggle-keymap-overlay` and the overlay is hidden
- **THEN** the keymap hint labels SHALL become visible on top of the display

#### Scenario: Keymap overlay toggle off
- **WHEN** the user activates `win.toggle-keymap-overlay` and the overlay is visible
- **THEN** the keymap hint labels SHALL be hidden

### Requirement: Sidebar toolbar layout
A vertical `GtkBox` toolbar SHALL be positioned on the right side of the main window, containing icon buttons for emulator actions. The toolbar SHALL have a fixed width and SHALL NOT expand when the window is resized.

#### Scenario: Toolbar visibility
- **WHEN** the main window is displayed in windowed mode
- **THEN** the sidebar toolbar SHALL be visible on the right edge

#### Scenario: Toolbar hidden in fullscreen
- **WHEN** the main window enters fullscreen mode
- **THEN** the sidebar toolbar SHALL be hidden to maximize display area

#### Scenario: Toolbar revealed in fullscreen on hover
- **WHEN** the window is in fullscreen mode and the user moves the mouse to the right edge
- **THEN** the sidebar toolbar SHALL temporarily reveal as an overlay

### Requirement: Fullscreen mode
The main window SHALL support fullscreen mode toggled via `Ctrl+F`, `F11`, or the toolbar fullscreen button. In fullscreen, the header bar and sidebar SHALL be hidden, with the display area filling the entire screen.

#### Scenario: Enter fullscreen
- **WHEN** the user activates the fullscreen action
- **THEN** the window SHALL enter fullscreen, hiding the header bar and sidebar toolbar

#### Scenario: Exit fullscreen
- **WHEN** the user presses `Escape` or `F11` while in fullscreen
- **THEN** the window SHALL exit fullscreen, restoring the header bar and sidebar toolbar

### Requirement: Toast overlay for notifications
The main window SHALL include an `AdwToastOverlay` for displaying transient notifications (e.g., "Screenshot saved", "APK installed").

#### Scenario: Toast display
- **WHEN** an operation completes (e.g., screenshot taken)
- **THEN** an `AdwToast` SHALL be displayed with a brief message, auto-dismissing after a few seconds
