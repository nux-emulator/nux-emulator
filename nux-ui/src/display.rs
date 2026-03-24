//! Display area — native Wayland compositor display.
//!
//! Receives raw ARGB8888 frames from crosvm via our Wayland compositor
//! and renders them directly in a GTK4 DrawingArea. Zero encode/decode overhead.

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::rc::Rc;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

use crate::scrcpy::control::ControlSocket;
use crate::wayland_compositor::{DmabufFrame, FrameData, WaylandFrame};

/// Handle to the running display stream.
pub struct ScrcpyHandle {
    running: Arc<AtomicBool>,
    video_width: Arc<AtomicU32>,
    video_height: Arc<AtomicU32>,
    #[allow(dead_code)]
    control: Arc<Mutex<Option<ControlSocket>>>,
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

/// Build the display widget.
pub fn build_display() -> (gtk::Overlay, gtk::DrawingArea) {
    let picture = gtk::Picture::builder()
        .hexpand(true)
        .vexpand(true)
        .content_fit(gtk::ContentFit::Contain)
        .build();

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

/// Start display from Wayland compositor frame slot.
pub fn start_wayland_display(
    overlay: &gtk::Overlay,
    input_area: &gtk::DrawingArea,
    _window: &adw::ApplicationWindow,
    frame_slot: std::sync::Arc<crate::wayland_compositor::FrameSlot>,
    _wayland_input: crate::wayland_compositor::WaylandInput,
) -> ScrcpyHandle {
    let running = Arc::new(AtomicBool::new(true));
    let video_width = Arc::new(AtomicU32::new(720));
    let video_height = Arc::new(AtomicU32::new(1280));
    let control: Arc<Mutex<Option<ControlSocket>>> = Arc::new(Mutex::new(None));

    let picture = overlay
        .child()
        .and_then(|w| w.downcast::<gtk::Picture>().ok())
        .expect("Overlay child should be a Picture");

    // Render on frame clock — take latest frame from shared slot
    let render_count = Rc::new(std::cell::Cell::new(0u64));
    let rc2 = render_count.clone();
    let vw2 = video_width.clone();
    let vh2 = video_height.clone();

    picture.add_tick_callback(move |pic, _clock| {
        if let Some(frame_data) = frame_slot.take() {
            rc2.set(rc2.get() + 1);

            match frame_data {
                FrameData::Dmabuf(dmabuf) => {
                    vw2.store(dmabuf.width, Ordering::Relaxed);
                    vh2.store(dmabuf.height, Ordering::Relaxed);
                    if let Some(texture) = create_dmabuf_texture(pic, &dmabuf) {
                        pic.set_paintable(Some(&texture));
                    }
                }
                FrameData::Shm(frame) => {
                    vw2.store(frame.width, Ordering::Relaxed);
                    vh2.store(frame.height, Ordering::Relaxed);
                    // Zero-copy: glib::Bytes references the mmap'd memory directly
                    let data = frame.data();
                    let bytes = glib::Bytes::from(data);
                    let texture = gdk::MemoryTexture::new(
                        frame.width as i32,
                        frame.height as i32,
                        gdk::MemoryFormat::R8g8b8a8Premultiplied,
                        &bytes,
                        frame.stride as usize,
                    );
                    pic.set_paintable(Some(&texture));
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // FPS logger
    let rc3 = render_count.clone();
    glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
        let rendered = rc3.get();
        if rendered > 0 {
            log::info!(
                "render: {rendered} frames rendered (render rate ~{}/s)",
                rendered / 2
            );
        }
        rc3.set(0);
        glib::ControlFlow::Continue
    });

    let handle = ScrcpyHandle {
        running,
        video_width: video_width.clone(),
        video_height: video_height.clone(),
        control: control.clone(),
    };

    // Use scrcpy control socket for input (ADB-based, works independently of display)
    // Wayland seat input goes to crosvm display layer, not Android.
    // Android input comes through vhost-user devices managed by webRTC.
    // Since we killed webRTC, we use scrcpy control socket instead.
    let control_for_input = control.clone();
    std::thread::spawn(move || {
        // Wait a moment for ADB to be ready
        std::thread::sleep(std::time::Duration::from_secs(3));
        match connect_scrcpy_control() {
            Ok(ctrl) => {
                *control_for_input.lock().unwrap() = Some(ctrl);
                log::info!("display: scrcpy control socket connected for input");
            }
            Err(e) => {
                log::error!("display: scrcpy control failed: {e}");
            }
        }
    });

    setup_input_controllers(input_area, input_area, video_width, video_height, control);
    log::info!("display: input controllers attached");

    handle
}

fn setup_input_controllers(
    area: &gtk::DrawingArea,
    display_area: &gtk::DrawingArea,
    video_width: Arc<AtomicU32>,
    video_height: Arc<AtomicU32>,
    control: Arc<Mutex<Option<ControlSocket>>>,
) {
    // Right click -> back
    let right_click = gtk::GestureClick::new();
    right_click.set_button(3);
    let ctrl2 = control.clone();
    right_click.connect_released(move |_, _, _, _| {
        if let Ok(mut guard) = ctrl2.lock() {
            if let Some(cs) = guard.as_mut() {
                cs.back();
            }
        }
    });
    area.add_controller(right_click);

    // Drag gesture for taps and swipes
    let drag = gtk::GestureDrag::new();
    drag.set_button(1);

    let da = display_area.clone();
    let vw = video_width.clone();
    let vh = video_height.clone();
    let ctrl3 = control.clone();
    let area_for_focus = area.clone();

    drag.connect_drag_begin(move |_, x, y| {
        area_for_focus.grab_focus();
        let vw = vw.load(Ordering::Relaxed);
        let vh = vh.load(Ordering::Relaxed);
        let (ax, ay) = widget_to_android(&da, x, y, vw, vh);
        if ax >= 0 && ay >= 0 {
            if let Ok(mut guard) = ctrl3.lock() {
                if let Some(cs) = guard.as_mut() {
                    cs.touch_down(ax as u32, ay as u32);
                }
            }
        }
    });

    let da3 = display_area.clone();
    let vw3 = video_width.clone();
    let vh3 = video_height.clone();
    let ctrl4 = control.clone();

    drag.connect_drag_update(move |gesture, offset_x, offset_y| {
        if let Some((start_x, start_y)) = gesture.start_point() {
            let x = start_x + offset_x;
            let y = start_y + offset_y;
            let vw = vw3.load(Ordering::Relaxed);
            let vh = vh3.load(Ordering::Relaxed);
            let (ax, ay) = widget_to_android(&da3, x, y, vw, vh);
            if ax >= 0 && ay >= 0 {
                if let Ok(mut guard) = ctrl4.lock() {
                    if let Some(cs) = guard.as_mut() {
                        cs.touch_move(ax as u32, ay as u32);
                    }
                }
            }
        }
    });

    let da4 = display_area.clone();
    let vw4 = video_width;
    let vh4 = video_height;
    let ctrl5 = control.clone();

    drag.connect_drag_end(move |gesture, offset_x, offset_y| {
        if let Some((start_x, start_y)) = gesture.start_point() {
            let x = start_x + offset_x;
            let y = start_y + offset_y;
            let vw = vw4.load(Ordering::Relaxed);
            let vh = vh4.load(Ordering::Relaxed);
            let (ax, ay) = widget_to_android(&da4, x, y, vw, vh);
            if ax >= 0 && ay >= 0 {
                if let Ok(mut guard) = ctrl5.lock() {
                    if let Some(cs) = guard.as_mut() {
                        cs.touch_up(ax as u32, ay as u32);
                    }
                }
            }
        }
    });
    area.add_controller(drag);

    // Keyboard input
    let key_ctrl = gtk::EventControllerKey::new();
    let ctrl6 = control;

    key_ctrl.connect_key_pressed(move |_, keyval, _keycode, modifier| {
        if let Ok(mut guard) = ctrl6.lock() {
            if let Some(cs) = guard.as_mut() {
                if let Some(android_key) = gdk_key_to_android(keyval) {
                    let meta = gdk_modifier_to_android(modifier);
                    cs.key_meta(android_key, meta);
                    return glib::Propagation::Stop;
                }
                if let Some(ch) = keyval.to_unicode() {
                    if !ch.is_control() {
                        let mut buf = [0u8; 4];
                        let s = ch.encode_utf8(&mut buf);
                        cs.inject_text(s);
                        return glib::Propagation::Stop;
                    }
                }
            }
        }
        glib::Propagation::Proceed
    });
    area.add_controller(key_ctrl);
}

fn widget_to_android(
    widget: &impl IsA<gtk::Widget>,
    wx: f64,
    wy: f64,
    video_w: u32,
    video_h: u32,
) -> (i32, i32) {
    let widget_w = widget.width() as f64;
    let widget_h = widget.height() as f64;

    if widget_w <= 0.0 || widget_h <= 0.0 || video_w == 0 || video_h == 0 {
        return (-1, -1);
    }

    let screen_w = 720.0f64;
    let screen_h = 1280.0f64;

    let video_aspect = video_w as f64 / video_h as f64;
    let widget_aspect = widget_w / widget_h;

    let (render_w, render_h, offset_x, offset_y) = if video_aspect > widget_aspect {
        let rw = widget_w;
        let rh = widget_w / video_aspect;
        (rw, rh, 0.0, (widget_h - rh) / 2.0)
    } else {
        let rh = widget_h;
        let rw = widget_h * video_aspect;
        (rw, rh, (widget_w - rw) / 2.0, 0.0)
    };

    if wx < offset_x || wx > offset_x + render_w || wy < offset_y || wy > offset_y + render_h {
        return (-1, -1);
    }

    let nx = (wx - offset_x) / render_w;
    let ny = (wy - offset_y) / render_h;

    let ax = (nx * screen_w).round().clamp(0.0, screen_w - 1.0) as i32;
    let ay = (ny * screen_h).round().clamp(0.0, screen_h - 1.0) as i32;

    (ax, ay)
}

/// Connect scrcpy control socket only (for input when using Wayland display).
fn connect_scrcpy_control() -> Result<ControlSocket, String> {
    use crate::scrcpy::server;

    server::check_device()?;

    // Push and start scrcpy server
    let conn = server::ScrcpyConnection::connect(0, 8_000_000)?;

    // We need to keep reading the video stream or scrcpy server dies.
    // Spawn a drain thread.
    let mut video = conn
        .video_stream
        .try_clone()
        .map_err(|e| format!("clone: {e}"))?;
    std::thread::spawn(move || {
        let mut buf = [0u8; 65536];
        loop {
            match std::io::Read::read(&mut video, &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {} // discard video data
            }
        }
    });

    let ctrl = ControlSocket::new(
        conn.control_stream
            .try_clone()
            .map_err(|e| format!("Clone control: {e}"))?,
        720,
        1280,
    );
    // Keep connection alive by leaking it (it owns the server process)
    std::mem::forget(conn);
    Ok(ctrl)
}

/// Stop display stream.
pub fn stop_scrcpy(overlay: &gtk::Overlay, handle: &ScrcpyHandle) {
    handle.stop();
    if let Some(da) = overlay
        .child()
        .and_then(|w| w.downcast::<gtk::DrawingArea>().ok())
    {
        da.queue_draw();
    }
}

pub fn show_stopped(_overlay: &gtk::Overlay) {}

fn gdk_key_to_android(keyval: gdk::Key) -> Option<u32> {
    use gdk::Key;
    Some(match keyval {
        Key::Return | Key::KP_Enter => 66,
        Key::BackSpace => 67,
        Key::Delete | Key::KP_Delete => 112,
        Key::Tab => 61,
        Key::Escape => 111,
        Key::Home => 122,
        Key::End => 123,
        Key::Page_Up => 92,
        Key::Page_Down => 93,
        Key::Left | Key::KP_Left => 21,
        Key::Right | Key::KP_Right => 22,
        Key::Up | Key::KP_Up => 19,
        Key::Down | Key::KP_Down => 20,
        Key::space => 62,
        Key::F1 => 131,
        Key::F2 => 132,
        Key::F3 => 133,
        Key::F4 => 134,
        Key::F5 => 135,
        Key::F6 => 136,
        Key::F7 => 137,
        Key::F8 => 138,
        Key::F9 => 139,
        Key::F10 => 140,
        Key::F11 => 141,
        Key::F12 => 142,
        _ => return None,
    })
}

fn gdk_modifier_to_android(modifier: gdk::ModifierType) -> u32 {
    let mut meta = 0u32;
    if modifier.contains(gdk::ModifierType::SHIFT_MASK) {
        meta |= 1;
    }
    if modifier.contains(gdk::ModifierType::CONTROL_MASK) {
        meta |= 0x1000;
    }
    if modifier.contains(gdk::ModifierType::ALT_MASK) {
        meta |= 0x02;
    }
    meta
}

/// Create a GdkDmabufTexture from a DMA-BUF fd via C FFI.
/// This is zero-copy — the GPU reads the buffer directly.
fn create_dmabuf_texture(pic: &gtk::Picture, dmabuf: &DmabufFrame) -> Option<gdk::Texture> {
    use glib::translate::*;

    unsafe {
        // GdkDmabufTextureBuilder FFI
        unsafe extern "C" {
            fn gdk_dmabuf_texture_builder_new() -> *mut glib::gobject_ffi::GObject;
            fn gdk_dmabuf_texture_builder_set_display(
                builder: *mut glib::gobject_ffi::GObject,
                display: *mut glib::gobject_ffi::GObject,
            );
            fn gdk_dmabuf_texture_builder_set_width(
                builder: *mut glib::gobject_ffi::GObject,
                width: u32,
            );
            fn gdk_dmabuf_texture_builder_set_height(
                builder: *mut glib::gobject_ffi::GObject,
                height: u32,
            );
            fn gdk_dmabuf_texture_builder_set_fourcc(
                builder: *mut glib::gobject_ffi::GObject,
                fourcc: u32,
            );
            fn gdk_dmabuf_texture_builder_set_modifier(
                builder: *mut glib::gobject_ffi::GObject,
                modifier: u64,
            );
            fn gdk_dmabuf_texture_builder_set_n_planes(
                builder: *mut glib::gobject_ffi::GObject,
                n_planes: u32,
            );
            fn gdk_dmabuf_texture_builder_set_fd(
                builder: *mut glib::gobject_ffi::GObject,
                plane: u32,
                fd: i32,
            );
            fn gdk_dmabuf_texture_builder_set_stride(
                builder: *mut glib::gobject_ffi::GObject,
                plane: u32,
                stride: u32,
            );
            fn gdk_dmabuf_texture_builder_set_offset(
                builder: *mut glib::gobject_ffi::GObject,
                plane: u32,
                offset: u32,
            );
            fn gdk_dmabuf_texture_builder_set_premultiplied(
                builder: *mut glib::gobject_ffi::GObject,
                premultiplied: i32,
            );
            fn gdk_dmabuf_texture_builder_build(
                builder: *mut glib::gobject_ffi::GObject,
                destroy: *const std::ffi::c_void,
                data: *mut std::ffi::c_void,
                error: *mut *mut glib::ffi::GError,
            ) -> *mut glib::gobject_ffi::GObject;
            fn g_object_unref(obj: *mut glib::gobject_ffi::GObject);
        }

        let builder = gdk_dmabuf_texture_builder_new();
        if builder.is_null() {
            return None;
        }

        let display: gdk::Display = pic.display();
        let display_ptr =
            glib::translate::ToGlibPtr::<*mut gdk::ffi::GdkDisplay>::to_glib_none(&display)
                .0
                .cast::<glib::gobject_ffi::GObject>();

        gdk_dmabuf_texture_builder_set_display(builder, display_ptr);
        gdk_dmabuf_texture_builder_set_width(builder, dmabuf.width);
        gdk_dmabuf_texture_builder_set_height(builder, dmabuf.height);
        gdk_dmabuf_texture_builder_set_fourcc(builder, dmabuf.fourcc);
        gdk_dmabuf_texture_builder_set_modifier(builder, dmabuf.modifier);
        gdk_dmabuf_texture_builder_set_premultiplied(builder, 1);
        gdk_dmabuf_texture_builder_set_n_planes(builder, 1);
        gdk_dmabuf_texture_builder_set_fd(builder, 0, dmabuf.fd);
        gdk_dmabuf_texture_builder_set_stride(builder, 0, dmabuf.stride);
        gdk_dmabuf_texture_builder_set_offset(builder, 0, dmabuf.offset);

        let mut error: *mut glib::ffi::GError = std::ptr::null_mut();
        let texture = gdk_dmabuf_texture_builder_build(
            builder,
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut error,
        );

        g_object_unref(builder);

        if texture.is_null() {
            if !error.is_null() {
                let msg = std::ffi::CStr::from_ptr((*error).message).to_string_lossy();
                log::error!("dmabuf texture build failed: {msg}");
                glib::ffi::g_error_free(error);
            }
            return None;
        }

        // Transfer ownership to gdk::Texture
        Some(from_glib_full(texture.cast::<gdk::ffi::GdkTexture>()))
    }
}
