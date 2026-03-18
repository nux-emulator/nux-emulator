//! Display area widget — `GtkGLArea` placeholder for Android framebuffer.

use gtk::prelude::*;
use gtk4 as gtk;

/// Build the display `GtkGLArea` that will render the Android surface.
///
/// For now this is a placeholder that clears to black. The actual frame
/// rendering from `nux-core::display::DisplayPipeline` will be wired in
/// the integration phase.
pub fn build_display() -> gtk::GLArea {
    let gl_area = gtk::GLArea::builder()
        .hexpand(true)
        .vexpand(true)
        .auto_render(true)
        .build();

    gl_area.connect_render(|_area, _ctx| {
        // Placeholder: clear to dark background
        // Safety: raw GL calls are needed for the clear; this is standard
        // GtkGLArea usage. The GL context is current when render fires.
        // We avoid `unsafe` by simply returning and letting GTK clear.
        gtk::glib::Propagation::Proceed
    });

    gl_area
}
