//! ADB protocol message framing and stream multiplexing.
//!
//! Implements the subset of the ADB protocol needed for Nux: connection
//! handshake (CNXN), stream open/close (OPEN/OKAY/CLSE), and data
//! transfer (WRTE). AUTH is not implemented — the guest `adbd` runs
//! in insecure mode inside the VM.

use crate::adb::types::{AdbError, AdbResult};

/// ADB protocol version used in CNXN messages.
pub const ADB_VERSION: u32 = 0x0100_0001; // A_VERSION

/// Default maximum payload size.
pub const MAX_PAYLOAD: u32 = 256 * 1024;

/// ADB command constants.
pub const CMD_CNXN: u32 = 0x4E58_4E43; // "CNXN"
pub const CMD_OPEN: u32 = 0x4E45_504F; // "OPEN"
pub const CMD_OKAY: u32 = 0x5941_4B4F; // "OKAY"
pub const CMD_CLSE: u32 = 0x4553_4C43; // "CLSE"
pub const CMD_WRTE: u32 = 0x4554_5257; // "WRTE"
pub const CMD_AUTH: u32 = 0x4854_5541; // "AUTH"

/// Size of the ADB message header in bytes.
pub const HEADER_SIZE: usize = 24;

/// An ADB protocol message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbMessage {
    /// Command identifier (CNXN, OPEN, OKAY, CLSE, WRTE, AUTH).
    pub command: u32,
    /// First argument (meaning depends on command).
    pub arg0: u32,
    /// Second argument (meaning depends on command).
    pub arg1: u32,
    /// Payload data.
    pub data: Vec<u8>,
}

impl AdbMessage {
    /// Create a new message with the given command, arguments, and data.
    pub fn new(command: u32, arg0: u32, arg1: u32, data: Vec<u8>) -> Self {
        Self {
            command,
            arg0,
            arg1,
            data,
        }
    }

    /// Create a CNXN (connection) message with the host banner.
    pub fn cnxn(banner: &str) -> Self {
        let mut data = banner.as_bytes().to_vec();
        data.push(0); // null-terminated
        Self::new(CMD_CNXN, ADB_VERSION, MAX_PAYLOAD, data)
    }

    /// Create an OPEN message to open a new stream.
    pub fn open(local_id: u32, destination: &str) -> Self {
        let mut data = destination.as_bytes().to_vec();
        data.push(0); // null-terminated
        Self::new(CMD_OPEN, local_id, 0, data)
    }

    /// Create an OKAY (ready) message.
    pub fn okay(local_id: u32, remote_id: u32) -> Self {
        Self::new(CMD_OKAY, local_id, remote_id, Vec::new())
    }

    /// Create a WRTE (write) message.
    pub fn wrte(local_id: u32, remote_id: u32, data: Vec<u8>) -> Self {
        Self::new(CMD_WRTE, local_id, remote_id, data)
    }

    /// Create a CLSE (close) message.
    pub fn clse(local_id: u32, remote_id: u32) -> Self {
        Self::new(CMD_CLSE, local_id, remote_id, Vec::new())
    }

    /// Calculate the data checksum per ADB protocol spec.
    pub fn checksum(data: &[u8]) -> u32 {
        data.iter().map(|&b| u32::from(b)).sum()
    }

    /// Serialize this message to bytes (header + data).
    pub fn to_bytes(&self) -> Vec<u8> {
        #[allow(clippy::cast_possible_truncation)]
        let data_len = self.data.len() as u32;
        let checksum = Self::checksum(&self.data);
        let magic = self.command ^ 0xFFFF_FFFF;

        let mut buf = Vec::with_capacity(HEADER_SIZE + self.data.len());
        buf.extend_from_slice(&self.command.to_le_bytes());
        buf.extend_from_slice(&self.arg0.to_le_bytes());
        buf.extend_from_slice(&self.arg1.to_le_bytes());
        buf.extend_from_slice(&data_len.to_le_bytes());
        buf.extend_from_slice(&checksum.to_le_bytes());
        buf.extend_from_slice(&magic.to_le_bytes());
        buf.extend_from_slice(&self.data);
        buf
    }

