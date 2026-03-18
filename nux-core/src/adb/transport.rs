//! ADB transport abstraction layer.
//!
//! Provides a trait for reading/writing ADB messages over different
//! transport mechanisms, with TCP and virtio-serial implementations.

use crate::adb::protocol::{AdbMessage, HEADER_SIZE};
use crate::adb::types::{AdbConfig, AdbError, AdbResult, TransportKind};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Abstraction over the underlying byte transport for ADB messages.
///
/// Both TCP and virtio-serial implement this trait so the protocol
/// layer doesn't care which transport is in use.
#[allow(async_fn_in_trait)]
pub trait AdbTransport: Send {
    /// Read the next ADB message from the transport.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::Io` on read failure or `AdbError::ProtocolError`
    /// if the message is malformed.
    async fn read_message(&mut self) -> AdbResult<AdbMessage>;

    /// Write an ADB message to the transport.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::Io` on write failure.
    async fn write_message(&mut self, msg: &AdbMessage) -> AdbResult<()>;

    /// Close the transport connection.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::Io` if shutdown fails.
    async fn close(&mut self) -> AdbResult<()>;
}

/// TCP transport connecting to the guest's `adbd` over the virtual network.
#[derive(Debug)]
pub struct TcpTransport {
    stream: TcpStream,
}

impl TcpTransport {
    /// Connect to the guest at the given address and port.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::ConnectionRefused` if the connection fails, or
    /// `AdbError::Timeout` if the connection times out.
    pub async fn connect(addr: &str, port: u16, timeout_ms: u64) -> AdbResult<Self> {
        let target = format!("{addr}:{port}");
        let duration = tokio::time::Duration::from_millis(timeout_ms);

        let stream = tokio::time::timeout(duration, TcpStream::connect(&target))
            .await
            .map_err(|_| AdbError::Timeout(format!("TCP connect to {target} timed out")))?
            .map_err(|e| AdbError::ConnectionRefused(format!("{target}: {e}")))?;

        stream.set_nodelay(true).ok();
        Ok(Self { stream })
    }
}

impl AdbTransport for TcpTransport {
    async fn read_message(&mut self) -> AdbResult<AdbMessage> {
        read_message_from(&mut self.stream).await
    }

    async fn write_message(&mut self, msg: &AdbMessage) -> AdbResult<()> {
        let bytes = msg.to_bytes();
        self.stream.write_all(&bytes).await?;
        self.stream.flush().await?;
        Ok(())
    }

    async fn close(&mut self) -> AdbResult<()> {
        self.stream.shutdown().await?;
        Ok(())
    }
}

/// Virtio-serial transport using a character device exposed by crosvm.
#[derive(Debug)]
pub struct VirtioSerialTransport {
    reader: tokio::io::BufReader<tokio::fs::File>,
    writer: tokio::fs::File,
}

impl VirtioSerialTransport {
    /// Open the virtio-serial device at the given path.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::Io` if the device path cannot be opened.
    pub async fn connect(path: &Path) -> AdbResult<Self> {
        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .await
            .map_err(|e| {
                AdbError::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "failed to open virtio-serial device {}: {e}",
                        path.display()
                    ),
                ))
            })?;

        let writer = file
            .try_clone()
            .await
            .map_err(|e| AdbError::Io(std::io::Error::new(e.kind(), format!("clone fd: {e}"))))?;

        Ok(Self {
            reader: tokio::io::BufReader::new(file),
            writer,
        })
    }
}

impl AdbTransport for VirtioSerialTransport {
    async fn read_message(&mut self) -> AdbResult<AdbMessage> {
        read_message_from(&mut self.reader).await
    }

    async fn write_message(&mut self, msg: &AdbMessage) -> AdbResult<()> {
        let bytes = msg.to_bytes();
        self.writer.write_all(&bytes).await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn close(&mut self) -> AdbResult<()> {
        self.writer.shutdown().await?;
        Ok(())
    }
}

/// Read a single ADB message from any `AsyncRead` source.
async fn read_message_from<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
) -> AdbResult<AdbMessage> {
    let mut header = [0u8; HEADER_SIZE];
    reader.read_exact(&mut header).await?;

    let data_len = u32::from_le_bytes([header[12], header[13], header[14], header[15]]) as usize;

    let mut buf = Vec::with_capacity(HEADER_SIZE + data_len);
    buf.extend_from_slice(&header);

    if data_len > 0 {
        buf.resize(HEADER_SIZE + data_len, 0);
        reader.read_exact(&mut buf[HEADER_SIZE..]).await?;
    }

    AdbMessage::from_bytes(&buf)
}

/// Possible connected transport (type-erased behind an enum for simplicity).
#[derive(Debug)]
pub enum ConnectedTransport {
    /// TCP transport.
    Tcp(TcpTransport),
    /// Virtio-serial transport.
    VirtioSerial(Box<VirtioSerialTransport>),
}

