## Context

Nux currently has no persistent configuration. Every subsystem (VM launcher, display pipeline, GPU backend) needs a reliable way to read user preferences and instance-specific settings. The config module sits at the bottom of the dependency graph — it's pure data with no runtime side effects, making it a clean foundation to build on.

Constraints:
- Rust-only, no FFI
- TOML format (already decided at project level)
- Must work on both Wayland and X11 (paths are filesystem-level, no display dependency)
- Single instance for v1, but schema should not preclude multi-instance

## Goals / Non-Goals

**Goals:**
- Provide a single, typed API for all config access across nux-core and nux-ui
- XDG-compliant file paths with environment variable overrides
- Two-tier merge: global defaults → instance overrides
- Validate config on load with actionable error messages
- Support schema versioning so future Nux releases can migrate configs forward

**Non-Goals:**
- Live config reloading / file watching (not needed for v1)
- Config UI (gtk-ui-shell scope)
- Encrypting or securing config files
- Remote/cloud config sync

## Decisions

### 1. Serde + toml crate for serialization

**Choice**: Use `serde` derive macros with the `toml` crate.
**Rationale**: This is the idiomatic Rust approach. Serde's `#[serde(default)]` gives us free default-value handling. The `toml` crate is mature and well-maintained.
**Alternatives considered**: `config-rs` crate (adds unnecessary abstraction and runtime type coercion — we want compile-time typed structs), hand-rolled parser (no reason to).

### 2. `dirs` crate for XDG paths (not `xdg`)

**Choice**: Use the `dirs` crate for resolving `config_dir()` and `data_dir()`.
**Rationale**: `dirs` is lighter, widely used, and covers our needs (we just need two base paths). The `xdg` crate is more featureful but heavier than necessary.
**Alternatives considered**: `xdg` crate (overkill), manual `$XDG_CONFIG_HOME` parsing (reinventing the wheel).

### 3. Flat struct hierarchy with nested sections

```
NuxConfig
├── schema_version: u32
├── instance: InstanceMeta { name }
├── hardware: HardwareConfig { cpu_cores, ram_mb }
├── display: DisplayConfig { width, height, dpi }
├── gpu: GpuConfig { backend }
├── root: RootConfig { mode }
├── google_services: GoogleServicesConfig { provider }
├── network: NetworkConfig { mode }
└── device: DeviceConfig { model, manufacturer }
```

**Rationale**: Each section maps 1:1 to a TOML table. Subsystems receive only their relevant sub-struct (e.g., VM launcher gets `&HardwareConfig`), enforcing minimal coupling.

### 4. Two-file merge strategy

**Choice**: Load global config first, then overlay instance config on top using a field-by-field merge where `Option::None` in the instance file means "inherit from global."
**Rationale**: Instance TOML files stay minimal — users only specify overrides. Global file acts as a defaults template.
**Implementation**: Instance config structs use `Option<T>` wrappers. A `merge()` function produces the final resolved `NuxConfig` with no Options.

### 5. Schema versioning with integer version field

**Choice**: `schema_version = 1` at the top of every config file. Migration functions are a `Vec<fn(toml::Value) -> Result<toml::Value>>` indexed by version.
**Rationale**: Simple, linear migration chain. Each migration transforms raw TOML before deserialization, so old struct definitions aren't needed.
**Alternatives considered**: SemVer (unnecessary complexity for a local config file), no versioning (makes upgrades fragile).

### 6. Validation as a separate pass

**Choice**: `Config::validate()` runs after deserialization and merge, returning `Vec<ConfigError>`.
**Rationale**: Serde handles type/format errors. Validation handles semantic rules (e.g., `ram_mb >= 512`, `cpu_cores >= 1`, resolution within sane bounds). Separating these gives clearer error messages.

## Risks / Trade-offs

- **[Risk] TOML format limits expressiveness** → Acceptable. Config is flat key-value data; TOML is a natural fit. If we ever need complex structures, we can nest tables.
- **[Risk] Migration on raw TOML is fragile** → Mitigation: Each migration has a unit test with before/after TOML snapshots. Migrations are append-only (never deleted).
- **[Risk] Option-wrapping for merge adds boilerplate** → Mitigation: Use a proc-macro or helper trait if it gets unwieldy. For v1 with ~10 sections, manual impl is fine.
- **[Trade-off] No live reload** → Simplifies the design significantly. Config is read once at launch. If users edit config.toml manually, they restart the instance. Acceptable for v1.

## Open Questions

- Should we write a default global config.toml on first launch, or only create it when the user explicitly saves settings? Leaning toward writing a commented template on first run for discoverability.
- Exact validation bounds for hardware fields (min/max RAM, CPU cores) — defer to spec phase.
