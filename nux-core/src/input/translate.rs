//! Input translation from host events to Linux evdev event sequences.
//!
//! Converts keyboard, mouse, touch, and scroll events into batches of
//! `InputEvent` structs ready for injection into the virtio-input socket.

use crate::input::coordinate::{DisplayMetrics, map_host_to_guest};
use crate::input::evdev::{
    ABS_MT_POSITION_X, ABS_MT_POSITION_Y, ABS_MT_SLOT, ABS_MT_TRACKING_ID, ABS_X, ABS_Y, BTN_TOUCH,
    EV_ABS, EV_KEY, EV_REL, InputEvent, KEY_BACK, REL_HWHEEL, REL_WHEEL, REL_X, REL_Y,
};

/// Mouse button identifiers matching GTK4 conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Left mouse button (button 1).
    Left,
    /// Right mouse button (button 3).
    Right,
}

/// Multi-touch pinch gesture state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinchState {
    /// No pinch gesture active.
    Idle,
    /// Two-finger pinch gesture in progress.
    Active,
}

/// Tracks multi-touch slot state for type-B protocol synthesis.
#[derive(Debug)]
pub struct MultiTouchState {
    /// Current pinch gesture state.
    pub pinch: PinchState,
    /// Next tracking ID to assign (monotonically increasing).
    next_tracking_id: i32,
    /// Whether slot 0 is currently active.
    slot0_active: bool,
    /// Whether slot 1 is currently active.
    slot1_active: bool,
}

impl Default for MultiTouchState {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiTouchState {
    /// Create a new multi-touch state with no active touches.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pinch: PinchState::Idle,
            next_tracking_id: 0,
            slot0_active: false,
            slot1_active: false,
        }
    }

    /// Allocate the next tracking ID.
    fn alloc_tracking_id(&mut self) -> i32 {
        let id = self.next_tracking_id;
        self.next_tracking_id = self.next_tracking_id.wrapping_add(1);
        id
    }
}

/// Translate a key press or release into evdev events.
///
/// GTK4 hardware keycodes map directly to Linux evdev keycodes (they are
/// the same numbering on Linux/evdev backends).
///
/// Returns a batch of events: `EV_KEY` + `SYN_REPORT`.
#[must_use]
pub fn translate_key(keycode: u16, pressed: bool) -> Vec<InputEvent> {
    let value = i32::from(pressed);
    vec![
        InputEvent::new(EV_KEY, keycode, value),
        InputEvent::syn_report(),
    ]
}

/// Translate mouse motion into absolute position evdev events.
///
/// Coordinates are mapped from host surface space to guest screen space.
///
/// Returns a batch: `EV_ABS/ABS_X` + `EV_ABS/ABS_Y` + `SYN_REPORT`.
#[must_use]
pub fn translate_motion(host_x: f64, host_y: f64, metrics: &DisplayMetrics) -> Vec<InputEvent> {
    let (gx, gy) = map_host_to_guest(host_x, host_y, metrics);
    vec![
        InputEvent::new(EV_ABS, ABS_X, gx),
        InputEvent::new(EV_ABS, ABS_Y, gy),
        InputEvent::syn_report(),
    ]
}

/// Translate a left-click press into touch-down evdev events.
///
/// Emits `BTN_TOUCH` down + absolute position + `SYN_REPORT`.
#[must_use]
pub fn translate_left_click_press(
    host_x: f64,
    host_y: f64,
    metrics: &DisplayMetrics,
) -> Vec<InputEvent> {
    let (gx, gy) = map_host_to_guest(host_x, host_y, metrics);
    vec![
        InputEvent::new(EV_KEY, BTN_TOUCH, 1),
        InputEvent::new(EV_ABS, ABS_X, gx),
        InputEvent::new(EV_ABS, ABS_Y, gy),
        InputEvent::syn_report(),
    ]
}

/// Translate a left-click release into touch-up evdev events.
///
/// Emits `BTN_TOUCH` up + `SYN_REPORT`.
#[must_use]
pub fn translate_left_click_release() -> Vec<InputEvent> {
    vec![
        InputEvent::new(EV_KEY, BTN_TOUCH, 0),
        InputEvent::syn_report(),
    ]
}

