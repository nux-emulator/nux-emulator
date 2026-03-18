## 1. Module Setup and Provider Types

- [x] 1.1 Create `nux-core/src/gservices/` module directory with `mod.rs` defining the public API surface
- [x] 1.2 Define `GoogleServicesProvider` enum (`MicroG`, `GApps`, `None`) with serde serialization and config integration
- [x] 1.3 Define `GAppsSource` enum (`OpenGApps`, `MindTheGapps`) with serde serialization and default of `OpenGApps`
- [x] 1.4 Define `GoogleServicesStatus` struct with fields: provider, version, freshness (`live`/`cached`), `restart_required`
- [x] 1.5 Extend `GoogleServicesConfig` in config-schema to add `provider_version: Option<String>` and `gapps_source: GAppsSource` fields with serde defaults
- [x] 1.6 Write unit tests for enum serialization round-trips and config default values

## 2. Provider Detection

- [x] 2.1 Implement `detect_provider()` function that queries `pm list packages` via ADB for known package identifiers (`com.google.android.gms`, `org.microg.gms.droidguard`)
- [x] 2.2 Implement `detect_version()` function that queries `dumpsys package <name>` via ADB and parses `versionName`
- [x] 2.3 Implement config-based fallback: when ADB is unavailable, return provider from `GoogleServicesConfig` with `cached` freshness
- [x] 2.4 Write unit tests for package list parsing logic (mock ADB output for MicroG, GApps, None scenarios)

## 3. GApps Download and Verification

- [x] 3.1 Add `reqwest` and `sha2` as workspace dependencies in root `Cargo.toml`
- [x] 3.2 Implement `download_gapps()` function: resolve URL from `GAppsSource`, download to XDG cache dir (`$XDG_DATA_HOME/nux/cache/gapps/`), report progress
- [x] 3.3 Implement SHA-256 hash verification of downloaded package; delete and return error on mismatch
- [x] 3.4 Implement cache lookup: skip download if a verified package already exists in cache
- [x] 3.5 Write integration test for download-and-verify flow using a mock HTTP server

## 4. Overlay Management

- [x] 4.1 Implement overlay backup: copy current instance overlay to a `.backup` path before modification
- [x] 4.2 Implement overlay restore: on apply failure, restore from backup automatically
- [x] 4.3 Implement `apply_gapps_overlay()`: extract GApps package contents into the instance overlay, replacing MicroG files
- [x] 4.4 Implement `reset_overlay_to_base()`: remove instance overlay modifications to restore base image state (MicroG pre-installed)
- [x] 4.5 Implement `apply_removal_overlay()`: overlay that removes both MicroG and GApps packages for None mode
- [x] 4.6 Write unit tests for backup/restore logic and overlay path construction

## 5. Provider Switching State Machine

- [x] 5.1 Implement `switch_provider()` entry point that validates preconditions (VM stopped) and dispatches to the correct transition
- [x] 5.2 Implement MicroG → GApps transition: download, verify, apply overlay, update config
- [x] 5.3 Implement GApps → MicroG transition: reset overlay to base, update config
- [x] 5.4 Implement MicroG/GApps → None transitions: apply removal overlay, update config
- [x] 5.5 Implement None → MicroG transition: reset overlay to base, update config
- [x] 5.6 Implement None → GApps transition: download, verify, apply overlay, update config
- [x] 5.7 Set `restart_required` flag after every successful switch; clear on next VM start
- [x] 5.8 Write unit tests for each transition path including precondition rejection when VM is running

## 6. Status Query API

- [x] 6.1 Implement `query_status()` function: attempt live detection, fall back to cached, assemble `GoogleServicesStatus`
- [x] 6.2 Persist updated provider and version to instance config after successful detection or switch
- [x] 6.3 Write unit tests for status query with mocked ADB available and unavailable scenarios

## 7. Integration and Wiring

- [x] 7.1 Export `gservices` module from `nux-core` lib.rs public API
- [x] 7.2 Verify `cargo build --workspace` compiles cleanly with new module
- [x] 7.3 Verify `cargo clippy --workspace` passes with no new warnings
- [x] 7.4 Run full test suite: `cargo test --workspace`
