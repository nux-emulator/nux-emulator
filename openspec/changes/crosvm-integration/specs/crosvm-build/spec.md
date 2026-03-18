## ADDED Requirements

### Requirement: Build crosvm from source with required features
The build system SHALL compile crosvm from the source tree at `/build2/nux-emulator/crosvm/` with the feature flags `gfxstream`, `audio`, and `x` enabled.

#### Scenario: Successful build with all features
- **WHEN** the build system compiles crosvm with features `gfxstream`, `audio`, and `x`
- **THEN** the build SHALL produce a `crosvm` binary with gfxstream GPU backend, audio support, and X11 support compiled in

#### Scenario: Source tree not found
- **WHEN** the build system attempts to compile crosvm and the source directory `/build2/nux-emulator/crosvm/` does not exist
- **THEN** the build SHALL fail with an error indicating the crosvm source path is missing

### Requirement: Cache crosvm build artifacts
The build system SHALL cache crosvm compilation artifacts so that subsequent builds without source changes complete without full recompilation.

#### Scenario: No source changes since last build
- **WHEN** the build system runs and no crosvm source files have changed since the last successful build
- **THEN** the build SHALL reuse cached artifacts and skip recompilation

#### Scenario: Source changes detected
- **WHEN** the build system runs and crosvm source files have changed since the last build
- **THEN** the build SHALL perform an incremental recompilation

### Requirement: Verify crosvm binary after build
The build system SHALL verify the built crosvm binary is executable and reports a version string when invoked with `--version`.

#### Scenario: Binary verification succeeds
- **WHEN** the build completes and the binary is invoked with `--version`
- **THEN** the binary SHALL output a version string and exit with code 0

#### Scenario: Binary verification fails
- **WHEN** the build completes but the binary fails to execute or does not return a version string
- **THEN** the build system SHALL report a build verification failure with the captured error output