/// Translate a right-click into Android back button press+release.
///
/// Emits `KEY_BACK` press + `SYN_REPORT` + `KEY_BACK` release + `SYN_REPORT`.
#[must_use]
pub fn translate_right_click() -> Vec<InputEvent> {
    vec![
        InputEvent::new(EV_KEY, KEY_BACK, 1),
        InputEvent::syn_report(),
        InputEvent::new(EV_KEY, KEY_BACK, 0),
        InputEvent::syn_report(),
    ]
}

/// Translate a scroll event into relative wheel evdev events.
///
/// `dx` is horizontal scroll delta, `dy` is vertical scroll delta.
/// Returns events only for non-zero deltas.
#[must_use]
pub fn translate_scroll(dx: f64, dy: f64) -> Vec<InputEvent> {
    let mut events = Vec::new();

    if dy.abs() > f64::EPSILON {
        #[allow(clippy::cast_possible_truncation)]
        let value = dy.round() as i32;
        events.push(InputEvent::new(EV_REL, REL_WHEEL, value));
        events.push(InputEvent::syn_report());
    }

    if dx.abs() > f64::EPSILON {
        #[allow(clippy::cast_possible_truncation)]
        let value = dx.round() as i32;
        events.push(InputEvent::new(EV_REL, REL_HWHEEL, value));
        events.push(InputEvent::syn_report());
    }

    events
}

/// Translate relative mouse motion (grabbed mode) into `EV_REL` events.
///
/// Returns `REL_X` + `REL_Y` + `SYN_REPORT`.
#[must_use]
pub fn translate_relative_motion(dx: i32, dy: i32) -> Vec<InputEvent> {
    vec![
        InputEvent::new(EV_REL, REL_X, dx),
        InputEvent::new(EV_REL, REL_Y, dy),
        InputEvent::syn_report(),
    ]
}

// --- Multi-touch synthesis ---

/// Initiate a two-finger pinch gesture (Ctrl+click).
///
/// Places slot 0 at the click position and slot 1 at the mirrored
/// position across the screen center.
///
/// Returns the multi-touch event batch.
#[must_use]
pub fn translate_pinch_begin(
    host_x: f64,
    host_y: f64,
    metrics: &DisplayMetrics,
    mt: &mut MultiTouchState,
) -> Vec<InputEvent> {
    let (gx, gy) = map_host_to_guest(host_x, host_y, metrics);
    let mirror_x = metrics.guest_width - gx;
    let mirror_y = metrics.guest_height - gy;

    mt.pinch = PinchState::Active;
    let id0 = mt.alloc_tracking_id();
    let id1 = mt.alloc_tracking_id();
    mt.slot0_active = true;
    mt.slot1_active = true;

    vec![
        // Slot 0: click position
        InputEvent::new(EV_ABS, ABS_MT_SLOT, 0),
        InputEvent::new(EV_ABS, ABS_MT_TRACKING_ID, id0),
        InputEvent::new(EV_ABS, ABS_MT_POSITION_X, gx),
        InputEvent::new(EV_ABS, ABS_MT_POSITION_Y, gy),
        // Slot 1: mirrored position
        InputEvent::new(EV_ABS, ABS_MT_SLOT, 1),
        InputEvent::new(EV_ABS, ABS_MT_TRACKING_ID, id1),
        InputEvent::new(EV_ABS, ABS_MT_POSITION_X, mirror_x),
        InputEvent::new(EV_ABS, ABS_MT_POSITION_Y, mirror_y),
        // BTN_TOUCH down + sync
        InputEvent::new(EV_KEY, BTN_TOUCH, 1),
        InputEvent::syn_report(),
    ]
}

/// Update both touch points during a pinch drag (Ctrl+drag).
///
/// Slot 0 follows the mouse; slot 1 mirrors around screen center.
#[must_use]
pub fn translate_pinch_move(host_x: f64, host_y: f64, metrics: &DisplayMetrics) -> Vec<InputEvent> {
    let (gx, gy) = map_host_to_guest(host_x, host_y, metrics);
    let mirror_x = metrics.guest_width - gx;
    let mirror_y = metrics.guest_height - gy;

    vec![
        InputEvent::new(EV_ABS, ABS_MT_SLOT, 0),
        InputEvent::new(EV_ABS, ABS_MT_POSITION_X, gx),
        InputEvent::new(EV_ABS, ABS_MT_POSITION_Y, gy),
        InputEvent::new(EV_ABS, ABS_MT_SLOT, 1),
        InputEvent::new(EV_ABS, ABS_MT_POSITION_X, mirror_x),
        InputEvent::new(EV_ABS, ABS_MT_POSITION_Y, mirror_y),
        InputEvent::syn_report(),
    ]
}

