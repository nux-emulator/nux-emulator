## ADDED Requirements

### Requirement: TCP transport connection
The system SHALL connect to the Android guest's `adbd` over TCP using the virtual network interface configured by crosvm. The default target SHALL be the guest IP on port 5555.

#### Scenario: Successful TCP connection
- **WHEN** the VM is running and `adbd` is listening on TCP port 5555
- **THEN** the ADB client SHALL establish a TCP connection and complete the ADB protocol handshake

#### Scenario: TCP connection refused
- **WHEN** the ADB client attempts a TCP connection and the guest is not yet listening
- **THEN** the ADB client SHALL retry with exponential backoff (starting at 500ms, max 10s) until a configurable timeout is reached

### Requirement: Virtio-serial fallback transport
The system SHALL support connecting to `adbd` via a virtio-serial port as a fallback when TCP transport is unavailable or explicitly configured.

#### Scenario: Fallback to virtio-serial
- **WHEN** TCP connection fails after the configured timeout and a virtio-serial port is available
- **THEN** the ADB client SHALL attempt connection over the virtio-serial channel

#### Scenario: Virtio-serial direct mode
- **WHEN** the user configures virtio-serial as the preferred transport in the TOML config
- **THEN** the ADB client SHALL use virtio-serial directly without attempting TCP first

### Requirement: ADB protocol handshake
The system SHALL implement the ADB CONNECT handshake (CNXN message exchange) to establish an authenticated session with `adbd`.

#### Scenario: Successful handshake
- **WHEN** a transport connection is established (TCP or virtio-serial)
- **THEN** the ADB client SHALL send a CNXN message and wait for the guest's CNXN response, establishing a valid session

#### Scenario: Handshake timeout
- **WHEN** the guest does not respond to the CNXN message within 5 seconds
- **THEN** the ADB client SHALL close the transport and retry the connection

### Requirement: Auto-reconnect on connection loss
The system SHALL automatically reconnect to `adbd` if the connection drops while the VM is still running.

#### Scenario: Connection drops during operation
- **WHEN** the ADB connection is lost unexpectedly (e.g., `adbd` restarts)
- **THEN** the ADB client SHALL attempt to reconnect with exponential backoff and report the disconnection state to observers

#### Scenario: VM shutdown
- **WHEN** the VM is shutting down
- **THEN** the ADB client SHALL close the connection gracefully without attempting reconnection

### Requirement: Connection state observability
The system SHALL expose the current ADB connection state so that the UI layer can reflect connectivity status.

#### Scenario: State transitions reported
- **WHEN** the ADB connection state changes (disconnected, connecting, connected, error)
- **THEN** the ADB client SHALL emit the new state on an observable channel that the UI can subscribe to

#### Scenario: Query current state
- **WHEN** a caller queries the ADB client's connection state
- **THEN** the ADB client SHALL return the current state synchronously without blocking
