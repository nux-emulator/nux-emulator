//! Display area — launches scrcpy as a companion window.
//!
//! Spawns scrcpy connected to the Android VM. On tiling WMs (like PaperWM),
//! it tiles alongside the Nux control window. On floating WMs, it appears
//! as a separate borderless window.
//!
//! Future: implement scrcpy protocol natively in Rust for true embedding
//! via `GtkGLArea` with FFmpeg H.264 decoding.

use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

const ADB_SERIAL: &str = "127.0.0.1:6520";

/// Shared state for the scrcpy subprocess.
#[derive(Debug)]
pub struct ScrcpyHandle {
    process: Arc<Mutex<Option<Child>>>,
}

impl ScrcpyHandle {
    fn new() -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
        }
    }

    fn stop(&self) {
        if let Some(mut child) = self.process.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ScrcpyHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Build the display placeholder widget.
pub fn build_display() -> gtk::Box {
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .spacing(16)
        .hexpand(true)
        .vexpand(true)
        .build();

    let icon = gtk::Image::builder()
        .icon_name("computer-symbolic")
        .pixel_size(96)
        .css_classes(["dim-label"])
        .build();

    let label = gtk::Label::builder()
        .label("Click ▶ Start VM to begin")
        .css_classes(["title-2", "dim-label"])
        .build();

    container.append(&icon);
    container.append(&label);

    container
}

/// Start scrcpy as a companion display window.
pub fn start_scrcpy(display: &gtk::Box, _window: &adw::ApplicationWindow) -> ScrcpyHandle {
    let handle = ScrcpyHandle::new();

    // Update placeholder
    set_placeholder_text(display, "Display opened in scrcpy window");

    // Spawn scrcpy
    let process = Command::new("scrcpy")
        .args([
            "--serial",
            ADB_SERIAL,
            "--window-title",
            "Nux Emulator - Display",
            "--no-audio",
            "--stay-awake",
            "--show-touches",
        ])
        .env(
            "DISPLAY",
            std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_owned()),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    match process {
        Ok(child_proc) => {
            *handle.process.lock().unwrap() = Some(child_proc);
        }
        Err(e) => {
            log::error!("Failed to start scrcpy: {e}");
            set_placeholder_text(display, &format!("scrcpy failed: {e}"));
        }
    }

    handle
}

/// Stop scrcpy and reset display.
pub fn stop_scrcpy(display: &gtk::Box, handle: &ScrcpyHandle) {
    handle.stop();
    show_stopped(display);
}

/// Reset display when VM stops.
pub fn show_stopped(display: &gtk::Box) {
    set_placeholder_text(display, "Click ▶ Start VM to begin");
}

fn set_placeholder_text(display: &gtk::Box, text: &str) {
    if let Some(label) = display.last_child() {
        if let Some(lbl) = label.downcast_ref::<gtk::Label>() {
            lbl.set_label(text);
        }
    }
}
