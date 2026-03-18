## ADDED Requirements

### Requirement: Toggle input grab with hotkey
The system SHALL toggle mouse capture (input grab) when the user presses the grab hotkey (default: Ctrl+Alt). When grabbed, the mouse cursor SHALL be confined to the emulator window and hidden. When released, the cursor SHALL be freed and visible.

#### Scenario: Activate input grab
- **WHEN** the user presses Ctrl+Alt while the cursor is free
- **THEN** the system confines the pointer to the emulator window, hides the cursor, and enters grabbed mode

#### Scenario: Deactivate input grab
- **WHEN** the user presses Ctrl+Alt while in grabbed mode
- **THEN** the system releases pointer confinement, shows the cursor, and enters free mode

#### Scenario: Initial state is ungrabbed
- **WHEN** the emulator window is first displayed
- **THEN** the cursor SHALL be free (not grabbed) by default

### Requirement: Report relative motion in grabbed mode
The system SHALL report mouse motion as relative deltas (dx, dy) when in grabbed mode, instead of absolute coordinates. These deltas SHALL be translated to `EV_REL/REL_X` and `EV_REL/REL_Y` evdev events for the VM.

#### Scenario: Relative motion while grabbed
- **WHEN** the mouse moves while input is grabbed
- **THEN** the system emits `EV_REL/REL_X` and `EV_REL/REL_Y` events with the motion delta, followed by `EV_SYN/SYN_REPORT`

#### Scenario: Absolute motion while ungrabbed
- **WHEN** the mouse moves while input is not grabbed
- **THEN** the system emits `EV_ABS/ABS_X` and `EV_ABS/ABS_Y` events with mapped absolute coordinates (no relative events)

### Requirement: Graceful fallback when pointer confinement is unavailable
The system SHALL attempt Wayland pointer confinement (`zwp_pointer_constraints`) or X11 pointer grab. If confinement is not supported by the compositor, the system SHALL fall back to computing relative deltas from consecutive absolute positions and log a warning.

#### Scenario: Wayland confinement supported
- **WHEN** the compositor supports `zwp_pointer_constraints`
- **THEN** the system uses native pointer confinement for grab mode

#### Scenario: Confinement not supported
- **WHEN** pointer confinement is not available on the current display server
- **THEN** the system falls back to delta computation from absolute coordinates and logs a warning message
