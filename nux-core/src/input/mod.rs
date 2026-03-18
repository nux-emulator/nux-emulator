//! Input routing from host events to Android VM via virtio-input.
//!
//! This module translates keyboard, mouse, touch, and scroll input from
//! the host into Linux evdev events and injects them into the crosvm
//! virtio-input socket for delivery to Android's `InputFlinger`.
//!
//! # Architecture
//!
//! - [`evdev`]: Linux `input_event` struct and evdev constants
//! - [`coordinate`]: Host-to-guest coordinate mapping with scaling/letterbox
//! - [`translate`]: Event translation functions (key, motion, click, scroll, multi-touch)
//! - [`grab`]: Input grab (cursor capture) state machine
//! - [`manager`]: Top-level `InputManager` that orchestrates everything

pub mod coordinate;
pub mod evdev;
pub mod grab;
pub mod manager;
pub mod translate;

pub use coordinate::{DisplayMetrics, SharedDisplayMetrics, map_host_to_guest};
pub use evdev::InputEvent;
pub use grab::{GrabMode, InputGrabState};
pub use manager::{InputError, InputManager, InputResult};
pub use translate::MouseButton;
