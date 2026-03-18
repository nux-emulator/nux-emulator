## ADDED Requirements

### Requirement: Config struct hierarchy
The system SHALL define a top-level `NuxConfig` struct containing typed sub-structs for each configuration domain: `InstanceMeta`, `HardwareConfig`, `DisplayConfig`, `GpuConfig`, `RootConfig`, `GoogleServicesConfig`, `NetworkConfig`, and `DeviceConfig`. All structs SHALL derive `serde::Serialize` and `serde::Deserialize`.

#### Scenario: Deserialize a complete config file
- **WHEN** a valid TOML file containing all sections is loaded
- **THEN** the system produces a fully populated `NuxConfig` with all sub-structs filled

#### Scenario: Serialize config back to TOML
- **WHEN** a `NuxConfig` value is serialized
- **THEN** the output is valid TOML that round-trips back to an identical struct

### Requirement: Hardware configuration fields
The `HardwareConfig` struct SHALL contain `cpu_cores: u32` and `ram_mb: u32` fields representing allocated CPU cores and RAM in megabytes.

#### Scenario: Parse hardware section
- **WHEN** the TOML contains `[hardware]` with `cpu_cores = 4` and `ram_mb = 4096`
- **THEN** the deserialized `HardwareConfig` has `cpu_cores == 4` and `ram_mb == 4096`

### Requirement: Display configuration fields
The `DisplayConfig` struct SHALL contain `width: u32`, `height: u32`, and `dpi: u32` fields.

#### Scenario: Parse display section
- **WHEN** the TOML contains `[display]` with `width = 1920`, `height = 1080`, `dpi = 240`
- **THEN** the deserialized `DisplayConfig` has matching values

### Requirement: GPU configuration fields
The `GpuConfig` struct SHALL contain a `backend` field as a string enum supporting at least `"virglrenderer"` and `"gfxstream"`.

#### Scenario: Parse GPU backend
- **WHEN** the TOML contains `[gpu]` with `backend = "gfxstream"`
- **THEN** the deserialized `GpuConfig` has `backend == GpuBackend::Gfxstream`

#### Scenario: Reject unknown GPU backend
- **WHEN** the TOML contains `[gpu]` with `backend = "invalid"`
- **THEN** deserialization returns an error indicating an unknown GPU backend variant

### Requirement: Root configuration fields
The `RootConfig` struct SHALL contain a `mode` field as a string enum supporting `"none"`, `"magisk"`, `"kernelsu"`, and `"apatch"`.

#### Scenario: Parse root mode
- **WHEN** the TOML contains `[root]` with `mode = "magisk"`
- **THEN** the deserialized `RootConfig` has `mode == RootMode::Magisk`

### Requirement: Google Services configuration fields
The `GoogleServicesConfig` struct SHALL contain a `provider` field as a string enum supporting `"none"`, `"microg"`, and `"gapps"`.

#### Scenario: Parse google services provider
- **WHEN** the TOML contains `[google_services]` with `provider = "microg"`
- **THEN** the deserialized `GoogleServicesConfig` has `provider == GoogleServicesProvider::MicroG`

### Requirement: Network configuration fields
The `NetworkConfig` struct SHALL contain a `mode` field as a string enum supporting at least `"nat"` and `"bridged"`.

#### Scenario: Parse network mode
- **WHEN** the TOML contains `[network]` with `mode = "nat"`
- **THEN** the deserialized `NetworkConfig` has `mode == NetworkMode::Nat`

### Requirement: Device identity configuration fields
The `DeviceConfig` struct SHALL contain `model: String` and `manufacturer: String` fields.

#### Scenario: Parse device identity
- **WHEN** the TOML contains `[device]` with `model = "Pixel 9"` and `manufacturer = "Google"`
- **THEN** the deserialized `DeviceConfig` has matching string values

### Requirement: Instance metadata fields
The `InstanceMeta` struct SHALL contain a `name: String` field identifying the instance.

#### Scenario: Parse instance name
- **WHEN** the TOML contains `[instance]` with `name = "default"`
- **THEN** the deserialized `InstanceMeta` has `name == "default"`

### Requirement: Schema version field
The `NuxConfig` SHALL contain a `schema_version: u32` field at the top level of the TOML file.

#### Scenario: Parse schema version
- **WHEN** the TOML contains `schema_version = 1`
- **THEN** the deserialized `NuxConfig` has `schema_version == 1`
