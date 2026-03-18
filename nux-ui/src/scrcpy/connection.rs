//! Scrcpy video stream connection — accepts the video socket and reads H.264 frames.

use std::io::{self, Read};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

/// Device metadata received at the start of the video stream.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
}

/// A single video frame packet from the scrcpy stream.
#[derive(Debug)]
pub struct VideoPacket {
    pub pts: u64,
    pub data: Vec<u8>,
}

/// Accepts the scrcpy video connection on the given port.
/// Blocks until the server connects (with timeout).
pub fn accept_video_connection(port: u16) -> Result<(TcpStream, DeviceInfo), String> {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}"))
        .map_err(|e| format!("Failed to bind port {port}: {e}"))?;

    listener
        .set_nonblocking(false)
        .map_err(|e| format!("set_nonblocking failed: {e}"))?;

    // Wait up to 30 seconds for the server to connect
    let stream = wait_for_connection(&listener, Duration::from_secs(30))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("set_read_timeout failed: {e}"))?;

    // Read device info (first 68 bytes)
    let device_info = read_device_info(&stream)?;

    Ok((stream, device_info))
}

fn wait_for_connection(listener: &TcpListener, timeout: Duration) -> Result<TcpStream, String> {
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("set_nonblocking failed: {e}"))?;

    let start = std::time::Instant::now();
    loop {
        match listener.accept() {
            Ok((stream, _addr)) => {
                stream
                    .set_nonblocking(false)
                    .map_err(|e| format!("set_nonblocking failed: {e}"))?;
                return Ok(stream);
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                if start.elapsed() > timeout {
                    return Err("Timeout waiting for scrcpy server connection".to_owned());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(format!("Accept failed: {e}")),
        }
    }
}

fn read_device_info(stream: &TcpStream) -> Result<DeviceInfo, String> {
    let mut buf = [0u8; 68];
    let mut stream_ref = stream;
    stream_ref
        .read_exact(&mut buf)
        .map_err(|e| format!("Failed to read device info: {e}"))?;

    // First 64 bytes: device name (UTF-8, null-padded)
    let name_end = buf[..64].iter().position(|&b| b == 0).unwrap_or(64);
    let name = String::from_utf8_lossy(&buf[..name_end]).to_string();

    // Bytes 64-67: not used in newer scrcpy (codec info)
    // Width/height come from the first video frame's SPS

    Ok(DeviceInfo {
        name,
        width: 0,  // Will be set from SPS
        height: 0, // Will be set from SPS
    })
}

/// Read the next video packet from the stream.
/// Scrcpy sends: [PTS: 8 bytes BE] [size: 4 bytes BE] [data: size bytes]
pub fn read_video_packet(stream: &mut TcpStream) -> Result<VideoPacket, String> {
    // Read PTS (8 bytes, big-endian)
    let mut pts_buf = [0u8; 8];
    stream
        .read_exact(&mut pts_buf)
        .map_err(|e| format!("Failed to read PTS: {e}"))?;
    let pts = u64::from_be_bytes(pts_buf);

    // Read packet size (4 bytes, big-endian)
    let mut size_buf = [0u8; 4];
    stream
        .read_exact(&mut size_buf)
        .map_err(|e| format!("Failed to read packet size: {e}"))?;
    let size = u32::from_be_bytes(size_buf) as usize;

    if size > 10_000_000 {
        return Err(format!("Packet too large: {size} bytes"));
    }

    // Read packet data
    let mut data = vec![0u8; size];
    stream
        .read_exact(&mut data)
        .map_err(|e| format!("Failed to read packet data ({size} bytes): {e}"))?;

    Ok(VideoPacket { pts, data })
}
