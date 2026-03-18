## ADDED Requirements

### Requirement: Maintain display metrics for coordinate transformation
The system SHALL maintain a `DisplayMetrics` struct containing: guest screen width, guest screen height, host surface width, host surface height, scale factor, and letterbox offsets (horizontal and vertical). This struct SHALL be updated whenever the host window is resized or the guest resolution changes.

#### Scenario: Window resize updates metrics
- **WHEN** the host emulator window is resized
- **THEN** the system recalculates and updates scale factor and letterbox offsets in `DisplayMetrics`

#### Scenario: Guest resolution change updates metrics
- **WHEN** the Android guest changes its display resolution
- **THEN** the system updates guest width and height in `DisplayMetrics` and recalculates derived values

### Requirement: Map host coordinates to Android screen coordinates
The system SHALL transform host surface coordinates to Android screen coordinates by: subtracting letterbox offsets, dividing by the scale factor, and clamping to the guest resolution bounds. Coordinates within the letterbox region (outside the rendered area) SHALL be clamped to the nearest edge of the Android screen.

#### Scenario: Coordinate mapping with uniform scaling
- **WHEN** a host coordinate (x, y) is received and the display is uniformly scaled with no letterboxing
- **THEN** the system divides by the scale factor to produce Android coordinates

#### Scenario: Coordinate mapping with letterboxing
- **WHEN** a host coordinate (x, y) falls within the letterbox offset region
- **THEN** the system clamps the coordinate to the nearest edge of the Android screen (0 or max)

#### Scenario: Coordinate mapping with combined scaling and letterbox
- **WHEN** a host coordinate (x, y) is received with both scaling and letterbox offsets active
- **THEN** the system subtracts the letterbox offset, divides by scale factor, and clamps to guest resolution bounds

### Requirement: Thread-safe access to display metrics
The system SHALL provide thread-safe read/write access to `DisplayMetrics` via `Arc<Mutex<DisplayMetrics>>` so that the display pipeline can update metrics and the input system can read them concurrently without data races.

#### Scenario: Concurrent read during resize
- **WHEN** the display pipeline updates `DisplayMetrics` while the input system reads coordinates
- **THEN** the input system reads either the old or new metrics atomically, never a torn/partial state
