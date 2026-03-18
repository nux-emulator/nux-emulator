## Why

Nux Emulator has no codebase yet. Before any feature work can begin, we need a well-structured Cargo workspace, dependency declarations, project tooling configuration, and documentation scaffolding. Setting this up correctly now avoids rework and establishes conventions that every future change builds on.

## What Changes

- Create a Cargo workspace root with two member crates: `nux-core` (library) and `nux-ui` (binary)
- Declare GTK4 + libadwaita dependencies in `nux-ui` via gtk-rs crate bindings
- Add project-level configuration: `rustfmt.toml`, `clippy.toml`, `.editorconfig`
- Add `README.md` with project description, build prerequisites, and build/run instructions
- Add `LICENSE` file (GPLv3)
- Create `.github/workflows/` directory structure with placeholder workflow files
- Create `scripts/` directory with development helper stubs (`act-build.sh`, `install.sh`, `setup-kvm.sh`)
- Create `keymaps/` directory for future keymap configuration files
- Update `openspec/config.yaml` with project context and conventions

## Capabilities

### New Capabilities

- `workspace-structure`: Cargo workspace layout, crate definitions, and inter-crate dependency wiring
- `dev-tooling`: Project-level linting, formatting, editor config, and development scripts
- `project-docs`: README, LICENSE, and contribution scaffolding

### Modified Capabilities

_None — this is a greenfield project._

## Non-goals

- No feature implementation — crates contain only minimal boilerplate (`fn main()` / `lib.rs` module stub)
- No CI pipeline logic — workflow files are created but left empty for a future change
- No Android image handling, VM management, or GPU setup
- No actual keymap definitions — just the directory structure

## Impact

- Every future change depends on this workspace structure
- Developers can `cargo build` and `cargo run` after this change (producing a blank GTK4 window or a no-op binary)
- CI scaffolding unblocks the CI/CD pipeline change that follows
- `openspec/config.yaml` updates ensure future OpenSpec artifacts have correct project context
