//! X11 input bridge — captures mouse/keyboard events from the X11Presenter window
//! and forwards them to Android via the scrcpy control socket.

use crate::scrcpy::control::ControlSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// ── Minimal X11 FFI ──

#[allow(non_camel_case_types)]
type Display = *mut std::ffi::c_void;
#[allow(non_camel_case_types)]
type Window = u64;
#[allow(non_camel_case_types)]
type KeySym = u64;

// Event masks
const KEY_PRESS_MASK: i64 = 1 << 0;
const KEY_RELEASE_MASK: i64 = 1 << 1;
const BUTTON_PRESS_MASK: i64 = 1 << 2;
const BUTTON_RELEASE_MASK: i64 = 1 << 3;
const POINTER_MOTION_MASK: i64 = 1 << 6;
const STRUCTURE_NOTIFY_MASK: i64 = 1 << 17;

// Event types
const KEY_PRESS: i32 = 2;
const KEY_RELEASE: i32 = 3;
const BUTTON_PRESS: i32 = 4;
const BUTTON_RELEASE: i32 = 5;
const MOTION_NOTIFY: i32 = 6;
const CONFIGURE_NOTIFY: i32 = 22;

// X11 button constants
const BUTTON1: u32 = 1; // left
const BUTTON3: u32 = 3; // right
const BUTTON4: u32 = 4; // scroll up
const BUTTON5: u32 = 5; // scroll down

// XEvent is a union of 192 bytes (24 longs on 64-bit)
#[repr(C)]
#[derive(Copy, Clone)]
struct XEvent {
    data: [u8; 192],
}

impl XEvent {
    fn event_type(&self) -> i32 {
        i32::from_ne_bytes([self.data[0], self.data[1], self.data[2], self.data[3]])
    }
}

// XKeyEvent/XButtonEvent layout on x86_64 (with alignment padding):
//  0: type (i32, 4 bytes)
//  4: padding (4 bytes)
//  8: serial (u64, 8 bytes)
// 16: send_event (i32, 4 bytes)
// 20: padding (4 bytes)
// 24: display (ptr, 8 bytes)
// 32: window (u64, 8 bytes)
// 40: root (u64, 8 bytes)
// 48: subwindow (u64, 8 bytes)
// 56: time (u64, 8 bytes)
// 64: x (i32, 4 bytes)
// 68: y (i32, 4 bytes)
// 72: x_root (i32, 4 bytes)
// 76: y_root (i32, 4 bytes)
// 80: state (u32, 4 bytes)
// 84: keycode/button (u32, 4 bytes)

fn xevent_xy(ev: &XEvent) -> (i32, i32) {
    let x = i32::from_ne_bytes([ev.data[64], ev.data[65], ev.data[66], ev.data[67]]);
    let y = i32::from_ne_bytes([ev.data[68], ev.data[69], ev.data[70], ev.data[71]]);
    (x, y)
}

fn xevent_button(ev: &XEvent) -> u32 {
    // XButtonEvent/XKeyEvent: keycode/button at offset 84
    u32::from_ne_bytes([ev.data[84], ev.data[85], ev.data[86], ev.data[87]])
}

fn xevent_state(ev: &XEvent) -> u32 {
    u32::from_ne_bytes([ev.data[80], ev.data[81], ev.data[82], ev.data[83]])
}

// XConfigureEvent layout on x86_64:
//  0: type, 8: serial, 16: send_event, 24: display, 32: event, 40: window
// 48: x (i32), 52: y (i32), 56: width (i32), 60: height (i32)
// Wait — XConfigureEvent has different layout:
//  0: type (4) + pad(4) + serial(8) + send_event(4) + pad(4) + display(8)
// 32: event (8), 40: window (8)
// 48: x (4), 52: y (4), 56: width (4), 60: height (4)
fn xconfigure_size(ev: &XEvent) -> (i32, i32) {
    let w = i32::from_ne_bytes([ev.data[56], ev.data[57], ev.data[58], ev.data[59]]);
    let h = i32::from_ne_bytes([ev.data[60], ev.data[61], ev.data[62], ev.data[63]]);
    (w, h)
}

