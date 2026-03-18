## ADDED Requirements

### Requirement: VSync-aligned presentation
The system SHALL present frames in sync with the compositor's VSync signal by using GTK4's `FrameClock` via `add_tick_callback`. Each tick SHALL present at most one frame — the latest available from the capture layer.

#### Scenario: Frame presented on tick
- **WHEN** the GTK frame clock fires a tick and a new frame is available in the watch channel
- **THEN** the system SHALL present that frame to the display widget

#### Scenario: No new frame on tick
- **WHEN** the GTK frame clock fires a tick but no new frame has arrived since the last tick
- **THEN** the system SHALL keep displaying the previous frame without requesting a widget redraw

#### Scenario: Multiple frames between ticks
- **WHEN** the capture layer produces multiple frames between two consecutive frame clock ticks
- **THEN** the system SHALL present only the latest frame, silently dropping intermediate frames

### Requirement: VSync toggle
The system SHALL support enabling or disabling VSync via the `[display]` configuration. When VSync is disabled, the system SHALL present frames as soon as they arrive rather than waiting for the next frame clock tick.

#### Scenario: VSync enabled (default)
- **WHEN** the display config has `vsync = true` (the default)
- **THEN** the system SHALL present frames only on frame clock ticks, synchronized with the compositor

#### Scenario: VSync disabled
- **WHEN** the display config has `vsync = false`
- **THEN** the system SHALL present frames immediately upon receipt from the capture layer by calling `queue_draw` on the display widget

### Requirement: FPS counter overlay
The system SHALL provide an optional FPS counter overlay that displays the current frame presentation rate. The overlay SHALL be togglable via the `[display]` configuration and at runtime.

#### Scenario: FPS overlay enabled via config
- **WHEN** the display config has `fps_overlay = true`
- **THEN** the system SHALL display a semi-transparent FPS counter in the top-left corner of the display widget showing the current frames-per-second, updated once per second

#### Scenario: FPS overlay disabled (default)
- **WHEN** the display config has `fps_overlay = false` (the default)
- **THEN** the system SHALL not display any FPS overlay

#### Scenario: FPS overlay runtime toggle
- **WHEN** the user toggles the FPS overlay at runtime (via UI action or keybinding)
- **THEN** the system SHALL show or hide the FPS counter without restarting the display pipeline

#### Scenario: FPS calculation accuracy
- **WHEN** the FPS overlay is active
- **THEN** the displayed FPS value SHALL reflect the actual number of unique frames presented to the widget in the previous one-second window, not the frame clock tick rate
