//! Scrcpy video stream — uses ADB screenrecord for raw H.264 streaming.
//!
//! Much simpler than the scrcpy protocol — just pipes raw H.264 NAL units
//! from `adb exec-out screenrecord --output-format=h264`.

use std::io::Read;
use std::process::{Child, Command, Stdio};

const ADB_SERIAL: &str = "127.0.0.1:6520";

/// Start ADB screenrecord and return the process with its stdout for reading H.264.
pub fn start_screen_stream(width: u16, height: u16) -> Result<Child, String> {
    let size = format!("{width}x{height}");

    let child = Command::new("adb")
        .args([
            "-s",
            ADB_SERIAL,
            "exec-out",
            "screenrecord",
            "--output-format=h264",
            &format!("--size={size}"),
            "--bit-rate",
            "8000000",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start screenrecord: {e}"))?;

    Ok(child)
}

/// Read H.264 NAL units from the stream.
/// Returns chunks of raw H.264 data for the decoder.
pub fn read_h264_chunk(stdout: &mut dyn Read, buf: &mut [u8]) -> Result<usize, String> {
    stdout.read(buf).map_err(|e| format!("Read error: {e}"))
}