#[link(name = "X11")]
unsafe extern "C" {
    fn XOpenDisplay(name: *const i8) -> Display;
    fn XSelectInput(display: Display, window: Window, event_mask: i64) -> i32;
    fn XNextEvent(display: Display, event: *mut XEvent) -> i32;
    fn XPending(display: Display) -> i32;
    fn XCloseDisplay(display: Display) -> i32;
    fn XLookupKeysym(event: *mut XEvent, index: i32) -> KeySym;
    fn XConnectionNumber(display: Display) -> i32;
    fn XCreateWindow(
        display: Display,
        parent: Window,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        border_width: u32,
        depth: i32,
        class: u32,
        visual: *mut std::ffi::c_void,
        valuemask: u64,
        attributes: *mut XSetWindowAttributes,
    ) -> Window;
    fn XMapWindow(display: Display, window: Window) -> i32;
    fn XRaiseWindow(display: Display, window: Window) -> i32;
    fn XFlush(display: Display) -> i32;
    fn XGetWindowAttributes(display: Display, window: Window, attrs: *mut XWindowAttributes)
    -> i32;
}

// InputOnly window class
const INPUT_ONLY: u32 = 2;
// CWEventMask | CWOverrideRedirect
const CW_EVENT_MASK: u64 = 1 << 11;
const CW_OVERRIDE_REDIRECT: u64 = 1 << 9;

#[repr(C)]
#[derive(Default)]
struct XSetWindowAttributes {
    background_pixmap: u64,
    background_pixel: u64,
    border_pixmap: u64,
    border_pixel: u64,
    bit_gravity: i32,
    win_gravity: i32,
    backing_store: i32,
    backing_planes: u64,
    backing_pixel: u64,
    save_under: i32,
    event_mask: i64,
    do_not_propagate_mask: i64,
    override_redirect: i32,
    colormap: u64,
    cursor: u64,
}

#[repr(C)]
#[derive(Default)]
struct XWindowAttributes {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    border_width: i32,
    depth: i32,
    visual: *mut std::ffi::c_void,
    root: Window,
    class: i32,
    bit_gravity: i32,
    win_gravity: i32,
    backing_store: i32,
    backing_planes: u64,
    backing_pixel: u64,
    save_under: i32,
    colormap: u64,
    map_installed: i32,
    map_state: i32,
    all_event_masks: i64,
    your_event_mask: i64,
    do_not_propagate_mask: i64,
    override_redirect: i32,
    screen: *mut std::ffi::c_void,
}

// ── Coordinate transform ──

/// Convert X11 window coordinates to Android screen coordinates,
/// accounting for letterbox viewport and orientation.
fn x11_to_android(
    wx: i32,
    wy: i32,
    win_w: i32,
    win_h: i32,
    fb_w: i32,
    fb_h: i32,
    landscape: bool,
) -> Option<(u32, u32)> {
    // Compute letterbox viewport (same logic as X11Presenter)
    // In landscape, effective aspect is height/width (rotated)
    let src_aspect = if landscape {
        fb_h as f64 / fb_w as f64
    } else {
        fb_w as f64 / fb_h as f64
    };
    let dst_aspect = win_w as f64 / win_h as f64;
    let (vp_x, vp_y, vp_w, vp_h) = if src_aspect > dst_aspect {
        let vp_w = win_w;
        let vp_h = (win_w as f64 / src_aspect) as i32;
        (0, (win_h - vp_h) / 2, vp_w, vp_h)
    } else {
        let vp_h = win_h;
        let vp_w = (win_h as f64 * src_aspect) as i32;
        ((win_w - vp_w) / 2, 0, vp_w, vp_h)
    };

    // Check if click is within viewport
    if wx < vp_x || wx >= vp_x + vp_w || wy < vp_y || wy >= vp_y + vp_h {
        return None;
    }

    // Normalize to [0,1] within viewport
    let nx = (wx - vp_x) as f64 / vp_w as f64;
    let ny = (wy - vp_y) as f64 / vp_h as f64;

    if landscape {
        // Scrcpy touch packets include screen_width=720, screen_height=1280 (portrait).
        // We must send portrait-space coordinates.
        // Landscape VBO texcoord interpolation at screen (nx, ny):
        //   u = 1 - ny,  v = nx
        // Portrait pixel: px = u * fb_w = (1-ny) * 720, py = v * fb_h = nx * 1280
        let ax = ((1.0 - ny) * fb_w as f64) as u32;
        let ay = (nx * fb_h as f64) as u32;
        log::debug!(
            "x11-input: landscape wx={wx} wy={wy} nx={nx:.2} ny={ny:.2} → portrait ax={ax} ay={ay}"
        );
        Some((ax.min(fb_w as u32 - 1), ay.min(fb_h as u32 - 1)))
    } else {
        let ax = (nx * fb_w as f64) as u32;
        let ay = (ny * fb_h as f64) as u32;
        Some((ax.min(fb_w as u32 - 1), ay.min(fb_h as u32 - 1)))
    }
}

// ── KeySym → Android keycode mapping ──

