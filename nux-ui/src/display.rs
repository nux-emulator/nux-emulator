//! Display area — GPU-accelerated rendering via GdkGLTextureBuilder.
//!
//! Uploads frame data to a GL texture, wraps it with GdkGLTextureBuilder,
//! and sets it as paintable on GtkPicture. GTK composites the texture
//! directly — no offscreen FBO (GLArea), no MemoryTexture copy.

use glow::HasContext;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

use crate::scrcpy::control::ControlSocket;
use crate::wayland_compositor::{FrameData, WaylandFrame};

// ── Types ──

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

// ── GL state ──

struct GlState {
    gl: glow::Context,
    texture: glow::Texture,
    tex_width: u32,
    tex_height: u32,
    gl_context: gdk::GLContext,
    /// 0=portrait, 1=landscape (90° CW rotation of content)
    rotation: u32,
}

// ── Widget ──

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

// ── Display start ──

pub fn start_wayland_display(
    overlay: &gtk::Overlay,
    input_area: &gtk::DrawingArea,
    _window: &adw::ApplicationWindow,
    frame_slot: Arc<crate::wayland_compositor::FrameSlot>,
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

    let gl_state: Rc<RefCell<Option<GlState>>> = Rc::new(RefCell::new(None));
    let render_count = Rc::new(std::cell::Cell::new(0u64));
    let rc2 = render_count.clone();
    let vw2 = video_width.clone();
    let vh2 = video_height.clone();
    let gs = gl_state.clone();

    let pic = picture.clone();
    // Poll at ~120Hz (8ms) to catch every frame from crosvm's 60fps output.
    // GTK's tick callback is limited to ~30Hz by the desktop compositor.
    glib::timeout_add_local(std::time::Duration::from_millis(8), move || {
        if let Some(frame_data) = frame_slot.take() {
            rc2.set(rc2.get() + 1);

            if let FrameData::Shm(frame) = frame_data {
                vw2.store(frame.width, Ordering::Relaxed);
                vh2.store(frame.height, Ordering::Relaxed);

                if gs.borrow().is_none() {
                    match init_gl_state(&pic) {
                        Ok(state) => {
                            log::info!("display: GL texture renderer initialized");
                            *gs.borrow_mut() = Some(state);
                        }
                        Err(e) => {
                            log::error!("display: GL init failed: {e}");
                            return glib::ControlFlow::Continue;
                        }
                    }
                }

                if let Some(state) = gs.borrow_mut().as_mut() {
                    upload_and_present(state, &pic, &frame);
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
                "render: {rendered} frames (render rate ~{}/s)",
                rendered / 2
            );
        }
        rc3.set(0);
        glib::ControlFlow::Continue
    });

    // Scrcpy control for input — with continuous reconnection
    let ctrl = control.clone();
    let running_ctrl = running.clone();

    let handle = ScrcpyHandle {
        running,
        video_width: video_width.clone(),
        video_height: video_height.clone(),
        control: control.clone(),
    };

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));

        // Start audio bridge once
        start_audio_bridge();

        // Continuous reconnection loop
        loop {
            if !running_ctrl.load(Ordering::Relaxed) {
                break;
            }
            match connect_scrcpy_control() {
                Ok(c) => {
                    *ctrl.lock().unwrap() = Some(c);
                    log::info!("display: scrcpy control connected");

                    // Monitor the connection — wait until it breaks
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        if !running_ctrl.load(Ordering::Relaxed) {
                            return;
                        }
                        // Check if control socket is still alive
                        let alive = ctrl
                            .lock()
                            .unwrap()
                            .as_ref()
                            .map_or(false, |cs| cs.is_alive());
                        if !alive {
                            log::info!("display: scrcpy control died, will reconnect");
                            break;
                        }
                    }
                }
                Err(e) => {
                    log::warn!("display: scrcpy control failed: {e}, retrying in 3s...");
                }
            }
            *ctrl.lock().unwrap() = None;
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
    });

    setup_input_controllers(input_area, input_area, video_width, video_height, control);
    log::info!("display: input controllers attached");
    handle
}

// ── GL init ──

