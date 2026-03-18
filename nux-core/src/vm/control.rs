//! crosvm control socket client.

use super::error::{VmError, VmResult};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::time::{Duration, timeout};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

/// Client for communicating with crosvm's control socket.
pub struct ControlClient {
    socket_path: PathBuf,
    stream: Option<UnixStream>,
    max_ram_mb: u32,
}

impl ControlClient {
    /// Create a new control client for the given socket path.
    pub fn new(socket_path: PathBuf, max_ram_mb: u32) -> Self {
        Self {
            socket_path,
            stream: None,
            max_ram_mb,
        }
    }

    /// Connect to the crosvm control socket.
    ///
    /// # Errors
    ///
    /// Returns `VmError::ControlSocket` if the socket doesn't exist or
    /// connection is refused.
    pub async fn connect(&mut self) -> VmResult<()> {
        self.stream = None;
        let stream = connect_to_socket(&self.socket_path).await?;
        self.stream = Some(stream);
        Ok(())
    }

    /// Send a pause command.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout or socket failure.
    pub async fn pause(&mut self) -> VmResult<()> {
        self.send_command("pause").await
    }

    /// Send a resume command.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout or socket failure.
    pub async fn resume(&mut self) -> VmResult<()> {
        self.send_command("resume").await
    }

    /// Send a stop (shutdown) command.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout or socket failure.
    pub async fn stop(&mut self) -> VmResult<()> {
        self.send_command("stop").await
    }

    /// Set balloon memory size in MB.
    ///
    /// # Errors
    ///
    /// Returns `VmError::ConfigValidation` if size exceeds configured RAM,
    /// or a socket error on failure.
    pub async fn balloon_set(&mut self, mb: u32) -> VmResult<()> {
        if mb > self.max_ram_mb {
            return Err(VmError::ConfigValidation(format!(
                "balloon size {mb} MB exceeds configured RAM {} MB",
                self.max_ram_mb
            )));
        }
        let cmd = format!("balloon {mb}");
        self.send_command(&cmd).await
    }

    /// Check if the client is connected.
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    async fn send_command(&mut self, cmd: &str) -> VmResult<()> {
        // Auto-reconnect if disconnected
        if self.stream.is_none() {
            self.connect().await?;
        }

        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| VmError::ControlSocket("not connected".to_owned()))?;

        // Send command
        let write_result = stream.write_all(cmd.as_bytes()).await;
        if let Err(e) = write_result {
            self.stream = None; // Mark as disconnected
            return Err(VmError::ControlSocket(format!(
                "socket disconnected during send: {e}"
            )));
        }

        // Read response with timeout
        let mut buf = vec![0u8; 4096];
        match timeout(COMMAND_TIMEOUT, stream.read(&mut buf)).await {
            Ok(Ok(0)) => {
                self.stream = None;
                Err(VmError::ControlSocket(
                    "socket disconnected (EOF)".to_owned(),
                ))
            }
            Ok(Ok(n)) => {
                let response = String::from_utf8_lossy(&buf[..n]);
                if response.contains("error") || response.contains("ERR") {
                    Err(VmError::ControlSocket(format!(
                        "command '{cmd}' failed: {response}"
                    )))
                } else {
                    Ok(())
                }
            }
            Ok(Err(e)) => {
                self.stream = None;
                Err(VmError::ControlSocket(format!("read error: {e}")))
            }
            Err(_) => Err(VmError::Timeout(format!(
                "command '{cmd}' timed out after {}s",
                COMMAND_TIMEOUT.as_secs()
            ))),
        }
    }
}

async fn connect_to_socket(path: &Path) -> VmResult<UnixStream> {
    if !path.exists() {
        return Err(VmError::ControlSocket(format!(
            "socket not found: {}",
            path.display()
        )));
    }

    UnixStream::connect(path).await.map_err(|e| {
        VmError::ControlSocket(format!("connection refused at {}: {e}", path.display()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UnixListener;

    #[tokio::test]
    async fn connect_to_nonexistent_socket() {
        let mut client = ControlClient::new(PathBuf::from("/tmp/nonexistent.sock"), 4096);
        let result = client.connect().await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("socket not found"));
    }

    #[tokio::test]
    async fn connect_and_send_command() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");

        // Start a mock listener that responds with "OK"
        let listener = UnixListener::bind(&sock_path).unwrap();
        let sock_path_clone = sock_path.clone();

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).await.unwrap();
            assert!(n > 0);
            stream.write_all(b"OK").await.unwrap();
        });

        let mut client = ControlClient::new(sock_path_clone, 4096);
        client.connect().await.unwrap();
        assert!(client.is_connected());

        let result = client.pause().await;
        assert!(result.is_ok());

        server.await.unwrap();
    }

    #[tokio::test]
    async fn balloon_exceeds_ram() {
        let mut client = ControlClient::new(PathBuf::from("/tmp/fake.sock"), 4096);
        let result = client.balloon_set(8192).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("exceeds configured RAM"));
    }

    #[tokio::test]
    async fn command_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("timeout.sock");

        // Listener that accepts but never responds
        let listener = UnixListener::bind(&sock_path).unwrap();
        let _server = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            // Hold connection open but never respond
            tokio::time::sleep(Duration::from_secs(30)).await;
        });

        let mut client = ControlClient::new(sock_path, 4096);
        client.connect().await.unwrap();

        // This should timeout (we use a shorter timeout for testing)
        let result = client.send_command("pause").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("timed out"));
    }

    #[tokio::test]
    async fn auto_reconnect_on_disconnect() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("reconnect.sock");

        let listener = UnixListener::bind(&sock_path).unwrap();
        let sock_path_clone = sock_path.clone();

        // Server accepts, responds, then closes
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let _ = stream.read(&mut buf).await;
            stream.write_all(b"OK").await.unwrap();
            drop(stream);

            // Accept second connection (reconnect)
            let (mut stream2, _) = listener.accept().await.unwrap();
            let mut buf2 = vec![0u8; 4096];
            let _ = stream2.read(&mut buf2).await;
            stream2.write_all(b"OK").await.unwrap();
        });

        let mut client = ControlClient::new(sock_path_clone, 4096);
        client.connect().await.unwrap();
        client.pause().await.unwrap();

        // Force disconnect
        client.stream = None;

        // Should auto-reconnect
        let result = client.resume().await;
        assert!(result.is_ok());

        server.await.unwrap();
    }
}
