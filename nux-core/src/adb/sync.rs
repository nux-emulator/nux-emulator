//! ADB sync protocol for file transfer (push/pull).
//!
//! Implements the sync sub-protocol used for transferring files between
//! host and guest. The sync session runs over an ADB stream opened to
//! the `sync:` service.

use crate::adb::protocol::{AdbMessage, CMD_CLSE, CMD_OKAY, CMD_WRTE, StreamManager};
use crate::adb::transport::ConnectedTransport;
use crate::adb::types::{AdbError, AdbResult};
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Sync protocol command identifiers (little-endian ASCII).
const _SYNC_STAT: [u8; 4] = *b"STAT";
const SYNC_SEND: &[u8; 4] = b"SEND";
const SYNC_RECV: &[u8; 4] = b"RECV";
const SYNC_DATA: &[u8; 4] = b"DATA";
const SYNC_DONE: &[u8; 4] = b"DONE";
const SYNC_FAIL: &[u8; 4] = b"FAIL";
const SYNC_OKAY: &[u8; 4] = b"OKAY";
const SYNC_QUIT: &[u8; 4] = b"QUIT";

/// Maximum chunk size for file data transfer (64 KB).
const SYNC_DATA_MAX: usize = 64 * 1024;

/// A sync protocol message (command + length + data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncMessage {
    /// 4-byte command identifier.
    pub command: [u8; 4],
    /// Payload data.
    pub data: Vec<u8>,
}

impl SyncMessage {
    /// Create a new sync message.
    pub fn new(command: [u8; 4], data: Vec<u8>) -> Self {
        Self { command, data }
    }

    /// Serialize to bytes: command (4) + length (4 LE) + data.
    pub fn to_bytes(&self) -> Vec<u8> {
        #[allow(clippy::cast_possible_truncation)]
        let len = self.data.len() as u32;
        let mut buf = Vec::with_capacity(8 + self.data.len());
        buf.extend_from_slice(&self.command);
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(&self.data);
        buf
    }

    /// Parse a sync message from bytes.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::SyncError` if the buffer is too short.
    pub fn from_bytes(buf: &[u8]) -> AdbResult<Self> {
        if buf.len() < 8 {
            return Err(AdbError::SyncError(format!(
                "sync message too short: {} bytes",
                buf.len()
            )));
        }
        let command: [u8; 4] = [buf[0], buf[1], buf[2], buf[3]];
        let data_len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;

        if buf.len() < 8 + data_len {
            return Err(AdbError::SyncError(format!(
                "sync message incomplete: have {}, need {}",
                buf.len() - 8,
                data_len
            )));
        }

        let data = buf[8..8 + data_len].to_vec();
        Ok(Self { command, data })
    }
}

/// A sync session running over an ADB stream.
///
/// Manages the lifecycle of a `sync:` stream for file push/pull operations.
struct SyncSession {
    local_id: u32,
    remote_id: u32,
}

impl SyncSession {
    /// Open a sync session by opening a `sync:` stream.
    async fn open(
        transport: &mut ConnectedTransport,
        streams: &mut StreamManager,
        timeout_ms: u64,
    ) -> AdbResult<Self> {
        let local_id = streams.open_stream();
        let open_msg = AdbMessage::open(local_id, "sync:");
        transport.write_message(&open_msg).await?;

        let timeout = Duration::from_millis(timeout_ms);
        let msg = tokio::time::timeout(timeout, transport.read_message())
            .await
            .map_err(|_| AdbError::Timeout("sync session open timed out".to_owned()))?
            .map_err(|e| AdbError::SyncError(format!("sync open failed: {e}")))?;

        if msg.command != CMD_OKAY {
            return Err(AdbError::SyncError(format!(
                "expected OKAY for sync open, got 0x{:08X}",
                msg.command
            )));
        }

        let remote_id = msg.arg0;
        streams.handle_okay(local_id, remote_id)?;

        Ok(Self {
            local_id,
            remote_id,
        })
    }

    /// Send sync data over the ADB stream.
    async fn send_sync_data(
        &self,
        transport: &mut ConnectedTransport,
        data: &[u8],
    ) -> AdbResult<()> {
        let msg = AdbMessage::wrte(self.local_id, self.remote_id, data.to_vec());
        transport.write_message(&msg).await?;
        // Wait for OKAY acknowledgment
        let resp = transport.read_message().await?;
        if resp.command != CMD_OKAY {
            return Err(AdbError::SyncError(format!(
                "expected OKAY after WRTE, got 0x{:08X}",
                resp.command
            )));
        }
        Ok(())
    }