fn init_gl_state(pic: &gtk::Picture) -> Result<GlState, String> {
    let display = pic.display();
    let gl_context = display
        .create_gl_context()
        .map_err(|e| format!("create GL context: {e}"))?;
    gl_context.make_current();

    let gl = unsafe {
        let egl = libc::dlopen(
            b"libEGL.so.1\0".as_ptr().cast(),
            libc::RTLD_LAZY | libc::RTLD_NOLOAD,
        );
        if !egl.is_null() {
            type F = unsafe extern "C" fn(*const std::ffi::c_char) -> *const std::ffi::c_void;
            let f: F =
                std::mem::transmute(libc::dlsym(egl, b"eglGetProcAddress\0".as_ptr().cast()));
            glow::Context::from_loader_function(|name| {
                let c = std::ffi::CString::new(name).unwrap();
                unsafe { f(c.as_ptr()) }
            })
        } else {
            let glx = libc::dlopen(
                b"libGL.so.1\0".as_ptr().cast(),
                libc::RTLD_LAZY | libc::RTLD_NOLOAD,
            );
            if glx.is_null() {
                return Err("No GL loader".into());
            }
            type F = unsafe extern "C" fn(*const u8) -> *const std::ffi::c_void;
            let f: F =
                std::mem::transmute(libc::dlsym(glx, b"glXGetProcAddressARB\0".as_ptr().cast()));
            glow::Context::from_loader_function(|name| {
                let c = std::ffi::CString::new(name).unwrap();
                unsafe { f(c.as_ptr().cast()) }
            })
        }
    };

    let texture = unsafe {
        let tex = gl.create_texture().map_err(|e| e.to_string())?;
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        tex
    };

    Ok(GlState {
        gl,
        texture,
        tex_width: 0,
        tex_height: 0,
        gl_context,
        rotation: 0,
    })
}

// ── Upload + present via GdkGLTextureBuilder ──

fn upload_and_present(state: &mut GlState, pic: &gtk::Picture, frame: &WaylandFrame) {
    state.gl_context.make_current();

    let gl = &state.gl;
    let w = frame.width;
    let h = frame.height;
    let data = frame.data();

    // Detect rotation: check actual display orientation every ~60 frames
    static FRAME_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let count = FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    if count % 60 == 0 {
        // Check actual display orientation (not user_rotation setting)
        // mCurrentOrientation: 0=portrait, 1=landscape, 2=reverse portrait, 3=reverse landscape
        if let Ok(output) = std::process::Command::new("adb")
            .args(["-s", "127.0.0.1:6520", "shell", "dumpsys", "display"])
            .output()
        {
            let out = String::from_utf8_lossy(&output.stdout);
            let rot = out
                .lines()
                .find(|l| l.contains("mCurrentOrientation="))
                .and_then(|l| l.trim().strip_prefix("mCurrentOrientation="))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            // Landscape = 1 or 3
            let is_landscape = if rot == 1 || rot == 3 { 1 } else { 0 };
            if is_landscape != state.rotation {
                log::info!(
                    "display: orientation changed to {} (mCurrentOrientation={})",
                    if is_landscape == 1 {
                        "landscape"
                    } else {
                        "portrait"
                    },
                    rot
                );
                state.rotation = is_landscape;
                CURRENT_ROTATION.store(is_landscape, Ordering::Relaxed);
            }
        }
    }

    unsafe {
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, (frame.stride / 4) as i32);
        gl.bind_texture(glow::TEXTURE_2D, Some(state.texture));

        if w != state.tex_width || h != state.tex_height {
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                w as i32,
                h as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(data)),
            );
            state.tex_width = w;
            state.tex_height = h;
        } else {
            gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                w as i32,
                h as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(data)),
            );
        }

        gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
        gl.flush();
    }

    // Wrap GL texture with GdkGLTextureBuilder
    let tex_id = state.texture.0.get();

    if state.rotation == 1 && w < h {
        // Landscape: create rotated MemoryTexture (GL texture can't be rotated)
        let src_stride = frame.stride as usize;
        let rot_w = h as i32;
        let rot_h = w as i32;
        let rot_stride = rot_w as usize * 4;
        let mut rotated = vec![0u8; rot_stride * rot_h as usize];

        for sy in 0..h as usize {
            for sx in 0..w as usize {
                let src = sy * src_stride + sx * 4;
                // 90° CCW: dst(x,y) = src(w-1-y, x) → dst_x=sy, dst_y=w-1-sx
                let dx = sy;
                let dy = w as usize - 1 - sx;
                let dst = dy * rot_stride + dx * 4;
                rotated[dst..dst + 4].copy_from_slice(&data[src..src + 4]);
            }
        }

        let bytes = glib::Bytes::from_owned(rotated);
        let texture = gdk::MemoryTexture::new(
            rot_w,
            rot_h,
            gdk::MemoryFormat::R8g8b8a8Premultiplied,
            &bytes,
            rot_stride,
        );
        pic.set_paintable(Some(&texture));
    } else if let Some(gdk_texture) =
        build_gl_texture(&state.gl_context, tex_id, w as i32, h as i32)
    {
        pic.set_paintable(Some(&gdk_texture));
    }
}

