//! passt userspace networking backend.

use super::config::NetworkVmConfig;
use super::error::{NetworkError, NetworkResult};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use tokio::process::{Child, Command};
use tokio::time::Duration;

/// Default timeout waiting for passt socket to appear.
const PASST_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);

/// Manages a passt child process for userspace networking.
#[derive(Debug)]
pub struct PasstProcess {
    child: Child,
    socket_path: PathBuf,
}

impl PasstProcess {
    /// Spawn a new passt process with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError::PasstSpawnFailed` if the process cannot be started,
    /// or `NetworkError::PasstSocketNotFound` if the socket doesn't appear within
    /// the startup timeout.
    pub async fn spawn(config: &NetworkVmConfig, socket_path: PathBuf) -> NetworkResult<Self> {
        // Clean up stale socket
        let _ = std::fs::remove_file(&socket_path);

        let mut cmd = Command::new("passt");
        cmd.arg("--socket").arg(&socket_path);

        // Port forwarding for ADB
        cmd.arg("--forward")
            .arg(format!("{}:{}", config.adb_port, config.guest_adb_port));

        // Run in foreground mode (no daemonize) so we can manage the lifecycle
        cmd.arg("--foreground");

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let child = cmd
            .spawn()
            .map_err(|e| NetworkError::PasstSpawnFailed(e.to_string()))?;

        let process = Self {
            child,
            socket_path: socket_path.clone(),
        };

        // Wait for the socket to appear
        process.wait_for_socket().await?;

        Ok(process)
    }

    /// Get the passt socket path for passing to crosvm.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Stop the passt process gracefully.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError::Io` if killing the process fails.
    pub async fn stop(&mut self) -> NetworkResult<()> {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        let _ = std::fs::remove_file(&self.socket_path);
        Ok(())
    }

    /// Check if the passt process is still running.
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    async fn wait_for_socket(&self) -> NetworkResult<()> {
        let start = tokio::time::Instant::now();
        loop {
            if self.socket_path.exists() {
                return Ok(());
            }
            if start.elapsed() > PASST_STARTUP_TIMEOUT {
                return Err(NetworkError::PasstSocketNotFound(self.socket_path.clone()));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

/// Check if the `passt` binary is available on `$PATH`.
pub fn passt_available() -> bool {
    which_passt().is_some()
}

/// Find the full path to the `passt` binary.
pub fn which_passt() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join("passt");
            if candidate.is_file() {
                Some(candidate)
            } else {
                None
            }
        })
    })
}

/// Build crosvm CLI arguments for passt networking.
///
/// Appends the passt socket path argument so crosvm connects to the
/// passt userspace network stack.
pub fn build_passt_args(socket_path: &Path) -> Vec<OsString> {
    vec![
        "--net".into(),
        format!("vhost-net=passt,socket={}", socket_path.display()).into(),
    ]
}

/// Get the default passt socket path for a Nux instance.
pub fn default_socket_path() -> PathBuf {
    let uid = nix::unistd::getuid();
    PathBuf::from(format!("/run/user/{uid}/nux/passt.sock"))
}

/// Get the ADB connection address when using passt (localhost forwarding).
pub fn passt_adb_address(config: &NetworkVmConfig) -> String {
    format!("localhost:{}", config.adb_port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_socket_path_contains_uid() {
        let path = default_socket_path();
        let uid = nix::unistd::getuid();
        assert!(path.to_str().unwrap().contains(&uid.to_string()));
        assert!(path.to_str().unwrap().contains("passt.sock"));
    }

    #[test]
    fn passt_args_contain_socket() {
        let path = PathBuf::from("/tmp/test-passt.sock");
        let args = build_passt_args(&path);
        let strings: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(strings[0], "--net");
        assert!(strings[1].contains("/tmp/test-passt.sock"));
    }

    #[test]
    fn passt_adb_address_default() {
        let config = NetworkVmConfig::default();
        assert_eq!(passt_adb_address(&config), "localhost:5555");
    }
}
