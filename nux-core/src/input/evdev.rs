//! Linux evdev event types and constants for virtio-input injection.
//!
//! Defines a pure-Rust `InputEvent` struct matching the Linux kernel's
//! `struct input_event` layout, plus the evdev constants needed for
//! keyboard, mouse, touch, and scroll event synthesis.

// Event types
/// Synchronization event type.
pub const EV_SYN: u16 = 0x00;
/// Key/button event type.
pub const EV_KEY: u16 = 0x01;
/// Relative axis event type.
pub const EV_REL: u16 = 0x02;
/// Absolute axis event type.
pub const EV_ABS: u16 = 0x03;

// Synchronization codes
/// End of event batch.
pub const SYN_REPORT: u16 = 0x00;

// Key/button codes
/// Touch contact (used as single-touch button).
pub const BTN_TOUCH: u16 = 0x14a;
/// Android back button.
pub const KEY_BACK: u16 = 158;

// Absolute axis codes
/// Absolute X position.
pub const ABS_X: u16 = 0x00;
/// Absolute Y position.
pub const ABS_Y: u16 = 0x01;
/// Multi-touch slot selector.
pub const ABS_MT_SLOT: u16 = 0x2f;
/// Multi-touch tracking ID (-1 = lift-off).
pub const ABS_MT_TRACKING_ID: u16 = 0x39;
/// Multi-touch X position.
pub const ABS_MT_POSITION_X: u16 = 0x35;
/// Multi-touch Y position.
pub const ABS_MT_POSITION_Y: u16 = 0x36;

// Relative axis codes
/// Relative X movement.
pub const REL_X: u16 = 0x00;
/// Relative Y movement.
pub const REL_Y: u16 = 0x01;
/// Vertical scroll wheel.
pub const REL_WHEEL: u16 = 0x08;
/// Horizontal scroll wheel.
pub const REL_HWHEEL: u16 = 0x06;

/// A Linux `input_event` compatible struct for virtio-input injection.
///
/// Uses the 16-byte virtio-input compact format that crosvm expects
/// on its virtio-input socket: two `u32` for timestamp (sec/usec),
/// `u16` type, `u16` code, `i32` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct InputEvent {
    /// Timestamp seconds (always 0 for injected events).
    pub tv_sec: u32,
    /// Timestamp microseconds (always 0 for injected events).
    pub tv_usec: u32,
    /// Event type (`EV_KEY`, `EV_ABS`, `EV_REL`, `EV_SYN`).
    pub event_type: u16,
    /// Event code (keycode, axis code, etc.).
    pub code: u16,
    /// Event value (key state, coordinate, delta, etc.).
    pub value: i32,
}

impl InputEvent {
    /// Create a new input event with zero timestamp.
    #[must_use]
    pub const fn new(event_type: u16, code: u16, value: i32) -> Self {
        Self {
            tv_sec: 0,
            tv_usec: 0,
            event_type,
            code,
            value,
        }
    }

    /// Create a `SYN_REPORT` event to terminate an event batch.
    #[must_use]
    pub const fn syn_report() -> Self {
        Self::new(EV_SYN, SYN_REPORT, 0)
    }

    /// Read the event type (safe copy from packed field).
    #[must_use]
    pub fn event_type(&self) -> u16 {
        self.event_type
    }

    /// Read the event code (safe copy from packed field).
    #[must_use]
    pub fn code(&self) -> u16 {
        self.code
    }

    /// Read the event value (safe copy from packed field).
    #[must_use]
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Serialize this event to its raw byte representation.
    ///
    /// Returns a 16-byte array matching the virtio-input event layout.
    #[must_use]
    #[allow(unsafe_code)]
    pub fn to_bytes(self) -> [u8; 16] {
        // SAFETY: `InputEvent` is `#[repr(C, packed)]` with no padding,
        // all fields are plain integer types, and the size is exactly 16 bytes.
        // This is a well-defined transmutation of a POD type to bytes.
        unsafe { std::mem::transmute::<Self, [u8; 16]>(self) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_event_size_is_16_bytes() {
        assert_eq!(std::mem::size_of::<InputEvent>(), 16);
    }

    #[test]
    fn input_event_field_offsets() {
        let evt = InputEvent::new(EV_KEY, KEY_BACK, 1);
        let bytes = evt.to_bytes();

        assert_eq!(
            u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            0
        );
        assert_eq!(
            u32::from_ne_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            0
        );
        assert_eq!(u16::from_ne_bytes([bytes[8], bytes[9]]), EV_KEY);
        assert_eq!(u16::from_ne_bytes([bytes[10], bytes[11]]), KEY_BACK);
        assert_eq!(
            i32::from_ne_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
            1
        );
    }

    #[test]
    fn syn_report_is_all_zeros() {
        let syn = InputEvent::syn_report();
        assert_eq!(syn.event_type(), EV_SYN);
        assert_eq!(syn.code(), SYN_REPORT);
        assert_eq!(syn.value(), 0);
        assert_eq!(syn.to_bytes(), [0u8; 16]);
    }

    #[test]
    fn accessor_methods_read_correctly() {
        let evt = InputEvent::new(EV_ABS, ABS_X, 1920);
        assert_eq!(evt.event_type(), EV_ABS);
        assert_eq!(evt.code(), ABS_X);
        assert_eq!(evt.value(), 1920);
    }

    #[test]
    fn roundtrip_bytes_consistency() {
        let evt = InputEvent::new(EV_ABS, ABS_X, 1920);
        let bytes = evt.to_bytes();
        assert_eq!(bytes.len(), 16);
        let event_type = u16::from_ne_bytes([bytes[8], bytes[9]]);
        let code = u16::from_ne_bytes([bytes[10], bytes[11]]);
        let value = i32::from_ne_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        assert_eq!(event_type, EV_ABS);
        assert_eq!(code, ABS_X);
        assert_eq!(value, 1920);
    }
}