    /// Receive sync response data from the ADB stream.
    async fn recv_sync_data(&self, transport: &mut ConnectedTransport) -> AdbResult<Vec<u8>> {
        let msg = transport.read_message().await?;
        match msg.command {
            CMD_WRTE => {
                // Acknowledge the data
                let ack = AdbMessage::okay(self.local_id, self.remote_id);
                transport.write_message(&ack).await?;
                Ok(msg.data)
            }
            CMD_CLSE => Err(AdbError::SyncError(
                "stream closed unexpectedly during sync".to_owned(),
            )),
            _ => Err(AdbError::SyncError(format!(
                "unexpected command 0x{:08X} during sync recv",
                msg.command
            ))),
        }
    }

    /// Close the sync session.
    async fn close(
        self,
        transport: &mut ConnectedTransport,
        streams: &mut StreamManager,
    ) -> AdbResult<()> {
        // Send QUIT sync command
        let quit = SyncMessage::new(*SYNC_QUIT, Vec::new());
        let quit_bytes = quit.to_bytes();
        let msg = AdbMessage::wrte(self.local_id, self.remote_id, quit_bytes);
        let _ = transport.write_message(&msg).await;

        // Close the ADB stream
        let clse = AdbMessage::clse(self.local_id, self.remote_id);
        let _ = transport.write_message(&clse).await;
        streams.handle_close(self.local_id);
        streams.remove_stream(self.local_id);
        Ok(())
    }
}

/// Push a file from the host to the guest.
///
/// Streams the file in 64 KB chunks so large files don't consume
/// excessive memory. Calls `progress(bytes_sent, total_bytes)` after
/// each chunk.
///
/// # Errors
///
/// Returns `AdbError::FileNotFound` if the host file doesn't exist,
/// `AdbError::SyncError` on protocol errors, or `AdbError::Io` on
/// read/write failures.
pub async fn push_file<F>(
    transport: &mut ConnectedTransport,
    streams: &mut StreamManager,
    host_path: &Path,
    guest_path: &str,
    permissions: u32,
    timeout_ms: u64,
    progress: F,
) -> AdbResult<()>
where
    F: Fn(u64, u64),
{
    if !host_path.exists() {
        return Err(AdbError::FileNotFound(host_path.to_path_buf()));
    }

    let metadata = tokio::fs::metadata(host_path).await?;
    let total_size = metadata.len();

    let session = SyncSession::open(transport, streams, timeout_ms).await?;

    // Send SEND command: "SEND" + length + "{path},{mode}"
    let send_arg = format!("{guest_path},{permissions:o}");
    let send_msg = SyncMessage::new(*SYNC_SEND, send_arg.into_bytes());
    session
        .send_sync_data(transport, &send_msg.to_bytes())
        .await?;

    // Stream file data in chunks
    let mut file = tokio::fs::File::open(host_path).await?;
    let mut bytes_sent: u64 = 0;
    let mut chunk_buf = vec![0u8; SYNC_DATA_MAX];

    loop {
        let n = file.read(&mut chunk_buf).await?;
        if n == 0 {
            break;
        }

        let data_msg = SyncMessage::new(*SYNC_DATA, chunk_buf[..n].to_vec());
        session
            .send_sync_data(transport, &data_msg.to_bytes())
            .await?;

        bytes_sent += n as u64;
        progress(bytes_sent, total_size);
    }

    // Send DONE with mtime (use current time)
    #[allow(clippy::cast_possible_truncation)]
    let mtime = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32;
    let done_msg = SyncMessage::new(*SYNC_DONE, mtime.to_le_bytes().to_vec());
    session
        .send_sync_data(transport, &done_msg.to_bytes())
        .await?;

    // Read response — expect OKAY or FAIL
    let resp_data = session.recv_sync_data(transport).await?;
    if resp_data.len() >= 4 {
        let resp_cmd: [u8; 4] = [resp_data[0], resp_data[1], resp_data[2], resp_data[3]];
        if resp_cmd == *SYNC_FAIL {
            let reason = String::from_utf8_lossy(&resp_data[8..]).to_string();
            session.close(transport, streams).await?;
            return Err(AdbError::SyncError(format!("push failed: {reason}")));
        }
        if resp_cmd != *SYNC_OKAY {
            session.close(transport, streams).await?;
            return Err(AdbError::SyncError(format!(
                "unexpected sync response: {:?}",
                String::from_utf8_lossy(&resp_cmd)
            )));
        }
    }

    session.close(transport, streams).await?;
    Ok(())
}

