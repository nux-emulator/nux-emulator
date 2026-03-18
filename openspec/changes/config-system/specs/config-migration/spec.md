## ADDED Requirements

### Requirement: Schema version presence
Every config file written by the system SHALL include a `schema_version` integer field at the top level. The current schema version SHALL be defined as a constant in the config module.

#### Scenario: Newly created config has version
- **WHEN** the system writes a default config file
- **THEN** the file contains `schema_version = 1` (or the current version constant)

### Requirement: Detect outdated schema version
The system SHALL compare the `schema_version` in a loaded config file against the current version constant. If the file version is lower, the system SHALL trigger migration before deserialization into typed structs.

#### Scenario: Current version config loads directly
- **WHEN** a config file has `schema_version` equal to the current version
- **THEN** the system deserializes it directly without running any migrations

#### Scenario: Outdated config triggers migration
- **WHEN** a config file has `schema_version = 1` and the current version is `2`
- **THEN** the system runs the v1→v2 migration function before deserialization

### Requirement: Sequential migration chain
The system SHALL apply migrations sequentially from the file's version to the current version. Each migration function SHALL transform the raw TOML value from version N to version N+1.

#### Scenario: Multi-step migration
- **WHEN** a config file has `schema_version = 1` and the current version is `3`
- **THEN** the system applies v1→v2 then v2→v3 in order

#### Scenario: Single-step migration
- **WHEN** a config file has `schema_version = 2` and the current version is `3`
- **THEN** the system applies only v2→v3

### Requirement: Migration updates schema version
After all migrations complete, the resulting TOML value SHALL have its `schema_version` field set to the current version.

#### Scenario: Version field updated after migration
- **WHEN** a v1 config is migrated to v3
- **THEN** the resulting TOML has `schema_version = 3`

### Requirement: Migration error handling
If any migration step fails, the system SHALL return an error indicating which version transition failed and the cause. The original config file SHALL NOT be modified on failure.

#### Scenario: Migration failure preserves original
- **WHEN** the v2→v3 migration function returns an error
- **THEN** the system returns an error mentioning "migration from v2 to v3" and the original file on disk is unchanged

### Requirement: Reject future schema versions
The system SHALL return an error if a config file's `schema_version` is greater than the current version, indicating the config was created by a newer version of Nux.

#### Scenario: Future version rejected
- **WHEN** a config file has `schema_version = 99` and the current version is `1`
- **THEN** the system returns an error indicating the config is from a newer Nux version

### Requirement: Missing schema version handling
If a config file has no `schema_version` field, the system SHALL treat it as version `1` (the initial version).

#### Scenario: No version field defaults to v1
- **WHEN** a config file contains valid TOML but no `schema_version` field
- **THEN** the system treats it as `schema_version = 1` and applies migrations from v1 if needed
