## ADDED Requirements

### Requirement: Connect to crosvm control socket
The system SHALL connect to the crosvm control socket at the configured Unix socket path using an async `UnixStream`.

#### Scenario: Successful connection
- **WHEN** the system connects to the control socket and crosvm is running
- **THEN** the system SHALL establish a connection and be ready to send commands

#### Scenario: Socket does not exist
- **WHEN** the system attempts to connect and the socket file does not exist
- **THEN** the system SHALL return an error indicating the control socket is not available

#### Scenario: Connection refused
- **WHEN** the system attempts to connect and the socket exists but crosvm is not listening
- **THEN** the system SHALL return an error indicating the connection was refused

### Requirement: Send pause command
The system SHALL send a pause command to crosvm via the control socket and wait for acknowledgment.

#### Scenario: Pause command accepted
- **WHEN** the system sends a pause command and crosvm acknowledges it
- **THEN** the system SHALL return success

#### Scenario: Pause command times out
- **WHEN** the system sends a pause command and receives no response within 5 seconds
- **THEN** the system SHALL return a timeout error

### Requirement: Send resume command
The system SHALL send a resume command to crosvm via the control socket and wait for acknowledgment.

#### Scenario: Resume command accepted
- **WHEN** the system sends a resume command and crosvm acknowledges it
- **THEN** the system SHALL return success

#### Scenario: Resume command times out
- **WHEN** the system sends a resume command and receives no response within 5 seconds
- **THEN** the system SHALL return a timeout error

### Requirement: Send stop command
The system SHALL send a stop (shutdown) command to crosvm via the control socket to initiate graceful VM shutdown.

#### Scenario: Stop command accepted
- **WHEN** the system sends a stop command and crosvm acknowledges it
- **THEN** the system SHALL return success and the VM process SHALL begin shutting down

#### Scenario: Stop command fails
- **WHEN** the system sends a stop command and receives an error response
- **THEN** the system SHALL return the error details from crosvm

### Requirement: Send balloon memory command
The system SHALL send balloon memory adjustment commands to crosvm via the control socket to dynamically adjust guest memory.

#### Scenario: Balloon set succeeds
- **WHEN** the system sends a balloon command to set guest memory to a specific size in MB
- **THEN** crosvm SHALL adjust the guest balloon device and the system SHALL return success

#### Scenario: Balloon set with invalid size
- **WHEN** the system sends a balloon command with a size exceeding the VM's configured RAM
- **THEN** the system SHALL return an error indicating the requested size is invalid

### Requirement: Handle control socket disconnection
The system SHALL detect when the control socket connection is lost and report the disconnection.

#### Scenario: Socket disconnected during command
- **WHEN** the system sends a command and the socket connection is broken (crosvm crashed or socket removed)
- **THEN** the system SHALL return a disconnection error indicating the VM may have crashed

#### Scenario: Reconnect after disconnection
- **WHEN** the control socket was previously disconnected and a new command is issued
- **THEN** the system SHALL attempt to reconnect to the socket before sending the command
