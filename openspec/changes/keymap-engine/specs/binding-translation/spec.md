## ADDED Requirements

### Requirement: Tap binding translates key press to touch down/up
When a tap binding's key is pressed, the engine SHALL emit a touch-down event at the binding's coordinates. When the key is released, the engine SHALL emit a touch-up event at the same coordinates.

#### Scenario: Key press and release for tap
- **WHEN** key `W` is pressed and the active keymap has a tap binding for `W` at `(540, 1400)`
- **THEN** the engine SHALL emit a touch-down event at `(540, 1400)` on key press and a touch-up event at `(540, 1400)` on key release

#### Scenario: Tap binding with coordinate scaling
- **WHEN** key `W` is pressed, the tap binding is at `(540, 1400)`, and the display scale factor is `(2.0, 2.0)`
- **THEN** the engine SHALL emit a touch-down event at `(1080, 2800)`

### Requirement: Long-press binding translates held key to sustained touch
When a long-press binding's key is pressed, the engine SHALL emit a touch-down event at the binding's coordinates. The touch SHALL remain active until the key is released, at which point a touch-up event is emitted. The `duration_ms` field defines the minimum hold time for the game to recognize it as a long-press.

#### Scenario: Key held for long-press duration
- **WHEN** key `F` is pressed for a long-press binding at `(300, 800)` with `duration_ms = 500`, and the key is held for 600ms then released
- **THEN** the engine SHALL emit touch-down on press and touch-up on release, with the touch held for the full 600ms

#### Scenario: Key released before long-press duration
- **WHEN** key `F` is pressed and released after 200ms for a long-press binding with `duration_ms = 500`
- **THEN** the engine SHALL emit touch-down on press and touch-up on release (the binding still functions; duration_ms is informational for the game's recognition)

### Requirement: Swipe binding translates key press to animated swipe
When a swipe binding's key is pressed, the engine SHALL emit a touch-down at the `from` coordinates, then interpolate touch-move events from `from` to `to` over the specified `duration_ms`, and finally emit a touch-up at the `to` coordinates.

#### Scenario: Complete swipe animation
- **WHEN** key `Q` is pressed for a swipe binding with `from = [100, 500]`, `to = [400, 500]`, `duration_ms = 200`
- **THEN** the engine SHALL emit touch-down at `(100, 500)`, a series of touch-move events interpolating to `(400, 500)` over 200ms, and touch-up at `(400, 500)`

#### Scenario: Swipe key released before animation completes
- **WHEN** key `Q` is pressed and released after 100ms for a swipe with `duration_ms = 200`
- **THEN** the engine SHALL complete the full swipe animation regardless of early key release (swipe is fire-and-forget)

### Requirement: Joystick binding translates WASD keys to virtual stick movement
The joystick binding SHALL map 4 directional keys to touch positions on a virtual circle. When a directional key is pressed, the engine SHALL emit a touch-down at the joystick center (if not already active) and a touch-move to the corresponding point on the circle's edge. Diagonal movement (two keys held) SHALL move to the combined direction. When all keys are released, the engine SHALL emit touch-up.

#### Scenario: Single direction press
- **WHEN** key `W` (up) is pressed for a joystick with `center = (200, 1400)` and `radius = 150`
- **THEN** the engine SHALL emit touch-down at `(200, 1400)` then touch-move to `(200, 1250)` (center_y - radius)

#### Scenario: Diagonal movement
- **WHEN** keys `W` (up) and `D` (right) are both held for a joystick with `center = (200, 1400)` and `radius = 150`
- **THEN** the engine SHALL emit touch-move to the point at 45 degrees (up-right) on the circle, approximately `(306, 1294)`

#### Scenario: All keys released
- **WHEN** the last held joystick key is released
- **THEN** the engine SHALL emit touch-up, releasing the virtual stick

#### Scenario: Direction change while held
- **WHEN** key `W` is held, then key `D` is pressed (W still held)
- **THEN** the engine SHALL emit touch-move from the up position to the up-right diagonal position

### Requirement: Aim binding translates mouse movement to touch drag
The aim binding SHALL capture mouse movement within the defined `region` and translate it into touch-move events. Mouse movement SHALL be scaled by the `sensitivity` factor. The touch drag SHALL be active only while a designated activation method is engaged (e.g., right mouse button held or pointer captured).

#### Scenario: Mouse movement within aim region
- **WHEN** the aim binding is active with `sensitivity = 1.5` and `region = [540, 960, 1080, 1920]`, and the mouse moves by `(dx=10, dy=20)`
- **THEN** the engine SHALL emit a touch-move event with delta `(15, 30)` (scaled by sensitivity) within the aim region

#### Scenario: Mouse movement clamped to region bounds
- **WHEN** the accumulated aim position would exceed the region boundary
- **THEN** the engine SHALL clamp the touch position to the region bounds and wrap or reset the accumulator to allow continued movement

#### Scenario: Aim binding deactivated
- **WHEN** the aim activation method is disengaged (e.g., right mouse button released)
- **THEN** the engine SHALL emit touch-up and stop translating mouse movement

### Requirement: Multi-touch slot allocation
The keymap engine SHALL manage Android multi-touch slots. Each active binding that produces touch events SHALL be assigned a unique slot ID. Slots SHALL be allocated on touch-down and released on touch-up. The engine SHALL support at least 10 concurrent touch slots.

#### Scenario: Concurrent tap and joystick
- **WHEN** a joystick binding is active on slot 0 and a tap key is pressed
- **THEN** the tap binding SHALL be assigned slot 1, and both bindings SHALL produce independent touch streams

#### Scenario: Slot exhaustion
- **WHEN** all 10 touch slots are in use and a new binding activation occurs
- **THEN** the engine SHALL ignore the new binding activation and log a warning

#### Scenario: Slot reuse after release
- **WHEN** a binding on slot 3 emits touch-up, then a new binding is activated
- **THEN** the new binding SHALL be assigned slot 3 (or any available released slot)

### Requirement: Unbound keys pass through
Input events for keys that have no binding in the active keymap SHALL be passed through to the VM untranslated.

#### Scenario: Unbound key press
- **WHEN** key `P` is pressed and no binding exists for `P` in the active keymap
- **THEN** the engine SHALL pass the key event through without generating any touch events
