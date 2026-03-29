//! Scrcpy control protocol — sends touch/key/text events over a persistent socket.
//!
//! Binary protocol (big-endian):
//! - Key:   type(1)=0 + action(1) + keycode(4) + repeat(4) + metaState(4)
//! - Text:  type(1)=1 + length(4) + text(variable UTF-8)
//! - Touch: type(1)=2 + action(1) + pointerId(8) + x(4) + y(4) + w(2) + h(2) + pressure(2) + actionButton(4) + buttons(4)
//! - Back:  type(1)=4 + action(1)

use std::io::Write;
use std::net::TcpStream;

// Control message types
const TYPE_INJECT_KEYCODE: u8 = 0;
const TYPE_INJECT_TEXT: u8 = 1;
const TYPE_INJECT_TOUCH: u8 = 2;
const TYPE_BACK_OR_SCREEN_ON: u8 = 4;

// Android MotionEvent actions
const ACTION_DOWN: u8 = 0;
const ACTION_UP: u8 = 1;
const ACTION_MOVE: u8 = 2;

// Android KeyEvent actions
const AKEY_ACTION_DOWN: u8 = 0;
const AKEY_ACTION_UP: u8 = 1;

/// Persistent control connection to the scrcpy server.
pub struct ControlSocket {
    stream: TcpStream,
    screen_width: u16,
    screen_height: u16,
    alive: bool,
}

impl ControlSocket {
    /// Create a control socket from an existing TCP connection.
    pub fn new(stream: TcpStream, screen_width: u16, screen_height: u16) -> Self {
        Self {
            stream,
            screen_width,
            screen_height,
            alive: true,
        }
    }

    /// Check if the connection is still alive.
    pub fn is_alive(&self) -> bool {
        self.alive
    }

    /// Update screen dimensions (when video resolution changes).
    pub fn set_screen_size(&mut self, width: u16, height: u16) {
        self.screen_width = width;
        self.screen_height = height;
    }

    /// Send a tap (touch down + up) at the given coordinates.
    pub fn tap(&mut self, x: u32, y: u32) {
        log::info!(
            "control: tap({x}, {y}) screen={}x{}",
            self.screen_width,
            self.screen_height
        );
        self.touch(ACTION_DOWN, x, y, 0xFFFF);
        self.touch(ACTION_UP, x, y, 0);
    }

    /// Send a touch event.
    pub fn touch(&mut self, action: u8, x: u32, y: u32, pressure: u16) {
        let mut buf = [0u8; 32];
        buf[0] = TYPE_INJECT_TOUCH;
        buf[1] = action;
        // pointerId (8 bytes) — use -1 (0xFFFFFFFFFFFFFFFF) for mouse
        buf[2..10].copy_from_slice(&0xFFFFFFFFFFFFFFFFu64.to_be_bytes());
        // position x, y (4 bytes each, signed i32)
        buf[10..14].copy_from_slice(&(x as i32).to_be_bytes());
        buf[14..18].copy_from_slice(&(y as i32).to_be_bytes());
        // screen width, height (2 bytes each)
        buf[18..20].copy_from_slice(&self.screen_width.to_be_bytes());
        buf[20..22].copy_from_slice(&self.screen_height.to_be_bytes());
        // pressure (2 bytes)
        buf[22..24].copy_from_slice(&pressure.to_be_bytes());
        // actionButton (4 bytes) + buttons (4 bytes) = 0
        // buf[24..32] already zeroed

        match self.stream.write_all(&buf) {
            Ok(()) => {}
            Err(e) => {
                log::error!("control: write failed: {e}");
                self.alive = false;
            }
        }
    }

    /// Send touch down.
    pub fn touch_down(&mut self, x: u32, y: u32) {
        self.touch(ACTION_DOWN, x, y, 0xFFFF);
    }

    /// Send touch move.
    pub fn touch_move(&mut self, x: u32, y: u32) {
        self.touch(ACTION_MOVE, x, y, 0xFFFF);
    }

    /// Send touch up.
    pub fn touch_up(&mut self, x: u32, y: u32) {
        self.touch(ACTION_UP, x, y, 0);
    }

    /// Send a key event (down + up).
    pub fn key(&mut self, keycode: u32) {
        self.key_event(AKEY_ACTION_DOWN, keycode);
        self.key_event(AKEY_ACTION_UP, keycode);
    }

    /// Send a single key action.
    pub fn key_event(&mut self, action: u8, keycode: u32) {
        let mut buf = [0u8; 14];
        buf[0] = TYPE_INJECT_KEYCODE;
        buf[1] = action;
        buf[2..6].copy_from_slice(&keycode.to_be_bytes());
        // repeat (4 bytes) = 0
        // metaState (4 bytes) = 0

        let _ = self.stream.write_all(&buf);
    }

    /// Send back button press.
    pub fn back(&mut self) {
        let buf = [TYPE_BACK_OR_SCREEN_ON, AKEY_ACTION_DOWN];
        let _ = self.stream.write_all(&buf);
        let buf = [TYPE_BACK_OR_SCREEN_ON, AKEY_ACTION_UP];
        let _ = self.stream.write_all(&buf);
    }

    /// Inject text directly (for printable characters).
    /// Uses TYPE_INJECT_TEXT which handles IME input correctly.
    pub fn inject_text(&mut self, text: &str) {
        let bytes = text.as_bytes();
        let len = bytes.len() as u32;
        let mut buf = Vec::with_capacity(5 + bytes.len());
        buf.push(TYPE_INJECT_TEXT);
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(bytes);
        let _ = self.stream.write_all(&buf);
    }

    /// Send a key event with meta state (shift, ctrl, alt).
    pub fn key_event_meta(&mut self, action: u8, keycode: u32, meta_state: u32) {
        let mut buf = [0u8; 14];
        buf[0] = TYPE_INJECT_KEYCODE;
        buf[1] = action;
        buf[2..6].copy_from_slice(&keycode.to_be_bytes());
        // repeat (4 bytes) = 0
        buf[10..14].copy_from_slice(&meta_state.to_be_bytes());
        let _ = self.stream.write_all(&buf);
    }

    /// Send a key with meta state (down + up).
    pub fn key_meta(&mut self, keycode: u32, meta_state: u32) {
        self.key_event_meta(AKEY_ACTION_DOWN, keycode, meta_state);
        self.key_event_meta(AKEY_ACTION_UP, keycode, meta_state);
    }
}

// Android keycodes
pub const KEYCODE_HOME: u32 = 3;
pub const KEYCODE_BACK: u32 = 4;
pub const KEYCODE_VOLUME_UP: u32 = 24;
pub const KEYCODE_VOLUME_DOWN: u32 = 25;
pub const KEYCODE_POWER: u32 = 26;
pub const KEYCODE_APP_SWITCH: u32 = 187;
