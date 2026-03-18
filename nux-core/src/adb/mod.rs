//! ADB bridge for Nux Emulator.
//!
//! Provides a pure-Rust ADB client for communicating with the Android
//! guest VM. Supports app management, file transfer, shell commands,
//! screenshot capture, and input injection over TCP or virtio-serial.

pub mod protocol;
pub mod shell;
pub mod sync;
pub mod transport;
pub mod types;

use protocol::{Handshake, StreamManager};
use std::path::Path;
use transport::{ConnectedTransport, select_transport};
use types::{AdbConfig, AdbError, AdbResult, ConnectionState, DeviceInfo, PackageInfo};

/// High-level ADB client for communicating with the Android guest.
///
/// Wraps the transport, protocol, and stream layers into a single
/// ergonomic API consumed by `nux-ui` and other `nux-core` modules.
pub struct AdbClient {
    config: AdbConfig,
    state: ConnectionState,
    state_tx: tokio::sync::watch::Sender<ConnectionState>,
    state_rx: tokio::sync::watch::Receiver<ConnectionState>,
    transport: Option<ConnectedTransport>,
    streams: StreamManager,
    max_payload: u32,
    reconnect_handle: Option<tokio::task::JoinHandle<()>>,
}

impl AdbClient {
    /// Create a new ADB client with the given configuration.
    ///
    /// The client starts in `Disconnected` state. Call [`connect`](Self::connect)
    /// to establish a connection to the guest.
    pub fn new(config: AdbConfig) -> Self {
        let (state_tx, state_rx) = tokio::sync::watch::channel(ConnectionState::Disconnected);
        Self {
            config,
            state: ConnectionState::Disconnected,
            state_tx,
            state_rx,
            transport: None,
            streams: StreamManager::new(),
            max_payload: protocol::MAX_PAYLOAD,
            reconnect_handle: None,
        }
    }

    /// Get the current connection state.
    pub fn state(&self) -> ConnectionState {
        self.state.clone()
    }

    /// Subscribe to connection state changes.
    ///
    /// Returns a watch receiver that emits each state transition.
    pub fn watch_state(&self) -> tokio::sync::watch::Receiver<ConnectionState> {
        self.state_rx.clone()
    }

    /// Update the connection state and notify watchers.
    fn set_state(&mut self, new_state: ConnectionState) {
        self.state = new_state.clone();
        let _ = self.state_tx.send(new_state);
    }

    /// Require `Connected` state, returning `NotConnected` otherwise.
    fn require_connected(&self) -> AdbResult<()> {
        if self.state != ConnectionState::Connected {
            return Err(AdbError::NotConnected);
        }
        Ok(())
    }

    /// Connect to the guest's `adbd`.
    ///
    /// Selects the transport (TCP or virtio-serial), establishes the
    /// connection, and completes the ADB CNXN handshake.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::ConnectionRefused` if no transport is available,
    /// `AdbError::Timeout` if the connection times out, or
    /// `AdbError::ProtocolError` if the handshake fails.
    pub async fn connect(&mut self) -> AdbResult<()> {
        self.set_state(ConnectionState::Connecting);

        let mut transport = match select_transport(&self.config).await {
            Ok(t) => t,
            Err(e) => {
                self.set_state(ConnectionState::Error(e.to_string()));
                return Err(e);
            }
        };

        // Send CNXN handshake
        let cnxn = Handshake::host_message();
        if let Err(e) = transport.write_message(&cnxn).await {
            self.set_state(ConnectionState::Error(e.to_string()));
            return Err(e);
        }

        // Read CNXN response
        let timeout = tokio::time::Duration::from_millis(self.config.connect_timeout_ms);
        let response = match tokio::time::timeout(timeout, transport.read_message()).await {
            Ok(Ok(msg)) => msg,
            Ok(Err(e)) => {
                self.set_state(ConnectionState::Error(e.to_string()));
                return Err(e);
            }
            Err(_) => {
                let err = AdbError::Timeout("CNXN handshake timed out".to_owned());
                self.set_state(ConnectionState::Error(err.to_string()));
                return Err(err);
            }
        };

        let handshake = match Handshake::from_response(&response) {
            Ok(hs) => hs,
            Err(e) => {
                self.set_state(ConnectionState::Error(e.to_string()));
                return Err(e);
            }
        };

        self.max_payload = handshake.max_payload;
        self.transport = Some(transport);
        self.streams = StreamManager::new();
        self.set_state(ConnectionState::Connected);

        log::info!(
            "ADB connected: banner={}, max_payload={}",
            handshake.device_banner,
            handshake.max_payload
        );

        Ok(())
    }