fn build_gl_texture(
    gl_context: &gdk::GLContext,
    tex_id: u32,
    width: i32,
    height: i32,
) -> Option<gdk::Texture> {
    use glib::translate::*;

    unsafe {
        unsafe extern "C" {
            fn gdk_gl_texture_builder_new() -> *mut glib::gobject_ffi::GObject;
            fn gdk_gl_texture_builder_set_context(
                b: *mut glib::gobject_ffi::GObject,
                ctx: *mut glib::gobject_ffi::GObject,
            );
            fn gdk_gl_texture_builder_set_id(b: *mut glib::gobject_ffi::GObject, id: u32);
            fn gdk_gl_texture_builder_set_width(b: *mut glib::gobject_ffi::GObject, w: i32);
            fn gdk_gl_texture_builder_set_height(b: *mut glib::gobject_ffi::GObject, h: i32);
            fn gdk_gl_texture_builder_set_format(b: *mut glib::gobject_ffi::GObject, fmt: i32);
            fn gdk_gl_texture_builder_build(
                b: *mut glib::gobject_ffi::GObject,
                destroy: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
                data: *mut std::ffi::c_void,
                error: *mut *mut glib::ffi::GError,
            ) -> *mut glib::gobject_ffi::GObject;
            fn g_object_unref(obj: *mut glib::gobject_ffi::GObject);
        }

        let builder = gdk_gl_texture_builder_new();
        if builder.is_null() {
            return None;
        }

        let ctx_ptr =
            glib::translate::ToGlibPtr::<*mut gdk::ffi::GdkGLContext>::to_glib_none(gl_context).0;
        gdk_gl_texture_builder_set_context(builder, ctx_ptr.cast());
        gdk_gl_texture_builder_set_id(builder, tex_id);
        gdk_gl_texture_builder_set_width(builder, width);
        gdk_gl_texture_builder_set_height(builder, height);
        // GDK_MEMORY_R8G8B8A8_PREMULTIPLIED = 1
        gdk_gl_texture_builder_set_format(builder, 1);

        let mut error: *mut glib::ffi::GError = std::ptr::null_mut();
        let texture = gdk_gl_texture_builder_build(builder, None, std::ptr::null_mut(), &mut error);

        g_object_unref(builder);

        if texture.is_null() {
            if !error.is_null() {
                let msg = std::ffi::CStr::from_ptr((*error).message).to_string_lossy();
                log::error!("GL texture build failed: {msg}");
                glib::ffi::g_error_free(error);
            }
            return None;
        }

        Some(from_glib_full(texture.cast::<gdk::ffi::GdkTexture>()))
    }
}

// ── Input ──

