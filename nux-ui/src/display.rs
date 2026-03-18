//! Display area widget — WebKitGTK view for Android display via WebRTC.

use webkit6::prelude::*;

/// Build the display widget that shows the Android screen.
///
/// Uses WebKitGTK to render the Cuttlefish WebRTC stream directly
/// in the GTK4 window. This gives us display + touch input + audio
/// with zero additional infrastructure.
pub fn build_display() -> webkit6::WebView {
    let web_view = webkit6::WebView::builder()
        .hexpand(true)
        .vexpand(true)
        .build();

    // Accept self-signed certs from Cuttlefish WebRTC server
    web_view.connect_load_failed_with_tls_errors(|_view, _uri, _cert, _errors| {
        true // Allow all TLS errors (local only)
    });

    // Initially show black screen
    web_view.load_html(
        "<html><body style='background:#000;margin:0'></body></html>",
        None,
    );

    web_view
}

/// Load the WebRTC display URL into the web view.
pub fn load_webrtc_display(web_view: &webkit6::WebView) {
    web_view.load_uri("https://localhost:8443/client.html?deviceId=cvd-1");
}

/// Clear the display (show black screen).
pub fn clear_display(web_view: &webkit6::WebView) {
    web_view.load_html(
        "<html><body style='background:#000;margin:0'></body></html>",
        None,
    );
}
