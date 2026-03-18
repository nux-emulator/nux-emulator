## ADDED Requirements

### Requirement: act-build script runs workflows locally
The `scripts/act-build.sh` script SHALL run the build workflow locally using nektos/act, allowing developers to produce packages without pushing to GitHub.

#### Scenario: Local build produces packages
- **WHEN** a developer runs `scripts/act-build.sh`
- **THEN** nektos/act executes the build workflow inside containers and the built packages are available on the host filesystem

### Requirement: act-build mounts output volume
The script SHALL mount `./build/` as a volume into the act container so that workflow steps can copy built packages to the host.

#### Scenario: Packages appear in build directory
- **WHEN** `scripts/act-build.sh` completes successfully
- **THEN** the `.deb`, `.rpm`, and `.tar.gz` packages are present in the `./build/` directory on the host

### Requirement: act-build creates output directory
The script SHALL create the `./build/` directory if it does not already exist.

#### Scenario: Build directory auto-created
- **WHEN** a developer runs `scripts/act-build.sh` and `./build/` does not exist
- **THEN** the script creates `./build/` before invoking act

### Requirement: act-build checks for act installation
The script SHALL verify that `act` is installed and available on PATH before attempting to run.

#### Scenario: Missing act produces helpful error
- **WHEN** a developer runs `scripts/act-build.sh` and `act` is not installed
- **THEN** the script prints an error message explaining how to install nektos/act and exits with a non-zero status

### Requirement: act-build is executable
The script SHALL have the executable permission bit set and use a bash shebang.

#### Scenario: Script runs without manual chmod
- **WHEN** the script is checked out from git
- **THEN** it has executable permissions and can be run directly as `./scripts/act-build.sh`
