## Context

Nux currently spawns a crosvm VM (via the `crosvm-integration` change) but the guest has no network connectivity and no audio output. Without networking, apps cannot access the internet, and ADB over TCP is unavailable. Without audio, gaming is a non-starter. Both are hard requirements before Nux can be used for real workloads.

crosvm supports virtio-net (TAP-backed) and has experimental passt integration for userspace networking. For audio, crosvm provides virtio-snd which can route to ALSA — and on modern Linux desktops, PipeWire or PulseAudio sit on top of ALSA.

Constraints:
- Must work on both Wayland and X11 (audio stack is display-server-independent, so no issue)
- TAP networking requires one-time root setup; passt does not
- crosvm must be built with the `audio` feature flag
- Nux forbids `unsafe` code — all interaction with crosvm is via process spawning and CLI args

## Goals / Non-Goals

**Goals:**
- Provide reliable internet access inside the Android guest via TAP (primary) or passt (fallback)
- Enable ADB TCP connections via port forwarding
- Route guest audio to the host with latency acceptable for gaming (<50ms round-trip target)
- Expose volume control in the Nux UI toolbar

**Non-Goals:**
- Bluetooth audio, microphone input, WiFi emulation, VPN passthrough (see proposal)
- Custom audio mixing or DSP — rely on host PipeWire/PulseAudio entirely
- Network traffic shaping or bandwidth limiting

## Decisions

### 1. TAP primary, passt fallback for networking

**Choice**: Use TAP + bridge as the default network backend; fall back to passt when TAP is unavailable or the user hasn't run the setup script.

**Alternatives considered**:
- *passt-only*: Simpler (no root), but TAP gives better throughput and lower latency — important for online gaming.
- *slirp*: Deprecated in QEMU ecosystem, poor performance, no upstream crosvm support.

**Rationale**: TAP is the gold standard for VM networking performance. The one-time sudo setup is acceptable since Nux already requires KVM access. passt as fallback ensures a zero-config first-run experience.

### 2. Network setup via helper script

**Choice**: Provide `scripts/setup-network.sh` that creates a persistent bridge (`nux-br0`) and configures NAT/forwarding. Run once with sudo, persists across reboots via systemd-networkd drop-in.

**Rationale**: Keeps privilege escalation out of the Rust codebase. The script is auditable and idempotent. nux-core detects whether the bridge exists at runtime and selects the backend accordingly.

### 3. Port forwarding for ADB

**Choice**: When using TAP, the guest gets a known static IP on the bridge subnet (e.g., `192.168.100.2`). ADB connects directly. When using passt, configure passt's `--forward` flag to map host port 5555 → guest 5555.

**Rationale**: ADB TCP is the primary debug/install channel. Both backends need a reliable path.

### 4. crosvm virtio-snd with ALSA backend

**Choice**: Pass `--virtio-snd` to crosvm, which exposes a virtio sound device to the guest. crosvm's ALSA backend connects to the host, where PipeWire/PulseAudio provides the actual ALSA interface.

**Alternatives considered**:
- *PulseAudio backend in crosvm*: Exists but less maintained; ALSA-over-PipeWire is the standard path on modern distros.
- *virtio-snd with custom socket*: Unnecessary complexity for v1.

**Rationale**: This is the simplest path — crosvm already supports it, and PipeWire's ALSA compatibility layer is mature. No custom audio code needed in nux-core beyond passing the right flags and managing configuration.

### 5. Audio latency optimization

**Choice**: Configure crosvm with a small period size (256 frames) and 2 periods. Measure round-trip latency at startup and log it. If latency exceeds 80ms, warn the user in the UI.

**Rationale**: Gaming audio needs to feel responsive. The default ALSA buffer sizes in crosvm are tuned for ChromeOS, not desktop Linux. We tune aggressively and surface problems rather than silently degrading.

### 6. Volume control in UI

**Choice**: Add a volume button to the `nux-ui` toolbar that controls the crosvm virtio-snd stream volume via crosvm's control socket. Mute/unmute toggle + slider.

**Rationale**: Users expect volume control in the emulator window. Routing through crosvm's control socket avoids needing to manipulate host PulseAudio streams directly.

## Risks / Trade-offs

- **[passt availability]** → passt is not installed by default on all distros. Mitigation: detect at startup, show clear error with install instructions if neither TAP nor passt is available.
- **[Audio latency variance]** → Different host audio stacks (PipeWire vs PulseAudio vs bare ALSA) may behave differently. Mitigation: measure and warn; document recommended PipeWire config.
- **[TAP bridge conflicts]** → User may have existing bridges or firewall rules that conflict. Mitigation: setup script checks for conflicts; nux-core validates bridge state before use.
- **[crosvm audio feature flag]** → crosvm must be compiled with `--features audio`. Mitigation: enforce this in the build system; fail fast with a clear error if the feature is missing.

## Open Questions

1. Should we support PulseAudio-only systems (no PipeWire) as a first-class target, or document PipeWire as recommended?
2. What is the exact crosvm control socket API for volume adjustment — need to verify this exists or if we need to contribute it upstream.
3. Should the setup script support NetworkManager in addition to systemd-networkd?
