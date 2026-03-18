//! Display area — native scrcpy client with embedded H.264 decoding.
//!
//! Implements the scrcpy protocol natively:
//! 1. Push scrcpy-server to device via ADB
//! 2. Start server, accept video connection
//! 3. Decode H.264 with FFmpeg
//! 4. Render RGB frames in GtkPicture

use gtk::gdk;
use gtk::glib;
use gtk4 as gtk;
use libadwaita as adw;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::scrcpy::{connection, decoder, server};

const VIDEO_PORT: u16 = 27183;
const MAX_SIZE: u16 = 0; // 0 = native resolution
const BIT_RATE: u32 = 8_000_000; // 8 Mbps

/// Handle to the running scrcpy client.
#[derive(Debug)]
pub struct ScrcpyHandle {
    running: Arc<AtomicBool>,
}

impl ScrcpyHandle {
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        server::cleanup_tunnel();
    }
}

impl Drop for ScrcpyHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Build the display widget.
pub fn build_display() -> gtk::Picture {
    let picture = gtk::Picture::builder()
        .hexpand(true)
        .vexpand(true)
        .content_fit(gtk::ContentFit::Contain)
        .build();

    picture
}

/// Start the native scrcpy client and render frames into the picture widget.
pub fn start_scrcpy(picture: &gtk::Picture, _window: &adw::ApplicationWindow) -> ScrcpyHandle {
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    let picture_clone = picture.clone();

    // Channel to send decoded frames from decoder thread to UI thread
    let (tx, rx) = std::sync::mpsc::channel::<decoder::DecodedFrame>();

    // Decoder thread: push server, connect, decode, send frames
    std::thread::spawn(move || {
        if let Err(e) = run_scrcpy_client(running_clone, tx) {
            log::error!("scrcpy client error: {e}");
        }
    });

    // UI thread: receive frames and render
    glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        // Drain all available frames, render the latest
        let mut latest_frame = None;
        while let Ok(frame) = rx.try_recv() {
            latest_frame = Some(frame);
        }

        if let Some(frame) = latest_frame {
            render_frame(&picture_clone, &frame);
        }

        glib::ControlFlow::Continue
    });

    ScrcpyHandle { running }
}

/// Run the scrcpy client protocol (blocking, runs in a thread).
fn run_scrcpy_client(
    running: Arc<AtomicBool>,
    tx: std::sync::mpsc::Sender<decoder::DecodedFrame>,
) -> Result<(), String> {
    log::info!("scrcpy: pushing server...");
    server::push_server()?;

    log::info!("scrcpy: setting up tunnel on port {VIDEO_PORT}...");
    server::setup_tunnel(VIDEO_PORT)?;

    log::info!("scrcpy: starting server...");
    let mut _server_proc = server::start_server(MAX_SIZE, BIT_RATE)?;

    log::info!("scrcpy: waiting for video connection...");
    let (mut stream, device_info) = connection::accept_video_connection(VIDEO_PORT)?;
    log::info!("scrcpy: connected to device: {}", device_info.name);

    log::info!("scrcpy: initializing H.264 decoder...");
    let mut h264_decoder = decoder::H264Decoder::new()?;

    log::info!("scrcpy: streaming frames...");
    while running.load(Ordering::Relaxed) {
        match connection::read_video_packet(&mut stream) {
            Ok(packet) => {
                let frames = h264_decoder.decode(&packet.data, packet.pts as i64);
                for frame in frames {
                    if tx.send(frame).is_err() {
                        // UI thread dropped the receiver
                        running.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            }
            Err(e) => {
                if running.load(Ordering::Relaxed) {
                    log::error!("scrcpy: read error: {e}");
                }
                break;
            }
        }
    }

    // Flush decoder
    for frame in h264_decoder.flush() {
        let _ = tx.send(frame);
    }

    server::cleanup_tunnel();
    log::info!("scrcpy: client stopped");
    Ok(())
}

/// Render a decoded RGB frame into the GtkPicture.
fn render_frame(picture: &gtk::Picture, frame: &decoder::DecodedFrame) {
    let bytes = glib::Bytes::from(&frame.data);

    let texture = gdk::MemoryTexture::new(
        frame.width as i32,
        frame.height as i32,
        gdk::MemoryFormat::R8g8b8,
        &bytes,
        frame.stride,
    );

    picture.set_paintable(Some(&texture));
}

/// Stop scrcpy and clear display.
pub fn stop_scrcpy(picture: &gtk::Picture, handle: &ScrcpyHandle) {
    handle.stop();
    picture.set_paintable(gdk::Paintable::NONE);
}

/// Show stopped state.
pub fn show_stopped(_picture: &gtk::Picture) {
    // Picture is already blank when no paintable is set
}
