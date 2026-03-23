//! Fast input routing via persistent ADB shell connection.
//!
//! Maintains a persistent `adb shell` process and writes input commands
//! to its stdin. This avoids the ~200ms overhead of spawning a new
//! process for each input event (~10-30ms instead).

use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

const ADB_SERIAL: &str = "127.0.0.1:6520";

/// Persistent ADB shell for fast input injection.
pub struct AdbInput {
    process: Mutex<Child>,
}

impl AdbInput {
    /// Start a persistent ADB shell session.
    pub fn new() -> Result<Self, String> {
        let child = Command::new("adb")
            .args(["-s", ADB_SERIAL, "shell"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start adb shell: {e}"))?;

        Ok(Self {
            process: Mutex::new(child),
        })
    }

    /// Send a tap at the given coordinates.
    pub fn tap(&self, x: i32, y: i32) {
        self.send_cmd(&format!("input tap {x} {y}\n"));
    }

    /// Send a swipe.
    pub fn swipe(&self, x1: i32, y1: i32, x2: i32, y2: i32, duration_ms: u32) {
        self.send_cmd(&format!("input swipe {x1} {y1} {x2} {y2} {duration_ms}\n"));
    }

    /// Send a key event.
    pub fn key(&self, keycode: u32) {
        self.send_cmd(&format!("input keyevent {keycode}\n"));
    }

    /// Send back button.
    pub fn back(&self) {
        self.key(4);
    }

    fn send_cmd(&self, cmd: &str) {
        if let Ok(mut guard) = self.process.lock() {
            if let Some(stdin) = guard.stdin.as_mut() {
                let _ = stdin.write_all(cmd.as_bytes());
                let _ = stdin.flush();
            }
        }
    }
}

impl Drop for AdbInput {
    fn drop(&mut self) {
        if let Ok(mut child) = self.process.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
