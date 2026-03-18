## ADDED Requirements

### Requirement: Push file to guest
The system SHALL transfer a file from the host filesystem to a specified path on the Android guest using the ADB sync PUSH protocol.

#### Scenario: Successful file push
- **WHEN** the caller provides a valid host file path and a target guest path
- **THEN** the system SHALL stream the file to the guest and return success with the number of bytes transferred

#### Scenario: Host file not found
- **WHEN** the caller provides a host path that does not exist
- **THEN** the system SHALL return an error indicating the source file was not found, without contacting the guest

#### Scenario: Guest path not writable
- **WHEN** the file is pushed but the guest target path is not writable (e.g., permission denied)
- **THEN** the system SHALL return an error containing the failure reason from the guest

#### Scenario: Push progress reporting
- **WHEN** a file is being pushed to the guest
- **THEN** the system SHALL report transfer progress (bytes transferred / total bytes) via a progress callback or channel

#### Scenario: Large file push
- **WHEN** a file larger than 100 MB is pushed
- **THEN** the system SHALL stream the file in chunks without buffering the entire file in memory

### Requirement: Pull file from guest
The system SHALL transfer a file from the Android guest to a specified path on the host filesystem using the ADB sync PULL protocol.

#### Scenario: Successful file pull
- **WHEN** the caller provides a valid guest file path and a target host path
- **THEN** the system SHALL stream the file from the guest to the host and return success with the number of bytes transferred

#### Scenario: Guest file not found
- **WHEN** the caller provides a guest path that does not exist
- **THEN** the system SHALL return an error indicating the source file was not found on the guest

#### Scenario: Pull progress reporting
- **WHEN** a file is being pulled from the guest
- **THEN** the system SHALL report transfer progress (bytes transferred / total bytes) via a progress callback or channel

#### Scenario: Large file pull
- **WHEN** a file larger than 100 MB is pulled
- **THEN** the system SHALL stream the file in chunks without buffering the entire file in memory
