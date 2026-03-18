//! Scrcpy server management — simplified for ADB screenrecord approach.
//! The scrcpy server push is kept for future use but not needed for screenrecord.

const ADB_SERIAL: &str = "127.0.0.1:6520";

/// Check if ADB is connected and device is ready.
pub fn check_device() -> Result<(), String> {
    let output = std::process::Command::new("adb")
        .args(["-s", ADB_SERIAL, "shell", "getprop", "sys.boot_completed"])
        .output()
        .map_err(|e| format!("ADB failed: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if stdout == "1" {
        Ok(())
    } else {
        Err("Device not ready".to_owned())
    }
}
