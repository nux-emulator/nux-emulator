//! Audio error types.

use thiserror::Error;

/// Errors that can occur during audio setup and control.
#[derive(Debug, Error)]
pub enum AudioError {
    /// Audio initialization failed on the host.
    #[error(
        "audio initialization failed: {0}. The VM will continue without audio. \
         Check that PipeWire or PulseAudio is running."
    )]
    InitFailed(String),

    /// Volume level out of range.
    #[error("volume level {0} is out of range (0-100)")]
    VolumeOutOfRange(u8),

    /// Control socket communication failed.
    #[error("audio control error: {0}")]
    ControlError(String),

    /// Latency measurement failed.
    #[error("latency measurement failed: {0}")]
    LatencyMeasurementFailed(String),

    /// I/O error wrapper.
    #[error("audio I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for audio operations.
pub type AudioResult<T> = Result<T, AudioError>;
