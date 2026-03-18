//! `VSync` timing, frame pacing, and FPS counting for the display pipeline.
//!
//! This module provides the core timing logic used by the presentation layer
//! (in `nux-ui`) to synchronize frame display with the compositor's `VSync`
//! signal. The actual GTK `FrameClock` integration lives in the UI crate;
//! this module supplies the frame-pacing state machine and FPS counter.

use std::collections::VecDeque;
use std::time::Instant;

/// Rolling window FPS counter.
///
/// Tracks frame presentation timestamps over a one-second window and
/// reports the number of unique frames presented in that window.
#[derive(Debug)]
pub struct FpsCounter {
    /// Timestamps of frames presented within the rolling window.
    timestamps: VecDeque<Instant>,
    /// Duration of the rolling window.
    window: std::time::Duration,
    /// Last computed FPS value (updated once per second).
    last_fps: u32,
    /// When the FPS display was last updated.
    last_update: Instant,
}

impl FpsCounter {
    /// Create a new FPS counter with a one-second rolling window.
    pub fn new() -> Self {
        Self {
            timestamps: VecDeque::with_capacity(120),
            window: std::time::Duration::from_secs(1),
            last_fps: 0,
            last_update: Instant::now(),
        }
    }

    /// Record that a frame was presented at the given instant.
    pub fn record_frame(&mut self, now: Instant) {
        self.timestamps.push_back(now);
        self.prune(now);
    }

    /// Record that a frame was presented right now.
    pub fn record_frame_now(&mut self) {
        self.record_frame(Instant::now());
    }

    /// Get the current FPS value.
    ///
    /// This returns the cached value that is updated once per second
    /// via [`update`](Self::update). Call `update` periodically to
    /// refresh the value.
    pub fn fps(&self) -> u32 {
        self.last_fps
    }

    /// Update the cached FPS value if at least one second has elapsed
    /// since the last update.
    ///
    /// Returns `true` if the value was refreshed.
    pub fn update(&mut self, now: Instant) -> bool {
        if now.duration_since(self.last_update) >= self.window {
            self.prune(now);
            // Frame count in a 1-second window will never exceed u32::MAX.
            #[allow(clippy::cast_possible_truncation)]
            let count = self.timestamps.len() as u32;
            self.last_fps = count;
            self.last_update = now;
            true
        } else {
            false
        }
    }

    /// Remove timestamps older than the rolling window.
    fn prune(&mut self, now: Instant) {
        let cutoff = now.checked_sub(self.window).unwrap_or(now);
        while self.timestamps.front().is_some_and(|&t| t < cutoff) {
            self.timestamps.pop_front();
        }
    }
}

impl Default for FpsCounter {
    fn default() -> Self {
        Self::new()
    }
}

/// Frame pacing state for VSync-aligned presentation.
///
/// Tracks whether a new frame is available and whether the presentation
/// layer should redraw on the next tick.
#[derive(Debug)]
pub struct FramePacer {
    /// Whether VSync-aligned presentation is enabled.
    vsync_enabled: bool,
    /// Sequence number of the last frame sent to the watch channel.
    last_capture_seq: u64,
    /// Sequence number of the last frame presented to the display.
    last_present_seq: u64,
}

impl FramePacer {
    /// Create a new frame pacer.
    pub fn new(vsync_enabled: bool) -> Self {
        Self {
            vsync_enabled,
            last_capture_seq: 0,
            last_present_seq: 0,
        }
    }

    /// Notify the pacer that a new frame was captured.
    pub fn on_frame_captured(&mut self) {
        self.last_capture_seq += 1;
    }

    /// Check whether a new frame is available for presentation.
    pub fn has_new_frame(&self) -> bool {
        self.last_capture_seq > self.last_present_seq
    }

    /// Mark the current frame as presented.
    pub fn on_frame_presented(&mut self) {
        self.last_present_seq = self.last_capture_seq;
    }

    /// Returns `true` if VSync-aligned presentation is enabled.
    pub fn vsync_enabled(&self) -> bool {
        self.vsync_enabled
    }

    /// Enable or disable VSync-aligned presentation.
    pub fn set_vsync(&mut self, enabled: bool) {
        self.vsync_enabled = enabled;
    }

    /// Returns the number of frames dropped since the last presentation.
    ///
    /// A dropped frame is one that was captured but never presented because
    /// a newer frame arrived before the next `VSync` tick.
    pub fn dropped_since_last_present(&self) -> u64 {
        self.last_capture_seq
            .saturating_sub(self.last_present_seq + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn fps_counter_empty() {
        let counter = FpsCounter::new();
        assert_eq!(counter.fps(), 0);
    }

    #[test]
    fn fps_counter_records_frames() {
        let mut counter = FpsCounter::new();
        let start = Instant::now();

        // Record 60 frames within one second
        for i in 0..60 {
            let t = start + Duration::from_millis(i * 16);
            counter.record_frame(t);
        }

        // Update at t=1s
        let update_time = start + Duration::from_secs(1);
        counter.update(update_time);
        // All 60 frames are within the 1-second window
        assert_eq!(counter.fps(), 60);
    }

    #[test]
    fn fps_counter_prunes_old_frames() {
        let mut counter = FpsCounter::new();
        let start = Instant::now();

        // Record 30 frames in the first half-second
        for i in 0..30 {
            counter.record_frame(start + Duration::from_millis(i * 16));
        }

        // Record 30 more frames in the second second (1.0s–1.5s)
        for i in 0..30 {
            counter.record_frame(start + Duration::from_millis(1000 + i * 16));
        }

        // Update at t=2s — only the second batch should remain
        counter.update(start + Duration::from_secs(2));
        assert_eq!(counter.fps(), 30);
    }

    #[test]
    fn fps_counter_update_returns_false_before_window() {
        let mut counter = FpsCounter::new();
        let now = Instant::now();
        // Immediately after creation, update should return true (1s has not
        // elapsed from `last_update` which is set to `now` in `new()`).
        // Actually, since last_update is set to Instant::now() in new(),
        // and we call update almost immediately, it should return false.
        assert!(!counter.update(now));
    }

    #[test]
    fn frame_pacer_new_frame_detection() {
        let mut pacer = FramePacer::new(true);
        assert!(!pacer.has_new_frame());

        pacer.on_frame_captured();
        assert!(pacer.has_new_frame());

        pacer.on_frame_presented();
        assert!(!pacer.has_new_frame());
    }

    #[test]
    fn frame_pacer_dropped_frames() {
        let mut pacer = FramePacer::new(true);

        // Capture 5 frames without presenting
        for _ in 0..5 {
            pacer.on_frame_captured();
        }

        // 4 frames were "dropped" (only the latest will be presented)
        assert_eq!(pacer.dropped_since_last_present(), 4);

        pacer.on_frame_presented();
        assert_eq!(pacer.dropped_since_last_present(), 0);
    }

    #[test]
    fn frame_pacer_vsync_toggle() {
        let mut pacer = FramePacer::new(true);
        assert!(pacer.vsync_enabled());

        pacer.set_vsync(false);
        assert!(!pacer.vsync_enabled());
    }
}
