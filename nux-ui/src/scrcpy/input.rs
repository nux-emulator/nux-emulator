//! Input routing — captures GTK4 mouse events and sends them to Android via ADB.

use std::process::Command;

const ADB_SERIAL: &str = "127.0.0.1:6520";

/// Send a tap event at the given Android screen coordinates.
pub fn send_tap(x: i32, y: i32) {
    let _ = Command::new("adb")
        .args([
            "-s",
            ADB_SERIAL,
            "shell",
            "input",
            "tap",
            &x.to_string(),
            &y.to_string(),
        ])
        .output();
}

/// Send a swipe event.
pub fn send_swipe(x1: i32, y1: i32, x2: i32, y2: i32, duration_ms: u32) {
    let _ = Command::new("adb")
        .args([
            "-s",
            ADB_SERIAL,
            "shell",
            "input",
            "swipe",
            &x1.to_string(),
            &y1.to_string(),
            &x2.to_string(),
            &y2.to_string(),
            &duration_ms.to_string(),
        ])
        .output();
}

/// Send a key event.
pub fn send_key(keycode: u32) {
    let _ = Command::new("adb")
        .args([
            "-s",
            ADB_SERIAL,
            "shell",
            "input",
            "keyevent",
            &keycode.to_string(),
        ])
        .output();
}

// Android keycodes
pub const KEYCODE_BACK: u32 = 4;
pub const KEYCODE_HOME: u32 = 3;
pub const KEYCODE_APP_SWITCH: u32 = 187;
pub const KEYCODE_VOLUME_UP: u32 = 24;
pub const KEYCODE_VOLUME_DOWN: u32 = 25;
pub const KEYCODE_POWER: u32 = 26;