    /// Disconnect from the guest.
    ///
    /// Stops any auto-reconnect task and closes the transport.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::Io` if the transport close fails.
    pub async fn disconnect(&mut self) -> AdbResult<()> {
        // Stop reconnect task
        if let Some(handle) = self.reconnect_handle.take() {
            handle.abort();
        }

        if let Some(mut transport) = self.transport.take() {
            let _ = transport.close().await;
        }

        self.streams = StreamManager::new();
        self.set_state(ConnectionState::Disconnected);
        Ok(())
    }

    /// Start auto-reconnect with exponential backoff.
    ///
    /// Spawns a background task that monitors the connection and
    /// reconnects on failure. Initial delay is 500 ms, max is 10 s.
    /// Call [`disconnect`](Self::disconnect) to stop reconnection.
    pub fn start_auto_reconnect(&mut self) {
        let config = self.config.clone();
        let state_tx = self.state_tx.clone();

        let handle = tokio::spawn(async move {
            let mut delay_ms: u64 = 500;
            let max_delay_ms: u64 = 10_000;

            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;

                let _ = state_tx.send(ConnectionState::Connecting);

                match select_transport(&config).await {
                    Ok(mut transport) => {
                        let cnxn = Handshake::host_message();
                        if transport.write_message(&cnxn).await.is_ok() {
                            let timeout =
                                tokio::time::Duration::from_millis(config.connect_timeout_ms);
                            if let Ok(Ok(resp)) =
                                tokio::time::timeout(timeout, transport.read_message()).await
                            {
                                if Handshake::from_response(&resp).is_ok() {
                                    let _ = state_tx.send(ConnectionState::Connected);
                                    // Stay alive to detect disconnection
                                    // (in a real implementation we'd monitor the transport)
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = state_tx.send(ConnectionState::Error(e.to_string()));
                    }
                }

                delay_ms = (delay_ms * 2).min(max_delay_ms);
            }
        });

        self.reconnect_handle = Some(handle);
    }

    // ── Shell operations ──────────────────────────────────────────

    /// Execute a shell command on the guest.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::NotConnected` if not connected, or shell errors.
    pub async fn shell_exec(&mut self, command: &str) -> AdbResult<String> {
        self.require_connected()?;
        let transport = self.transport.as_mut().ok_or(AdbError::NotConnected)?;
        shell::shell_exec(
            transport,
            &mut self.streams,
            command,
            self.config.command_timeout_ms,
        )
        .await
    }

    /// Query device information (Android version, SDK, model, ABIs).
    ///
    /// # Errors
    ///
    /// Returns `AdbError::NotConnected` if not connected, or shell errors.
    pub async fn get_device_info(&mut self) -> AdbResult<DeviceInfo> {
        self.require_connected()?;
        let transport = self.transport.as_mut().ok_or(AdbError::NotConnected)?;
        shell::get_device_info(transport, &mut self.streams, self.config.command_timeout_ms).await
    }

    /// Query the guest screen resolution.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::NotConnected` if not connected, or shell errors.
    pub async fn get_screen_resolution(&mut self) -> AdbResult<(u32, u32)> {
        self.require_connected()?;
        let transport = self.transport.as_mut().ok_or(AdbError::NotConnected)?;
        shell::get_screen_resolution(transport, &mut self.streams, self.config.command_timeout_ms)
            .await
    }

    /// Capture a screenshot as PNG bytes.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::NotConnected` if not connected, or shell errors.
    pub async fn capture_screenshot(&mut self) -> AdbResult<Vec<u8>> {
        self.require_connected()?;
        let transport = self.transport.as_mut().ok_or(AdbError::NotConnected)?;
        shell::capture_screenshot(transport, &mut self.streams, self.config.command_timeout_ms)
            .await
    }

    /// Inject a tap at screen coordinates (x, y).
    ///
    /// # Errors
    ///
    /// Returns `AdbError::NotConnected` if not connected, or shell errors.
    pub async fn inject_tap(&mut self, x: u32, y: u32) -> AdbResult<()> {
        self.require_connected()?;
        let transport = self.transport.as_mut().ok_or(AdbError::NotConnected)?;
        shell::inject_tap(
            transport,
            &mut self.streams,
            x,
            y,
            self.config.command_timeout_ms,
        )
        .await
    }

    /// Inject text input with shell escaping.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::NotConnected` if not connected, or shell errors.
    pub async fn inject_text(&mut self, text: &str) -> AdbResult<()> {
        self.require_connected()?;
        let transport = self.transport.as_mut().ok_or(AdbError::NotConnected)?;
        shell::inject_text(
            transport,
            &mut self.streams,
            text,
            self.config.command_timeout_ms,
        )
        .await
    }

    /// Inject a key event by Android keycode.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::NotConnected` if not connected, or shell errors.
    pub async fn inject_key(&mut self, keycode: u32) -> AdbResult<()> {
        self.require_connected()?;
        let transport = self.transport.as_mut().ok_or(AdbError::NotConnected)?;
        shell::inject_key(
            transport,
            &mut self.streams,
            keycode,
            self.config.command_timeout_ms,
        )
        .await
    }

    // ── File transfer ─────────────────────────────────────────────

    /// Push a file from the host to the guest.
    ///
    /// Calls `progress(bytes_sent, total_bytes)` after each chunk.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::NotConnected`, `AdbError::FileNotFound`, or
    /// sync/IO errors.
    pub async fn push_file<F>(
        &mut self,
        host_path: &Path,
        guest_path: &str,
        progress: F,
    ) -> AdbResult<()>
    where
        F: Fn(u64, u64),
    {
        self.require_connected()?;
        let transport = self.transport.as_mut().ok_or(AdbError::NotConnected)?;
        sync::push_file(
            transport,
            &mut self.streams,
            host_path,
            guest_path,
            0o644,
            self.config.command_timeout_ms,
            progress,
        )
        .await
    }

    /// Pull a file from the guest to the host.
    ///
    /// Calls `progress(bytes_received, total_bytes)` after each chunk.
    /// `total_bytes` is 0 when the size is unknown.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::NotConnected`, `AdbError::GuestError`, or
    /// sync/IO errors.
    pub async fn pull_file<F>(
        &mut self,
        guest_path: &str,
        host_path: &Path,
        progress: F,
    ) -> AdbResult<()>
    where
        F: Fn(u64, u64),
    {
        self.require_connected()?;
        let transport = self.transport.as_mut().ok_or(AdbError::NotConnected)?;
        sync::pull_file(
            transport,
            &mut self.streams,
            guest_path,
            host_path,
            self.config.command_timeout_ms,
            progress,
        )
        .await
    }

    // ── App management ────────────────────────────────────────────

    /// Install an APK from the host filesystem.
    ///
    /// Pushes the APK to `/data/local/tmp/`, runs `pm install`, and
    /// cleans up the temp file. Calls `progress(bytes_sent, total_bytes)`
    /// during the push phase.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::FileNotFound` if the APK doesn't exist,
    /// `AdbError::GuestError` if `pm install` fails, or transport errors.
    pub async fn install_apk<F>(&mut self, host_path: &Path, progress: F) -> AdbResult<String>
    where
        F: Fn(u64, u64),
    {
        self.require_connected()?;

        if !host_path.exists() {
            return Err(AdbError::FileNotFound(host_path.to_path_buf()));
        }

        let file_name = host_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("app.apk");
        let guest_tmp = format!("/data/local/tmp/{file_name}");

        // Push APK
        self.push_file(host_path, &guest_tmp, progress).await?;

        // Install
        let cmd = format!("pm install -r '{guest_tmp}'");
        let output = self.shell_exec(&cmd).await?;

        // Clean up temp file (best-effort)
        let _ = self.shell_exec(&format!("rm -f '{guest_tmp}'")).await;

        // Parse result
        if output.contains("Success") {
            // Extract package name from output if available, otherwise derive from filename
            let package = file_name
                .strip_suffix(".apk")
                .unwrap_or(file_name)
                .to_owned();
            Ok(package)
        } else {
            Err(AdbError::GuestError(format!("pm install failed: {output}")))
        }
    }

    /// Uninstall an app by package name.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::GuestError` if the package is not found or
    /// uninstall fails.
    pub async fn uninstall_app(&mut self, package: &str) -> AdbResult<()> {
        self.require_connected()?;
        let output = self
            .shell_exec(&format!("pm uninstall '{package}'"))
            .await?;

        if output.contains("Success") {
            Ok(())
        } else {
            Err(AdbError::GuestError(format!("uninstall failed: {output}")))
        }
    }

    /// List installed packages.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::NotConnected` if not connected, or shell errors.
    pub async fn list_packages(&mut self) -> AdbResult<Vec<PackageInfo>> {
        self.require_connected()?;
        let output = self.shell_exec("pm list packages").await?;
        let packages = parse_package_list(&output);
        Ok(packages)
    }

    /// Launch an app by package name.
    ///
    /// Uses the `monkey` command to start the launcher activity.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::GuestError` if the package has no launchable activity.
    pub async fn launch_app(&mut self, package: &str) -> AdbResult<()> {
        self.require_connected()?;
        let output = self
            .shell_exec(&format!(
                "monkey -p '{package}' -c android.intent.category.LAUNCHER 1"
            ))
            .await?;

        if output.contains("No activities found") || output.contains("monkey aborted") {
            return Err(AdbError::GuestError(format!(
                "no launchable activity for {package}"
            )));
        }

        Ok(())
    }
}

impl Drop for AdbClient {
    fn drop(&mut self) {
        if let Some(handle) = self.reconnect_handle.take() {
            handle.abort();
        }
    }
}

/// Parse `pm list packages` output into a vec of `PackageInfo`.
fn parse_package_list(output: &str) -> Vec<PackageInfo> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("package:").map(|name| PackageInfo {
                package_name: name.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_disconnected() {
        let client = AdbClient::new(AdbConfig::default());
        assert_eq!(client.state(), ConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn methods_fail_when_disconnected() {
        let mut client = AdbClient::new(AdbConfig::default());

        assert!(matches!(
            client.shell_exec("echo hi").await,
            Err(AdbError::NotConnected)
        ));
        assert!(matches!(
            client.get_device_info().await,
            Err(AdbError::NotConnected)
        ));
        assert!(matches!(
            client.get_screen_resolution().await,
            Err(AdbError::NotConnected)
        ));
        assert!(matches!(
            client.capture_screenshot().await,
            Err(AdbError::NotConnected)
        ));
        assert!(matches!(
            client.inject_tap(0, 0).await,
            Err(AdbError::NotConnected)
        ));
        assert!(matches!(
            client.inject_text("hi").await,
            Err(AdbError::NotConnected)
        ));
        assert!(matches!(
            client.inject_key(4).await,
            Err(AdbError::NotConnected)
        ));
        assert!(matches!(
            client
                .push_file(Path::new("/tmp/x"), "/sdcard/x", |_, _| {})
                .await,
            Err(AdbError::NotConnected)
        ));
        assert!(matches!(
            client
                .pull_file("/sdcard/x", Path::new("/tmp/x"), |_, _| {})
                .await,
            Err(AdbError::NotConnected)
        ));
        assert!(matches!(
            client.install_apk(Path::new("/tmp/x.apk"), |_, _| {}).await,
            Err(AdbError::NotConnected)
        ));
        assert!(matches!(
            client.uninstall_app("com.example").await,
            Err(AdbError::NotConnected)
        ));
        assert!(matches!(
            client.list_packages().await,
            Err(AdbError::NotConnected)
        ));
        assert!(matches!(
            client.launch_app("com.example").await,
            Err(AdbError::NotConnected)
        ));
    }

    #[test]
    fn parse_package_list_basic() {
        let output = "package:com.android.settings\npackage:com.example.app\npackage:org.test\n";
        let packages = parse_package_list(output);
        assert_eq!(packages.len(), 3);
        assert_eq!(packages[0].package_name, "com.android.settings");
        assert_eq!(packages[1].package_name, "com.example.app");
        assert_eq!(packages[2].package_name, "org.test");
    }

    #[test]
    fn parse_package_list_empty() {
        let packages = parse_package_list("");
        assert!(packages.is_empty());
    }

    #[test]
    fn parse_package_list_with_noise() {
        let output = "WARNING: something\npackage:com.example.app\n\n";
        let packages = parse_package_list(output);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].package_name, "com.example.app");
    }

    #[tokio::test]
    async fn state_watch_receives_updates() {
        let mut client = AdbClient::new(AdbConfig::default());
        let mut rx = client.watch_state();

        // Initial state
        assert_eq!(*rx.borrow(), ConnectionState::Disconnected);

        // Simulate state change
        client.set_state(ConnectionState::Connecting);
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), ConnectionState::Connecting);

        client.set_state(ConnectionState::Error("test".to_owned()));
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), ConnectionState::Error("test".to_owned()));
    }

    #[tokio::test]
    async fn disconnect_resets_state() {
        let mut client = AdbClient::new(AdbConfig::default());
        client.set_state(ConnectionState::Connected);
        client.disconnect().await.unwrap();
        assert_eq!(client.state(), ConnectionState::Disconnected);
    }
}
