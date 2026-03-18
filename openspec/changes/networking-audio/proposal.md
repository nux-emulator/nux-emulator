## Why

The Android VM currently runs without network connectivity or audio output, making it unusable for gaming or general app usage. Network access is required for app stores, online games, and ADB TCP connections. Audio output is essential for any gaming experience. These are blocking prerequisites before Nux can be tested with real workloads.

## What Changes

- Add TAP-based networking with bridge configuration for full internet access (requires one-time sudo setup via `scripts/setup-network.sh`)
- Add passt userspace networking as a rootless fallback (spawns passt process, connects via socket)
- Configure DNS forwarding so the guest resolves hostnames correctly
- Implement port forwarding to enable ADB TCP connections to the guest
- Enable crosvm's virtio-snd device for audio output routed to the host's PipeWire/PulseAudio
- Measure and optimize audio latency for acceptable gaming performance
- Expose volume control in the Nux UI toolbar

## Non-goals

- Bluetooth audio passthrough
- Microphone input (deferred to v2)
- WiFi emulation (TAP/passt provides connectivity at the network layer)
- VPN passthrough

## Capabilities

### New Capabilities

- `vm-networking`: TAP and passt network backends, DNS configuration, port forwarding, and network helper scripts
- `vm-audio`: virtio-snd audio output, host audio integration (PipeWire/PulseAudio), latency optimization, and UI volume control

### Modified Capabilities

_None — no existing spec-level requirements change._

## Impact

- **nux-core**: New `network` and `audio` modules; new dependencies on TAP/passt configuration and crosvm audio flags
- **nux-ui**: Volume control button added to toolbar
- **scripts/**: New `setup-network.sh` for one-time TAP/bridge setup
- **Dependencies**: crosvm must be built with `audio` feature; passt binary expected on `$PATH` for fallback mode
- **Depends on**: `crosvm-integration` change (VM must be launchable before networking/audio can be wired)
