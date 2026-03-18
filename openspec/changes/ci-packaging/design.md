## Context

Nux Emulator is an all-Rust Cargo workspace (`nux-core` library + `nux-ui` binary) targeting Linux. There is currently no CI pipeline and no automated packaging. Developers build locally with `cargo build` and there is no standardized way to produce distributable packages. The project needs CI/CD before its first public release so that every tagged version automatically produces .deb, .rpm, and tar.gz packages and publishes them as a GitHub Release.

The project already has placeholder locations for workflow files (`.github/workflows/`) and helper scripts (`scripts/`). Desktop integration files (`.desktop`, icon, metainfo) do not yet exist and are required by Linux packaging standards.

## Goals / Non-Goals

**Goals:**
- Automated, reproducible builds for every push and PR (build verification)
- Three package formats produced from a single workflow: `.deb`, `.rpm`, `tar.gz`
- Tag-triggered releases that create GitHub Releases with all packages attached
- Local build reproducibility via `scripts/act-build.sh` using nektos/act
- Proper freedesktop integration files shipped inside packages

**Non-Goals:**
- Flatpak, Snap, or AppImage packaging
- Publishing to distro repositories (PPA, COPR, AUR)
- macOS or Windows builds
- Code signing
- Android image building (separate repo/pipeline)
- Auto-update mechanism

## Decisions

### 1. Reusable build workflow + separate release workflow

The build workflow (`build-linux.yml`) is a standalone callable workflow (`workflow_call`) that can be triggered by PRs, pushes, and the release workflow. The release workflow (`release.yml`) calls the build workflow then creates the GitHub Release.

**Why:** Avoids duplicating build logic. PRs get build verification for free. The release workflow stays thin — it only orchestrates artifact collection and release creation.

**Alternative considered:** Single monolithic workflow with conditional release step. Rejected because it makes the build workflow harder to reuse and test independently.

### 2. Three parallel build jobs instead of a matrix

Each package format (deb, rpm, tar.gz) runs as a separate job rather than using a GitHub Actions matrix strategy.

**Why:** Each job needs a different base image (Ubuntu for deb, Fedora for rpm) and different packaging tools (`dpkg-deb` vs `rpmbuild` vs `tar`). A matrix would require excessive conditional logic. Separate jobs are clearer and independently retriable.

### 3. nektos/act for local builds with volume mounting

`scripts/act-build.sh` runs the build workflow locally via `act`. It mounts `./build/` into the container so that workflow steps can copy finished packages to the mounted path, making them available on the host after the run completes.

**Why:** Contributors can produce packages without pushing to GitHub. Volume mounting is simpler than extracting artifacts from act's internal storage. The script is a thin wrapper — the real logic lives in the workflow YAML, keeping things DRY.

**Alternative considered:** A separate Makefile/shell-based build system. Rejected because it would duplicate the workflow logic and drift over time.

### 4. Package structure conventions

- Binary installed to `/usr/bin/nux-emulator`
- Desktop file to `/usr/share/applications/nux-emulator.desktop`
- Icon to `/usr/share/icons/hicolor/scalable/apps/nux-emulator.svg`
- Metainfo to `/usr/share/metainfo/nux-emulator.metainfo.xml`
- Keymaps to `/usr/share/nux-emulator/keymaps/`

**Why:** Follows freedesktop and Filesystem Hierarchy Standard conventions. Scalable SVG icon avoids needing multiple raster sizes.

### 5. Runner images

- `build-deb`: `ubuntu-latest` (Ubuntu 24.04 at time of writing)
- `build-rpm`: Fedora container on `ubuntu-latest` (via `container: fedora:latest`)
- `build-binary`: `ubuntu-latest`, statically linked where possible

**Why:** GitHub-hosted runners are Ubuntu-based. For RPM builds, running inside a Fedora container is the simplest way to get `rpmbuild` and Fedora-native GTK4 dev packages.

## Risks / Trade-offs

- **[GTK4/libadwaita dev packages may differ across distro versions]** → Pin specific distro versions in workflow (e.g., `ubuntu-24.04`, `fedora:41`) rather than using `latest` to avoid surprise breakage.
- **[act may not perfectly replicate GitHub Actions environment]** → Document known differences in `act-build.sh` header comments. Keep the script as a convenience tool, not the source of truth.
- **[Large build times due to Rust compilation]** → Use `actions/cache` for the Cargo registry and target directory. Expect ~10-15 min builds with warm cache.
- **[Binary in tar.gz may have dynamic GTK4 dependencies]** → The tar.gz format targets users who have GTK4 installed. Document runtime dependencies in `install.sh`. Fully static GTK4 linking is not feasible.
- **[RPM spec maintenance burden]** → Keep the rpmbuild spec inline in the workflow as a heredoc to avoid a separate `.spec` file that drifts. Revisit if complexity grows.

## Open Questions

- Should the build workflow run on every push to `main` only, or on all branches? (Recommendation: all branches for PRs, `main` for artifact uploads.)
- Minimum supported Ubuntu/Fedora versions for packages? (Recommendation: Ubuntu 24.04+, Fedora 40+ — matching GTK4 0.9.x / libadwaita 0.7.x availability.)
