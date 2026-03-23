//! Display area — native H.264 decoding from ADB screenrecord.
//!
//! Streams raw H.264 from `adb exec-out screenrecord`, decodes with FFmpeg,
//! and renders RGB frames directly into a GtkPicture widget.

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

use crate::scrcpy::{decoder, input::AdbInput, server};

/// Handle to the running display stream.
pub struct ScrcpyHandle {
    running: Arc<AtomicBool>,
    video_width: Arc<AtomicU32>,
    video_height: Arc<AtomicU32>,
    input: Arc<Mutex<Option<AdbInput>>>,
}

impl std::fmt::Debug for ScrcpyHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrcpyHandle")
            .field("running", &self.running.load(Ordering::Relaxed))
            .finish()
    }
}

impl ScrcpyHandle {
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn video_width(&self) -> u32 {
        self.video_width.load(Ordering::Relaxed)
    }

    #[allow(dead_code)]
    pub fn video_height(&self) -> u32 {
        self.video_height.load(Ordering::Relaxed)
    }
}

impl Drop for ScrcpyHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Build the display widget with input controllers.
pub fn build_display() -> (gtk::Overlay, gtk::DrawingArea) {
    let picture = gtk::Picture::builder()
        .hexpand(true)
        .vexpand(true)
        .content_fit(gtk::ContentFit::Contain)
        .build();

    // Transparent overlay to capture input events
    let input_area = gtk::DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .can_focus(true)
        .focusable(true)
        .build();

    let overlay = gtk::Overlay::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&picture)
        .build();
    overlay.add_overlay(&input_area);

    (overlay, input_area)
}

/// Start streaming and set up input routing.
pub fn start_scrcpy(
    overlay: &gtk::Overlay,
    input_area: &gtk::DrawingArea,
    _window: &adw::ApplicationWindow,
) -> ScrcpyHandle {
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    let video_width = Arc::new(AtomicU32::new(720));
    let video_height = Arc::new(AtomicU32::new(1280));
    let vw = video_width.clone();
    let vh = video_height.clone();

    // Start persistent ADB input shell
    let input: Arc<Mutex<Option<AdbInput>>> = Arc::new(Mutex::new(AdbInput::new().ok()));

    let picture = overlay
        .child()
        .and_then(|w| w.downcast::<gtk::Picture>().ok())
        .expect("Overlay child should be a Picture");
    let picture_clone = picture.clone();

    let (tx, rx) = std::sync::mpsc::channel::<decoder::DecodedFrame>();

    // Decoder thread
    std::thread::spawn(move || {
        if let Err(e) = run_stream(running_clone, tx, vw, vh) {
            log::error!("Display stream error: {e}");
        }
    });

    // UI thread: render frames
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

    let handle = ScrcpyHandle {
        running,
        video_width: video_width.clone(),
        video_height: video_height.clone(),
        input: input.clone(),
    };

    setup_input_controllers(input_area, &picture, video_width, video_height, input);
    log::info!("display: input controllers attached");

    handle
}

/// Set up mouse click and drag controllers using persistent ADB input.
fn setup_input_controllers(
    area: &gtk::DrawingArea,
    picture: &gtk::Picture,
    video_width: Arc<AtomicU32>,
    video_height: Arc<AtomicU32>,
    input: Arc<Mutex<Option<AdbInput>>>,
) {
    // Click gesture → tap
    let click = gtk::GestureClick::new();
    click.set_button(1);

    let pic = picture.clone();
    let vw = video_width.clone();
    let vh = video_height.clone();
    let inp = input.clone();

    click.connect_released(move |_, _, x, y| {
        let w = pic.width() as f64;
        let h = pic.height() as f64;
        let vw_val = vw.load(Ordering::Relaxed);
        let vh_val = vh.load(Ordering::Relaxed);
        log::info!("CLICK widget({x:.0},{y:.0}) picture={w}x{h} video={vw_val}x{vh_val}");
        let (ax, ay) = widget_to_android(&pic, x, y, vw_val, vh_val);
        log::info!("  → android({ax},{ay})");
        if ax >= 0 && ay >= 0 {
            if let Ok(guard) = inp.lock() {
                if let Some(adb) = guard.as_ref() {
                    adb.tap(ax, ay);
                }
            }
        }
    });
    area.add_controller(click);

    // Right click → back button
    let right_click = gtk::GestureClick::new();
    right_click.set_button(3);
    let inp2 = input.clone();
    right_click.connect_released(move |_, _, _, _| {
        if let Ok(guard) = inp2.lock() {
            if let Some(adb) = guard.as_ref() {
                adb.back();
            }
        }
    });
    area.add_controller(right_click);

    // Drag gesture → swipe
    let drag = gtk::GestureDrag::new();
    let pic2 = picture.clone();
    let vw2 = video_width;
    let vh2 = video_height;
    let inp3 = input;

    drag.connect_drag_end(move |gesture, offset_x, offset_y| {
        if let Some((start_x, start_y)) = gesture.start_point() {
            let end_x = start_x + offset_x;
            let end_y = start_y + offset_y;
            let vw = vw2.load(Ordering::Relaxed);
            let vh = vh2.load(Ordering::Relaxed);
            let (ax1, ay1) = widget_to_android(&pic2, start_x, start_y, vw, vh);
            let (ax2, ay2) = widget_to_android(&pic2, end_x, end_y, vw, vh);

            let dist = ((offset_x * offset_x + offset_y * offset_y) as f64).sqrt();
            if dist > 10.0 && ax1 >= 0 && ay1 >= 0 {
                if let Ok(guard) = inp3.lock() {
                    if let Some(adb) = guard.as_ref() {
                        adb.swipe(ax1, ay1, ax2, ay2, 300);
                    }
                }
            }
        }
    });
    area.add_controller(drag);
}

