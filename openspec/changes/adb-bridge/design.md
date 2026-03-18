## Context

Nux Emulator needs to communicate with the Android guest for app management, file transfer, and device queries. The Android Debug Bridge (ADB) protocol is the standard mechanism for host-guest communication on Android. The guest runs `adbd` (ADB daemon) which listens for connections.

Currently, crosvm-integration provides VM lifecycle management and configures guest networking (virtual TAP device) and virtio-serial ports. The ADB bridge builds on top of these transport channels to provide a high-level Rust API consumed by both `nux-core` internals and `nux-ui`.

There is no existing ADB client code in the codebase. The Android guest image ships with `adbd` enabled by default.

## Goals / Non-Goals

**Goals:**
- Implement a pure-Rust ADB client library within `nux-core::adb` — no dependency on the external `adb` binary
- Support TCP transport (over crosvm's virtual network) as the primary mode, with virtio-serial as a fallback
- Provide async APIs for all operations so the GTK4 UI thread is never blocked
- Handle connection lifecycle: connect, reconnect on failure, health checks, graceful disconnect on VM shutdown
- Expose a clean public API surface that `nux-ui` can consume for drag-and-drop APK install, file browsing, and device info display

**Non-Goals:**
- Full ADB protocol implementation — only the subset needed for Nux operations (no port forwarding, logcat streaming, jdwp, etc.)
- ADB server mode — Nux acts as a direct client to `adbd`, not as an ADB server
- Multi-device support — single VM instance only
- USB transport — not applicable for a virtual machine

## Decisions

### 1. Pure-Rust ADB client vs. shelling out to `adb` binary

**Decision:** Implement ADB protocol directly in Rust.

**Rationale:** Shelling out to `adb` introduces an external dependency users must install, adds process management complexity, requires parsing text output, and makes error handling fragile. A direct implementation gives us typed responses, proper error propagation, and no external dependencies.

**Alternatives considered:**
- Shell out to `adb` CLI — simpler initially but brittle, requires `adb` on PATH, text parsing is error-prone
- Use an existing Rust ADB crate — no mature crate exists that covers our needs; the protocol subset we need is small enough to implement directly

### 2. TCP as primary transport, virtio-serial as fallback

**Decision:** Default to TCP over the virtual network (guest IP on a known port, typically 5555). Fall back to virtio-serial if TCP is unavailable.

**Rationale:** TCP transport is simpler to implement and debug — it's standard sockets. The virtual network is already configured by crosvm-integration. Virtio-serial provides a direct channel that doesn't depend on guest networking being fully up, making it useful as a fallback during early boot or if the guest network stack has issues.

**Alternatives considered:**
- Virtio-serial only — more reliable but harder to debug, and some ADB protocol features assume stream sockets
- TCP only — simpler but no fallback if guest networking is slow to initialize

### 3. Async API with tokio

**Decision:** Use `tokio` for async I/O in the ADB module. Expose `async fn` APIs.

**Rationale:** ADB operations (especially file transfers and app installs) are I/O-bound and can take seconds to minutes. Blocking the main thread is unacceptable in a GTK4 app. The rest of nux-core already uses tokio for VM process management, so this aligns with existing patterns.

**Alternatives considered:**
- Synchronous API + spawn threads in UI layer — pushes complexity to callers, inconsistent with codebase direction
- `async-std` — tokio is already a workspace dependency

### 4. Module structure

**Decision:** Organize as `nux-core/src/adb/` with submodules:
- `mod.rs` — public API re-exports, `AdbClient` struct
- `protocol.rs` — ADB protocol message framing, AUTH, OPEN, WRITE, CLOSE
- `transport.rs` — TCP and virtio-serial transport abstraction (`AdbTransport` trait)
- `shell.rs` — shell command execution, output parsing
- `sync.rs` — file sync protocol (PUSH/PULL), the ADB sync sub-protocol
- `types.rs` — shared types: `PackageInfo`, `DeviceInfo`, `AdbError`

**Rationale:** Keeps protocol details internal while exposing a clean `AdbClient` API. The transport trait allows swapping TCP/virtio-serial without changing higher-level code.

### 5. Connection management strategy

**Decision:** `AdbClient` owns the connection and implements auto-reconnect with exponential backoff. Connection state is observable via a channel so the UI can show connection status.

**Rationale:** The VM may take variable time to boot `adbd`. The UI needs to know when ADB is ready (e.g., to enable the "Install APK" button). Exponential backoff prevents busy-looping during boot.

## Risks / Trade-offs

- **ADB protocol complexity** → We only implement the subset we need (shell, sync, install). The protocol is well-documented and the subset is small. Risk is low but we should add integration tests against a real guest early.

- **virtio-serial reliability** → virtio-serial ADB transport is less commonly used than TCP. May encounter edge cases. → Mitigation: TCP is primary; virtio-serial is opt-in fallback. Can be disabled in config.

- **Guest `adbd` not starting** → If the Android image has issues, ADB won't connect. → Mitigation: Connection health monitoring with clear error reporting to the UI. Timeout after configurable period.

- **Large file transfers** → Pushing/pulling large files (multi-GB) could be slow or consume memory. → Mitigation: Stream-based transfer with progress reporting. Never buffer entire files in memory.

- **Protocol version drift** → Future Android versions might change ADB protocol details. → Mitigation: Target ADB protocol v1 which has been stable for years. AOSP 16 uses this version.

## Open Questions

1. Should we expose ADB connection settings (port, transport preference) in the user-facing config, or keep it internal with sensible defaults?
2. What is the exact virtio-serial port name/path that crosvm-integration will configure for ADB? Need to coordinate with that change.
3. Should screenshot capture return raw bytes or write to a temp file? UI layer preference may dictate this.
