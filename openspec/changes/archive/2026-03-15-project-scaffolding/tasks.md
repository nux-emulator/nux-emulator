## 1. Cargo Workspace Setup

- [x] 1.1 Create root `Cargo.toml` with workspace declaration, `members = ["nux-core", "nux-ui"]`, `resolver = "2"`, `edition = "2024"`, and `[workspace.dependencies]` table with shared deps (`serde`, `anyhow`, `log`)
- [x] 1.2 Create `nux-core/Cargo.toml` (library crate) inheriting edition, license, and lints from workspace; create `nux-core/src/lib.rs` with a module-level doc comment
- [x] 1.3 Create `nux-ui/Cargo.toml` (binary crate) with path dependency on `nux-core`, `gtk4` and `libadwaita` deps via gtk-rs; create `nux-ui/src/main.rs` with minimal compilable entry point
- [x] 1.4 Add `[workspace.lints.rust]` and `[workspace.lints.clippy]` sections to root `Cargo.toml` with pedantic-level warnings; add `[lints] workspace = true` to both member crates
- [x] 1.5 Verify: `cargo metadata` lists both members; `cargo build --workspace` succeeds; `cargo clippy --workspace` runs with workspace lints applied

## 2. Project Tooling Configuration

- [x] 2.1 Create `rustfmt.toml` at repo root with `edition = "2024"` and `max_width = 100`
- [x] 2.2 Create `clippy.toml` at repo root (minimal initial config, e.g., `msrv` setting)
- [x] 2.3 Create `.editorconfig` at repo root: `indent_style = space`, `indent_size = 4` for `*.rs`, `indent_size = 2` for `*.yaml`/`*.toml`, `end_of_line = lf`, `insert_final_newline = true`, `charset = utf-8`
- [x] 2.4 Verify: `cargo fmt --check` passes; `.editorconfig` rules are syntactically valid

## 3. Development Scripts

- [x] 3.1 Create `scripts/act-build.sh` — executable bash stub with shebang, `set -euo pipefail`, and purpose comment
- [x] 3.2 Create `scripts/install.sh` — executable bash stub with shebang, `set -euo pipefail`, and purpose comment
- [x] 3.3 Create `scripts/setup-kvm.sh` — executable bash stub with shebang, `set -euo pipefail`, and purpose comment
- [x] 3.4 Verify: all scripts have executable bit set; `bash -n scripts/*.sh` passes syntax check

## 4. Directory Scaffolding

- [x] 4.1 Create `keymaps/.gitkeep`
- [x] 4.2 Create `.github/workflows/.gitkeep`
- [x] 4.3 Verify: directories `keymaps/`, `.github/workflows/`, `scripts/` all exist

## 5. Documentation and License

- [x] 5.1 Create `LICENSE` with full GPLv3 text; ensure both crate `Cargo.toml` files have `license = "GPL-3.0"`
- [x] 5.2 Create `README.md` with project name, description, feature highlights, build prerequisites (Rust, GTK4, libadwaita with distro-specific install commands), build/run instructions (`cargo build --workspace`, `cargo run -p nux-ui`)
- [x] 5.3 Verify: `LICENSE` file is present; both `Cargo.toml` files reference GPLv3; `README.md` contains build section

## 6. OpenSpec Config Update

- [x] 6.1 Update `openspec/config.yaml` with project context reflecting the new workspace structure, crate names, and conventions established by this change
- [x] 6.2 Verify: `openspec status` runs without errors
