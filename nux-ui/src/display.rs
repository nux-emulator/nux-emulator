//! Display area widget — placeholder with browser launch for Android display.

use gtk::prelude::*;
use gtk4 as gtk;

/// Build the display placeholder widget.
///
/// Shows a status message and a button to open the display in the browser.
/// The actual Android display is rendered via WebRTC in the default browser
/// until we implement direct frame rendering from crosvm's Wayland socket.
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

    let open_display_btn = gtk::Button::builder()
        .label("Open Display in Browser")
        .css_classes(["suggested-action", "pill"])
        .halign(gtk::Align::Center)
        .visible(false)
        .build();

    open_display_btn.connect_clicked(|_| {
        let _ = std::process::Command::new("xdg-open")
            .arg("https://localhost:8443")
            .spawn();
    });

    container.append(&icon);
    container.append(&label);
    container.append(&open_display_btn);

    container
}

/// Update display status when VM boots.
pub fn show_running(display: &gtk::Box) {
    if let Some(label) = display.last_child().and_then(|w| w.prev_sibling()) {
        if let Some(lbl) = label.downcast_ref::<gtk::Label>() {
            lbl.set_label("Android is running");
        }
    }
    // Show the "Open Display" button
    if let Some(btn) = display.last_child() {
        btn.set_visible(true);
    }
    // Auto-open browser
    let _ = std::process::Command::new("xdg-open")
        .arg("https://localhost:8443")
        .spawn();
}

/// Reset display when VM stops.
pub fn show_stopped(display: &gtk::Box) {
    if let Some(label) = display.last_child().and_then(|w| w.prev_sibling()) {
        if let Some(lbl) = label.downcast_ref::<gtk::Label>() {
            lbl.set_label("Click Start VM to begin");
        }
    }
    if let Some(btn) = display.last_child() {
        btn.set_visible(false);
    }
}
