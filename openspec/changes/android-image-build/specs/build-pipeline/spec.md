## ADDED Requirements

### Requirement: Single entry point build script
The build system SHALL provide a `build.sh` script as the single entry point for building Android images, encapsulating repo sync, environment setup, lunch target selection, and image compilation.

#### Scenario: Full build from clean state
- **WHEN** a developer runs `./build.sh` on a machine with AOSP prerequisites installed
- **THEN** the script SHALL initialize the AOSP repo, sync sources, configure the build environment, and produce all output images

#### Scenario: Incremental build
- **WHEN** `./build.sh` is run after a previous successful build with source changes
- **THEN** the script SHALL perform an incremental build, recompiling only changed components

### Requirement: Output artifacts produced
The build SHALL produce four image files: `boot.img`, `system.img`, `vendor.img`, and `userdata.img`.

#### Scenario: All images generated
- **WHEN** the build completes successfully
- **THEN** `boot.img`, `system.img`, `vendor.img`, and `userdata.img` SHALL exist in the output directory

#### Scenario: Images are bootable
- **WHEN** the output images are loaded by crosvm via the Nux emulator
- **THEN** Android SHALL boot to the home screen without errors

### Requirement: Image versioning
Each build SHALL embed a version string in `build.prop` using CalVer format (`YYYY.MM.patch`) and include the version in output artifact filenames.

#### Scenario: Version in build.prop
- **WHEN** the image is built
- **THEN** `ro.nux.image.version` SHALL be set to the CalVer version string in `build.prop`

#### Scenario: Version in filenames
- **WHEN** the build completes and artifacts are collected
- **THEN** output files SHALL be named with the version (e.g., `nux-system-2026.03.1.img`)

### Requirement: Checksum generation
The build SHALL generate SHA-256 checksums for all output image files.

#### Scenario: Checksums file created
- **WHEN** the build completes successfully
- **THEN** a `SHA256SUMS` file SHALL be generated in the output directory containing SHA-256 hashes for each `.img` file

#### Scenario: Checksums are valid
- **WHEN** `sha256sum -c SHA256SUMS` is run in the output directory
- **THEN** all checksums SHALL verify successfully

### Requirement: CI pipeline for automated builds
The build system SHALL include a GitHub Actions workflow for automated image builds triggered on version tags and manual dispatch.

#### Scenario: Tag-triggered build
- **WHEN** a version tag (e.g., `v2026.03.1`) is pushed to the repository
- **THEN** the CI pipeline SHALL trigger a full image build and upload artifacts to the GitHub release

#### Scenario: Manual dispatch build
- **WHEN** a developer triggers the workflow manually via `workflow_dispatch`
- **THEN** the CI pipeline SHALL execute a full build with the specified parameters

#### Scenario: Build artifacts uploaded
- **WHEN** the CI build completes successfully
- **THEN** `boot.img`, `system.img`, `vendor.img`, `userdata.img`, and `SHA256SUMS` SHALL be uploaded as release artifacts

### Requirement: Build documentation
The repository SHALL include documentation covering prerequisites, build instructions, customization options, and troubleshooting.

#### Scenario: README covers build steps
- **WHEN** a developer reads the repository README
- **THEN** it SHALL contain step-by-step instructions for building images from source, including system requirements (disk space, RAM, OS) and required packages

#### Scenario: Build flags documented
- **WHEN** a developer wants to customize the build (e.g., disable ARM translation)
- **THEN** the documentation SHALL list all available build flags with descriptions and default values
