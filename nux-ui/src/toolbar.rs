//! Sidebar toolbar with emulator action buttons.

use gtk::prelude::*;
use gtk4 as gtk;

/// Build the right-side vertical toolbar.
///
/// Contains icon buttons for: Screenshot, Volume Up, Volume Down, Shake,
/// Rotate, Install APK, Toggle Keymap Overlay, Settings, Fullscreen.
/// Each button is wired to a `win.*` action.
pub fn build_toolbar() -> gtk::Box {
    let toolbar = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .width_request(48)
        .hexpand(false)
        .vexpand(true)
        .valign(gtk::Align::Start)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(4)
        .margin_end(4)
        .css_classes(["toolbar"])
        .build();

    let buttons: &[(&str, &str, &str)] = &[
        ("camera-photo-symbolic", "Screenshot", "win.screenshot"),
        ("audio-volume-high-symbolic", "Volume Up", "win.volume-up"),
        (
            "audio-volume-low-symbolic",
            "Volume Down",
            "win.volume-down",
        ),
        ("phone-oldschool-symbolic", "Shake Device", "win.shake"),
        ("object-rotate-right-symbolic", "Rotate", "win.rotate"),
        (
            "application-x-executable-symbolic",
            "Install APK",
            "win.install-apk",
        ),
        (
            "input-keyboard-symbolic",
            "Toggle Keymap Overlay",
            "win.toggle-keymap-overlay",
        ),
        ("emblem-system-symbolic", "Settings", "win.open-settings"),
        (
            "view-fullscreen-symbolic",
            "Fullscreen",
            "win.toggle-fullscreen",
        ),
    ];

    for &(icon, tooltip, action) in buttons {
        let btn = gtk::Button::builder()
            .icon_name(icon)
            .tooltip_text(tooltip)
            .action_name(action)
            .has_frame(false)
            .build();
        toolbar.append(&btn);
    }

    toolbar
}
