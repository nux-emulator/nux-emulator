## Context

Nux Emulator currently has no VM backend. The `nux-core` crate needs a `vm` module that can spawn crosvm as a child process, construct its CLI from user configuration, manage its lifecycle, and communicate with it at runtime via the control socket.

crosvm is a Chrome OS VMM written in Rust. We use it as an external process (not a library) because it's designed to run as a standalone binary with a well-defined CLI and control socket interface. The crosvm source lives at `/build2/nux-emulator/crosvm/` and must be built with specific feature flags for our GPU and audio needs.

The user's Nux configuration (TOML) drives everything — CPU count, RAM, disk images, GPU settings, audio, networking. This module translates that config into a crosvm invocation and manages the process for its entire lifetime.

## Goals / Non-Goals

**Goals:**
- Reliable detection of KVM capability before attempting to start a VM
- Deterministic, reproducible crosvm command construction from TOML config
- Clean process lifecycle with proper signal handling and resource cleanup
- Runtime control via crosvm's Unix socket (pause, resume, stop, balloon)
- Clear error reporting when crosvm fails (exit codes, stderr capture)

**Non-Goals:**
- Rendering pipeline or display surface management
- Input device routing to virtio-input
- TAP device creation or network bridge setup (assumes pre-configured `nux0`)
- GTK4 UI bindings for VM state
- Multi-instance (v2)

## Decisions

### 1. crosvm as external process, not linked library

**Choice:** Spawn crosvm as a child process via `tokio::process::Command`.

**Alternatives considered:**
- Linking crosvm as a Rust library crate: Would give tighter integration but crosvm's internal APIs are unstable and not designed for embedding. The CLI and control socket are the stable interfaces.
- Using QEMU instead: More mature but C-based, heavier, and lacks native gfxstream integration. crosvm is purpose-built for Android VMs.

**Rationale:** Process isolation gives us crash containment, simpler upgrades, and uses crosvm's supported interface. The control socket provides all the runtime interaction we need.

### 2. Module structure under `nux-core::vm`

```
nux-core/src/vm/
├── mod.rs          // VmManager: top-level orchestrator
├── detect.rs       // KVM detection and capability checks
├── command.rs      // CrosvmCommand builder
├── process.rs      // Process spawning, monitoring, signal handling
├── control.rs      // Control socket client
├── config.rs       // VmConfig: typed config mapping from TOML
└── error.rs        // VmError enum
```

**Rationale:** Each concern is isolated and independently testable. `VmManager` composes them into the public API.

### 3. Typed config with `VmConfig` struct

**Choice:** Define a `VmConfig` struct that deserializes from the `[vm]` TOML section, with validation at parse time. `CrosvmCommand` consumes `VmConfig` to produce `Vec<OsString>` args.

**Alternatives considered:**
- Passing raw TOML values through: Error-prone, no validation until crosvm rejects the args.
- Builder pattern without config struct: Harder to serialize/deserialize and test.

**Rationale:** Type-safe config catches errors early. Separating config parsing from command building makes both testable in isolation.

### 4. Control socket via `UnixStream`

**Choice:** Use `tokio::net::UnixStream` to send JSON commands to crosvm's control socket at `/tmp/nux-control.sock`.

crosvm's socket protocol accepts JSON-formatted commands. We'll wrap this in a `ControlClient` that provides typed methods: `pause()`, `resume()`, `stop()`, `balloon_set(mb)`.

**Rationale:** This is crosvm's official runtime control interface. No need to reinvent — just wrap it with proper error handling and timeouts.

### 5. Process monitoring with `tokio::select!`

**Choice:** After spawning crosvm, monitor it with a `tokio::select!` loop that watches for:
- Process exit (expected or crash)
- Shutdown signal from Nux
- Health check timeouts

On unexpected exit, capture stderr, parse the exit code, and report a structured `VmError`. No automatic restart by default — the UI layer decides whether to retry.

**Rationale:** Automatic restart is dangerous for a VM (could corrupt disk images). Better to surface the error and let the user/UI decide.

### 6. KVM detection strategy

**Choice:** Multi-step check in `detect.rs`:
1. `/dev/kvm` exists and is accessible (file permissions)
2. `KVM_GET_API_VERSION` ioctl returns expected version
3. `KVM_CHECK_EXTENSION` for required capabilities (e.g., `KVM_CAP_IRQCHIP`, `KVM_CAP_USER_MEMORY`)
4. CPUID check for VT-x/AMD-V and EPT/NPT

**Rationale:** Failing fast with a clear message ("KVM not available: add your user to the `kvm` group") is far better than a cryptic crosvm crash.

## Risks / Trade-offs

- **crosvm CLI instability** → Pin to a specific crosvm commit/tag. Track upstream changes in a dedicated update process.
- **Control socket path conflicts** → Use a unique socket path per instance (even for v1, to avoid stale sockets): `/run/user/<uid>/nux/control.sock`.
- **Build complexity** → crosvm has many dependencies (minijail, etc.). Document the build process thoroughly and cache build artifacts in CI.
- **Process zombie risk** → If Nux crashes, crosvm may keep running. Use a PID file and check for orphaned processes on startup. Set `prctl(PR_SET_PDEATHSIG)` on the child so it receives SIGTERM when the parent dies.
- **Disk image corruption on force-kill** → Always attempt graceful shutdown via control socket first. Force-kill only after a timeout (default 10s). Document that `userdata.img` should use a journaling filesystem.

## Open Questions

1. **crosvm version pinning**: Which commit/tag of crosvm should we target for the initial build? Need to verify gfxstream compatibility with our AOSP 16 images.
2. **Socket path**: Should we use `/run/user/<uid>/nux/` (XDG-compliant) or `/tmp/nux-<pid>/`? Leaning toward the former.
3. **Restart policy**: Should `VmManager` expose a configurable restart policy, or keep it strictly manual for v1?
