## 1. Project Setup

- [x] 1.1 Add `serde`, `toml`, and `dirs` dependencies to nux-core's Cargo.toml
- [x] 1.2 Create the `nux-core/src/config/` module directory with `mod.rs` and re-export from `nux-core/src/lib.rs`

## 2. Config Schema Structs

- [x] 2.1 Define enum types: `GpuBackend`, `RootMode`, `GoogleServicesProvider`, `NetworkMode` with serde rename_all kebab-case serialization
- [x] 2.2 Define section structs: `InstanceMeta`, `HardwareConfig`, `DisplayConfig`, `GpuConfig`, `RootConfig`, `GoogleServicesConfig`, `NetworkConfig`, `DeviceConfig` with serde Serialize/Deserialize derives
- [x] 2.3 Define top-level `NuxConfig` struct with `schema_version: u32` and all section fields, with `#[serde(default)]` on each
- [x] 2.4 Implement `Default` for all structs and enums with the values specified in the config-resolution spec (cpu_cores=2, ram_mb=2048, etc.)
- [x] 2.5 Write unit tests: round-trip serialize/deserialize, deserialize complete TOML, reject unknown enum variants

## 3. Config Path Resolution

- [x] 3.1 Implement `config_paths` module with `global_config_path()` and `instance_config_path(name: &str)` using the `dirs` crate, respecting XDG overrides
- [x] 3.2 Write unit tests: verify default paths match `~/.config/nux/config.toml` and `~/.local/share/nux/instances/<name>/config.toml`, verify custom XDG env vars are respected

## 4. Config Loading and Merge

- [x] 4.1 Define `InstanceConfigOverlay` — a mirror of `NuxConfig` where every field is `Option<T>` — for partial instance configs
- [x] 4.2 Implement `merge(global: NuxConfig, overlay: InstanceConfigOverlay) -> NuxConfig` that applies present fields from overlay onto global
- [x] 4.3 Implement `Config::load(instance_name: &str) -> Result<NuxConfig>` that reads global config, reads instance config, merges, and returns the resolved config. Return defaults when files are missing.
- [x] 4.4 Write unit tests: merge with empty overlay, merge with partial overlay, merge with full overlay, load when no files exist returns defaults

## 5. Config Validation

- [x] 5.1 Define `ConfigError` enum and implement `Config::validate(&self) -> Vec<ConfigError>` with rules: cpu_cores >= 1, ram_mb >= 512, display dimensions >= 1, dpi >= 1
- [x] 5.2 Write unit tests: valid config returns empty vec, each invalid field produces its own error, multiple errors returned together

## 6. Config Save

- [x] 6.1 Implement `Config::save(config: &NuxConfig, path: &Path) -> Result<()>` that creates parent directories and writes TOML
- [x] 6.2 Write unit tests: save to new path creates directories, save overwrites existing file, saved file round-trips back to identical struct

## 7. Schema Migration

- [x] 7.1 Define `CURRENT_SCHEMA_VERSION` constant and migration registry: `Vec<fn(toml::Value) -> Result<toml::Value>>`
- [x] 7.2 Implement `migrate(raw: toml::Value) -> Result<toml::Value>` that reads `schema_version`, rejects future versions, treats missing version as 1, and applies sequential migrations
- [x] 7.3 Integrate migration into the load path — run `migrate()` on raw TOML before deserializing into typed structs
- [x] 7.4 Write unit tests: current version skips migration, missing version treated as v1, future version rejected, multi-step migration applies in order, failed migration preserves original file

## 8. Integration and Verification

- [x] 8.1 Write an integration test: create temp dirs, write global + instance TOML files, load/merge/validate/save round-trip
- [x] 8.2 Verify `cargo test` passes for all config module tests, `cargo clippy` clean, no warnings
