## 1. Module Structure and Types

- [x] 1.1 Create `nux-core/src/adb/` module directory with `mod.rs`, `types.rs`, `protocol.rs`, `transport.rs`, `shell.rs`, `sync.rs` stub files. Wire up `pub mod adb` in `nux-core/src/lib.rs`. Verify: `cargo check -p nux-core` passes.
- [x] 1.2 Define shared types in `types.rs`: `AdbError` enum (with `ConnectionRefused`, `Timeout`, `ProtocolError`, `GuestError(String)`, `IoError`), `ConnectionState` enum (`Disconnected`, `Connecting`, `Connected`, `Error(String)`), `PackageInfo`, `DeviceInfo`, `TransportKind` enum (`Tcp`, `VirtioSerial`). Verify: types compile and `AdbError` implements `std::error::Error`.

## 2. ADB Protocol Layer

- [x] 2.1 Implement ADB message framing in `protocol.rs`: `AdbMessage` struct (command, arg0, arg1, data), serialization to/from bytes, command constants (`CNXN`, `OPEN`, `OKAY`, `CLSE`, `WRTE`, `AUTH`), and message checksum calculation. Verify: unit tests round-trip serialize/deserialize messages correctly.
- [x] 2.2 Implement CNXN handshake logic in `protocol.rs`: send CNXN with host banner, parse guest CNXN response, extract max payload size and device banner. Verify: unit test with mock stream completes handshake.
- [x] 2.3 Implement stream multiplexing in `protocol.rs`: OPEN/OKAY/WRTE/CLSE message handling for managing multiple logical streams over a single ADB connection. Track local and remote stream IDs. Verify: unit test opens a stream, writes data, and closes it against a mock transport.

## 3. Transport Layer

- [x] 3.1 Define `AdbTransport` trait in `transport.rs` with async `connect`, `read_message`, `write_message`, and `close` methods. Implement `TcpTransport` that connects to a guest IP:port via `tokio::net::TcpStream`. Verify: compiles, unit test connects to a local TCP listener.
- [x] 3.2 Implement `VirtioSerialTransport` in `transport.rs` that opens a virtio-serial device path (e.g., `/dev/virtio-ports/adb`) via async file I/O. Verify: compiles, unit test opens a mock file descriptor.
- [x] 3.3 Implement transport selection logic: try TCP first, fall back to virtio-serial on failure, respect config override for preferred transport. Verify: unit test with TCP failure triggers virtio-serial fallback.

## 4. Connection Management

- [x] 4.1 Implement `AdbClient` struct in `mod.rs` with `connect()` async method that selects transport, establishes connection, and completes the CNXN handshake. Expose `state()` method returning current `ConnectionState`. Verify: integration test creates `AdbClient` and reports `Disconnected` before connect.
- [x] 4.2 Implement auto-reconnect with exponential backoff (500ms initial, 10s max) in `AdbClient`. Spawn a background tokio task that monitors connection health and reconnects on drop. Stop reconnection when `disconnect()` is called. Verify: unit test simulates connection drop and observes reconnect attempts with increasing delays.
- [x] 4.3 Implement `ConnectionState` observable channel using `tokio::sync::watch`. Emit state transitions (`Disconnected` → `Connecting` → `Connected`, or `Error`). Verify: unit test subscribes to channel and receives state transitions.

## 5. Shell Command Execution

- [x] 5.1 Implement `shell_exec` in `shell.rs`: open an ADB shell stream, send command, collect stdout, parse exit code from shell protocol v2 (or trailing return code). Support caller-specified timeout via `tokio::time::timeout`. Verify: unit test with mock transport executes `echo hello` and returns output.
- [x] 5.2 Implement `DeviceInfo` query in `shell.rs`: `get_device_info()` that runs `getprop` commands for `ro.build.version.release`, `ro.build.version.sdk`, `ro.product.model`, `ro.product.cpu.abilist`. Return `DeviceInfo` struct with `Option<String>` fields for missing properties. Verify: unit test with mock shell output parses all fields correctly, including missing ones.
- [x] 5.3 Implement screen resolution query in `shell.rs`: `get_screen_resolution()` that runs `wm size`, parses output (handling both physical and override sizes), returns `(width, height)`. Verify: unit test parses `Physical size: 1080x1920` and `Override size: 720x1280` formats.