/// End a pinch gesture by lifting both touch slots.
///
/// Emits `ABS_MT_TRACKING_ID = -1` for both slots to signal lift-off.
#[must_use]
pub fn translate_pinch_end(mt: &mut MultiTouchState) -> Vec<InputEvent> {
    mt.pinch = PinchState::Idle;
    mt.slot0_active = false;
    mt.slot1_active = false;

    vec![
        InputEvent::new(EV_ABS, ABS_MT_SLOT, 0),
        InputEvent::new(EV_ABS, ABS_MT_TRACKING_ID, -1),
        InputEvent::new(EV_ABS, ABS_MT_SLOT, 1),
        InputEvent::new(EV_ABS, ABS_MT_TRACKING_ID, -1),
        InputEvent::new(EV_KEY, BTN_TOUCH, 0),
        InputEvent::syn_report(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::evdev::*;

    fn default_metrics() -> DisplayMetrics {
        DisplayMetrics::default()
    }

    #[test]
    fn key_press_produces_ev_key_and_syn() {
        let events = translate_key(30, true); // keycode 30 = 'a'
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type(), EV_KEY);
        assert_eq!(events[0].code(), 30);
        assert_eq!(events[0].value(), 1);
        assert_eq!(events[1].event_type(), EV_SYN);
    }

    #[test]
    fn key_release_produces_ev_key_value_0() {
        let events = translate_key(30, false);
        assert_eq!(events[0].value(), 0);
    }

    #[test]
    fn motion_produces_abs_x_y_syn() {
        let m = default_metrics();
        let events = translate_motion(960.0, 540.0, &m);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type(), EV_ABS);
        assert_eq!(events[0].code(), ABS_X);
        assert_eq!(events[0].value(), 960);
        assert_eq!(events[1].event_type(), EV_ABS);
        assert_eq!(events[1].code(), ABS_Y);
        assert_eq!(events[1].value(), 540);
        assert_eq!(events[2].event_type(), EV_SYN);
    }

    #[test]
    fn left_click_press_produces_btn_touch_and_position() {
        let m = default_metrics();
        let events = translate_left_click_press(100.0, 200.0, &m);
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].event_type(), EV_KEY);
        assert_eq!(events[0].code(), BTN_TOUCH);
        assert_eq!(events[0].value(), 1);
        assert_eq!(events[1].code(), ABS_X);
        assert_eq!(events[2].code(), ABS_Y);
        assert_eq!(events[3].event_type(), EV_SYN);
    }

    #[test]
    fn left_click_release_produces_btn_touch_up() {
        let events = translate_left_click_release();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type(), EV_KEY);
        assert_eq!(events[0].code(), BTN_TOUCH);
        assert_eq!(events[0].value(), 0);
    }

    #[test]
    fn right_click_produces_back_press_release() {
        let events = translate_right_click();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].code(), KEY_BACK);
        assert_eq!(events[0].value(), 1);
        assert_eq!(events[1].event_type(), EV_SYN);
        assert_eq!(events[2].code(), KEY_BACK);
        assert_eq!(events[2].value(), 0);
        assert_eq!(events[3].event_type(), EV_SYN);
    }

    #[test]
    fn vertical_scroll_produces_rel_wheel() {
        let events = translate_scroll(0.0, 3.0);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type(), EV_REL);
        assert_eq!(events[0].code(), REL_WHEEL);
        assert_eq!(events[0].value(), 3);
    }

    #[test]
    fn horizontal_scroll_produces_rel_hwheel() {
        let events = translate_scroll(2.0, 0.0);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].code(), REL_HWHEEL);
        assert_eq!(events[0].value(), 2);
    }

    #[test]
    fn both_scroll_axes() {
        let events = translate_scroll(1.0, -1.0);
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].code(), REL_WHEEL);
        assert_eq!(events[0].value(), -1);
        assert_eq!(events[2].code(), REL_HWHEEL);
        assert_eq!(events[2].value(), 1);
    }

    #[test]
    fn zero_scroll_produces_no_events() {
        let events = translate_scroll(0.0, 0.0);
        assert!(events.is_empty());
    }

    #[test]
    fn relative_motion_produces_rel_x_y() {
        let events = translate_relative_motion(5, -3);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type(), EV_REL);
        assert_eq!(events[0].code(), REL_X);
        assert_eq!(events[0].value(), 5);
        assert_eq!(events[1].code(), REL_Y);
        assert_eq!(events[1].value(), -3);
    }

    #[test]
    fn pinch_begin_creates_two_slots() {
        let m = default_metrics();
        let mut mt = MultiTouchState::new();
        let events = translate_pinch_begin(480.0, 270.0, &m, &mut mt);

        assert_eq!(mt.pinch, PinchState::Active);

        // Slot 0 at (480, 270)
        assert_eq!(events[0].code(), ABS_MT_SLOT);
        assert_eq!(events[0].value(), 0);
        assert_eq!(events[1].code(), ABS_MT_TRACKING_ID);
        assert_eq!(events[1].value(), 0); // first ID
        assert_eq!(events[2].code(), ABS_MT_POSITION_X);
        assert_eq!(events[2].value(), 480);
        assert_eq!(events[3].code(), ABS_MT_POSITION_Y);
        assert_eq!(events[3].value(), 270);

        // Slot 1 mirrored: (1920-480, 1080-270) = (1440, 810)
        assert_eq!(events[4].code(), ABS_MT_SLOT);
        assert_eq!(events[4].value(), 1);
        assert_eq!(events[5].code(), ABS_MT_TRACKING_ID);
        assert_eq!(events[5].value(), 1); // second ID
        assert_eq!(events[6].code(), ABS_MT_POSITION_X);
        assert_eq!(events[6].value(), 1440);
        assert_eq!(events[7].code(), ABS_MT_POSITION_Y);
        assert_eq!(events[7].value(), 810);
    }

    #[test]
    fn pinch_move_updates_both_slots() {
        let m = default_metrics();
        let events = translate_pinch_move(600.0, 400.0, &m);
        assert_eq!(events.len(), 7);
        // Slot 0
        assert_eq!(events[0].value(), 0); // slot 0
        assert_eq!(events[1].value(), 600); // x
        assert_eq!(events[2].value(), 400); // y
        // Slot 1 mirrored: (1920-600, 1080-400) = (1320, 680)
        assert_eq!(events[3].value(), 1); // slot 1
        assert_eq!(events[4].value(), 1320); // mirror x
        assert_eq!(events[5].value(), 680); // mirror y
    }

    #[test]
    fn pinch_end_lifts_both_slots() {
        let m = default_metrics();
        let mut mt = MultiTouchState::new();
        // Start a pinch so state is Active
        let _ = translate_pinch_begin(480.0, 270.0, &m, &mut mt);

        let events = translate_pinch_end(&mut mt);
        assert_eq!(mt.pinch, PinchState::Idle);

        // Slot 0 tracking ID = -1
        assert_eq!(events[0].code(), ABS_MT_SLOT);
        assert_eq!(events[0].value(), 0);
        assert_eq!(events[1].code(), ABS_MT_TRACKING_ID);
        assert_eq!(events[1].value(), -1);

        // Slot 1 tracking ID = -1
        assert_eq!(events[2].code(), ABS_MT_SLOT);
        assert_eq!(events[2].value(), 1);
        assert_eq!(events[3].code(), ABS_MT_TRACKING_ID);
        assert_eq!(events[3].value(), -1);

        // BTN_TOUCH up
        assert_eq!(events[4].code(), BTN_TOUCH);
        assert_eq!(events[4].value(), 0);
    }

    #[test]
    fn pinch_full_lifecycle() {
        let m = default_metrics();
        let mut mt = MultiTouchState::new();

        // Begin
        let _ = translate_pinch_begin(400.0, 300.0, &m, &mut mt);
        assert_eq!(mt.pinch, PinchState::Active);

        // Move
        let _ = translate_pinch_move(500.0, 350.0, &m);

        // End
        let _ = translate_pinch_end(&mut mt);
        assert_eq!(mt.pinch, PinchState::Idle);
    }
}
