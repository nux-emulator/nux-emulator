## ADDED Requirements

### Requirement: Overlay widget renders key hints on game display
The `nux-ui` crate SHALL provide a transparent GTK4 overlay widget that renders semi-transparent key labels on top of the game display surface. Each label SHALL show the bound key name positioned at the binding's screen coordinates (scaled to current display resolution).

#### Scenario: Overlay renders all bindings
- **WHEN** the overlay is visible and the active keymap has 8 bindings
- **THEN** the overlay SHALL render 8 key labels at their respective scaled screen positions

#### Scenario: Joystick binding overlay
- **WHEN** the active keymap contains a joystick binding with keys `["W", "A", "S", "D"]` at center `(200, 1400)`
- **THEN** the overlay SHALL render a circle at the joystick center with directional key labels at the cardinal positions

#### Scenario: Aim region overlay
- **WHEN** the active keymap contains an aim binding with `region = [540, 960, 1080, 1920]`
- **THEN** the overlay SHALL render a semi-transparent rectangle indicating the aim region bounds

### Requirement: Overlay visibility toggle
The overlay SHALL be togglable on and off. The toggle SHALL be accessible via a toolbar button in the main window and a keyboard shortcut.

#### Scenario: Toggle overlay off
- **WHEN** the overlay is visible and the user activates the toggle
- **THEN** the overlay SHALL become hidden and no key labels SHALL be rendered

#### Scenario: Toggle overlay on
- **WHEN** the overlay is hidden and the user activates the toggle
- **THEN** the overlay SHALL become visible and render all key labels for the active keymap

### Requirement: Overlay updates on keymap switch
The overlay SHALL refresh its contents when the active keymap changes. It SHALL read the new overlay data from the keymap engine and redraw.

#### Scenario: Keymap switched while overlay visible
- **WHEN** the overlay is visible and the user switches from keymap A (5 bindings) to keymap B (3 bindings)
- **THEN** the overlay SHALL redraw showing only the 3 bindings from keymap B

#### Scenario: Keymap cleared while overlay visible
- **WHEN** the overlay is visible and the active keymap is cleared
- **THEN** the overlay SHALL render an empty state (no key labels)

### Requirement: Overlay updates on resolution change
The overlay SHALL reposition key labels when the display resolution changes, using the updated scale factors from the keymap engine.

#### Scenario: Resolution change with overlay visible
- **WHEN** the display resolution changes from `1080x1920` to `1440x2560` while the overlay is visible
- **THEN** the overlay SHALL redraw all key labels at positions scaled to the new resolution
