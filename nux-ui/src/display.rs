//! Display area — native H.264 decoding from ADB screenrecord.
//!
//! Streams raw H.264 from `adb exec-out screenrecord`, decodes with FFmpeg,
//! and renders RGB frames directly into a GtkPicture widget.

use gtk::gdk;
use gtk::glib;
use gtk4 as gtk;
use libadwaita as adw;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::scrcpy::{connection, decoder, server};

/// Handle to the running display stream.
#[derive(Debug)]
pub struct ScrcpyHandle {
    running: Arc<AtomicBool>,
}

impl ScrcpyHandle {
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl Drop for ScrcpyHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Build the display widget.
pub fn build_display() -> gtk::Picture {
    gtk::Picture::builder()
        .hexpand(true)
        .vexpand(true)
        .content_fit(gtk::ContentFit::Contain)
        .build()
}

/// Start streaming Android display into the picture widget.
pub fn start_scrcpy(picture: &gtk::Picture, _window: &adw::ApplicationWindow) -> ScrcpyHandle {
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    let picture_clone = picture.clone();

    let (tx, rx) = std::sync::mpsc::channel::<decoder::DecodedFrame>();

    // Decoder thread
    std::thread::spawn(move || {
        if let Err(e) = run_stream(running_clone, tx) {
            log::error!("Display stream error: {e}");
        }
    });

    // UI thread: render frames at ~60fps
    glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        let mut latest = None;
        while let Ok(frame) = rx.try_recv() {
            latest = Some(frame);
        }
        if let Some(frame) = latest {
            render_frame(&picture_clone, &frame);
        }
        glib::ControlFlow::Continue
    });

    ScrcpyHandle { running }
}

fn run_stream(
    running: Arc<AtomicBool>,
    tx: std::sync::mpsc::Sender<decoder::DecodedFrame>,
) -> Result<(), String> {
    log::info!("display: checking device...");
    server::check_device()?;

    log::info!("display: starting screenrecord stream...");
    let mut child = connection::start_screen_stream(720, 1280)?;

    let stdout = child
        .stdout
        .as_mut()
        .ok_or_else(|| "No stdout from screenrecord".to_owned())?;

    log::info!("display: initializing H.264 decoder...");
    let mut h264 = decoder::H264Decoder::new()?;

    log::info!("display: streaming frames...");
    let mut buf = vec![0u8; 65536];

    while running.load(Ordering::Relaxed) {
        match connection::read_h264_chunk(stdout, &mut buf) {
            Ok(0) => {
                log::info!("display: stream ended (EOF)");
                break;
            }
            Ok(n) => {
                let frames = h264.decode_chunk(&buf[..n]);
                for frame in frames {
                    if tx.send(frame).is_err() {
                        running.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            }
            Err(e) => {
                if running.load(Ordering::Relaxed) {
                    log::error!("display: read error: {e}");
                }
                break;
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    log::info!("display: stream stopped");
    Ok(())
}

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

/// Stop display stream.
pub fn stop_scrcpy(picture: &gtk::Picture, handle: &ScrcpyHandle) {
    handle.stop();
    picture.set_paintable(gdk::Paintable::NONE);
}

/// Show stopped state.
pub fn show_stopped(_picture: &gtk::Picture) {}