## 6. Screenshot Capture

- [x] 6.1 Implement `capture_screenshot()` in `shell.rs`: execute `screencap -p` via shell stream, collect binary PNG output, return as `Vec<u8>`. Handle display-off error case. Verify: unit test with mock PNG data returns correct bytes; error case returns `AdbError`.

## 7. Input Injection Fallback

- [x] 7.1 Implement input injection functions in `shell.rs`: `inject_tap(x, y)`, `inject_text(text)` (with proper shell escaping of special characters), `inject_key(keycode)`. Each runs the corresponding `input` command via `shell_exec`. Verify: unit tests confirm correct command strings are generated, including escaped special characters in text input.

## 8. File Transfer (Sync Protocol)

- [x] 8.1 Implement ADB sync protocol framing in `sync.rs`: STAT, SEND, RECV, DATA, DONE, FAIL, QUIT message types. Sync session open/close over an ADB stream. Verify: unit tests serialize and deserialize sync messages correctly.
- [x] 8.2 Implement `push_file()` in `sync.rs`: open sync session, send SEND with target path and permissions, stream file data in chunks (64KB), send DONE with mtime, verify OKAY response. Report progress via callback `Fn(u64, u64)`. Verify: unit test with mock transport pushes a small file and receives OKAY.
- [x] 8.3 Implement `pull_file()` in `sync.rs`: open sync session, send RECV with guest path, receive DATA chunks and write to host file, handle DONE/FAIL. Report progress via callback. Verify: unit test with mock transport pulls a small file and writes correct bytes.
- [x] 8.4 Verify streaming behavior for large files: ensure neither push nor pull buffers the entire file in memory. Add a test that pushes/pulls a simulated 200MB file using chunked mock data and confirm peak memory stays bounded. Verify: test passes without OOM or excessive allocation.

## 9. App Management

- [x] 9.1 Implement `install_apk(path)` in `mod.rs` (or a dedicated `app.rs`): validate host file exists, push APK to `/data/local/tmp/` via `push_file`, run `pm install` via `shell_exec`, clean up temp file, parse install result. Report push progress. Verify: unit test with mock transport completes install flow and returns package name.
- [x] 9.2 Implement `uninstall_app(package)` in app management: run `pm uninstall <package>` via `shell_exec`, parse success/failure. Verify: unit test for success and package-not-found cases.
- [x] 9.3 Implement `list_packages()` in app management: run `pm list packages` via `shell_exec`, parse output into `Vec<PackageInfo>`. Verify: unit test parses multi-line `package:com.example.app` output correctly.
- [x] 9.4 Implement `launch_app(package)` in app management: run `monkey -p <package> -c android.intent.category.LAUNCHER 1` via `shell_exec`, detect success vs. no-launchable-activity error. Verify: unit test for both success and failure cases.

## 10. Public API and Integration

- [x] 10.1 Wire up all operations as public async methods on `AdbClient`: `install_apk`, `uninstall_app`, `list_packages`, `launch_app`, `push_file`, `pull_file`, `capture_screenshot`, `get_device_info`, `get_screen_resolution`, `inject_tap`, `inject_text`, `inject_key`. Each method SHALL return `Result<T, AdbError>` and require `Connected` state. Verify: `cargo doc -p nux-core` generates docs for all public methods.
- [x] 10.2 Add connection-state guard to all public methods: if `AdbClient` is not in `Connected` state, return `AdbError::NotConnected` immediately. Verify: unit test calling any method on a disconnected client returns the correct error.
- [x] 10.3 Run `cargo clippy -p nux-core -- -D warnings` and `cargo fmt --check -p nux-core`. Fix any lint or format issues. Verify: both commands pass cleanly.
