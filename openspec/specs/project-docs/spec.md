## ADDED Requirements

### Requirement: README with project description
The repository root SHALL contain a `README.md` file. The README SHALL include: the project name ("Nux Emulator"), a one-paragraph description of the project (gaming-focused Android emulator for Linux), and a feature highlights section.

#### Scenario: README exists and describes the project
- **WHEN** a developer reads `README.md`
- **THEN** it contains the project name, a description mentioning Android emulation on Linux, and key feature highlights

### Requirement: README with build instructions
The `README.md` SHALL include a "Building" or "Build" section that lists: system prerequisites (Rust toolchain, GTK4 development libraries, libadwaita development libraries), the exact `cargo build` command to compile the project, and how to run the resulting binary.

#### Scenario: Build instructions are actionable
- **WHEN** a developer follows the build instructions on a system with the listed prerequisites installed
- **THEN** `cargo build --workspace` succeeds without errors

#### Scenario: Prerequisites are listed
- **WHEN** a developer reads the build section of `README.md`
- **THEN** it lists the minimum required versions of Rust, GTK4, and libadwaita, plus distro-specific package install commands

### Requirement: LICENSE file
The repository root SHALL contain a `LICENSE` file with the full text of the GNU General Public License v3.0. Both crate `Cargo.toml` files SHALL reference `license = "GPL-3.0"` (or the equivalent SPDX identifier).

#### Scenario: License file is present and correct
- **WHEN** a developer reads the `LICENSE` file
- **THEN** it contains the complete GPLv3 license text

#### Scenario: Crate manifests reference the license
- **WHEN** inspecting `nux-core/Cargo.toml` and `nux-ui/Cargo.toml`
- **THEN** both contain a `license` field with the GPLv3 SPDX identifier
