//! Audio support for Nux Emulator.
//!
//! Provides crosvm virtio-snd configuration, latency measurement,
//! volume control, and audio event reporting for the UI layer.

pub mod command;
pub mod config;
pub mod error;
pub mod latency;
pub mod volume;

pub use command::build_audio_args;
pub use config::{AudioConfig, LATENCY_WARNING_THRESHOLD_MS};
pub use error::{AudioError, AudioResult};
pub use latency::{AudioEvent, LatencyReport, check_and_report_latency, estimate_latency};
pub use volume::{MAX_VOLUME, VolumeState};
