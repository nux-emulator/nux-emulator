## Why

Nux Emulator has no automated build pipeline or packaging. Every release requires manual compilation, manual packaging into .deb/.rpm/tarball, and manual upload to GitHub Releases. This is error-prone, slow, and blocks contributors who can't easily produce distributable packages. CI/CD automation and local reproducibility via nektos/act are needed before the first public release.

## What Changes

- Add `.github/workflows/build-linux.yml` with three parallel jobs:
  - `build-deb`: Builds a `.deb` package targeting Ubuntu/Debian (installs Rust toolchain + GTK4/libadwaita dev deps, runs `cargo build --release`, packages binary + desktop file + icon + metainfo into `.deb`)
  - `build-rpm`: Builds an `.rpm` package targeting Fedora/RHEL via `rpmbuild`
  - `build-binary`: Builds a portable `tar.gz` archive containing the release binary and `scripts/install.sh`
  - All jobs upload build artifacts via `actions/upload-artifact`
- Add `.github/workflows/release.yml`:
  - Triggered on tag push matching `v*`
  - Calls the build workflow, then creates a GitHub Release with all packages attached
- Add `scripts/act-build.sh`:
  - Runs the build workflow locally using nektos/act
  - Mounts `./build/` as a volume so built packages are available on the host after the run
- Add desktop integration files:
  - `nux-emulator.desktop` (freedesktop `.desktop` entry)
  - App icon in SVG format
  - AppStream metainfo XML for software center discovery

## Capabilities

### New Capabilities
- `ci-build-workflow`: GitHub Actions workflow that compiles Nux and produces .deb, .rpm, and tar.gz packages
- `ci-release-workflow`: GitHub Actions workflow that creates GitHub Releases from tagged commits with all built packages
- `local-act-build`: Shell script for running CI workflows locally via nektos/act with host-mounted output
- `desktop-integration`: Freedesktop desktop entry, app icon, and AppStream metainfo files

### Modified Capabilities
<!-- None — this change introduces new CI/packaging infrastructure without modifying existing specs. -->

## Impact

- New files under `.github/workflows/`, `scripts/`, and project root (desktop/icon/metainfo)
- New dev dependency: nektos/act (optional, local builds only — not a Cargo dependency)
- Workflows depend on the full Cargo workspace building successfully (`cargo build --release --workspace`)
- Packaging jobs depend on system packages: `dpkg-deb`, `rpmbuild`, GTK4/libadwaita dev libraries
- This change is a leaf dependency — it builds the final product and depends on all other changes being merged first

## Non-goals

- Flatpak or Snap packaging (deferred to v2)
- Auto-update mechanism within the emulator
- Android image CI (lives in a separate repository)
- macOS or Windows builds
- Code signing or notarization
- Publishing to distro package repositories (PPA, COPR, AUR)
