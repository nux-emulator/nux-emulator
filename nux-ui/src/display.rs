//! Display area widget — WebKitGTK view for Android display via WebRTC.

use webkit6::glib;
use webkit6::prelude::*;

/// Build the display widget that shows the Android screen.
///
/// Uses WebKitGTK to render the Cuttlefish WebRTC stream directly
/// in the GTK4 window. This gives us display + touch input + audio
/// with zero additional infrastructure.
pub fn build_display() -> webkit6::WebView {
    // Use ephemeral (private) session — more lenient with certs
    let network_session = webkit6::NetworkSession::new_ephemeral();

    let web_view = webkit6::WebView::builder()
        .hexpand(true)
        .vexpand(true)
        .network_session(&network_session)
        .build();

    // When TLS fails for localhost, allow the cert and schedule a reload
    web_view.connect_load_failed_with_tls_errors(|view, uri, cert, _errors| {
        if uri.starts_with("https://localhost") || uri.starts_with("https://127.0.0.1") {
            if let Some(session) = view.network_session() {
                session.allow_tls_certificate_for_host(cert, "localhost");
                session.allow_tls_certificate_for_host(cert, "127.0.0.1");
            }
            let uri_owned = uri.to_owned();
            let view_ref = view.clone();
            glib::idle_add_local_once(move || {
                view_ref.load_uri(&uri_owned);
            });
            true
        } else {
            false
        }
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
