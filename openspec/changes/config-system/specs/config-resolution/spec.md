## ADDED Requirements

### Requirement: XDG-compliant global config path
The system SHALL resolve the global config file path as `$XDG_CONFIG_HOME/nux/config.toml`, falling back to `~/.config/nux/config.toml` when `XDG_CONFIG_HOME` is unset.

#### Scenario: Default global config path
- **WHEN** `XDG_CONFIG_HOME` is not set
- **THEN** the global config path resolves to `~/.config/nux/config.toml`

#### Scenario: Custom XDG_CONFIG_HOME
- **WHEN** `XDG_CONFIG_HOME` is set to `/tmp/myconfig`
- **THEN** the global config path resolves to `/tmp/myconfig/nux/config.toml`

### Requirement: XDG-compliant instance config path
The system SHALL resolve instance config file paths as `$XDG_DATA_HOME/nux/instances/<name>/config.toml`, falling back to `~/.local/share/nux/instances/<name>/config.toml` when `XDG_DATA_HOME` is unset.

#### Scenario: Default instance config path
- **WHEN** `XDG_DATA_HOME` is not set and instance name is `"default"`
- **THEN** the instance config path resolves to `~/.local/share/nux/instances/default/config.toml`

#### Scenario: Custom XDG_DATA_HOME
- **WHEN** `XDG_DATA_HOME` is set to `/tmp/mydata` and instance name is `"gaming"`
- **THEN** the instance config path resolves to `/tmp/mydata/nux/instances/gaming/config.toml`

### Requirement: Sensible defaults for all fields
The system SHALL provide default values for every configuration field so that an empty or minimal TOML file produces a valid, usable `NuxConfig`. Defaults SHALL include at minimum: `cpu_cores = 2`, `ram_mb = 2048`, `width = 1080`, `height = 1920`, `dpi = 320`, `backend = "gfxstream"`, `root.mode = "none"`, `google_services.provider = "microg"`, `network.mode = "nat"`.

#### Scenario: Load empty config file
- **WHEN** a config file exists but contains only `schema_version = 1`
- **THEN** all fields are populated with their default values

#### Scenario: Load missing config file
- **WHEN** no config file exists at the expected path
- **THEN** the system returns a `NuxConfig` populated entirely with defaults

### Requirement: Global-to-instance merge
The system SHALL load the global config first, then overlay instance-specific config on top. Instance fields that are present SHALL override the corresponding global values. Instance fields that are absent SHALL inherit the global value.

#### Scenario: Instance overrides a single field
- **WHEN** global config has `ram_mb = 2048` and instance config has `ram_mb = 8192`
- **THEN** the resolved config has `ram_mb == 8192` and all other fields match global

#### Scenario: Instance config is empty
- **WHEN** global config has `cpu_cores = 4` and instance config is empty
- **THEN** the resolved config has `cpu_cores == 4`

#### Scenario: Instance overrides multiple sections
- **WHEN** instance config sets `[hardware]` and `[display]` fields
- **THEN** both sections reflect instance values while remaining sections inherit from global

### Requirement: Config validation
The system SHALL validate the resolved config and return a list of all validation errors. Validation rules SHALL include: `cpu_cores >= 1`, `ram_mb >= 512`, `display.width >= 1`, `display.height >= 1`, `display.dpi >= 1`.

#### Scenario: Valid config passes validation
- **WHEN** a resolved config has `cpu_cores = 4`, `ram_mb = 4096`, `width = 1080`, `height = 1920`, `dpi = 320`
- **THEN** validation returns an empty error list

#### Scenario: Invalid RAM rejected
- **WHEN** a resolved config has `ram_mb = 128`
- **THEN** validation returns an error indicating RAM must be at least 512 MB

#### Scenario: Multiple validation errors
- **WHEN** a resolved config has `cpu_cores = 0` and `ram_mb = 0`
- **THEN** validation returns errors for both fields (not just the first)

### Requirement: Config save
The system SHALL support saving a `NuxConfig` to a specified file path, creating parent directories if they do not exist. The output SHALL be valid TOML.

#### Scenario: Save creates parent directories
- **WHEN** saving config to a path whose parent directory does not exist
- **THEN** the system creates the directory tree and writes the file

#### Scenario: Save overwrites existing file
- **WHEN** saving config to a path where a config file already exists
- **THEN** the existing file is replaced with the new content
