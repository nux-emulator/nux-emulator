//! Input manager: orchestrates event translation and socket injection.
//!
//! `InputManager` is the main entry point for the input system. It holds
//! the virtio-input socket connection, display metrics, grab state, and
//! multi-touch state, and exposes high-level methods for each input type.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;

use thiserror::Error;

use crate::input::coordinate::SharedDisplayMetrics;
use crate::input::evdev::InputEvent;
use crate::input::grab::{GrabMode, InputGrabState};
use crate::input::translate::{self, MouseButton, MultiTouchState, PinchState};

/// Errors from input manager operations.
#[derive(Debug, Error)]
pub enum InputError {
    /// Failed to connect to the virtio-input socket.
    #[error("failed to connect to virtio-input socket: {0}")]
    SocketConnect(std::io::Error),

    /// Failed to write events to the socket.
    #[error("failed to write to virtio-input socket: {0}")]
    SocketWrite(std::io::Error),

    /// Display metrics lock was poisoned.
    #[error("display metrics lock poisoned")]
    MetricsPoisoned,
}

/// Result type for input operations.
pub type InputResult<T> = Result<T, InputError>;

/// Manages input event translation and injection into the VM.
///
/// Connects to the crosvm virtio-input Unix socket and translates
/// host input events into Linux evdev events for the Android guest.
pub struct InputManager {
    /// Socket connection to crosvm virtio-input device.
    socket: Option<UnixStream>,
    /// Shared display metrics for coordinate mapping.
    metrics: SharedDisplayMetrics,
    /// Input grab (cursor capture) state.
    grab: InputGrabState,
    /// Multi-touch synthesis state.
    multi_touch: MultiTouchState,
}

impl std::fmt::Debug for InputManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputManager")
            .field("socket_connected", &self.socket.is_some())
            .field("grab", &self.grab)
            .field("multi_touch", &self.multi_touch)
            .finish_non_exhaustive()
    }
}

impl InputManager {
    /// Create a new input manager with shared display metrics but no socket.
    ///
    /// Call [`connect`](Self::connect) to establish the socket connection
    /// before injecting events.
    #[must_use]
    pub fn new(metrics: SharedDisplayMetrics) -> Self {
        Self {
            socket: None,
            metrics,
            grab: InputGrabState::new(),
            multi_touch: MultiTouchState::new(),
        }
    }

    /// Connect to the virtio-input Unix socket at the given path.
    ///
    /// # Errors
    ///
    /// Returns `InputError::SocketConnect` if the socket cannot be opened.
    pub fn connect(&mut self, path: &Path) -> InputResult<()> {
        let stream = UnixStream::connect(path).map_err(InputError::SocketConnect)?;
        stream
            .set_nonblocking(false)
            .map_err(InputError::SocketConnect)?;
        self.socket = Some(stream);
        Ok(())
    }

