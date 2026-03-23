//! Scrcpy server management and connection via forward mode.
//!
//! Uses `adb forward` — the server listens on the device,
//! we connect from the host. This is how the real scrcpy client works.
//! With `send_frame_meta=false`, the stream is raw H.264 Annex B.

use std::io::Read;
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const SCRCPY_SERVER_PATH: &str = "/usr/share/scrcpy/scrcpy-server";
const DEVICE_SERVER_PATH: &str = "/data/local/tmp/scrcpy-server.jar";
const ADB_SERIAL: &str = "127.0.0.1:6520";
const SCRCPY_VERSION: &str = "3.3.4";

/// Active scrcpy connection with video stream.
pub struct ScrcpyConnection {
    pub video_stream: TcpStream,
    pub device_name: String,
    _server_process: Child,
    local_port: u16,
}

impl ScrcpyConnection {
    /// Establish a scrcpy connection to the device.
    pub fn connect(max_size: u16, bit_rate: u32) -> Result<Self, String> {
        // 1. Push server
        log::info!("scrcpy: pushing server...");
        push_server()?;

        // 2. Set up forward tunnel
        let local_port = 27183u16;
        log::info!("scrcpy: setting up forward tunnel on port {local_port}...");
        setup_forward(local_port)?;

        // 3. Start server in forward mode
        log::info!("scrcpy: starting server...");
        let server_process = start_server(local_port, max_size, bit_rate)?;

        // 4. Connect to the video socket (retry until server is ready)
        log::info!("scrcpy: connecting to video stream...");
        let video_stream = connect_with_retry(local_port, 30)?;

        // 5. Read device name (64 bytes)
        let device_name = read_device_name(&video_stream)?;
        log::info!("scrcpy: connected to device: {device_name}");

        Ok(Self {
            video_stream,
            device_name,
            _server_process: server_process,
            local_port,
        })
    }

    /// Read raw H.264 data from the video stream.
    pub fn read_video(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        self.video_stream
            .read(buf)
            .map_err(|e| format!("Video read: {e}"))
    }
}

impl Drop for ScrcpyConnection {
    fn drop(&mut self) {
        let _ = self._server_process.kill();
        let _ = self._server_process.wait();
        cleanup_forward(self.local_port);
    }
}

fn push_server() -> Result<(), String> {
    let output = Command::new("adb")
        .args([
            "-s",
            ADB_SERIAL,
            "push",
            SCRCPY_SERVER_PATH,
            DEVICE_SERVER_PATH,
        ])
        .output()
        .map_err(|e| format!("adb push: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "adb push: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn setup_forward(local_port: u16) -> Result<(), String> {
    let _ = Command::new("adb")
        .args(["-s", ADB_SERIAL, "forward", "--remove-all"])
        .output();

    let output = Command::new("adb")
        .args([
            "-s",
            ADB_SERIAL,
            "forward",
            &format!("tcp:{local_port}"),
            "localabstract:scrcpy",
        ])
        .output()
        .map_err(|e| format!("adb forward: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "adb forward: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn start_server(local_port: u16, max_size: u16, bit_rate: u32) -> Result<Child, String> {
    let cmd = format!(
        "CLASSPATH={DEVICE_SERVER_PATH} app_process / com.genymobile.scrcpy.Server \
         {SCRCPY_VERSION} \
         tunnel_forward=true \
         audio=false \
         control=false \
         cleanup=false \
         max_size={max_size} \
         video_bit_rate={bit_rate} \
         max_fps=60 \
         video_codec=h264 \
         send_frame_meta=false"
    );

    Command::new("adb")
        .args(["-s", ADB_SERIAL, "shell", &cmd])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Start server: {e}"))
}

fn connect_with_retry(port: u16, max_attempts: u32) -> Result<TcpStream, String> {
    // Give server time to start listening
    std::thread::sleep(Duration::from_secs(2));

    for attempt in 1..=max_attempts {
        match TcpStream::connect(format!("127.0.0.1:{port}")) {
            Ok(stream) => {
                stream.set_nodelay(true).ok();
                stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
                log::info!("scrcpy: connected on attempt {attempt}");
                return Ok(stream);
            }
            Err(_) => {
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
    Err(format!("Failed to connect after {max_attempts} attempts"))
}

fn read_device_name(stream: &TcpStream) -> Result<String, String> {
    let mut s = stream;

    // scrcpy 3.x sends 1-byte status first
    let mut status = [0u8; 1];
    s.read_exact(&mut status)
        .map_err(|e| format!("Read status byte: {e}"))?;

    if status[0] != 0 {
        return Err(format!("Server returned error status: {}", status[0]));
    }

    // Then 64-byte device name
    let mut buf = [0u8; 64];
    s.read_exact(&mut buf)
        .map_err(|e| format!("Read device name: {e}"))?;
    let end = buf.iter().position(|&b| b == 0).unwrap_or(64);
    Ok(String::from_utf8_lossy(&buf[..end]).to_string())
}

fn cleanup_forward(port: u16) {
    let _ = Command::new("adb")
        .args([
            "-s",
            ADB_SERIAL,
            "forward",
            "--remove",
            &format!("tcp:{port}"),
        ])
        .output();
}

/// Check if device is ready.
pub fn check_device() -> Result<(), String> {
    let output = Command::new("adb")
        .args(["-s", ADB_SERIAL, "shell", "getprop", "sys.boot_completed"])
        .output()
        .map_err(|e| format!("ADB: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if stdout == "1" {
        Ok(())
    } else {
        Err("Device not ready".to_owned())
    }
}
