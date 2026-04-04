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

/// Default pointer ID for single-touch (mouse).
pub const POINTER_MOUSE: u64 = 0xFFFFFFFFFFFFFFFF;

/// Persistent control connection to the scrcpy server.
pub struct ControlSocket {
    stream: TcpStream,
    /// Base screen dimensions (portrait: 720x1280).
    base_width: u16,
    base_height: u16,
    /// Current screen dimensions sent in touch packets.
    /// Swapped when landscape.
    screen_width: u16,
    screen_height: u16,
    /// Whether the display is in landscape orientation.
    landscape: bool,
    alive: bool,
}

impl ControlSocket {
    /// Create a control socket from an existing TCP connection.
    pub fn new(stream: TcpStream, screen_width: u16, screen_height: u16) -> Self {
        Self {
            stream,
            base_width: screen_width,
            base_height: screen_height,
            screen_width,
            screen_height,
            landscape: false,
            alive: true,
        }
    }

    /// Check if the connection is still alive.
    pub fn is_alive(&self) -> bool {
        self.alive
    }

    /// Update base screen dimensions (when video resolution changes).
    pub fn set_screen_size(&mut self, width: u16, height: u16) {
        self.base_width = width;
        self.base_height = height;
        self.update_orientation(self.landscape);
    }

    /// Set display orientation. Swaps screen dimensions for touch packets.
    pub fn set_orientation(&mut self, landscape: bool) {
        if self.landscape != landscape {
            self.update_orientation(landscape);
            log::info!(
                "control: orientation → {} (screen {}x{})",
                if landscape { "landscape" } else { "portrait" },
                self.screen_width,
                self.screen_height
            );
        }
    }

    /// Whether the display is currently in landscape mode.
    pub fn is_landscape(&self) -> bool {
        self.landscape
    }

    /// Current screen width (orientation-aware).
    pub fn current_width(&self) -> u16 {
        self.screen_width
    }

    /// Current screen height (orientation-aware).
    pub fn current_height(&self) -> u16 {
        self.screen_height
    }

    fn update_orientation(&mut self, landscape: bool) {
        self.landscape = landscape;
        if landscape {
            // Landscape: swap dimensions (720x1280 → 1280x720)
            self.screen_width = self.base_width.max(self.base_height);
            self.screen_height = self.base_width.min(self.base_height);
        } else {
            // Portrait: normal dimensions (720x1280)
            self.screen_width = self.base_width.min(self.base_height);
            self.screen_height = self.base_width.max(self.base_height);
        }
    }

    /// Send a tap (touch down + up) at the given coordinates.
    pub fn tap(&mut self, x: u32, y: u32) {
        self.touch_id(ACTION_DOWN, POINTER_MOUSE, x, y, 0xFFFF);
        self.touch_id(ACTION_UP, POINTER_MOUSE, x, y, 0);
    }

    /// Send a touch event with the default pointer ID (mouse).
    pub fn touch(&mut self, action: u8, x: u32, y: u32, pressure: u16) {
        self.touch_id(action, POINTER_MOUSE, x, y, pressure);
    }

    /// Send a touch event with a specific pointer ID (for multi-touch).
    pub fn touch_id(&mut self, action: u8, pointer_id: u64, x: u32, y: u32, pressure: u16) {
        let mut buf = [0u8; 32];
        buf[0] = TYPE_INJECT_TOUCH;
        buf[1] = action;
        // pointerId (8 bytes)
        buf[2..10].copy_from_slice(&pointer_id.to_be_bytes());
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

    /// Send touch down with default pointer.
    pub fn touch_down(&mut self, x: u32, y: u32) {
        self.touch(ACTION_DOWN, x, y, 0xFFFF);
    }

    /// Send touch move with default pointer.
    pub fn touch_move(&mut self, x: u32, y: u32) {
        self.touch(ACTION_MOVE, x, y, 0xFFFF);
    }

    /// Send touch up with default pointer.
    pub fn touch_up(&mut self, x: u32, y: u32) {
        self.touch(ACTION_UP, x, y, 0);
    }

    /// Send touch down with specific pointer ID.
    pub fn touch_down_id(&mut self, pointer_id: u64, x: u32, y: u32) {
        self.touch_id(ACTION_DOWN, pointer_id, x, y, 0xFFFF);
    }

    /// Send touch move with specific pointer ID.
    pub fn touch_move_id(&mut self, pointer_id: u64, x: u32, y: u32) {
        self.touch_id(ACTION_MOVE, pointer_id, x, y, 0xFFFF);
    }

    /// Send touch up with specific pointer ID.
    pub fn touch_up_id(&mut self, pointer_id: u64, x: u32, y: u32) {
        self.touch_id(ACTION_UP, pointer_id, x, y, 0);
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