/// Pull a file from the guest to the host.
///
/// Streams the file in chunks so large files don't consume excessive
/// memory. Calls `progress(bytes_received, total_bytes)` after each
/// chunk. `total_bytes` is 0 if the size is unknown.
///
/// # Errors
///
/// Returns `AdbError::SyncError` on protocol errors, `AdbError::GuestError`
/// if the file doesn't exist on the guest, or `AdbError::Io` on write failures.
pub async fn pull_file<F>(
    transport: &mut ConnectedTransport,
    streams: &mut StreamManager,
    guest_path: &str,
    host_path: &Path,
    timeout_ms: u64,
    progress: F,
) -> AdbResult<()>
where
    F: Fn(u64, u64),
{
    let session = SyncSession::open(transport, streams, timeout_ms).await?;

    // Send RECV command
    let recv_msg = SyncMessage::new(*SYNC_RECV, guest_path.as_bytes().to_vec());
    session
        .send_sync_data(transport, &recv_msg.to_bytes())
        .await?;

    // Receive DATA chunks and write to host file
    let mut file = tokio::fs::File::create(host_path).await?;
    let mut bytes_received: u64 = 0;

    loop {
        let chunk = session.recv_sync_data(transport).await?;
        if chunk.len() < 4 {
            return Err(AdbError::SyncError("sync response too short".to_owned()));
        }

        let cmd: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];

        if cmd == *SYNC_DATA {
            if chunk.len() < 8 {
                return Err(AdbError::SyncError(
                    "DATA message missing length".to_owned(),
                ));
            }
            let data_len = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as usize;
            let data = &chunk[8..8 + data_len.min(chunk.len() - 8)];

            file.write_all(data).await?;
            bytes_received += data.len() as u64;
            progress(bytes_received, 0);
        } else if cmd == *SYNC_DONE {
            break;
        } else if cmd == *SYNC_FAIL {
            let reason = if chunk.len() > 8 {
                String::from_utf8_lossy(&chunk[8..]).to_string()
            } else {
                "unknown error".to_owned()
            };
            session.close(transport, streams).await?;
            return Err(AdbError::GuestError(format!("pull failed: {reason}")));
        } else {
            return Err(AdbError::SyncError(format!(
                "unexpected sync command: {:?}",
                String::from_utf8_lossy(&cmd)
            )));
        }
    }

    file.flush().await?;

    session.close(transport, streams).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_message_round_trip() {
        let msg = SyncMessage::new(*SYNC_SEND, b"/data/local/tmp/test.apk,0644".to_vec());
        let bytes = msg.to_bytes();
        let parsed = SyncMessage::from_bytes(&bytes).unwrap();
        assert_eq!(msg, parsed);
    }

    #[test]
    fn sync_message_empty_data() {
        let msg = SyncMessage::new(*SYNC_QUIT, Vec::new());
        let bytes = msg.to_bytes();
        assert_eq!(bytes.len(), 8); // 4 cmd + 4 length
        let parsed = SyncMessage::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.command, *SYNC_QUIT);
        assert!(parsed.data.is_empty());
    }

    #[test]
    fn sync_message_too_short() {
        let result = SyncMessage::from_bytes(&[0u8; 3]);
        assert!(result.is_err());
    }

    #[test]
    fn sync_message_data_chunk() {
        let data = vec![0xAB; 1024];
        let msg = SyncMessage::new(*SYNC_DATA, data.clone());
        let bytes = msg.to_bytes();
        let parsed = SyncMessage::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.command, *SYNC_DATA);
        assert_eq!(parsed.data, data);
    }

    #[test]
    fn sync_stat_message() {
        let msg = SyncMessage::new(_SYNC_STAT, b"/sdcard/test.txt".to_vec());
        let bytes = msg.to_bytes();
        let parsed = SyncMessage::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.command, _SYNC_STAT);
    }

    #[test]
    fn sync_done_with_mtime() {
        let mtime: u32 = 1_700_000_000;
        let msg = SyncMessage::new(*SYNC_DONE, mtime.to_le_bytes().to_vec());
        let bytes = msg.to_bytes();
        let parsed = SyncMessage::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.command, *SYNC_DONE);
        let parsed_mtime = u32::from_le_bytes([
            parsed.data[0],
            parsed.data[1],
            parsed.data[2],
            parsed.data[3],
        ]);
        assert_eq!(parsed_mtime, mtime);
    }

    #[test]
    fn sync_fail_message() {
        let reason = b"No such file or directory";
        let msg = SyncMessage::new(*SYNC_FAIL, reason.to_vec());
        let bytes = msg.to_bytes();
        let parsed = SyncMessage::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.command, *SYNC_FAIL);
        assert_eq!(
            String::from_utf8_lossy(&parsed.data),
            "No such file or directory"
        );
    }

    #[test]
    fn sync_okay_message() {
        let msg = SyncMessage::new(*SYNC_OKAY, Vec::new());
        let bytes = msg.to_bytes();
        let parsed = SyncMessage::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.command, *SYNC_OKAY);
    }
}
