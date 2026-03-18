## ADDED Requirements

### Requirement: Frame texture display
The system SHALL present captured frames in the GTK4 window as `GdkTexture` objects. For the dmabuf path, frames SHALL be imported via `GdkDmabufTextureBuilder`. For the shared memory fallback, frames SHALL be uploaded as GL textures in a `GtkGLArea`.

#### Scenario: Dmabuf frame displayed as GdkTexture
- **WHEN** a dmabuf frame is received from the capture layer
- **THEN** the system SHALL create a `GdkTexture` via `GdkDmabufTextureBuilder` and display it in the presentation widget with no CPU-side pixel copy

#### Scenario: Shared memory frame displayed via GL upload
- **WHEN** a shared memory frame is received from the capture layer and the dmabuf path is not active
- **THEN** the system SHALL upload the frame data to a GL texture within a `GtkGLArea` and render it to screen

### Requirement: Aspect ratio preservation
The system SHALL preserve the Android guest's display aspect ratio at all window sizes. The presentation widget SHALL letterbox or pillarbox as needed, filling unused space with black.

#### Scenario: Window wider than guest aspect ratio
- **WHEN** the GTK window is resized to a wider aspect ratio than the guest display (e.g., guest is 16:9, window is 21:9)
- **THEN** the system SHALL pillarbox the frame with black bars on the left and right, maintaining the guest's aspect ratio

#### Scenario: Window taller than guest aspect ratio
- **WHEN** the GTK window is resized to a taller aspect ratio than the guest display
- **THEN** the system SHALL letterbox the frame with black bars on the top and bottom, maintaining the guest's aspect ratio

#### Scenario: Window matches guest aspect ratio
- **WHEN** the GTK window aspect ratio matches the guest display
- **THEN** the system SHALL scale the frame to fill the entire widget area with no bars

### Requirement: Scaling modes
The system SHALL support at least two scaling modes configurable via `DisplayConfig`: `contain` (scale to fit, preserve aspect ratio) and `integer` (scale to the largest integer multiple that fits). The default SHALL be `contain`.

#### Scenario: Contain scaling mode
- **WHEN** the scaling mode is set to `contain`
- **THEN** the system SHALL scale the frame to the largest size that fits within the widget while preserving aspect ratio

#### Scenario: Integer scaling mode
- **WHEN** the scaling mode is set to `integer`
- **THEN** the system SHALL scale the frame to the largest integer multiple of the native resolution that fits within the widget, centering the result

#### Scenario: Integer scaling with small window
- **WHEN** the scaling mode is `integer` and the widget is smaller than the native guest resolution
- **THEN** the system SHALL fall back to `contain` scaling for that frame to ensure the content remains visible

### Requirement: Resolution change handling
The system SHALL handle resolution changes from the Android guest without restarting the display pipeline. When the guest resolution changes, the presentation widget SHALL adapt its scaling and aspect ratio calculations to the new resolution.

#### Scenario: Guest resolution increases
- **WHEN** the Android guest changes its display resolution from 720p to 1080p
- **THEN** the system SHALL detect the new resolution from the incoming frame dimensions and update scaling calculations accordingly without dropping frames

#### Scenario: Guest resolution decreases
- **WHEN** the Android guest changes its display resolution to a smaller value
- **THEN** the system SHALL adapt to the new resolution and continue presenting frames with correct aspect ratio
