//! Display area — launches scrcpy and overlays it on the GTK4 display area.
//!
//! Spawns scrcpy as a borderless X11 window and positions it over the
//! display area of the Nux GTK4 window, keeping it synced on resize/move.

use gtk::glib;
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
    window_id: Arc<Mutex<Option<String>>>,
}

impl ScrcpyHandle {
    fn new() -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            window_id: Arc::new(Mutex::new(None)),
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
        *self.window_id.lock().unwrap() = None;
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
        .css_classes(["nux-display"])
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

/// Start scrcpy and overlay it on the display area.
pub fn start_scrcpy(display: &gtk::Box, window: &adw::ApplicationWindow) -> ScrcpyHandle {
    let handle = ScrcpyHandle::new();

    // Hide placeholder
    set_placeholder_visible(display, false);

    // Spawn scrcpy with X11 backend
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
        .env("SDL_VIDEODRIVER", "x11")
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

            // After scrcpy starts, find and position its window
            let handle_wid = handle.window_id.clone();
            let display_clone = display.clone();
            let window_clone = window.clone();

            glib::timeout_add_seconds_local_once(4, move || {
                if let Some(wid) = find_scrcpy_window() {
                    *handle_wid.lock().unwrap() = Some(wid.clone());
                    position_scrcpy_over_display(&wid, &display_clone, &window_clone);

                    // Keep syncing position on resize/move
                    start_position_sync(wid, display_clone, window_clone);
                }
            });
        }
        Err(e) => {
            log::error!("Failed to start scrcpy: {e}");
            set_placeholder_visible(display, true);
            set_placeholder_text(display, &format!("scrcpy failed: {e}"));
        }
    }

    handle
}

/// Stop scrcpy and show placeholder.
pub fn stop_scrcpy(display: &gtk::Box, handle: &ScrcpyHandle) {
    handle.stop();
    set_placeholder_visible(display, true);
    set_placeholder_text(display, "Click Start VM to begin");
}

/// Show/hide placeholder content.
fn set_placeholder_visible(display: &gtk::Box, visible: bool) {
    let mut child = display.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        widget.set_visible(visible);
    }
}

fn set_placeholder_text(display: &gtk::Box, text: &str) {
    if let Some(label) = display.last_child() {
        if let Some(lbl) = label.downcast_ref::<gtk::Label>() {
            lbl.set_label(text);
        }
    }
}

/// Find scrcpy's X11 window ID.
fn find_scrcpy_window() -> Option<String> {
    let output = Command::new("xdotool")
        .args(["search", "--name", "NuxDisplay"])
        .env(
            "DISPLAY",
            std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_owned()),
        )
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let wid = stdout.trim().lines().next()?.to_owned();
    if wid.is_empty() { None } else { Some(wid) }
}

/// Position scrcpy window over the display area of the Nux window.
fn position_scrcpy_over_display(
    scrcpy_wid: &str,
    display: &gtk::Box,
    _window: &adw::ApplicationWindow,
) {
    // Get the display widget's position on screen
    let (x, y) = get_widget_screen_position(display);
    let width = display.width();
    let height = display.height();

    if width <= 0 || height <= 0 {
        return;
    }

    let display_env = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_owned());

    // Move and resize scrcpy window
    let _ = Command::new("xdotool")
        .args(["windowmove", scrcpy_wid, &x.to_string(), &y.to_string()])
        .env("DISPLAY", &display_env)
        .output();

    let _ = Command::new("xdotool")
        .args([
            "windowsize",
            scrcpy_wid,
            &width.to_string(),
            &height.to_string(),
        ])
        .env("DISPLAY", &display_env)
        .output();

    // Keep scrcpy above our window
    let _ = Command::new("xdotool")
        .args(["windowactivate", scrcpy_wid])
        .env("DISPLAY", &display_env)
        .output();
}

/// Get widget's absolute screen position using xdotool on the parent window.
fn get_widget_screen_position(widget: &gtk::Box) -> (i32, i32) {
    let display_env = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_owned());

    // Find the Nux Emulator window position
    let output = Command::new("xdotool")
        .args(["search", "--name", "Nux Emulator"])
        .env("DISPLAY", &display_env)
        .output();

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        if let Some(nux_wid) = stdout.trim().lines().next() {
            let geo = Command::new("xdotool")
                .args(["getwindowgeometry", "--shell", nux_wid])
                .env("DISPLAY", &display_env)
                .output();

            if let Ok(geo_out) = geo {
                let geo_str = String::from_utf8_lossy(&geo_out.stdout);
                let mut x = 0i32;
                let mut y = 0i32;
                for line in geo_str.lines() {
                    if let Some(val) = line.strip_prefix("X=") {
                        x = val.parse().unwrap_or(0);
                    }
                    if let Some(val) = line.strip_prefix("Y=") {
                        y = val.parse().unwrap_or(0);
                    }
                }
                // Offset for header bar (~47px) and sidebar position
                let header_offset = 47;
                let _ = widget; // widget position within window
                return (x, y + header_offset);
            }
        }
    }

    (0, 47)
}

/// Periodically sync scrcpy window position with display area.
fn start_position_sync(scrcpy_wid: String, display: gtk::Box, window: adw::ApplicationWindow) {
    glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        // Check if scrcpy window still exists
        let display_env = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_owned());
        let exists = Command::new("xdotool")
            .args(["getwindowname", &scrcpy_wid])
            .env("DISPLAY", &display_env)
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);

        if !exists {
            return glib::ControlFlow::Break;
        }

        position_scrcpy_over_display(&scrcpy_wid, &display, &window);
        glib::ControlFlow::Continue
    });
}

/// Reset display when VM stops.
pub fn show_stopped(display: &gtk::Box) {
    set_placeholder_visible(display, true);
    set_placeholder_text(display, "Click Start VM to begin");
}

#[allow(dead_code)]
pub fn show_running(display: &gtk::Box) {
    set_placeholder_visible(display, false);
}