    /// Deserialize a message from bytes (header + data).
    ///
    /// # Errors
    ///
    /// Returns `AdbError::ProtocolError` if the buffer is too short, the magic
    /// value doesn't match, or the checksum is invalid.
    pub fn from_bytes(buf: &[u8]) -> AdbResult<Self> {
        if buf.len() < HEADER_SIZE {
            return Err(AdbError::ProtocolError(format!(
                "message too short: {} bytes, need at least {HEADER_SIZE}",
                buf.len()
            )));
        }

        let command = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let arg0 = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let arg1 = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let data_len = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]) as usize;
        let checksum = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
        let magic = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);

        if magic != command ^ 0xFFFF_FFFF {
            return Err(AdbError::ProtocolError(
                "magic mismatch in message header".to_owned(),
            ));
        }

        if buf.len() < HEADER_SIZE + data_len {
            return Err(AdbError::ProtocolError(format!(
                "incomplete message: have {} data bytes, expected {data_len}",
                buf.len() - HEADER_SIZE
            )));
        }

        let data = buf[HEADER_SIZE..HEADER_SIZE + data_len].to_vec();

        if Self::checksum(&data) != checksum {
            return Err(AdbError::ProtocolError("data checksum mismatch".to_owned()));
        }

        Ok(Self {
            command,
            arg0,
            arg1,
            data,
        })
    }
}

/// Manages the CNXN handshake with the guest `adbd`.
pub struct Handshake {
    /// Maximum payload size negotiated with the guest.
    pub max_payload: u32,
    /// Device banner from the guest (e.g. "`device::ro.product.model`=...").
    pub device_banner: String,
}

impl Handshake {
    /// Perform the CNXN handshake.
    ///
    /// Sends a CNXN message with the host banner and waits for the guest's
    /// CNXN response. Returns the negotiated parameters.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::ProtocolError` if the guest sends an unexpected
    /// response or the handshake fails.
    pub fn from_response(response: &AdbMessage) -> AdbResult<Self> {
        if response.command != CMD_CNXN {
            return Err(AdbError::ProtocolError(format!(
                "expected CNXN response, got command 0x{:08X}",
                response.command
            )));
        }

        let max_payload = response.arg1;
        let device_banner = String::from_utf8_lossy(&response.data)
            .trim_end_matches('\0')
            .to_owned();

        Ok(Self {
            max_payload,
            device_banner,
        })
    }

    /// Build the host CNXN message to send to the guest.
    pub fn host_message() -> AdbMessage {
        AdbMessage::cnxn("host::nux-emulator")
    }
}

/// Tracks a single logical ADB stream (multiplexed over the connection).
#[derive(Debug, Clone)]
pub struct StreamState {
    /// Local stream ID (assigned by us).
    pub local_id: u32,
    /// Remote stream ID (assigned by the guest).
    pub remote_id: u32,
    /// Whether the stream is open.
    pub open: bool,
}

/// Manages multiple logical streams over a single ADB connection.
pub struct StreamManager {
    next_local_id: u32,
    streams: Vec<StreamState>,
}

impl StreamManager {
    /// Create a new stream manager.
    pub fn new() -> Self {
        Self {
            next_local_id: 1,
            streams: Vec::new(),
        }
    }

    /// Allocate a new local stream ID and register it as pending (no remote ID yet).
    pub fn open_stream(&mut self) -> u32 {
        let id = self.next_local_id;
        self.next_local_id += 1;
        self.streams.push(StreamState {
            local_id: id,
            remote_id: 0,
            open: false,
        });
        id
    }

    /// Handle an OKAY response: set the remote ID and mark the stream as open.
    ///
    /// # Errors
    ///
    /// Returns `AdbError::ProtocolError` if the local ID is not found.
    pub fn handle_okay(&mut self, local_id: u32, remote_id: u32) -> AdbResult<()> {
        let stream = self
            .streams
            .iter_mut()
            .find(|s| s.local_id == local_id)
            .ok_or_else(|| {
                AdbError::ProtocolError(format!("OKAY for unknown local stream {local_id}"))
            })?;
        stream.remote_id = remote_id;
        stream.open = true;
        Ok(())
    }

