//! Keymap hint overlay rendered on top of the display area.

use gtk::prelude::*;
use gtk4 as gtk;

/// Build the keymap overlay widget.
///
/// Returns a `GtkBox` that is placed as an overlay child on top of the
/// display `GtkGLArea`. It starts hidden and is toggled by the
/// `win.toggle-keymap-overlay` action.
///
/// In the integration phase this will read `nux-core::keymap::OverlayHint`
/// entries and position labels at the correct screen coordinates.
pub fn build_keymap_overlay() -> gtk::Box {
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .css_classes(["keymap-overlay"])
        .visible(false)
        .build();

    let label = gtk::Label::builder()
        .label("Keymap overlay — hints will appear here")
        .css_classes(["dim-label"])
        .build();
    container.append(&label);

    container
}