fn setup_input_controllers(
    area: &gtk::DrawingArea,
    display_area: &gtk::DrawingArea,
    video_width: Arc<AtomicU32>,
    video_height: Arc<AtomicU32>,
    control: Arc<Mutex<Option<ControlSocket>>>,
) {
    let right_click = gtk::GestureClick::new();
    right_click.set_button(3);
    let c2 = control.clone();
    right_click.connect_released(move |_, _, _, _| {
        if let Ok(mut g) = c2.lock() {
            if let Some(cs) = g.as_mut() {
                if cs.is_alive() {
                    cs.back();
                    return;
                }
            }
        }
        // Fallback: ADB input
        std::thread::spawn(|| {
            let _ = std::process::Command::new("adb")
                .args(["-s", "127.0.0.1:6520", "shell", "input", "keyevent", "4"])
                .output();
        });
    });
    area.add_controller(right_click);

    let drag = gtk::GestureDrag::new();
    drag.set_button(1);
    let da = display_area.clone();
    let vw = video_width.clone();
    let vh = video_height.clone();
    let c3 = control.clone();
    let af = area.clone();
    drag.connect_drag_begin(move |_, x, y| {
        af.grab_focus();
        let (ax, ay) = w2a(
            &da,
            x,
            y,
            vw.load(Ordering::Relaxed),
            vh.load(Ordering::Relaxed),
        );
        if ax >= 0 && ay >= 0 {
            let mut used_scrcpy = false;
            let rotated = CURRENT_ROTATION.load(Ordering::Relaxed) == 1;
            // Only use scrcpy in portrait — scrcpy doesn't handle rotation
            if !rotated {
                if let Ok(mut g) = c3.lock() {
                    if let Some(cs) = g.as_mut() {
                        if cs.is_alive() {
                            cs.touch_down(ax as u32, ay as u32);
                            used_scrcpy = true;
                        }
                    }
                }
            }
            if !used_scrcpy {
                // ADB input uses current display coordinates (not portrait-rotated)
                let rotated = CURRENT_ROTATION.load(Ordering::Relaxed) == 1;
                let (disp_w, disp_h) = if rotated {
                    (1280.0, 720.0)
                } else {
                    (720.0, 1280.0)
                };
                let ww = da.width() as f64;
                let wh = da.height() as f64;
                let va = disp_w / disp_h;
                let wa = ww / wh;
                let (rw, rh, ox2, oy2) = if va > wa {
                    (ww, ww / va, 0.0, (wh - ww / va) / 2.0)
                } else {
                    (wh * va, wh, (ww - wh * va) / 2.0, 0.0)
                };
                let nx = (x - ox2) / rw;
                let ny = (y - oy2) / rh;
                let tap_x = (nx * disp_w).round().clamp(0.0, disp_w - 1.0) as u32;
                let tap_y = (ny * disp_h).round().clamp(0.0, disp_h - 1.0) as u32;
                log::info!("input: ADB fallback tap({tap_x}, {tap_y}) rotated={rotated} widget=({x:.0},{y:.0}) widget_size={ww:.0}x{wh:.0}");
                std::thread::spawn(move || {
                    let _ = std::process::Command::new("adb")
                        .args([
                            "-s",
                            "127.0.0.1:6520",
                            "shell",
                            "input",
                            "tap",
                            &tap_x.to_string(),
                            &tap_y.to_string(),
                        ])
                        .output();
                });
            }
        }
    });
    let da3 = display_area.clone();
    let vw3 = video_width.clone();
    let vh3 = video_height.clone();
    let c4 = control.clone();
    drag.connect_drag_update(move |gesture, ox, oy| {
        if let Some((sx, sy)) = gesture.start_point() {
            let (ax, ay) = w2a(
                &da3,
                sx + ox,
                sy + oy,
                vw3.load(Ordering::Relaxed),
                vh3.load(Ordering::Relaxed),
            );
            if ax >= 0 && ay >= 0 {
                if let Ok(mut g) = c4.lock() {
                    if let Some(cs) = g.as_mut() {
                        if cs.is_alive() {
                            cs.touch_move(ax as u32, ay as u32);
                        }
                    }
                }
            }
        }
    });
    let da4 = display_area.clone();
    let vw4 = video_width;
    let vh4 = video_height;
    let c5 = control.clone();
    drag.connect_drag_end(move |gesture, ox, oy| {
        if let Some((sx, sy)) = gesture.start_point() {
            let (ax, ay) = w2a(
                &da4,
                sx + ox,
                sy + oy,
                vw4.load(Ordering::Relaxed),
                vh4.load(Ordering::Relaxed),
            );
            if ax >= 0 && ay >= 0 {
                if let Ok(mut g) = c5.lock() {
                    if let Some(cs) = g.as_mut() {
                        if cs.is_alive() {
                            cs.touch_up(ax as u32, ay as u32);
                        }
                    }
                }
            }
        }
    });
    area.add_controller(drag);

    let key = gtk::EventControllerKey::new();
    let c6 = control;
    key.connect_key_pressed(move |_, keyval, _kc, modifier| {
        if let Ok(mut g) = c6.lock() {
            if let Some(cs) = g.as_mut() {
                if let Some(ak) = k2a(keyval) {
                    cs.key_meta(ak, m2a(modifier));
                    return glib::Propagation::Stop;
                }
                if let Some(ch) = keyval.to_unicode() {
                    if !ch.is_control() {
                        let mut b = [0u8; 4];
                        cs.inject_text(ch.encode_utf8(&mut b));
                        return glib::Propagation::Stop;
                    }
                }
            }
        }
        glib::Propagation::Proceed
    });
    area.add_controller(key);
}

