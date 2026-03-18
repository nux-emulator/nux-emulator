## MODIFIED Requirements

### Requirement: Google Services configuration fields
The `GoogleServicesConfig` struct SHALL contain a `provider` field as a string enum supporting `"none"`, `"microg"`, and `"gapps"`, a `provider_version` field as `Option<String>` storing the last known version of the active provider, and a `gapps_source` field as a string enum supporting `"opengapps"` and `"mindthegapps"` with a default of `"opengapps"`.

#### Scenario: Parse google services provider
- **WHEN** the TOML contains `[google_services]` with `provider = "microg"`
- **THEN** the deserialized `GoogleServicesConfig` has `provider == GoogleServicesProvider::MicroG`

#### Scenario: Parse provider version
- **WHEN** the TOML contains `[google_services]` with `provider_version = "0.3.1.4"`
- **THEN** the deserialized `GoogleServicesConfig` has `provider_version == Some("0.3.1.4")`

#### Scenario: Default provider version when absent
- **WHEN** the TOML `[google_services]` section does not contain `provider_version`
- **THEN** the deserialized `GoogleServicesConfig` has `provider_version == None`

#### Scenario: Parse GApps source preference
- **WHEN** the TOML contains `[google_services]` with `gapps_source = "mindthegapps"`
- **THEN** the deserialized `GoogleServicesConfig` has `gapps_source == GAppsSource::MindTheGapps`

#### Scenario: Default GApps source when absent
- **WHEN** the TOML `[google_services]` section does not contain `gapps_source`
- **THEN** the deserialized `GoogleServicesConfig` has `gapps_source == GAppsSource::OpenGApps`
