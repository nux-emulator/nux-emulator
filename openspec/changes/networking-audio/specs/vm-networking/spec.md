## ADDED Requirements

### Requirement: TAP network backend
The system SHALL create and configure a TAP device connected to the `nux-br0` bridge when the bridge is detected on the host. The TAP device SHALL be passed to crosvm as the virtio-net backend, providing the guest with full internet connectivity.

#### Scenario: TAP backend selected when bridge exists
- **WHEN** the host has a `nux-br0` bridge interface configured
- **THEN** nux-core SHALL create a TAP device, attach it to `nux-br0`, and pass it to crosvm via `--tap` argument

#### Scenario: Guest obtains network connectivity via TAP
- **WHEN** the VM boots with the TAP backend active
- **THEN** the Android guest SHALL be able to resolve DNS names and reach the public internet

### Requirement: Passt fallback network backend
The system SHALL fall back to passt userspace networking when the TAP bridge (`nux-br0`) is not available and the `passt` binary is found on `$PATH`. nux-core SHALL spawn the passt process and configure crosvm to use the passt socket.

#### Scenario: Passt fallback when no TAP bridge
- **WHEN** `nux-br0` does not exist and `passt` is on `$PATH`
- **THEN** nux-core SHALL spawn a passt process and pass its socket path to crosvm for networking

#### Scenario: Passt provides internet access without root
- **WHEN** the VM boots with the passt backend active
- **THEN** the Android guest SHALL be able to resolve DNS names and reach the public internet without any prior sudo setup

### Requirement: No network backend error
The system SHALL fail fast with a clear, actionable error message when neither TAP nor passt is available.

#### Scenario: Neither TAP nor passt available
- **WHEN** `nux-br0` does not exist and `passt` is not found on `$PATH`
- **THEN** nux-core SHALL return an error indicating that networking is unavailable and provide instructions for setting up TAP (via `setup-network.sh`) or installing passt

### Requirement: DNS configuration
The system SHALL configure DNS forwarding so the Android guest can resolve hostnames. For TAP, DNS SHALL be provided via the bridge's DHCP/NAT configuration. For passt, DNS SHALL be forwarded automatically by passt.

#### Scenario: DNS resolution works with TAP backend
- **WHEN** the guest is connected via TAP
- **THEN** the guest SHALL successfully resolve public DNS names (e.g., `dns.google`)

#### Scenario: DNS resolution works with passt backend
- **WHEN** the guest is connected via passt
- **THEN** the guest SHALL successfully resolve public DNS names

### Requirement: Port forwarding for ADB TCP
The system SHALL configure port forwarding to allow ADB TCP connections from the host to the guest on port 5555.

#### Scenario: ADB connection via TAP
- **WHEN** the TAP backend is active
- **THEN** ADB SHALL be able to connect to the guest using the guest's static IP address (`192.168.100.2`) on port 5555

#### Scenario: ADB connection via passt
- **WHEN** the passt backend is active
- **THEN** passt SHALL forward host port 5555 to guest port 5555, and ADB SHALL be able to connect via `localhost:5555`

### Requirement: Network setup helper script
The system SHALL provide `scripts/setup-network.sh` that creates the `nux-br0` bridge, configures NAT and IP forwarding, and persists the configuration across reboots. The script SHALL be idempotent.

#### Scenario: First-time bridge setup
- **WHEN** a user runs `sudo scripts/setup-network.sh` and no `nux-br0` bridge exists
- **THEN** the script SHALL create `nux-br0`, configure NAT/forwarding, and persist the configuration via systemd-networkd

#### Scenario: Idempotent re-run
- **WHEN** a user runs `sudo scripts/setup-network.sh` and `nux-br0` already exists
- **THEN** the script SHALL verify the configuration is correct and exit successfully without duplicating rules
