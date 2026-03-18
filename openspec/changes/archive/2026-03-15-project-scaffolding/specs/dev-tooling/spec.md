## ADDED Requirements

### Requirement: Rust formatting configuration
The repository root SHALL contain a `rustfmt.toml` that sets `edition = "2024"` and `max_width = 100`. All Rust source files in the workspace SHALL conform to this configuration.

#### Scenario: rustfmt config is applied
- **WHEN** a developer runs `cargo fmt --check` at the workspace root
- **THEN** all Rust files are validated against the rules in `rustfmt.toml`

#### Scenario: Formatting is consistent
- **WHEN** a developer runs `cargo fmt` followed by `cargo fmt --check`
- **THEN** the check passes with no differences reported

### Requirement: Clippy configuration file
The repository root SHALL contain a `clippy.toml` file for any clippy-specific configuration knobs (e.g., `msrv`, cognitive complexity thresholds). This file SHALL be present even if initially minimal.

#### Scenario: Clippy respects configuration
- **WHEN** a developer runs `cargo clippy --workspace`
- **THEN** clippy reads settings from the root `clippy.toml`

### Requirement: EditorConfig
The repository root SHALL contain an `.editorconfig` file that defines: `indent_style = space`, `indent_size = 4` for Rust files, `indent_size = 2` for YAML/TOML files, `end_of_line = lf`, `insert_final_newline = true`, and `charset = utf-8`.

#### Scenario: Editor picks up indentation settings
- **WHEN** a developer opens any `.rs` file in an EditorConfig-aware editor
- **THEN** the editor uses 4-space indentation

#### Scenario: YAML files use 2-space indent
- **WHEN** a developer opens any `.yaml` or `.toml` file in an EditorConfig-aware editor
- **THEN** the editor uses 2-space indentation

### Requirement: Development helper scripts
The `scripts/` directory SHALL contain the following executable bash scripts: `act-build.sh` (for local CI via nektos/act), `install.sh` (for local installation), `setup-kvm.sh` (for KVM permission setup). Each script SHALL begin with `#!/usr/bin/env bash`, include `set -euo pipefail`, and contain a comment describing its purpose. Scripts SHALL be executable (`chmod +x`).

#### Scenario: Scripts are executable
- **WHEN** a developer runs `ls -l scripts/`
- **THEN** all `.sh` files have the executable permission bit set

#### Scenario: Scripts run without syntax errors
- **WHEN** a developer runs `bash -n scripts/act-build.sh`
- **THEN** the syntax check passes with no errors
