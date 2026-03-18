## ADDED Requirements

### Requirement: Cargo workspace root manifest
The repository root SHALL contain a `Cargo.toml` that declares a Cargo workspace with `nux-core` and `nux-ui` as members. The workspace manifest SHALL define a `[workspace.dependencies]` table for shared dependency versions. The workspace SHALL set `resolver = "2"` and `edition = "2024"`.

#### Scenario: Workspace members are recognized
- **WHEN** a developer runs `cargo metadata` at the repository root
- **THEN** both `nux-core` and `nux-ui` are listed as workspace members

#### Scenario: Shared dependencies are inherited
- **WHEN** a member crate declares a dependency with `workspace = true`
- **THEN** the version resolved matches the one declared in `[workspace.dependencies]`

### Requirement: nux-core library crate
The workspace SHALL contain a `nux-core/` directory with a `Cargo.toml` declaring a library crate. The crate SHALL have `src/lib.rs` as its entry point. The `Cargo.toml` SHALL inherit `edition`, `license`, and shared dependencies from the workspace. The `src/lib.rs` file SHALL compile without errors and MAY be empty or contain only a module-level doc comment.

#### Scenario: nux-core compiles independently
- **WHEN** a developer runs `cargo build -p nux-core`
- **THEN** the build succeeds with no errors

#### Scenario: nux-core has no UI dependencies
- **WHEN** inspecting `nux-core/Cargo.toml`
- **THEN** there are no dependencies on `gtk4`, `libadwaita`, or any gtk-rs crate

### Requirement: nux-ui binary crate
The workspace SHALL contain a `nux-ui/` directory with a `Cargo.toml` declaring a binary crate. The crate SHALL have `src/main.rs` as its entry point. The `Cargo.toml` SHALL declare dependencies on `nux-core` (path dependency), `gtk4`, and `libadwaita` via gtk-rs bindings. The `src/main.rs` SHALL compile without errors and produce a runnable binary.

#### Scenario: nux-ui compiles with GTK4 dependencies
- **WHEN** a developer runs `cargo build -p nux-ui`
- **THEN** the build succeeds and produces a `nux-ui` binary

#### Scenario: nux-ui depends on nux-core
- **WHEN** inspecting `nux-ui/Cargo.toml`
- **THEN** `nux-core` is listed as a path dependency

### Requirement: Workspace-level lint configuration
The workspace `Cargo.toml` SHALL define `[workspace.lints.rust]` and `[workspace.lints.clippy]` sections with pedantic-level warnings. Member crates SHALL inherit these lints via `[lints] workspace = true`.

#### Scenario: Clippy lints are enforced workspace-wide
- **WHEN** a developer runs `cargo clippy --workspace`
- **THEN** the pedantic lint set defined in the workspace root is applied to all member crates

### Requirement: Directory scaffolding
The repository SHALL contain the following empty directories preserved with `.gitkeep` files: `keymaps/`, `.github/workflows/`. The `scripts/` directory SHALL exist and contain executable shell script stubs.

#### Scenario: Directory structure exists after clone
- **WHEN** a developer clones the repository
- **THEN** the directories `keymaps/`, `.github/workflows/`, and `scripts/` all exist
