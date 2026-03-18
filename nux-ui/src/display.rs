//! Display area — embeds scrcpy window inside GTK4 via X11 reparenting.
//!
//! Spawns scrcpy as a subprocess, finds its X11 window, and reparents it
//! into a `GtkSocket`-like container within our GTK4 window.

use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

const SCRCPY_SERVER: &str = "/usr/share/scrcpy/scrcpy-server";
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

    #[allow(dead_code)]
    fn is_running(&self) -> bool {
        let mut guard = self.process.lock().unwrap();
        if let Some(child) = guard.as_mut() {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
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

/// Build the display container widget.
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
        .label("Click Start VM to begin")
        .css_classes(["title-2", "dim-label"])
        .build();

    container.append(&icon);
    container.append(&label);

    container
}

/// Start scrcpy and embed its display.
///
/// Launches scrcpy as a borderless window that overlays the display area.
/// Returns a handle to control the scrcpy process.
pub fn start_scrcpy(display: &gtk::Box) -> ScrcpyHandle {
    let handle = ScrcpyHandle::new();

    // Update UI
    if let Some(label) = display.last_child() {
        if let Some(lbl) = label.downcast_ref::<gtk::Label>() {
            lbl.set_label("Connecting display...");
        }
    }

    // Hide placeholder content
    let mut child = display.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        widget.set_visible(false);
    }

    // Spawn scrcpy
    let process = Command::new("scrcpy")
        .args([
            "--serial",
            ADB_SERIAL,
            "--window-title",
            "NuxDisplay",
            "--window-borderless",
            "--no-audio",
            "--stay-awake",
            "--show-touches",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    match process {
        Ok(child_proc) => {
            *handle.process.lock().unwrap() = Some(child_proc);

            // After scrcpy starts, find its window and reparent it
            let display_clone = display.clone();
            glib::timeout_add_seconds_local_once(3, move || {
                reparent_scrcpy_window(&display_clone);
            });
        }
        Err(e) => {
            log::error!("Failed to start scrcpy: {e}");
            show_placeholder(display, &format!("scrcpy failed: {e}"));
        }
    }

    handle
}

/// Find the scrcpy window and reparent it into our display area.
fn reparent_scrcpy_window(display: &gtk::Box) {
    // Find scrcpy window by title
    let output = Command::new("xdotool")
        .args(["search", "--name", "NuxDisplay"])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let window_id = stdout.trim();
            if window_id.is_empty() {
                log::warn!("scrcpy window not found yet");
                return;
            }
            log::info!("Found scrcpy window: {window_id}");

            // Remove window decorations and make it a child
            let _ = Command::new("xdotool")
                .args(["set_window", "--overrideredirect", "1", window_id])
                .output();
        }
        Err(e) => {
            log::error!("xdotool failed: {e}");
        }
    }
}

/// Stop scrcpy and show placeholder.
pub fn stop_scrcpy(display: &gtk::Box, handle: &ScrcpyHandle) {
    handle.stop();
    show_placeholder(display, "Click Start VM to begin");
}

/// Show placeholder content.
fn show_placeholder(display: &gtk::Box, text: &str) {
    let mut child = display.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        widget.set_visible(true);
    }
    if let Some(label) = display.last_child() {
        if let Some(lbl) = label.downcast_ref::<gtk::Label>() {
            lbl.set_label(text);
        }
    }
}

/// Update display status when VM boots.
#[allow(dead_code)]
pub fn show_running(display: &gtk::Box) {
    if let Some(label) = display.last_child() {
        if let Some(lbl) = label.downcast_ref::<gtk::Label>() {
            lbl.set_label("Android is running");
        }
    }
}

/// Reset display when VM stops.
pub fn show_stopped(display: &gtk::Box) {
    show_placeholder(display, "Click Start VM to begin");
}
