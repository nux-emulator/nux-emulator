## ADDED Requirements

### Requirement: Define evdev InputEvent struct in pure Rust
The system SHALL define a `#[repr(C)]` `InputEvent` struct matching the Linux `input_event` layout (timeval, u16 type, u16 code, i32 value). The struct SHALL be serializable to raw bytes for socket transmission without any external crate dependency.

#### Scenario: Struct layout matches Linux ABI
- **WHEN** an `InputEvent` is serialized to bytes
- **THEN** the byte layout SHALL match the Linux kernel's `struct input_event` (16 bytes on x86_64)

### Requirement: Translate keyboard events to evdev key events
The system SHALL translate GTK4 key press events to `EV_KEY` evdev events with value 1 (press) and key release events to `EV_KEY` with value 0 (release). Each key event batch SHALL be terminated by an `EV_SYN/SYN_REPORT` event.

#### Scenario: Key press translation
- **WHEN** a GTK4 key press event is received with a hardware keycode
- **THEN** the system emits an `EV_KEY` event with the corresponding Linux keycode and value 1, followed by `EV_SYN/SYN_REPORT`

#### Scenario: Key release translation
- **WHEN** a GTK4 key release event is received
- **THEN** the system emits an `EV_KEY` event with value 0, followed by `EV_SYN/SYN_REPORT`

### Requirement: Translate mouse motion to absolute pointer events
The system SHALL translate mouse motion events to `EV_ABS` events using `ABS_X` and `ABS_Y` with coordinates mapped to the Android screen space. Each motion event SHALL be terminated by `EV_SYN/SYN_REPORT`.

#### Scenario: Mouse motion translation
- **WHEN** a mouse motion event is received with host coordinates (x, y)
- **THEN** the system emits `EV_ABS/ABS_X` and `EV_ABS/ABS_Y` events with Android-mapped coordinates, followed by `EV_SYN/SYN_REPORT`

### Requirement: Translate left-click to Android touch events
The system SHALL translate left mouse button press to `EV_KEY/BTN_TOUCH` value 1 (touch down) and left mouse button release to `BTN_TOUCH` value 0 (touch up).

#### Scenario: Left click to touch
- **WHEN** a left mouse button press is received
- **THEN** the system emits `EV_KEY/BTN_TOUCH` with value 1 and current absolute position, followed by `EV_SYN/SYN_REPORT`

#### Scenario: Left release to touch up
- **WHEN** a left mouse button release is received
- **THEN** the system emits `EV_KEY/BTN_TOUCH` with value 0, followed by `EV_SYN/SYN_REPORT`

### Requirement: Translate right-click to Android back button
The system SHALL translate right mouse button press to an `EV_KEY` event with `KEY_BACK` keycode.

#### Scenario: Right click to back
- **WHEN** a right mouse button press is received
- **THEN** the system emits `EV_KEY/KEY_BACK` with value 1, then `EV_SYN/SYN_REPORT`, then `EV_KEY/KEY_BACK` with value 0, then `EV_SYN/SYN_REPORT`

### Requirement: Translate scroll to Android scroll events
The system SHALL translate mouse scroll events to `EV_REL/REL_WHEEL` (vertical) and `EV_REL/REL_HWHEEL` (horizontal) evdev events.

#### Scenario: Vertical scroll translation
- **WHEN** a vertical scroll event with delta is received
- **THEN** the system emits `EV_REL/REL_WHEEL` with the scroll delta, followed by `EV_SYN/SYN_REPORT`

#### Scenario: Horizontal scroll translation
- **WHEN** a horizontal scroll event with delta is received
- **THEN** the system emits `EV_REL/REL_HWHEEL` with the scroll delta, followed by `EV_SYN/SYN_REPORT`

### Requirement: Inject events via crosvm virtio-input socket
The system SHALL write serialized evdev event batches to the crosvm virtio-input Unix socket. The socket path SHALL be provided by the crosvm integration layer at VM startup.

#### Scenario: Successful event injection
- **WHEN** an evdev event batch is ready for injection
- **THEN** the system writes the raw bytes to the virtio-input Unix socket

#### Scenario: Socket write failure
- **WHEN** a write to the virtio-input socket fails
- **THEN** the system SHALL log the error and drop the event without crashing
