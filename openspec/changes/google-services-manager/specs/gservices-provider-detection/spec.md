## ADDED Requirements

### Requirement: Detect active provider by package query
The system SHALL detect the currently active Google Services provider by querying the Android guest's package manager via ADB for known package identifiers: `com.google.android.gms` and `com.google.android.gsf` for GApps, `org.microg.gms.droidguard` for MicroG.

#### Scenario: Detect MicroG as active provider
- **WHEN** ADB is connected and `pm list packages` returns `org.microg.gms.droidguard` but not `com.google.android.gms`
- **THEN** the system reports the active provider as `MicroG`

#### Scenario: Detect GApps as active provider
- **WHEN** ADB is connected and `pm list packages` returns `com.google.android.gms` and `com.google.android.gsf`
- **THEN** the system reports the active provider as `GApps`

#### Scenario: Detect None provider
- **WHEN** ADB is connected and `pm list packages` returns neither MicroG nor GApps package identifiers
- **THEN** the system reports the active provider as `None`

### Requirement: Query provider version
The system SHALL retrieve the version string of the active provider by querying `dumpsys package <package_name>` via ADB for the primary provider package.

#### Scenario: Retrieve MicroG version
- **WHEN** the active provider is MicroG and ADB is connected
- **THEN** the system returns the `versionName` from `dumpsys package org.microg.gms.droidguard`

#### Scenario: Retrieve GApps version
- **WHEN** the active provider is GApps and ADB is connected
- **THEN** the system returns the `versionName` from `dumpsys package com.google.android.gms`

#### Scenario: Version for None provider
- **WHEN** the active provider is None
- **THEN** the system returns `None` for the version field

### Requirement: Fallback to config-persisted state
The system SHALL fall back to the provider value stored in `GoogleServicesConfig` when ADB is not connected or the VM is not running, and SHALL mark the result as `cached` rather than `live`.

#### Scenario: ADB unavailable fallback
- **WHEN** a provider detection is requested but ADB is not connected
- **THEN** the system returns the provider from config with a `cached` freshness indicator

#### Scenario: Live detection freshness
- **WHEN** a provider detection succeeds via ADB
- **THEN** the system returns the provider with a `live` freshness indicator
