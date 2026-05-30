//! Stub control socket for keymap engine.
//!
//! Previously backed by scrcpy's control protocol. Will be re-wired to
//! QEMU virtio-input in a future integration pass.

/// Placeholder control socket for sending touch events to the guest.
#[derive(Debug)]
pub struct ControlSocket;

impl ControlSocket {
    /// Send touch-down at (x, y) with the given pointer ID.
    pub fn touch_down_id(&mut self, _id: u64, _x: u32, _y: u32) {}

    /// Send touch-up at (x, y) with the given pointer ID.
    pub fn touch_up_id(&mut self, _id: u64, _x: u32, _y: u32) {}

    /// Send touch-move to (x, y) with the given pointer ID.
    pub fn touch_move_id(&mut self, _id: u64, _x: u32, _y: u32) {}

    /// Tap at (x, y) — touch down + brief hold + touch up.
    pub fn tap(&mut self, _x: u32, _y: u32) {}
}