    /// Handle a CLSE message: mark the stream as closed.
    pub fn handle_close(&mut self, local_id: u32) {
        if let Some(stream) = self.streams.iter_mut().find(|s| s.local_id == local_id) {
            stream.open = false;
        }
    }

    /// Look up a stream by local ID.
    pub fn get_stream(&self, local_id: u32) -> Option<&StreamState> {
        self.streams.iter().find(|s| s.local_id == local_id)
    }

    /// Remove a closed stream.
    pub fn remove_stream(&mut self, local_id: u32) {
        self.streams.retain(|s| s.local_id != local_id);
    }
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_round_trip() {
        let msg = AdbMessage::new(CMD_CNXN, ADB_VERSION, MAX_PAYLOAD, b"host::test\0".to_vec());
        let bytes = msg.to_bytes();
        let parsed = AdbMessage::from_bytes(&bytes).unwrap();
        assert_eq!(msg, parsed);
    }

    #[test]
    fn message_round_trip_empty_data() {
        let msg = AdbMessage::okay(1, 2);
        let bytes = msg.to_bytes();
        let parsed = AdbMessage::from_bytes(&bytes).unwrap();
        assert_eq!(msg, parsed);
    }

    #[test]
    fn message_bad_magic_rejected() {
        let msg = AdbMessage::okay(1, 2);
        let mut bytes = msg.to_bytes();
        // Corrupt the magic field (last 4 bytes of header)
        bytes[20] ^= 0xFF;
        let result = AdbMessage::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn message_bad_checksum_rejected() {
        let msg = AdbMessage::new(CMD_WRTE, 1, 2, b"hello".to_vec());
        let mut bytes = msg.to_bytes();
        // Corrupt a data byte
        bytes[HEADER_SIZE] ^= 0xFF;
        let result = AdbMessage::from_bytes(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn message_too_short_rejected() {
        let result = AdbMessage::from_bytes(&[0u8; 10]);
        assert!(result.is_err());
    }

    #[test]
    fn cnxn_handshake_parses_response() {
        let guest_cnxn = AdbMessage::new(
            CMD_CNXN,
            ADB_VERSION,
            4096,
            b"device::ro.product.model=Nux\0".to_vec(),
        );
        let hs = Handshake::from_response(&guest_cnxn).unwrap();
        assert_eq!(hs.max_payload, 4096);
        assert_eq!(hs.device_banner, "device::ro.product.model=Nux");
    }

    #[test]
    fn cnxn_handshake_rejects_non_cnxn() {
        let msg = AdbMessage::okay(1, 2);
        let result = Handshake::from_response(&msg);
        assert!(result.is_err());
    }

    #[test]
    fn stream_manager_open_and_okay() {
        let mut mgr = StreamManager::new();
        let id = mgr.open_stream();
        assert_eq!(id, 1);
        assert!(!mgr.get_stream(id).unwrap().open);

        mgr.handle_okay(id, 100).unwrap();
        let stream = mgr.get_stream(id).unwrap();
        assert!(stream.open);
        assert_eq!(stream.remote_id, 100);
    }

    #[test]
    fn stream_manager_close_and_remove() {
        let mut mgr = StreamManager::new();
        let id = mgr.open_stream();
        mgr.handle_okay(id, 100).unwrap();

        mgr.handle_close(id);
        assert!(!mgr.get_stream(id).unwrap().open);

        mgr.remove_stream(id);
        assert!(mgr.get_stream(id).is_none());
    }

    #[test]
    fn stream_manager_okay_unknown_id_errors() {
        let mut mgr = StreamManager::new();
        let result = mgr.handle_okay(999, 100);
        assert!(result.is_err());
    }

    #[test]
    fn checksum_calculation() {
        assert_eq!(AdbMessage::checksum(b""), 0);
        assert_eq!(AdbMessage::checksum(b"A"), 65);
        assert_eq!(
            AdbMessage::checksum(b"hello"),
            u32::from(b'h') + u32::from(b'e') + u32::from(b'l') + u32::from(b'l') + u32::from(b'o')
        );
    }
}
