## ADDED Requirements

### Requirement: Build workflow produces Debian package
The build workflow SHALL include a `build-deb` job that compiles the Nux Emulator Cargo workspace in release mode on Ubuntu and produces a `.deb` package containing the binary, keymaps, desktop file, icon, and metainfo.

#### Scenario: Successful deb build on push
- **WHEN** the build workflow is triggered (via push, PR, or workflow_call)
- **THEN** the `build-deb` job installs the Rust toolchain and GTK4/libadwaita dev packages, runs `cargo build --release --workspace`, and packages the output into a `.deb` file

#### Scenario: Deb package contains required files
- **WHEN** the `build-deb` job completes successfully
- **THEN** the `.deb` package contains `/usr/bin/nux-emulator`, `/usr/share/applications/nux-emulator.desktop`, `/usr/share/icons/hicolor/scalable/apps/nux-emulator.svg`, `/usr/share/metainfo/nux-emulator.metainfo.xml`, and `/usr/share/nux-emulator/keymaps/`

#### Scenario: Deb package declares runtime dependencies
- **WHEN** the `.deb` package is inspected
- **THEN** it SHALL declare dependencies on GTK4 and libadwaita runtime libraries

### Requirement: Build workflow produces RPM package
The build workflow SHALL include a `build-rpm` job that compiles the Nux Emulator in release mode inside a Fedora container and produces an `.rpm` package with the same contents as the deb.

#### Scenario: Successful rpm build on push
- **WHEN** the build workflow is triggered
- **THEN** the `build-rpm` job runs inside a Fedora container, installs the Rust toolchain and GTK4/libadwaita dev packages, runs `cargo build --release --workspace`, and produces an `.rpm` via `rpmbuild`

#### Scenario: RPM package contains required files
- **WHEN** the `build-rpm` job completes successfully
- **THEN** the `.rpm` package contains the same file set as the deb package at the same FHS paths

### Requirement: Build workflow produces portable tarball
The build workflow SHALL include a `build-binary` job that compiles the Nux Emulator in release mode and produces a `tar.gz` archive containing the binary, assets, and an install script.

#### Scenario: Successful tarball build
- **WHEN** the build workflow is triggered
- **THEN** the `build-binary` job produces a `.tar.gz` archive containing the `nux-emulator` binary, keymaps directory, desktop file, icon, metainfo, and `install.sh`

#### Scenario: Install script places files correctly
- **WHEN** a user extracts the tarball and runs `install.sh`
- **THEN** the script copies files to their FHS-standard locations (`/usr/bin/`, `/usr/share/applications/`, etc.)

### Requirement: Build jobs upload artifacts
All three build jobs SHALL upload their produced packages as GitHub Actions artifacts.

#### Scenario: Artifacts available after workflow run
- **WHEN** any build job completes successfully
- **THEN** the produced package file is uploaded via `actions/upload-artifact` and is downloadable from the workflow run summary

### Requirement: Build workflow uses Cargo caching
The build workflow SHALL cache the Cargo registry and target directory to reduce build times.

#### Scenario: Warm cache reduces build time
- **WHEN** a build job runs and a cache exists from a previous run
- **THEN** the job restores the cached Cargo registry and target directory before compilation

### Requirement: Build workflow is callable
The build workflow SHALL be triggerable via `workflow_call` so other workflows (e.g., release) can invoke it.

#### Scenario: Release workflow calls build workflow
- **WHEN** the release workflow invokes the build workflow via `workflow_call`
- **THEN** all three build jobs execute and their artifacts are available to the calling workflow