    /// Check whether the socket is connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.socket.is_some()
    }

    /// Get a reference to the shared display metrics.
    #[must_use]
    pub fn metrics(&self) -> &SharedDisplayMetrics {
        &self.metrics
    }

    /// Get the current grab mode.
    #[must_use]
    pub fn grab_mode(&self) -> GrabMode {
        self.grab.mode()
    }

    /// Inject a batch of evdev events into the virtio-input socket.
    ///
    /// Serializes each event to bytes and writes them to the socket.
    /// On write failure, logs the error and returns it, but does not panic.
    ///
    /// # Errors
    ///
    /// Returns `InputError::SocketWrite` if the socket write fails.
    pub fn inject(&mut self, events: &[InputEvent]) -> InputResult<()> {
        let Some(socket) = &mut self.socket else {
            log::warn!(
                "input: no socket connected, dropping {} events",
                events.len()
            );
            return Ok(());
        };

        let mut buf = Vec::with_capacity(events.len() * 16);
        for event in events {
            buf.extend_from_slice(&event.to_bytes());
        }

        if let Err(e) = socket.write_all(&buf) {
            log::error!("input: socket write failed: {e}");
            return Err(InputError::SocketWrite(e));
        }

        Ok(())
    }

    /// Handle a key press or release event.
    ///
    /// Translates the GTK4 hardware keycode to an evdev key event and
    /// injects it into the VM.
    ///
    /// # Errors
    ///
    /// Returns an error if event injection fails.
    pub fn handle_key(&mut self, keycode: u16, pressed: bool) -> InputResult<()> {
        let events = translate::translate_key(keycode, pressed);
        self.inject(&events)
    }

    /// Handle mouse motion.
    ///
    /// In free mode, translates to absolute position events.
    /// In grabbed mode, computes relative deltas and translates to
    /// relative motion events.
    ///
    /// When a pinch gesture is active, updates both touch slots.
    ///
    /// # Errors
    ///
    /// Returns an error if event injection fails.
    pub fn handle_motion(&mut self, host_x: f64, host_y: f64) -> InputResult<()> {
        let metrics = self
            .metrics
            .lock()
            .map_err(|_| InputError::MetricsPoisoned)?;

        // If pinch is active, move both touch points
        if self.multi_touch.pinch == PinchState::Active {
            let events = translate::translate_pinch_move(host_x, host_y, &metrics);
            drop(metrics);
            return self.inject(&events);
        }

        let events = if self.grab.is_grabbed() {
            let (dx, dy) = self.grab.compute_delta(host_x, host_y);
            translate::translate_relative_motion(dx, dy)
        } else {
            translate::translate_motion(host_x, host_y, &metrics)
        };

        drop(metrics);
        self.inject(&events)
    }

    /// Handle a mouse button click (press or release).
    ///
    /// Left-click maps to Android touch; right-click maps to Android back.
    /// Ctrl+left-click initiates a pinch-zoom gesture.
    ///
    /// # Errors
    ///
    /// Returns an error if event injection fails.
    pub fn handle_click(
        &mut self,
        button: MouseButton,
        pressed: bool,
        host_x: f64,
        host_y: f64,
        ctrl_held: bool,
    ) -> InputResult<()> {
        let events = match (button, pressed) {
            (MouseButton::Left, true) if ctrl_held => {
                let metrics = self
                    .metrics
                    .lock()
                    .map_err(|_| InputError::MetricsPoisoned)?;
                translate::translate_pinch_begin(host_x, host_y, &metrics, &mut self.multi_touch)
            }
            (MouseButton::Left, true) => {
                let metrics = self
                    .metrics
                    .lock()
                    .map_err(|_| InputError::MetricsPoisoned)?;
                translate::translate_left_click_press(host_x, host_y, &metrics)
            }
            (MouseButton::Left, false) => {
                if self.multi_touch.pinch == PinchState::Active {
                    translate::translate_pinch_end(&mut self.multi_touch)
                } else {
                    translate::translate_left_click_release()
                }
            }
            (MouseButton::Right, true) => translate::translate_right_click(),
            (MouseButton::Right, false) => {
                // Right-click release — already sent press+release on press
                return Ok(());
            }
        };

        self.inject(&events)
    }

    /// Handle a scroll event.
    ///
    /// `dx` is horizontal delta, `dy` is vertical delta.
    ///
    /// # Errors
    ///
    /// Returns an error if event injection fails.
    pub fn handle_scroll(&mut self, dx: f64, dy: f64) -> InputResult<()> {
        let events = translate::translate_scroll(dx, dy);
        if events.is_empty() {
            return Ok(());
        }
        self.inject(&events)
    }

    /// Toggle input grab (cursor capture) mode.
    ///
    /// Returns the new grab mode after toggling.
    pub fn toggle_grab(&mut self) -> GrabMode {
        let new_mode = self.grab.toggle();
        log::info!("input: grab mode toggled to {new_mode:?}");
        new_mode
    }

    /// End any active pinch gesture (e.g., when Ctrl is released).
    ///
    /// # Errors
    ///
    /// Returns an error if event injection fails.
    pub fn end_pinch_if_active(&mut self) -> InputResult<()> {
        if self.multi_touch.pinch == PinchState::Active {
            let events = translate::translate_pinch_end(&mut self.multi_touch);
            self.inject(&events)?;
        }
        Ok(())
    }

    /// Reset grab position tracking (e.g., on cursor re-enter).
    pub fn reset_grab_position(&mut self) {
        self.grab.reset_position();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::coordinate::shared_display_metrics;
    use std::io::Read;
    use std::os::unix::net::UnixListener;

    /// Create a mock Unix socket pair for testing.
    fn mock_socket() -> (InputManager, UnixStream) {
        let dir = tempfile::tempdir().expect("tempdir");
        let sock_path = dir.path().join("test-input.sock");

        let listener = UnixListener::bind(&sock_path).expect("bind");

        let metrics = shared_display_metrics();
        let mut mgr = InputManager::new(metrics);
        mgr.connect(&sock_path).expect("connect");

        let (server, _) = listener.accept().expect("accept");
        server.set_nonblocking(false).expect("nonblocking");

        // Keep tempdir alive by leaking it (test only)
        std::mem::forget(dir);

        (mgr, server)
    }

    #[test]
    fn inject_writes_correct_bytes() {
        let (mut mgr, mut server) = mock_socket();

        let events = vec![InputEvent::new(0x01, 30, 1), InputEvent::syn_report()];
        mgr.inject(&events).expect("inject");

        let mut buf = [0u8; 32];
        let n = server.read(&mut buf).expect("read");
        assert_eq!(n, 32); // 2 events × 16 bytes

        // Verify first event
        let event_type = u16::from_ne_bytes([buf[8], buf[9]]);
        assert_eq!(event_type, 0x01); // EV_KEY
    }

    #[test]
    fn handle_key_end_to_end() {
        let (mut mgr, mut server) = mock_socket();

        mgr.handle_key(30, true).expect("handle_key");

        let mut buf = [0u8; 32];
        let n = server.read(&mut buf).expect("read");
        assert_eq!(n, 32); // EV_KEY + SYN_REPORT
    }

    #[test]
    fn handle_motion_absolute() {
        let (mut mgr, mut server) = mock_socket();

        mgr.handle_motion(960.0, 540.0).expect("handle_motion");

        let mut buf = [0u8; 48];
        let n = server.read(&mut buf).expect("read");
        assert_eq!(n, 48); // ABS_X + ABS_Y + SYN_REPORT
    }

    #[test]
    fn handle_click_left() {
        let (mut mgr, mut server) = mock_socket();

        mgr.handle_click(MouseButton::Left, true, 100.0, 200.0, false)
            .expect("click press");

        let mut buf = [0u8; 64];
        let n = server.read(&mut buf).expect("read");
        assert_eq!(n, 64); // BTN_TOUCH + ABS_X + ABS_Y + SYN_REPORT
    }

    #[test]
    fn handle_scroll_vertical() {
        let (mut mgr, mut server) = mock_socket();

        mgr.handle_scroll(0.0, 1.0).expect("scroll");

        let mut buf = [0u8; 32];
        let n = server.read(&mut buf).expect("read");
        assert_eq!(n, 32); // REL_WHEEL + SYN_REPORT
    }

    #[test]
    fn toggle_grab_cycles() {
        let metrics = shared_display_metrics();
        let mut mgr = InputManager::new(metrics);

        assert_eq!(mgr.grab_mode(), GrabMode::Free);

        let mode = mgr.toggle_grab();
        assert_eq!(mode, GrabMode::Grabbed);

        let mode = mgr.toggle_grab();
        assert_eq!(mode, GrabMode::Free);
    }

    #[test]
    fn no_socket_drops_events_gracefully() {
        let metrics = shared_display_metrics();
        let mut mgr = InputManager::new(metrics);

        // Should not panic or error — just logs and returns Ok
        let result = mgr.handle_key(30, true);
        assert!(result.is_ok());
    }

    #[test]
    fn handle_motion_grabbed_mode() {
        let (mut mgr, mut server) = mock_socket();

        mgr.toggle_grab();

        // First motion establishes baseline (delta = 0,0)
        mgr.handle_motion(100.0, 200.0).expect("motion 1");
        let mut buf = [0u8; 48];
        let n = server.read(&mut buf).expect("read");
        assert_eq!(n, 48); // REL_X + REL_Y + SYN

        // Second motion produces actual delta
        mgr.handle_motion(110.0, 195.0).expect("motion 2");
        let n = server.read(&mut buf).expect("read");
        assert_eq!(n, 48);
    }

    #[test]
    fn pinch_gesture_end_to_end() {
        let (mut mgr, mut server) = mock_socket();

        // Ctrl+click starts pinch
        mgr.handle_click(MouseButton::Left, true, 480.0, 270.0, true)
            .expect("pinch begin");

        let mut buf = [0u8; 256];
        let n = server.read(&mut buf).expect("read");
        assert!(n > 0);

        // Motion during pinch
        mgr.handle_motion(500.0, 300.0).expect("pinch move");
        let n = server.read(&mut buf).expect("read");
        assert!(n > 0);

        // Release ends pinch
        mgr.handle_click(MouseButton::Left, false, 500.0, 300.0, true)
            .expect("pinch end");
        let n = server.read(&mut buf).expect("read");
        assert!(n > 0);
    }
}
