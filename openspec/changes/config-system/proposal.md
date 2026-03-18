## Why

Nux needs a structured configuration system to persist instance settings (hardware, display, GPU, network, device identity) and global preferences. Without this, every launch would require manual parameterization, and there's no foundation for the UI settings panel or future multi-instance support. The config module is a prerequisite for nearly every other subsystem — VM launch reads hardware config, display pipeline reads resolution/DPI, GPU backend selection reads gpu config, etc.

## What Changes

- Introduce `nux-core::config` module providing typed Rust structs for all configuration domains
- Define TOML-based config file format with per-instance and global scopes
- Per-instance config at `~/.local/share/nux/instances/<name>/config.toml`
- Global config (defaults, app preferences) at `~/.config/nux/config.toml`
- XDG Base Directory compliance (`XDG_CONFIG_HOME`, `XDG_DATA_HOME` overrides)
- Config validation with meaningful error reporting on load
- Sensible defaults for all fields so a minimal/empty config is valid
- Schema versioning and migration support for forward-compatible upgrades
- Merge semantics: instance config inherits from global defaults, with per-field override

## Non-goals

- UI for editing configuration — that belongs to `gtk-ui-shell`
- VM lifecycle management — config is read-only data consumed by the VM launcher
- Actual hardware allocation or capability detection
- Multi-instance orchestration (v2 scope)
- Config file encryption or access control

## Capabilities

### New Capabilities
- `config-schema`: Typed Rust structs and TOML serialization covering all config domains (hardware, display, gpu, root, google-services, network, device)
- `config-resolution`: Loading, merging (global defaults + instance overrides), validation, and default generation
- `config-migration`: Schema versioning and forward migration across Nux releases

### Modified Capabilities
<!-- No existing specs to modify — this is a new module -->

## Impact

- **Code**: New `nux-core::config` module with public API consumed by VM launcher, display pipeline, GPU backend selector, and eventually the UI settings panel
- **Dependencies**: `serde`, `toml` (serialize/deserialize), `dirs` or `xdg` crate (XDG paths)
- **Filesystem**: Creates config directories and files under XDG-compliant paths on first run
- **APIs**: Exposes `Config::load()`, `Config::save()`, `Config::validate()`, and per-section accessors as the public contract for the rest of nux-core and nux-ui
