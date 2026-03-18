## ADDED Requirements

### Requirement: Synthesize two-finger touch from keyboard+mouse combo
The system SHALL synthesize a two-finger multi-touch gesture when the user holds Ctrl and clicks the left mouse button. The first touch point SHALL be placed at the click location, and the second touch point SHALL be placed at a position mirrored across the center of the Android screen.

#### Scenario: Ctrl+click initiates pinch gesture
- **WHEN** the user holds Ctrl and presses the left mouse button at position (x, y)
- **THEN** the system emits multi-touch events for two slots: slot 0 at (x, y) and slot 1 at the mirrored position (guest_width - x, guest_height - y)

#### Scenario: Ctrl+drag moves both touch points
- **WHEN** the user holds Ctrl and drags the mouse after initiating a pinch gesture
- **THEN** both touch points move symmetrically — slot 0 follows the mouse, slot 1 mirrors around the screen center — producing a pinch-zoom effect

#### Scenario: Ctrl release ends pinch gesture
- **WHEN** the user releases Ctrl or the mouse button during a pinch gesture
- **THEN** the system emits touch-up events for both slots, ending the multi-touch gesture

### Requirement: Use multi-touch type-B slot protocol
The system SHALL use the Linux multi-touch type-B (slot-based) protocol with `ABS_MT_SLOT`, `ABS_MT_TRACKING_ID`, `ABS_MT_POSITION_X`, and `ABS_MT_POSITION_Y` events. The system SHALL support exactly 2 slots (slot 0 and slot 1).

#### Scenario: Slot assignment for single touch
- **WHEN** a normal (non-Ctrl) left click occurs
- **THEN** the system uses slot 0 with a valid `ABS_MT_TRACKING_ID` for the single touch point

#### Scenario: Slot assignment for dual touch
- **WHEN** a Ctrl+click pinch gesture is active
- **THEN** the system uses slot 0 and slot 1, each with a unique `ABS_MT_TRACKING_ID`

#### Scenario: Touch release clears tracking ID
- **WHEN** a touch point is released
- **THEN** the system emits `ABS_MT_TRACKING_ID` with value -1 for that slot to signal lift-off