/// Current rotation state shared between renderer and input
static CURRENT_ROTATION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn w2a(w: &impl IsA<gtk::Widget>, wx: f64, wy: f64, vw: u32, vh: u32) -> (i32, i32) {
    let ww = w.width() as f64;
    let wh = w.height() as f64;
    if ww <= 0.0 || wh <= 0.0 || vw == 0 || vh == 0 {
        return (-1, -1);
    }

    let rotated = CURRENT_ROTATION.load(Ordering::Relaxed) == 1;

    // In landscape, the displayed image is rotated (1280x720)
    let (disp_w, disp_h) = if rotated { (vh, vw) } else { (vw, vh) };

    let va = disp_w as f64 / disp_h as f64;
    let wa = ww / wh;
    let (rw, rh, ox, oy) = if va > wa {
        (ww, ww / va, 0.0, (wh - ww / va) / 2.0)
    } else {
        (wh * va, wh, (ww - wh * va) / 2.0, 0.0)
    };
    if wx < ox || wx > ox + rw || wy < oy || wy > oy + rh {
        return (-1, -1);
    }

    let nx = (wx - ox) / rw;
    let ny = (wy - oy) / rh;

    if rotated {
        // Reverse 90° CCW rotation: portrait_x = (1-ny)*720, portrait_y = nx*1280
        let ax = ((1.0 - ny) * 720.0).round().clamp(0.0, 719.0) as i32;
        let ay = (nx * 1280.0).round().clamp(0.0, 1279.0) as i32;
        (ax, ay)
    } else {
        let ax = (nx * 720.0).round().clamp(0.0, 719.0) as i32;
        let ay = (ny * 1280.0).round().clamp(0.0, 1279.0) as i32;
        (ax, ay)
    }
}

fn connect_scrcpy_control() -> Result<ControlSocket, String> {
    use crate::scrcpy::server;
    server::check_device()?;

    // Clear stale ADB forwards (previous scrcpy server may have died)
    let _ = std::process::Command::new("adb")
        .args(["-s", "127.0.0.1:6520", "forward", "--remove-all"])
        .output();

    // Get actual screen dimensions (may be rotated)
    let wm_output = std::process::Command::new("adb")
        .args(["-s", "127.0.0.1:6520", "shell", "wm", "size"])
        .output()
        .map_err(|e| format!("wm size: {e}"))?;
    let wm_str = String::from_utf8_lossy(&wm_output.stdout);
    let (sw, sh) = parse_screen_size(&wm_str).unwrap_or((720, 1280));
    log::info!("display: screen size {}x{}", sw, sh);

    let conn = server::ScrcpyConnection::connect(0, 8_000_000)?;
    let mut video = conn.video_stream.try_clone().map_err(|e| format!("{e}"))?;
    std::thread::spawn(move || {
        let mut b = [0u8; 65536];
        loop {
            match std::io::Read::read(&mut video, &mut b) {
                Ok(0) | Err(_) => break,
                _ => {}
            }
        }
    });
    let ctrl = ControlSocket::new(
        conn.control_stream
            .try_clone()
            .map_err(|e| format!("{e}"))?,
        sw,
        sh,
    );
    std::mem::forget(conn);
    Ok(ctrl)
}

fn parse_screen_size(s: &str) -> Option<(u16, u16)> {
    // Parse "Physical size: 720x1280" or "Override size: 1280x720"
    let line = s.lines().last()?;
    let dims = line.split(':').nth(1)?.trim();
    let mut parts = dims.split('x');
    let w = parts.next()?.trim().parse().ok()?;
    let h = parts.next()?.trim().parse().ok()?;
    Some((w, h))
}

pub fn stop_scrcpy(overlay: &gtk::Overlay, handle: &ScrcpyHandle) {
    handle.stop();
}

pub fn show_stopped(_overlay: &gtk::Overlay) {}

/// Start scrcpy in audio-only mode — plays Android audio on host via PulseAudio/PipeWire.
fn start_audio_bridge() {
    use std::process::{Command, Stdio};
    match Command::new("scrcpy")
        .args([
            "--serial",
            "127.0.0.1:6520",
            "--no-video",
            "--no-control",
            "--no-window",
            "--audio-codec=raw",
            "--audio-buffer=20",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => log::info!("audio: scrcpy audio bridge started (pid={})", child.id()),
        Err(e) => log::error!("audio: failed to start scrcpy audio bridge: {e}"),
    }
}

fn k2a(k: gdk::Key) -> Option<u32> {
    use gdk::Key;
    Some(match k {
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

fn m2a(m: gdk::ModifierType) -> u32 {
    let mut r = 0u32;
    if m.contains(gdk::ModifierType::SHIFT_MASK) {
        r |= 1;
    }
    if m.contains(gdk::ModifierType::CONTROL_MASK) {
        r |= 0x1000;
    }
    if m.contains(gdk::ModifierType::ALT_MASK) {
        r |= 0x02;
    }
    r
}
