## ADDED Requirements

### Requirement: Dmabuf frame capture
The system SHALL capture rendered frames from crosvm by importing dmabuf file descriptors exported by gfxstream's surfaceless display mode. The captured dmabufs SHALL be wrapped in a safe Rust abstraction that owns the file descriptor and closes it on drop.

#### Scenario: Successful dmabuf capture on supported hardware
- **WHEN** crosvm is running with gfxstream in surfaceless mode and the host GPU driver supports dmabuf export
- **THEN** the system SHALL import each frame as a dmabuf FD and deliver it to the frame presentation layer via a `tokio::sync::watch` channel

#### Scenario: Dmabuf capture with multiple frames in flight
- **WHEN** crosvm produces a new frame before the previous frame has been presented
- **THEN** the system SHALL replace the previous frame in the watch channel, dropping the stale frame and its associated dmabuf FD

### Requirement: Shared memory fallback capture
The system SHALL provide a shared memory fallback capture path for systems where dmabuf import is not available. The fallback SHALL map crosvm's shared memory rendering region via `mmap` and deliver frame data to the presentation layer.

#### Scenario: Automatic fallback when dmabuf is unavailable
- **WHEN** the system attempts dmabuf capture at startup and it fails (unsupported driver, missing kernel support, or import error)
- **THEN** the system SHALL automatically fall back to shared memory capture and log a warning indicating the active capture path

#### Scenario: Shared memory frame delivery
- **WHEN** crosvm renders a frame to the shared memory region
- **THEN** the system SHALL read the frame data and deliver it to the presentation layer via the same watch channel interface used by the dmabuf path

### Requirement: Capture path auto-detection
The system SHALL auto-detect the best available capture path at startup by attempting dmabuf import first, then falling back to shared memory. The active capture path SHALL be queryable at runtime.

#### Scenario: Detection on system with full dmabuf support
- **WHEN** the display pipeline initializes on a system with GPU dmabuf export support and GTK 4.14+
- **THEN** the system SHALL select the dmabuf capture path and report it as active

#### Scenario: Detection on system without dmabuf support
- **WHEN** the display pipeline initializes on a system where dmabuf import fails
- **THEN** the system SHALL select the shared memory capture path and report it as active

#### Scenario: Capture path logged at startup
- **WHEN** the display pipeline completes capture path selection
- **THEN** the system SHALL log the selected path (dmabuf or shared memory) at info level

### Requirement: Display configuration
The system SHALL read display settings from the `[display]` section of the Nux TOML configuration file. The configuration SHALL include resolution, scaling mode, vsync toggle, and FPS overlay toggle with sensible defaults.

#### Scenario: Valid display configuration
- **WHEN** the Nux config contains a `[display]` section with valid fields
- **THEN** the system SHALL parse it into a typed `DisplayConfig` struct and use the values for pipeline initialization

#### Scenario: Missing display configuration section
- **WHEN** the Nux config does not contain a `[display]` section
- **THEN** the system SHALL use default values: 1080p resolution, `contain` scaling mode, vsync enabled, FPS overlay disabled

#### Scenario: Invalid display configuration values
- **WHEN** the `[display]` section contains invalid values (e.g., zero resolution, unknown scaling mode)
- **THEN** the system SHALL return a `DisplayError` at startup with a descriptive message identifying the invalid field
