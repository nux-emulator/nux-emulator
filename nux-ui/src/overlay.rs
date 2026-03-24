//! Keymap hint overlay rendered on top of the display area.
//!
//! Renders semi-transparent key labels at the correct screen coordinates
//! on top of the Android display. Toggled by `win.toggle-keymap-overlay`.

use gtk::prelude::*;
use gtk4 as gtk;
use nux_core::keymap::OverlayHint;

/// Build the keymap overlay widget.
///
/// Returns a `GtkFixed` positioned over the display. Key labels are placed
/// at scaled coordinates matching the Android screen layout.
pub fn build_keymap_overlay() -> gtk::Fixed {
    let fixed = gtk::Fixed::builder()
        .can_target(false) // Don't intercept input events
        .visible(false)
        .build();

    // Add CSS for styling
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        ".keymap-hint {
            background-color: rgba(0, 0, 0, 0.6);
            color: white;
            border-radius: 6px;
            padding: 4px 8px;
            font-size: 12px;
            font-weight: bold;
            font-family: monospace;
            min-width: 24px;
            min-height: 24px;
        }
        .keymap-hint-joystick {
            background-color: rgba(50, 120, 255, 0.5);
            border: 2px solid rgba(100, 160, 255, 0.8);
            border-radius: 50%;
            min-width: 48px;
            min-height: 48px;
        }",
    );
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().unwrap(),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    fixed
}

/// Update the overlay with hints from the keymap engine.
/// Call this when the keymap is loaded/changed or the display is resized.
pub fn update_overlay(
    fixed: &gtk::Fixed,
    hints: &[OverlayHint],
    display_width: i32,
    display_height: i32,
    video_width: u32,
    video_height: u32,
) {
    // Remove existing children
    while let Some(child) = fixed.first_child() {
        fixed.remove(&child);
    }

    if hints.is_empty() || display_width <= 0 || display_height <= 0 {
        return;
    }

    // Compute aspect-ratio-preserving mapping from video coords to widget coords
    let vw = video_width as f64;
    let vh = video_height as f64;
    let dw = display_width as f64;
    let dh = display_height as f64;

    let video_aspect = vw / vh;
    let display_aspect = dw / dh;

    let (scale, offset_x, offset_y) = if video_aspect > display_aspect {
        let s = dw / vw;
        (s, 0.0, (dh - vh * s) / 2.0)
    } else {
        let s = dh / vh;
        (s, (dw - vw * s) / 2.0, 0.0)
    };

    for hint in hints {
        let x = offset_x + hint.position.0 as f64 * scale;
        let y = offset_y + hint.position.1 as f64 * scale;

        let css_class = if hint.binding_type == "Joystick" {
            "keymap-hint-joystick"
        } else {
            "keymap-hint"
        };

        let label = gtk::Label::builder()
            .label(&hint.key.to_uppercase())
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .css_classes([css_class])
            .can_target(false)
            .build();

        // Center the label on the position
        fixed.put(&label, x - 16.0, y - 16.0);
    }
}
