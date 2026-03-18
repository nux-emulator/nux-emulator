## 1. Module Structure and Error Types

- [x] 1.1 Create `nux-core/src/vm/` module directory with `mod.rs`, `error.rs`, `config.rs`, `detect.rs`, `command.rs`, `process.rs`, `control.rs` stub files and wire them into the `nux-core` crate root
- [x] 1.2 Define `VmError` enum in `error.rs` covering all failure modes: `KvmNotAvailable`, `KvmPermissionDenied`, `KvmUnsupportedVersion`, `MissingExtension`, `CpuFeatureMissing`, `CrosvmNotFound`, `CrosvmStartFailed`, `CrosvmCrashed`, `ConfigValidation`, `ControlSocket`, `Timeout`, `ProcessSignal`. Implement `std::error::Error` and `Display`.
- [x] 1.3 Define `VmState` enum (`Idle`, `Starting`, `Running`, `Paused`, `Stopping`, `Stopped`, `Crashed`, `Failed`) with valid transition checks. Write unit tests for allowed and disallowed transitions.

## 2. KVM Detection

- [x] 2.1 Implement `/dev/kvm` existence and permission check in `detect.rs`. Return typed errors for missing device vs permission denied. Test with mock file paths.
- [x] 2.2 Implement `KVM_GET_API_VERSION` ioctl check verifying version 12. Implement `KVM_CHECK_EXTENSION` checks for `KVM_CAP_IRQCHIP`, `KVM_CAP_USER_MEMORY`, `KVM_CAP_SET_TSS_ADDR`. Test with a helper that wraps the ioctl calls.
- [x] 2.3 Implement CPUID-based CPU feature detection for VT-x/AMD-V and EPT/NPT. Return error for missing virtualization, warning for missing EPT/NPT.
- [x] 2.4 Implement `KvmReadinessReport` struct and `check_kvm_readiness()` function that aggregates all detection checks into a single structured report with overall ready/not-ready status. Write integration test that runs on a real host.

## 3. VM Configuration

- [x] 3.1 Define `VmConfig` struct in `config.rs` with fields: `cpus`, `ram_mb`, `gpu` (sub-struct with `enabled`, `width`, `height`), `disks` (vec of `DiskConfig` with `path` and `readonly`), `kernel`, `boot_image`, `audio_enabled`, `network_tap`, `input_devices`, `control_socket_path`. Derive `Deserialize` for TOML.
- [x] 3.2 Implement `VmConfig::validate()` that checks: cpus >= 1, ram_mb >= 512, system image path exists, kernel path exists, boot image path exists. Return `VmError::ConfigValidation` with descriptive messages. Write unit tests for each validation rule.

## 4. crosvm Command Builder

- [x] 4.1 Implement `CrosvmCommand` struct in `command.rs` with `fn build(config: &VmConfig) -> Vec<OsString>` that produces the base args: `crosvm run --cpus N --mem MB --boot <boot.img> <kernel>`. Write unit test asserting correct arg order.
- [x] 4.2 Add GPU argument construction: `--gpu backend=gfxstream[,width=W,height=H]` when enabled, omitted when disabled. Write unit tests for enabled-with-resolution, enabled-default, and disabled cases.
- [x] 4.3 Add block device arguments: `--block path=<img>[,ro]` for each disk. Add `--socket`, `--sound` (when audio enabled), `--net` (when tap configured), `--input-ev` for each input device. Write unit tests for full and minimal configs.
- [x] 4.4 Add default control socket path logic: use configured path or fall back to `/run/user/<uid>/nux/control.sock`. Write unit test verifying default path generation.

## 5. crosvm Build Integration

- [x] 5.1 Create build script or Makefile target that compiles crosvm from `/build2/nux-emulator/crosvm/` with `--features gfxstream,audio,x`. Verify the output binary exists and is executable.
- [x] 5.2 Add binary verification step: invoke built crosvm with `--version`, assert exit code 0 and non-empty output. Add error handling for missing source tree.

## 6. Process Lifecycle

- [x] 6.1 Implement `VmProcess::spawn()` in `process.rs` using `tokio::process::Command`. Set `PR_SET_PDEATHSIG` to `SIGTERM` via `pre_exec`. Capture stdout/stderr. Write PID file to `/run/user/<uid>/nux/vm.pid`. Write test that spawns a mock process.
- [x] 6.2 Implement `VmProcess::monitor()` using `tokio::select!` that watches for process exit. On unexpected exit, capture stderr and exit code, emit crash event. On expected exit (after stop), clean up normally. Write test with a process that exits immediately.
- [x] 6.3 Implement `VmProcess::stop()` — send stop via control socket, wait up to configurable timeout, then SIGKILL if still running. Clean up PID file and socket. Write test with a mock process that responds to signals.
- [x] 6.4 Implement `VmProcess::force_kill()` — send SIGKILL immediately, clean up resources. Handle already-stopped case without error. Write unit tests for both running and stopped cases.
- [x] 6.5 Implement orphan detection on startup: read PID file, check if process exists via `kill(pid, 0)`, terminate if running, remove stale files. Write test with a stale PID file.

## 7. Control Socket Client

- [x] 7.1 Implement `ControlClient::connect()` in `control.rs` using `tokio::net::UnixStream`. Handle socket-not-found and connection-refused errors. Write test with a mock Unix socket listener.
- [x] 7.2 Implement `ControlClient::pause()`, `resume()`, `stop()` methods that send JSON commands and await acknowledgment with a 5-second timeout. Write tests for success and timeout cases.
- [x] 7.3 Implement `ControlClient::balloon_set(mb)` for dynamic memory adjustment. Validate size against configured RAM. Write test for valid and invalid sizes.
- [x] 7.4 Implement disconnection detection and auto-reconnect logic: detect broken pipe on send, attempt reconnect on next command. Write test simulating socket disconnection mid-session.

## 8. VmManager Orchestrator

- [x] 8.1 Implement `VmManager` in `mod.rs` that composes `VmConfig`, `CrosvmCommand`, `VmProcess`, `ControlClient`, and `VmState`. Expose public API: `start()`, `stop()`, `pause()`, `resume()`, `force_kill()`, `state()`. Wire state transitions through each operation.
- [x] 8.2 Implement `VmManager::start()` flow: validate config → check KVM readiness → build command → spawn process → connect control socket → set state to Running. Handle errors at each step with proper state transitions.
- [x] 8.3 Write integration test for `VmManager` full lifecycle: start → pause → resume → stop, verifying state at each step. Use a mock crosvm binary (simple shell script that responds to signals and creates a socket).

## 9. Verification

- [x] 9.1 Run `cargo check` and `cargo test` for the `nux-core` crate, fix any compilation errors or test failures
- [x] 9.2 Run `cargo clippy` on the `vm` module and address all warnings
- [x] 9.3 Verify all spec scenarios have corresponding test coverage by cross-referencing specs against test names
