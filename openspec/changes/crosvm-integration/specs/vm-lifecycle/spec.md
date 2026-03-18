## ADDED Requirements

### Requirement: Start VM process
The system SHALL spawn crosvm as a child process using the constructed command arguments, capturing stdout and stderr for logging.

#### Scenario: Successful VM start
- **WHEN** the system starts the VM with a valid configuration and KVM is available
- **THEN** the system SHALL spawn the crosvm process, record its PID, write a PID file, and transition the VM state to `Running`

#### Scenario: crosvm binary not found
- **WHEN** the system attempts to start the VM and the crosvm binary is not found at the expected path
- **THEN** the system SHALL return an error indicating the crosvm binary is missing

#### Scenario: crosvm fails to start
- **WHEN** the system spawns crosvm and it exits immediately with a non-zero exit code
- **THEN** the system SHALL capture stderr, return an error with the exit code and stderr content, and set the VM state to `Failed`

### Requirement: Stop VM gracefully
The system SHALL stop the VM by first sending a shutdown command via the control socket, then waiting up to a configurable timeout (default 10 seconds) for the process to exit.

#### Scenario: Graceful shutdown succeeds
- **WHEN** the system sends a stop command and crosvm exits within the timeout
- **THEN** the system SHALL clean up the PID file and control socket, and set the VM state to `Stopped`

#### Scenario: Graceful shutdown times out
- **WHEN** the system sends a stop command and crosvm does not exit within the timeout
- **THEN** the system SHALL send SIGKILL to the process, clean up resources, and set the VM state to `Stopped`

### Requirement: Force-kill VM
The system SHALL provide a force-kill operation that immediately sends SIGKILL to the crosvm process without attempting graceful shutdown.

#### Scenario: Force-kill running VM
- **WHEN** the system force-kills a running VM
- **THEN** the system SHALL send SIGKILL, clean up the PID file and control socket, and set the VM state to `Stopped`

#### Scenario: Force-kill already stopped VM
- **WHEN** the system force-kills a VM that is not running
- **THEN** the system SHALL return without error and ensure cleanup of any stale resources

### Requirement: Pause and resume VM
The system SHALL support pausing and resuming the VM via the crosvm control socket.

#### Scenario: Pause a running VM
- **WHEN** the system pauses a VM in the `Running` state
- **THEN** the system SHALL send the pause command via the control socket and set the VM state to `Paused`

#### Scenario: Resume a paused VM
- **WHEN** the system resumes a VM in the `Paused` state
- **THEN** the system SHALL send the resume command via the control socket and set the VM state to `Running`

#### Scenario: Pause a non-running VM
- **WHEN** the system attempts to pause a VM that is not in the `Running` state
- **THEN** the system SHALL return an error indicating the VM is not running

#### Scenario: Resume a non-paused VM
- **WHEN** the system attempts to resume a VM that is not in the `Paused` state
- **THEN** the system SHALL return an error indicating the VM is not paused

### Requirement: Monitor VM process for unexpected exit
The system SHALL continuously monitor the crosvm process and detect unexpected exits (crashes).

#### Scenario: crosvm crashes during operation
- **WHEN** the crosvm process exits unexpectedly while the VM state is `Running` or `Paused`
- **THEN** the system SHALL capture the exit code and any stderr output, set the VM state to `Crashed`, clean up resources, and emit a crash event with diagnostic details

#### Scenario: crosvm exits normally after stop command
- **WHEN** the crosvm process exits with code 0 after a stop command was issued
- **THEN** the system SHALL treat this as a normal shutdown and set the VM state to `Stopped`

### Requirement: Track VM state
The system SHALL maintain a state machine for the VM with states: `Idle`, `Starting`, `Running`, `Paused`, `Stopping`, `Stopped`, `Crashed`, `Failed`.

#### Scenario: State transitions on successful start
- **WHEN** the VM is started successfully
- **THEN** the state SHALL transition from `Idle` → `Starting` → `Running`

#### Scenario: State transitions on graceful stop
- **WHEN** a running VM is stopped gracefully
- **THEN** the state SHALL transition from `Running` → `Stopping` → `Stopped`

#### Scenario: State transitions on crash
- **WHEN** a running VM crashes
- **THEN** the state SHALL transition from `Running` → `Crashed`

#### Scenario: Query current state
- **WHEN** any component queries the VM state
- **THEN** the system SHALL return the current state without blocking

### Requirement: Clean up orphaned crosvm processes on startup
The system SHALL check for orphaned crosvm processes (from a previous Nux crash) on startup by reading the PID file.

#### Scenario: Orphaned process found
- **WHEN** the system starts and finds a PID file with a running crosvm process that it did not spawn
- **THEN** the system SHALL terminate the orphaned process, remove the stale PID file and socket, and log a warning

#### Scenario: Stale PID file with no running process
- **WHEN** the system starts and finds a PID file but no process with that PID exists
- **THEN** the system SHALL remove the stale PID file and socket

### Requirement: Set child process death signal
The system SHALL set `PR_SET_PDEATHSIG` to `SIGTERM` on the crosvm child process so that it receives SIGTERM if the parent Nux process dies unexpectedly.

#### Scenario: Parent process dies
- **WHEN** the Nux parent process is killed unexpectedly
- **THEN** the crosvm child process SHALL receive SIGTERM and begin shutting down
