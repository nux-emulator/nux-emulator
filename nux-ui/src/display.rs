//! Display area — input controllers + scrcpy control for the X11Presenter window.
//!
//! Video rendering is handled by X11Presenter (AOSP side).
//! This module provides: GTK input area, scrcpy control connection,
//! audio bridge, and keyboard/mouse event forwarding.

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

use crate::scrcpy::control::ControlSocket;

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

/// Start input controllers + scrcpy control + audio (no video rendering).
/// Used when WebRTC handles video but we still need input via scrcpy.
pub fn start_input_only(input_area: &gtk::DrawingArea) -> ScrcpyHandle {
    let running = Arc::new(AtomicBool::new(true));
    let video_width = Arc::new(AtomicU32::new(720));
    let video_height = Arc::new(AtomicU32::new(1280));
    let control: Arc<Mutex<Option<ControlSocket>>> = Arc::new(Mutex::new(None));

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
        start_audio_bridge();

        loop {
            if !running_ctrl.load(Ordering::Relaxed) {
                break;
            }
            match connect_scrcpy_control() {
                Ok(c) => {
                    *ctrl.lock().unwrap() = Some(c);
                    log::info!("display: scrcpy control connected (input-only)");

                    // Signal X11Presenter that input is ready — it will map the window
                    let ready_path = "/tmp/nux-x11-ready";
                    if std::fs::write(ready_path, "1").is_ok() {
                        log::info!("display: wrote X11 ready signal");
                    }

                    // Start orientation polling in a separate thread
                    let running_orient = running_ctrl.clone();
                    std::thread::spawn(move || {
                        let orient_path = "/tmp/nux-x11-orientation";
                        let mut last_orient = 255u8; // invalid initial
                        while running_orient.load(Ordering::Relaxed) {
                            if let Ok(output) = std::process::Command::new("adb")
                                .args(["-s", "127.0.0.1:6520", "shell", "dumpsys", "display"])
                                .output()
                            {
                                let out = String::from_utf8_lossy(&output.stdout);
                                if let Some(orient) = out
                                    .lines()
                                    .find(|l| l.contains("mCurrentOrientation="))
                                    .and_then(|l| l.trim().strip_prefix("mCurrentOrientation="))
                                    .and_then(|s| s.parse::<u8>().ok())
                                {
                                    if orient != last_orient {
                                        let _ = std::fs::write(orient_path, orient.to_string());
                                        last_orient = orient;
                                        log::info!("display: orientation changed to {orient}");
                                    }
                                }
                            }
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        }
                    });
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        if !running_ctrl.load(Ordering::Relaxed) {
                            return;
                        }
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

    setup_input_controllers(
        input_area,
        input_area,
        video_width,
        video_height,
        control.clone(),
    );
    log::info!("display: input controllers attached (input-only mode)");

    // Start X11 input bridge for the GPU-rendered window
    crate::x11_input::start_x11_input_bridge(control, handle.running.clone());

    handle
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

    // Mouse scroll → Android scroll
    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    let da5 = display_area.clone();
    scroll.connect_scroll(move |_, _dx, dy| {
        // Convert scroll to swipe: scroll down = swipe up on screen
        let ww = da5.width() as f64;
        let wh = da5.height() as f64;
        let _cx = (ww / 2.0) as u32;
        let _cy = (wh / 2.0) as u32;
        let distance = (dy * 200.0) as i32; // 200px per scroll tick

        let rotated = CURRENT_ROTATION.load(Ordering::Relaxed) == 1;
        let (disp_w, disp_h) = if rotated {
            (1280.0, 720.0)
        } else {
            (720.0, 1280.0)
        };

        // Map center of widget to display coordinates
        let va = disp_w / disp_h;
        let wa = ww / wh;
        let (rw, rh, ox, oy) = if va > wa {
            (ww, ww / va, 0.0, (wh - ww / va) / 2.0)
        } else {
            (wh * va, wh, (ww - wh * va) / 2.0, 0.0)
        };
        let nx = (ww / 2.0 - ox) / rw;
        let ny = (wh / 2.0 - oy) / rh;
        let sx = (nx * disp_w).round().clamp(0.0, disp_w - 1.0) as i32;
        let sy = (ny * disp_h).round().clamp(0.0, disp_h - 1.0) as i32;
        let ey = (sy + distance).clamp(0, disp_h as i32 - 1);

        std::thread::spawn(move || {
            let _ = std::process::Command::new("adb")
                .args([
                    "-s",
                    "127.0.0.1:6520",
                    "shell",
                    "input",
                    "swipe",
                    &sx.to_string(),
                    &sy.to_string(),
                    &sx.to_string(),
                    &ey.to_string(),
                    "100",
                ])
                .output();
        });
        glib::Propagation::Stop
    });
    area.add_controller(scroll);

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

pub fn stop_scrcpy(_overlay: &gtk::Overlay, handle: &ScrcpyHandle) {
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
