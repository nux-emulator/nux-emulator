## Context

Nux Emulator is a greenfield all-Rust project. There is no existing codebase — only the OpenSpec configuration. This design covers the initial repository scaffolding: Cargo workspace layout, dependency declarations, tooling config, and documentation. Every future change builds on the structure established here.

The target environment is Linux desktops running GNOME (Wayland primary, X11 supported). The UI layer uses GTK4 + libadwaita via gtk-rs bindings. The VM backend (crosvm, gfxstream) is out of scope for this change but informs crate separation.

## Goals / Non-Goals

**Goals:**
- Establish a Cargo workspace that compiles cleanly with `cargo build`
- Separate concerns: `nux-core` (library, no UI dependencies) and `nux-ui` (binary, GTK4 frontend)
- Configure consistent code style and linting across the project
- Provide enough documentation for a new contributor to clone, build, and run
- Create directory scaffolding for CI workflows, scripts, and keymaps

**Non-Goals:**
- No runtime functionality — `nux-ui` produces a minimal GTK4 window or exits cleanly; `nux-core` exposes no public API yet
- No CI pipeline logic — `.github/workflows/` contains empty placeholder files
- No crosvm integration, GPU setup, or Android image handling

## Decisions

### 1. Cargo workspace with two crates

`nux-core` is a library crate; `nux-ui` is a binary crate that depends on `nux-core`. This separation keeps the VM/engine logic free of UI dependencies, enabling future headless operation and independent testing.

**Alternative considered:** Single crate with feature flags. Rejected because the UI and core have fundamentally different dependency trees (gtk-rs vs. system-level VM APIs), and a workspace gives cleaner build isolation.

### 2. gtk-rs 0.9.x (GTK4 4.16+ / libadwaita 1.6+)

Using the latest stable gtk-rs release line. This targets GTK4 ≥ 4.16 and libadwaita ≥ 1.6, which are available on current Fedora, Ubuntu 24.04+, and Arch.

**Alternative considered:** Relying on older gtk-rs 0.7.x for broader distro support. Rejected because Nux targets modern GNOME desktops and benefits from recent libadwaita widgets and Wayland improvements.

### 3. Shared workspace dependencies

Common dependencies (e.g., `serde`, `anyhow`, `log`) are declared in the workspace `[workspace.dependencies]` table and inherited by member crates via `workspace = true`. This avoids version drift between crates.

### 4. rustfmt + clippy configuration at workspace root

`rustfmt.toml` enforces consistent formatting (edition 2024, max width 100). `clippy.toml` and workspace-level `Cargo.toml` lint configuration set pedantic-level warnings. Both are checked in CI (once workflows are populated).

### 5. GPLv3 license

Matches the declared project license. The `LICENSE` file is placed at the workspace root and referenced in both crate `Cargo.toml` files.

### 6. Script stubs as shell scripts

`scripts/` contains bash stubs (`act-build.sh`, `install.sh`, `setup-kvm.sh`). These are intentionally minimal — just a shebang, description comment, and `set -euo pipefail`. Actual logic is added by future changes.

## Risks / Trade-offs

- **[gtk-rs version pinning]** → Pinning to gtk-rs 0.9.x means requiring GTK4 4.16+. Users on older LTS distros (e.g., Ubuntu 22.04) won't be able to build. Mitigation: document minimum versions in README; this is acceptable for a gaming-focused tool targeting current desktops.
- **[Empty scaffolding may rot]** → Placeholder files (empty workflows, script stubs) can become stale if not populated promptly. Mitigation: follow-up changes for CI and scripts are already planned.
- **[Two-crate overhead for a skeleton]** → Slightly more boilerplate than a single crate for what is currently zero functionality. Mitigation: the separation pays off immediately once core logic lands, and the overhead is trivial.

## Open Questions

- Exact minimum GTK4/libadwaita version to declare in README — needs testing against gtk-rs 0.9.x actual requirements.
- Whether `keymaps/` should contain a sample `.toml` file or remain empty. Current decision: empty directory with a `.gitkeep`.