impl ConnectedTransport {
    /// Read the next ADB message.
    ///
    /// # Errors
    ///
    /// Returns transport-level or protocol errors.
    pub async fn read_message(&mut self) -> AdbResult<AdbMessage> {
        match self {
            Self::Tcp(t) => t.read_message().await,
            Self::VirtioSerial(t) => t.read_message().await,
        }
    }

    /// Write an ADB message.
    ///
    /// # Errors
    ///
    /// Returns transport-level errors.
    pub async fn write_message(&mut self, msg: &AdbMessage) -> AdbResult<()> {
        match self {
            Self::Tcp(t) => t.write_message(msg).await,
            Self::VirtioSerial(t) => t.write_message(msg).await,
        }
    }

    /// Close the transport.
    ///
    /// # Errors
    ///
    /// Returns transport-level errors.
    pub async fn close(&mut self) -> AdbResult<()> {
        match self {
            Self::Tcp(t) => t.close().await,
            Self::VirtioSerial(t) => t.close().await,
        }
    }
}

/// Select and connect a transport based on configuration.
///
/// Tries the preferred transport first, then falls back to the other.
///
/// # Errors
///
/// Returns `AdbError::ConnectionRefused` if both transports fail.
pub async fn select_transport(config: &AdbConfig) -> AdbResult<ConnectedTransport> {
    match config.preferred_transport {
        TransportKind::Tcp => {
            match TcpTransport::connect(
                &config.guest_ip,
                config.guest_port,
                config.connect_timeout_ms,
            )
            .await
            {
                Ok(t) => Ok(ConnectedTransport::Tcp(t)),
                Err(tcp_err) => {
                    log::warn!("TCP transport failed, trying virtio-serial: {tcp_err}");
                    let vs = VirtioSerialTransport::connect(&config.virtio_serial_path)
                        .await
                        .map_err(|vs_err| {
                            AdbError::ConnectionRefused(format!(
                                "all transports failed — TCP: {tcp_err}, virtio-serial: {vs_err}"
                            ))
                        })?;
                    Ok(ConnectedTransport::VirtioSerial(Box::new(vs)))
                }
            }
        }
        TransportKind::VirtioSerial => {
            match VirtioSerialTransport::connect(&config.virtio_serial_path).await {
                Ok(t) => Ok(ConnectedTransport::VirtioSerial(Box::new(t))),
                Err(vs_err) => {
                    log::warn!("virtio-serial transport failed, trying TCP: {vs_err}");
                    let tcp = TcpTransport::connect(
                        &config.guest_ip,
                        config.guest_port,
                        config.connect_timeout_ms,
                    )
                    .await
                    .map_err(|tcp_err| {
                        AdbError::ConnectionRefused(format!(
                            "all transports failed — virtio-serial: {vs_err}, TCP: {tcp_err}"
                        ))
                    })?;
                    Ok(ConnectedTransport::Tcp(tcp))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adb::protocol::{ADB_VERSION, AdbMessage, CMD_CNXN, MAX_PAYLOAD};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn tcp_transport_connects_to_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let connect_fut = TcpTransport::connect("127.0.0.1", addr.port(), 2000);

        let (server_result, client_result) = tokio::join!(listener.accept(), connect_fut);

        assert!(server_result.is_ok());
        assert!(client_result.is_ok());
    }

    #[tokio::test]
    async fn tcp_transport_read_write_message() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            // Write a CNXN message
            let msg = AdbMessage::cnxn("device::test");
            let bytes = msg.to_bytes();
            socket.write_all(&bytes).await.unwrap();
            socket.flush().await.unwrap();
        });

        let mut transport = TcpTransport::connect("127.0.0.1", addr.port(), 2000)
            .await
            .unwrap();

        let msg = transport.read_message().await.unwrap();
        assert_eq!(msg.command, CMD_CNXN);
        assert_eq!(msg.arg0, ADB_VERSION);
        assert_eq!(msg.arg1, MAX_PAYLOAD);

        server.await.unwrap();
    }

    #[tokio::test]
    async fn tcp_transport_connection_refused() {
        // Connect to a port that nothing is listening on
        let result = TcpTransport::connect("127.0.0.1", 1, 500).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn transport_fallback_on_tcp_failure() {
        // Both should fail since neither is available, but we verify the
        // fallback logic runs without panicking.
        let config = AdbConfig {
            guest_ip: "127.0.0.1".to_owned(),
            guest_port: 1, // nothing listening
            virtio_serial_path: "/dev/nonexistent-virtio-test".into(),
            preferred_transport: TransportKind::Tcp,
            connect_timeout_ms: 500,
            command_timeout_ms: 1000,
        };
        let result = select_transport(&config).await;
        assert!(result.is_err());
        // Error message should mention both transports
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("all transports failed"));
    }
}