fn keysym_to_android(keysym: KeySym) -> Option<u32> {
    Some(match keysym {
        0xff08 => 67,           // BackSpace → KEYCODE_DEL
        0xff09 => 61,           // Tab → KEYCODE_TAB
        0xff0d => 66,           // Return → KEYCODE_ENTER
        0xff1b => 111,          // Escape → KEYCODE_ESCAPE
        0xffff => 112,          // Delete → KEYCODE_FORWARD_DEL
        0xff50 => 122,          // Home → KEYCODE_MOVE_HOME
        0xff51 => 21,           // Left → KEYCODE_DPAD_LEFT
        0xff52 => 19,           // Up → KEYCODE_DPAD_UP
        0xff53 => 22,           // Right → KEYCODE_DPAD_RIGHT
        0xff54 => 20,           // Down → KEYCODE_DPAD_DOWN
        0xff55 => 92,           // Page_Up → KEYCODE_PAGE_UP
        0xff56 => 93,           // Page_Down → KEYCODE_PAGE_DOWN
        0xff57 => 123,          // End → KEYCODE_MOVE_END
        0xffe1 | 0xffe2 => 59,  // Shift → KEYCODE_SHIFT_LEFT
        0xffe3 | 0xffe4 => 113, // Control → KEYCODE_CTRL_LEFT
        0xffe9 | 0xffea => 57,  // Alt → KEYCODE_ALT_LEFT
        0x0020 => 62,           // space → KEYCODE_SPACE
        // Letters a-z
        k @ 0x0061..=0x007a => (k - 0x0061 + 29) as u32,
        // Digits 0-9
        k @ 0x0030..=0x0039 => (k - 0x0030 + 7) as u32,
        // F1-F12
        k @ 0xffbe..=0xffc9 => (k - 0xffbe + 131) as u32,
        _ => return None,
    })
}

fn x11_state_to_android_meta(state: u32) -> u32 {
    let mut meta = 0u32;
    if state & 1 != 0 {
        meta |= 1;
    } // Shift → META_SHIFT_ON
    if state & 4 != 0 {
        meta |= 0x1000;
    } // Control → META_CTRL_ON
    if state & 8 != 0 {
        meta |= 2;
    } // Alt → META_ALT_ON
    meta
}

// ── Public API ──

/// Start the X11 input bridge in a background thread.
/// Reads the X11 window ID from the file written by X11Presenter,
/// captures input events, and forwards them via the scrcpy control socket.
pub fn start_x11_input_bridge(
    control: Arc<Mutex<Option<ControlSocket>>>,
    running: Arc<AtomicBool>,
) {
    std::thread::Builder::new()
        .name("x11-input".into())
        .spawn(move || {
            if let Err(e) = run_input_loop(&control, &running) {
                log::error!("x11-input: bridge failed: {e}");
            }
        })
        .expect("failed to spawn x11-input thread");
}

