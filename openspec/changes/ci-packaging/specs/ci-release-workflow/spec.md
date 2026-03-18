## ADDED Requirements

### Requirement: Release workflow triggers on version tags
The release workflow SHALL trigger on pushes of tags matching the pattern `v*` (e.g., `v0.1.0`, `v1.0.0-beta.1`).

#### Scenario: Tag push triggers release
- **WHEN** a tag matching `v*` is pushed to the repository
- **THEN** the release workflow starts automatically

#### Scenario: Non-tag push does not trigger release
- **WHEN** a regular commit is pushed to any branch without a tag
- **THEN** the release workflow does NOT run

### Requirement: Release workflow invokes build workflow
The release workflow SHALL call the build workflow (`build-linux.yml`) via `workflow_call` to produce all packages.

#### Scenario: Build workflow runs as part of release
- **WHEN** the release workflow is triggered by a version tag
- **THEN** it invokes the build workflow and waits for all three build jobs to complete

### Requirement: Release workflow creates GitHub Release
The release workflow SHALL create a GitHub Release using the tag name as the release title, and attach all built packages (.deb, .rpm, .tar.gz) as release assets.

#### Scenario: GitHub Release created with all assets
- **WHEN** the build workflow completes successfully within the release workflow
- **THEN** a GitHub Release is created with the tag name, and the `.deb`, `.rpm`, and `.tar.gz` packages are attached as downloadable assets

#### Scenario: Release body contains package checksums
- **WHEN** the GitHub Release is created
- **THEN** the release body SHALL include SHA256 checksums for each attached package file

### Requirement: Release workflow fails if build fails
The release workflow SHALL NOT create a GitHub Release if any build job fails.

#### Scenario: Build failure prevents release
- **WHEN** any of the three build jobs fails during a release workflow run
- **THEN** no GitHub Release is created and the workflow reports failure
