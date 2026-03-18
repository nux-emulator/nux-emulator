//! Scrcpy server management — push and start the server on the Android device.

use std::process::{Child, Command, Stdio};

const SCRCPY_SERVER_PATH: &str = "/usr/share/scrcpy/scrcpy-server";
const DEVICE_SERVER_PATH: &str = "/data/local/tmp/scrcpy-server.jar";
const ADB_SERIAL: &str = "127.0.0.1:6520";
const SCRCPY_VERSION: &str = "3.3.4";

/// Push the scrcpy server to the device.
pub fn push_server() -> Result<(), String> {
    let output = Command::new("adb")
        .args([
            "-s",
            ADB_SERIAL,
            "push",
            SCRCPY_SERVER_PATH,
            DEVICE_SERVER_PATH,
        ])
        .output()
        .map_err(|e| format!("adb push failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "adb push failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Set up ADB reverse tunnel for the video socket.
pub fn setup_tunnel(local_port: u16) -> Result<(), String> {
    // Remove any existing reverse tunnel
    let _ = Command::new("adb")
        .args([
            "-s",
            ADB_SERIAL,
            "reverse",
            "--remove",
            &format!("localabstract:scrcpy"),
        ])
        .output();

    // Create reverse tunnel: device connects to scrcpy abstract socket → host TCP port
    let output = Command::new("adb")
        .args([
            "-s",
            ADB_SERIAL,
            "reverse",
            "localabstract:scrcpy",
            &format!("tcp:{local_port}"),
        ])
        .output()
        .map_err(|e| format!("adb reverse failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "adb reverse failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Start the scrcpy server on the device. Returns the server process handle.
pub fn start_server(max_size: u16, bit_rate: u32) -> Result<Child, String> {
    let child = Command::new("adb")
        .args([
            "-s",
            ADB_SERIAL,
            "shell",
            &format!(
                "CLASSPATH={DEVICE_SERVER_PATH} app_process / com.genymobile.scrcpy.Server \
                 {SCRCPY_VERSION} \
                 tunnel_forward=false \
                 audio=false \
                 control=true \
                 max_size={max_size} \
                 video_bit_rate={bit_rate} \
                 max_fps=60 \
                 video_codec=h264 \
                 send_frame_meta=true"
            ),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start scrcpy server: {e}"))?;

    Ok(child)
}

/// Clean up: remove reverse tunnel.
pub fn cleanup_tunnel() {
    let _ = Command::new("adb")
        .args(["-s", ADB_SERIAL, "reverse", "--remove-all"])
        .output();
}
