## Why

Nux Emulator needs a VMM backend to actually run Android. crosvm is the chosen hypervisor — it's Rust-native, KVM-accelerated, and supports gfxstream GPU passthrough out of the box. Without this integration, nothing else (display, input, UI) has a VM to talk to. This is the foundational layer that everything else depends on.

## What Changes

- Build crosvm from source with feature flags: `gfxstream`, `audio`, `x` (X11 support), stored at `/build2/nux-emulator/crosvm/`
- New `nux-core::vm` module to spawn, manage, and communicate with the crosvm process
- crosvm command builder that maps Nux configuration (TOML) to the full crosvm CLI invocation (CPU, RAM, GPU, disks, networking, audio, input, display)
- VM lifecycle management: start, stop, pause, resume, force-kill with proper cleanup
- crosvm control socket integration (`/tmp/nux-control.sock`) for runtime commands (pause, resume, balloon, hotplug)
- KVM capability detection: check `/dev/kvm` availability, permissions, and required CPU features (VT-x/AMD-V, EPT/NPT)
- Error handling: crosvm crash detection, exit code interpretation, optional restart logic
- Health monitoring: track VM state, detect hangs or unexpected exits

## Non-goals

- Display pipeline (gfxstream rendering, window surface) — separate change
- Input routing (keyboard/mouse/gamepad to virtio-input) — separate change
- UI integration (GTK4 VM controls, status indicators) — separate change
- Android image building or AOSP customization
- Multi-instance VM support (deferred to v2)
- Network bridge/TAP device creation (handled by networking change)

## Capabilities

### New Capabilities
- `kvm-detection`: Detect KVM availability, permissions, and CPU virtualization features
- `crosvm-build`: Build crosvm from source with required feature flags
- `crosvm-command`: Construct crosvm CLI invocations from Nux configuration
- `vm-lifecycle`: Spawn, stop, pause, resume, and force-kill the crosvm process
- `vm-control-socket`: Communicate with running crosvm via its control socket

### Modified Capabilities
_(none — no existing specs to modify)_

## Impact

- **New crate dependency**: Adds build-time dependency on crosvm source tree at `/build2/nux-emulator/crosvm/`
- **Code**: New `nux-core::vm` module with submodules for detection, command building, lifecycle, and control socket
- **System requirements**: Requires `/dev/kvm` access, Linux kernel with KVM enabled, user in `kvm` group
- **Dependencies**: `tokio` (async process management), `serde` (config deserialization), `nix` (Unix socket/signal handling)
- **Config**: New `[vm]` section in Nux TOML config for CPU, RAM, GPU, disk, and device settings