fn run_input_loop(
    control: &Arc<Mutex<Option<ControlSocket>>>,
    running: &Arc<AtomicBool>,
) -> anyhow::Result<()> {
    // Wait for the window ID file to appear
    let id_path = "/tmp/nux-cf/cuttlefish/instances/cvd-1/internal/x11_window_id";
    let mut window_id: Window = 0;
    let mut fb_w: i32 = 720;
    let mut fb_h: i32 = 1280;

    log::info!("x11-input: waiting for X11 window ID at {id_path}");
    loop {
        if !running.load(Ordering::Relaxed) {
            return Ok(());
        }
        if let Ok(content) = std::fs::read_to_string(id_path) {
            let parts: Vec<&str> = content.trim().split_whitespace().collect();
            if parts.len() >= 3 {
                window_id = parts[0].parse().unwrap_or(0);
                fb_w = parts[1].parse().unwrap_or(720);
                fb_h = parts[2].parse().unwrap_or(1280);
            }
            if window_id != 0 {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    log::info!("x11-input: attaching to X11 window {window_id} ({fb_w}x{fb_h})");

    // Open X11 display
    let display = unsafe { XOpenDisplay(std::ptr::null()) };
    if display.is_null() {
        anyhow::bail!("XOpenDisplay failed");
    }

    // Wait for scrcpy control to connect before creating the input overlay.
    // The parent window is unmapped until the ready signal, so we must wait
    // for it to be mapped before creating child windows on it.
    log::info!("x11-input: waiting for scrcpy control...");
    loop {
        if !running.load(Ordering::Relaxed) {
            unsafe {
                XCloseDisplay(display);
            }
            return Ok(());
        }
        let has_control = control
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|cs| cs.is_alive()))
            .unwrap_or(false);
        if has_control {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    log::info!("x11-input: scrcpy connected, setting up input overlay");

    // Small delay to let X11Presenter map the window after seeing the ready file
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Get parent window size (now that it's mapped)
    let mut parent_attrs: XWindowAttributes = unsafe { std::mem::zeroed() };
    unsafe {
        XGetWindowAttributes(display, window_id, &mut parent_attrs);
    }
    let mut win_w = if parent_attrs.width > 0 {
        parent_attrs.width
    } else {
        fb_w
    };
    let mut win_h = if parent_attrs.height > 0 {
        parent_attrs.height
    } else {
        fb_h
    };

    // Create an InputOnly child window covering the entire parent.
    // This lets us capture ButtonPress/KeyPress which are exclusive per-window.
    let event_mask = KEY_PRESS_MASK
        | KEY_RELEASE_MASK
        | BUTTON_PRESS_MASK
        | BUTTON_RELEASE_MASK
        | POINTER_MOTION_MASK
        | STRUCTURE_NOTIFY_MASK;

    let mut attrs: XSetWindowAttributes = unsafe { std::mem::zeroed() };
    attrs.event_mask = event_mask;
    attrs.override_redirect = 1; // don't let WM interfere

    let input_win = unsafe {
        XCreateWindow(
            display,
            window_id,
            0,
            0,
            win_w as u32,
            win_h as u32,
            0,
            0,
            INPUT_ONLY,
            std::ptr::null_mut(), // CopyFromParent visual
            CW_EVENT_MASK | CW_OVERRIDE_REDIRECT,
            &mut attrs,
        )
    };
    if input_win == 0 {
        anyhow::bail!("XCreateWindow (InputOnly) failed");
    }
    unsafe {
        XMapWindow(display, input_win);
        XRaiseWindow(display, input_win);
        XFlush(display);
    }

    // Also listen for StructureNotify on parent to track resizes
    unsafe {
        XSelectInput(display, window_id, STRUCTURE_NOTIFY_MASK);
    }

    log::info!(
        "x11-input: created input overlay {input_win} on parent {window_id} ({win_w}x{win_h})"
    );

    let mut dragging = false;
    let mut landscape = false;
    let mut orient_check_counter = 0u32;

    log::info!("x11-input: event loop started");

    // Use poll to avoid busy-waiting
    let x11_fd = unsafe { XConnectionNumber(display) };

    while running.load(Ordering::Relaxed) {
        // Check orientation periodically (~every 500ms)
        orient_check_counter += 1;
        if orient_check_counter % 10 == 0 {
            if let Ok(content) = std::fs::read_to_string("/tmp/nux-x11-orientation") {
                let orient = content.trim().parse::<u8>().unwrap_or(0);
                landscape = orient == 1 || orient == 3;
            }
        }

        // Poll X11 fd with 50ms timeout
        let mut pfd = libc::pollfd {
            fd: x11_fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ret = unsafe { libc::poll(&mut pfd, 1, 50) };
        if ret <= 0 {
            continue;
        }

        while unsafe { XPending(display) } > 0 {
            let mut event = XEvent { data: [0u8; 192] };
            unsafe {
                XNextEvent(display, &mut event);
            }

            match event.event_type() {
                CONFIGURE_NOTIFY => {
                    let (w, h) = xconfigure_size(&event);
                    if w > 0 && h > 0 {
                        win_w = w;
                        win_h = h;
                    }
                }
                BUTTON_PRESS => {
                    let btn = xevent_button(&event);
                    let (wx, wy) = xevent_xy(&event);
                    match btn {
                        BUTTON1 => {
                            if landscape {
                                // In landscape, use ADB input (scrcpy doesn't handle rotation well)
                                let src_aspect = fb_h as f64 / fb_w as f64;
                                let dst_aspect = win_w as f64 / win_h as f64;
                                let (vp_x, vp_y, vp_w, vp_h) = if src_aspect > dst_aspect {
                                    let vw = win_w;
                                    let vh = (win_w as f64 / src_aspect) as i32;
                                    (0, (win_h - vh) / 2, vw, vh)
                                } else {
                                    let vh = win_h;
                                    let vw = (win_h as f64 * src_aspect) as i32;
                                    ((win_w - vw) / 2, 0, vw, vh)
                                };
                                if wx >= vp_x && wx < vp_x + vp_w && wy >= vp_y && wy < vp_y + vp_h
                                {
                                    let nx = (wx - vp_x) as f64 / vp_w as f64;
                                    let ny = (wy - vp_y) as f64 / vp_h as f64;
                                    let tap_x = (nx * fb_h as f64).round() as u32;
                                    let tap_y = (ny * fb_w as f64).round() as u32;
                                    let tx = tap_x.to_string();
                                    let ty = tap_y.to_string();
                                    std::thread::spawn(move || {
                                        let _ = std::process::Command::new("adb")
                                            .args([
                                                "-s",
                                                "127.0.0.1:6520",
                                                "shell",
                                                "input",
                                                "tap",
                                                &tx,
                                                &ty,
                                            ])
                                            .output();
                                    });
                                }
                            } else if let Some((ax, ay)) =
                                x11_to_android(wx, wy, win_w, win_h, fb_w, fb_h, landscape)
                            {
                                if let Ok(mut guard) = control.lock() {
                                    if let Some(cs) = guard.as_mut() {
                                        cs.touch_down(ax, ay);
                                        dragging = true;
                                    }
                                }
                            }
                        }
                        BUTTON3 => {
                            // Right click → Back
                            if let Ok(mut guard) = control.lock() {
                                if let Some(cs) = guard.as_mut() {
                                    cs.back();
                                }
                            }
                        }
                        BUTTON4 => {
                            // Scroll up → swipe down
                            if let Some((ax, ay)) =
                                x11_to_android(wx, wy, win_w, win_h, fb_w, fb_h, landscape)
                            {
                                do_scroll(control, ax, ay, -120, fb_w, fb_h);
                            }
                        }
                        BUTTON5 => {
                            // Scroll down → swipe up
                            if let Some((ax, ay)) =
                                x11_to_android(wx, wy, win_w, win_h, fb_w, fb_h, landscape)
                            {
                                do_scroll(control, ax, ay, 120, fb_w, fb_h);
                            }
                        }
                        _ => {}
                    }
                }
                BUTTON_RELEASE => {
                    let btn = xevent_button(&event);
                    if btn == BUTTON1 && dragging {
                        let (wx, wy) = xevent_xy(&event);
                        let (ax, ay) = x11_to_android(wx, wy, win_w, win_h, fb_w, fb_h, landscape)
                            .unwrap_or((0, 0));
                        if let Ok(mut guard) = control.lock() {
                            if let Some(cs) = guard.as_mut() {
                                cs.touch_up(ax, ay);
                            }
                        }
                        dragging = false;
                    }
                }
                MOTION_NOTIFY => {
                    if dragging {
                        let (wx, wy) = xevent_xy(&event);
                        if let Some((ax, ay)) =
                            x11_to_android(wx, wy, win_w, win_h, fb_w, fb_h, landscape)
                        {
                            if let Ok(mut guard) = control.lock() {
                                if let Some(cs) = guard.as_mut() {
                                    cs.touch_move(ax, ay);
                                }
                            }
                        }
                    }
                }
                KEY_PRESS | KEY_RELEASE => {
                    let keysym = unsafe { XLookupKeysym(&mut event, 0) };
                    let is_down = event.event_type() == KEY_PRESS;
                    let state = xevent_state(&event);
                    let meta = x11_state_to_android_meta(state);

                    // Try printable character first (on key press only)
                    if is_down && keysym >= 0x20 && keysym <= 0x7e {
                        let ch = keysym as u8 as char;
                        if let Ok(mut guard) = control.lock() {
                            if let Some(cs) = guard.as_mut() {
                                cs.inject_text(&ch.to_string());
                            }
                        }
                    } else if let Some(akc) = keysym_to_android(keysym) {
                        let action = if is_down { 0u8 } else { 1u8 };
                        if let Ok(mut guard) = control.lock() {
                            if let Some(cs) = guard.as_mut() {
                                cs.key_event_meta(action, akc, meta);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    unsafe {
        XCloseDisplay(display);
    }
    log::info!("x11-input: bridge stopped");
    Ok(())
}

/// Simulate a scroll by sending a short swipe via touch events.
fn do_scroll(
    control: &Arc<Mutex<Option<ControlSocket>>>,
    x: u32,
    y: u32,
    delta: i32,
    _fb_w: i32,
    fb_h: i32,
) {
    let distance = (fb_h / 8) as i32; // ~160px per scroll tick
    let end_y = (y as i32 + if delta > 0 { -distance } else { distance }).clamp(0, fb_h - 1) as u32;

    if let Ok(mut guard) = control.lock() {
        if let Some(cs) = guard.as_mut() {
            cs.touch_down(x, y);
            // Intermediate points for smooth scroll
            let steps = 4;
            for i in 1..=steps {
                let iy = y as i32 + (end_y as i32 - y as i32) * i / steps;
                cs.touch_move(x, iy.max(0) as u32);
            }
            cs.touch_up(x, end_y);
        }
    }
}
