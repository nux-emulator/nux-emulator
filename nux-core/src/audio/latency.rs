//! Audio latency measurement and monitoring.

use super::config::LATENCY_WARNING_THRESHOLD_MS;
use std::time::Duration;

/// Result of an audio latency measurement.
#[derive(Debug, Clone)]
pub struct LatencyReport {
    /// Measured round-trip latency.
    pub latency: Duration,
    /// Whether the latency exceeds the warning threshold.
    pub exceeds_threshold: bool,
}

/// Audio event that can be sent to the UI layer.
#[derive(Debug, Clone)]
pub enum AudioEvent {
    /// Audio initialized successfully.
    Initialized,
    /// Audio initialization failed; VM continues without audio.
    InitFailed { reason: String },
    /// Latency measurement completed.
    LatencyMeasured(LatencyReport),
    /// High latency warning.
    HighLatencyWarning { latency_ms: u64 },
}

/// Estimate audio latency from the configured buffer parameters.
///
/// This computes the theoretical minimum latency based on period size,
/// period count, and a standard 48 kHz sample rate. Actual latency will
/// be higher due to host audio stack overhead.
pub fn estimate_latency(period_size: u32, period_count: u32, sample_rate: u32) -> LatencyReport {
    // Buffer latency = (period_size * period_count) / sample_rate
    let total_frames = u64::from(period_size) * u64::from(period_count);
    let latency_us = (total_frames * 1_000_000) / u64::from(sample_rate);
    let latency = Duration::from_micros(latency_us);

    let exceeds_threshold = latency.as_millis() > u128::from(LATENCY_WARNING_THRESHOLD_MS);

    LatencyReport {
        latency,
        exceeds_threshold,
    }
}

/// Log the latency measurement and emit a warning if it exceeds the threshold.
///
/// Returns an `AudioEvent` for the UI layer.
pub fn check_and_report_latency(report: &LatencyReport) -> Option<AudioEvent> {
    let ms = report.latency.as_millis();
    log::info!("audio latency estimate: {ms}ms");

    if report.exceeds_threshold {
        let latency_ms = u64::try_from(report.latency.as_millis()).unwrap_or(u64::MAX);
        log::warn!(
            "audio latency {ms}ms exceeds {LATENCY_WARNING_THRESHOLD_MS}ms threshold — \
             consider adjusting period_size/period_count or checking PipeWire config"
        );
        Some(AudioEvent::HighLatencyWarning { latency_ms })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_latency_is_below_threshold() {
        // 256 frames * 2 periods / 48000 Hz ≈ 10.67ms
        let report = estimate_latency(256, 2, 48000);
        assert!(!report.exceeds_threshold);
        assert!(report.latency.as_millis() < 80);
    }

    #[test]
    fn large_buffer_exceeds_threshold() {
        // 4096 frames * 4 periods / 48000 Hz ≈ 341ms
        let report = estimate_latency(4096, 4, 48000);
        assert!(report.exceeds_threshold);
        assert!(report.latency.as_millis() > 80);
    }

    #[test]
    fn check_report_emits_warning_when_exceeded() {
        let report = LatencyReport {
            latency: Duration::from_millis(100),
            exceeds_threshold: true,
        };
        let event = check_and_report_latency(&report);
        assert!(event.is_some());
        match event.unwrap() {
            AudioEvent::HighLatencyWarning { latency_ms } => {
                assert_eq!(latency_ms, 100);
            }
            other => panic!("expected HighLatencyWarning, got {other:?}"),
        }
    }

    #[test]
    fn check_report_no_warning_when_ok() {
        let report = LatencyReport {
            latency: Duration::from_millis(10),
            exceeds_threshold: false,
        };
        let event = check_and_report_latency(&report);
        assert!(event.is_none());
    }
}