/// Map widget coordinates to Android screen coordinates.
/// Maps from widget space → video space → actual screen space.
/// Video may be scaled down (max_size), but adb input expects real screen coords.
fn widget_to_android(
    picture: &gtk::Picture,
    wx: f64,
    wy: f64,
    video_w: u32,
    video_h: u32,
) -> (i32, i32) {
    let widget_w = picture.width() as f64;
    let widget_h = picture.height() as f64;

    if widget_w <= 0.0 || widget_h <= 0.0 || video_w == 0 || video_h == 0 {
        return (-1, -1);
    }

    // Actual Android screen resolution (adb input expects these coords)
    // The video may be scaled down by scrcpy's max_size parameter
    let screen_w = 720.0f64;
    let screen_h = 1280.0f64;

    let video_aspect = video_w as f64 / video_h as f64;
    let widget_aspect = widget_w / widget_h;

    // Calculate the rendered area within the widget (content-fit: contain)
    let (render_w, render_h, offset_x, offset_y) = if video_aspect > widget_aspect {
        let rw = widget_w;
        let rh = widget_w / video_aspect;
        (rw, rh, 0.0, (widget_h - rh) / 2.0)
    } else {
        let rh = widget_h;
        let rw = widget_h * video_aspect;
        (rw, rh, (widget_w - rw) / 2.0, 0.0)
    };

    // Check if click was outside the rendered area
    if wx < offset_x || wx > offset_x + render_w || wy < offset_y || wy > offset_y + render_h {
        return (-1, -1);
    }

    // Map widget coords → normalized (0..1) → actual screen coords
    let nx = (wx - offset_x) / render_w;
    let ny = (wy - offset_y) / render_h;

    let ax = (nx * screen_w).round().clamp(0.0, screen_w - 1.0) as i32;
    let ay = (ny * screen_h).round().clamp(0.0, screen_h - 1.0) as i32;

    (ax, ay)
}

fn run_stream(
    running: Arc<AtomicBool>,
    tx: std::sync::mpsc::Sender<decoder::DecodedFrame>,
    video_width: Arc<AtomicU32>,
    video_height: Arc<AtomicU32>,
) -> Result<(), String> {
    log::info!("display: checking device...");
    server::check_device()?;

    log::info!("display: connecting via scrcpy protocol...");
    let mut conn = server::ScrcpyConnection::connect(0, 16_000_000)?;

    log::info!("display: initializing H.264 decoder...");
    let mut h264 = decoder::H264Decoder::new()?;

    log::info!("display: streaming frames...");
    let mut buf = [0u8; 65536];

    while running.load(Ordering::Relaxed) {
        match conn.read_video(&mut buf) {
            Ok(0) => {
                log::info!("display: stream ended (EOF)");
                break;
            }
            Ok(n) => {
                let frames = h264.decode_chunk(&buf[..n]);
                for frame in frames {
                    video_width.store(frame.width, Ordering::Relaxed);
                    video_height.store(frame.height, Ordering::Relaxed);
                    if tx.send(frame).is_err() {
                        running.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            }
            Err(e) => {
                if running.load(Ordering::Relaxed) {
                    log::error!("display: stream error: {e}");
                }
                break;
            }
        }
    }

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
pub fn stop_scrcpy(overlay: &gtk::Overlay, handle: &ScrcpyHandle) {
    handle.stop();
    if let Some(picture) = overlay
        .child()
        .and_then(|w| w.downcast::<gtk::Picture>().ok())
    {
        picture.set_paintable(gdk::Paintable::NONE);
    }
}

/// Show stopped state.
pub fn show_stopped(_overlay: &gtk::Overlay) {}
