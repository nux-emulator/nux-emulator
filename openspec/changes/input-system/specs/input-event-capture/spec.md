## ADDED Requirements

### Requirement: Capture keyboard events from GTK4 surface
The system SHALL capture key press and key release events from the GTK4 drawing surface using `EventControllerKey`. Each event SHALL include the hardware keycode, the pressed/released state, and any active modifier flags (Shift, Ctrl, Alt, Super).

#### Scenario: Key press detected
- **WHEN** the user presses a key while the emulator window has focus
- **THEN** the system captures the event with keycode, press state, and modifier flags

#### Scenario: Key release detected
- **WHEN** the user releases a previously pressed key
- **THEN** the system captures the event with keycode and release state

#### Scenario: Window not focused
- **WHEN** the emulator window does not have keyboard focus
- **THEN** the system SHALL NOT capture or forward any keyboard events

### Requirement: Capture mouse motion events from GTK4 surface
The system SHALL capture pointer motion events from the GTK4 drawing surface using `EventControllerMotion`. Each event SHALL include the x and y coordinates relative to the surface.

#### Scenario: Mouse moved over surface
- **WHEN** the user moves the mouse pointer over the emulator drawing surface
- **THEN** the system captures a motion event with surface-relative x and y coordinates

### Requirement: Capture mouse click events from GTK4 surface
The system SHALL capture mouse button press and release events using `GestureClick`. The system SHALL distinguish left-click (button 1) and right-click (button 3).

#### Scenario: Left click
- **WHEN** the user presses and releases the left mouse button on the drawing surface
- **THEN** the system captures both press and release events identified as button 1

#### Scenario: Right click
- **WHEN** the user presses and releases the right mouse button on the drawing surface
- **THEN** the system captures both press and release events identified as button 3

### Requirement: Capture scroll events from GTK4 surface
The system SHALL capture scroll wheel events from the GTK4 drawing surface using `EventControllerScroll`. Each event SHALL include the scroll direction and delta.

#### Scenario: Vertical scroll
- **WHEN** the user scrolls the mouse wheel vertically over the drawing surface
- **THEN** the system captures a scroll event with vertical delta value

#### Scenario: Horizontal scroll
- **WHEN** the user scrolls horizontally (e.g., tilt wheel or touchpad gesture)
- **THEN** the system captures a scroll event with horizontal delta value
