## ADDED Requirements

### Requirement: Map VM configuration to crosvm arguments
The system SHALL construct a complete crosvm CLI invocation from a `VmConfig` struct that is deserialized from the `[vm]` section of the Nux TOML configuration file.

#### Scenario: Minimal valid configuration
- **WHEN** the config specifies CPU count, RAM size, a system image path, and a kernel path
- **THEN** the system SHALL produce arguments: `crosvm run --cpus <N> --mem <MB> --block path=<system.img>,ro --boot <boot.img> <kernel>`

#### Scenario: Full configuration with all options
- **WHEN** the config specifies CPU, RAM, GPU backend with resolution, system image, userdata image, audio, networking, input devices, control socket path, and kernel
- **THEN** the system SHALL produce arguments including all corresponding crosvm flags: `--gpu`, `--block` (for each disk), `--sound`, `--net`, `--input-ev`, `--socket`, `--boot`, and the kernel path

### Requirement: Validate configuration before command construction
The system SHALL validate `VmConfig` fields before constructing the crosvm command and reject invalid configurations with descriptive errors.

#### Scenario: CPU count is zero
- **WHEN** the config specifies `cpus = 0`
- **THEN** the system SHALL return a validation error indicating CPU count must be at least 1

#### Scenario: RAM below minimum
- **WHEN** the config specifies `ram_mb` less than 512
- **THEN** the system SHALL return a validation error indicating minimum RAM is 512 MB

#### Scenario: System image path does not exist
- **WHEN** the config specifies a system image path that does not exist on disk
- **THEN** the system SHALL return a validation error indicating the system image file was not found

#### Scenario: Kernel path does not exist
- **WHEN** the config specifies a kernel path that does not exist on disk
- **THEN** the system SHALL return a validation error indicating the kernel file was not found

### Requirement: GPU argument construction
The system SHALL construct the `--gpu` argument with the `backend=gfxstream` parameter and optional `width`, `height` parameters from config.

#### Scenario: GPU enabled with custom resolution
- **WHEN** the config specifies `gpu.enabled = true`, `gpu.width = 1920`, `gpu.height = 1080`
- **THEN** the system SHALL produce `--gpu backend=gfxstream,width=1920,height=1080`

#### Scenario: GPU enabled with default resolution
- **WHEN** the config specifies `gpu.enabled = true` without width/height
- **THEN** the system SHALL produce `--gpu backend=gfxstream` without width/height parameters, letting crosvm use its defaults

#### Scenario: GPU disabled
- **WHEN** the config specifies `gpu.enabled = false`
- **THEN** the system SHALL omit the `--gpu` argument entirely

### Requirement: Block device argument construction
The system SHALL construct `--block` arguments for each disk image, with the `ro` flag for read-only images.

#### Scenario: System image as read-only
- **WHEN** the config specifies a system image with `readonly = true`
- **THEN** the system SHALL produce `--block path=<system.img>,ro`

#### Scenario: Userdata image as read-write
- **WHEN** the config specifies a userdata image with `readonly = false` or unset
- **THEN** the system SHALL produce `--block path=<userdata.img>` without the `ro` flag

### Requirement: Control socket path argument
The system SHALL include the `--socket` argument with the configured control socket path.

#### Scenario: Custom socket path
- **WHEN** the config specifies `control_socket = "/run/user/1000/nux/control.sock"`
- **THEN** the system SHALL produce `--socket /run/user/1000/nux/control.sock`

#### Scenario: Default socket path
- **WHEN** the config does not specify a control socket path
- **THEN** the system SHALL use the default path `/run/user/<uid>/nux/control.sock` where `<uid>` is the current user's UID
