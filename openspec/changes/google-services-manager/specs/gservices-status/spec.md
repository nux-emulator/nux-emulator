## ADDED Requirements

### Requirement: Provider status struct
The system SHALL expose a `GoogleServicesStatus` struct containing: the active provider (`GoogleServicesProvider`), the provider version (`Option<String>`), a freshness indicator (`live` or `cached`), and a `restart_required` boolean flag.

#### Scenario: Full status with live detection
- **WHEN** the VM is running and ADB is connected and no provider switch is pending
- **THEN** the status contains the live-detected provider, its version, freshness `live`, and `restart_required` is `false`

#### Scenario: Status after provider switch
- **WHEN** a provider switch has been applied but the VM has not been restarted
- **THEN** the status contains the new provider from config, version `None`, freshness `cached`, and `restart_required` is `true`

### Requirement: Status query API
The system SHALL provide a `query_status()` function in the `gservices` module that returns `GoogleServicesStatus`. This function SHALL attempt live detection first and fall back to cached config state.

#### Scenario: Live status query
- **WHEN** `query_status()` is called and ADB is available
- **THEN** the function performs live detection and returns a status with freshness `live`

#### Scenario: Cached status query
- **WHEN** `query_status()` is called and ADB is not available
- **THEN** the function reads config and returns a status with freshness `cached`

### Requirement: Restart-required flag management
The system SHALL set `restart_required` to `true` after any provider switch operation and SHALL clear it to `false` when the VM is next started.

#### Scenario: Flag set after switch
- **WHEN** a provider switch completes successfully
- **THEN** `restart_required` is `true` in subsequent status queries

#### Scenario: Flag cleared on VM start
- **WHEN** the VM is started after a provider switch
- **THEN** `restart_required` is cleared to `false`

### Requirement: Persist provider state to config
The system SHALL update `GoogleServicesConfig` in the instance config whenever the provider changes, ensuring the persisted state survives application restarts.

#### Scenario: Config updated after switch
- **WHEN** a provider switch completes
- **THEN** the instance's `config.toml` `[google_services]` section reflects the new provider

#### Scenario: Config survives restart
- **WHEN** the application is restarted after a provider switch
- **THEN** loading the instance config returns the previously switched provider
