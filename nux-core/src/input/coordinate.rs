//! Coordinate mapping between host window and Android guest screen.
//!
//! Handles scaling, letterboxing, and clamping so that host pointer
//! coordinates are correctly transformed to guest screen coordinates.

use std::sync::{Arc, Mutex};

/// Display geometry used to map host coordinates to guest coordinates.
///
/// Updated by the display pipeline on window resize or guest resolution
/// change; read by the input system for coordinate transformation.
#[derive(Debug, Clone)]
pub struct DisplayMetrics {
    /// Android guest screen width in pixels.
    pub guest_width: i32,
    /// Android guest screen height in pixels.
    pub guest_height: i32,
    /// Host GTK surface width in pixels.
    pub host_width: f64,
    /// Host GTK surface height in pixels.
    pub host_height: f64,
    /// Scale factor: guest pixels per host pixel.
    pub scale: f64,
    /// Horizontal letterbox offset in host pixels (black bars on left/right).
    pub letterbox_x: f64,
    /// Vertical letterbox offset in host pixels (black bars on top/bottom).
    pub letterbox_y: f64,
}

impl Default for DisplayMetrics {
    fn default() -> Self {
        Self {
            guest_width: 1920,
            guest_height: 1080,
            host_width: 1920.0,
            host_height: 1080.0,
            scale: 1.0,
            letterbox_x: 0.0,
            letterbox_y: 0.0,
        }
    }
}

impl DisplayMetrics {
    /// Create new display metrics with the given dimensions.
    ///
    /// Automatically computes scale factor and letterbox offsets to fit
    /// the guest resolution within the host surface while preserving
    /// aspect ratio.
    #[must_use]
    pub fn new(guest_width: i32, guest_height: i32, host_width: f64, host_height: f64) -> Self {
        let mut m = Self {
            guest_width,
            guest_height,
            host_width,
            host_height,
            scale: 1.0,
            letterbox_x: 0.0,
            letterbox_y: 0.0,
        };
        m.recalculate();
        m
    }

    /// Recalculate scale factor and letterbox offsets from current dimensions.
    pub fn recalculate(&mut self) {
        if self.guest_width <= 0 || self.guest_height <= 0 {
            self.scale = 1.0;
            self.letterbox_x = 0.0;
            self.letterbox_y = 0.0;
            return;
        }

        let scale_x = self.host_width / f64::from(self.guest_width);
        let scale_y = self.host_height / f64::from(self.guest_height);
        self.scale = scale_x.min(scale_y);

        let rendered_width = f64::from(self.guest_width) * self.scale;
        let rendered_height = f64::from(self.guest_height) * self.scale;

        self.letterbox_x = (self.host_width - rendered_width) / 2.0;
        self.letterbox_y = (self.host_height - rendered_height) / 2.0;
    }

    /// Update host surface dimensions and recalculate derived values.
    pub fn update_host(&mut self, host_width: f64, host_height: f64) {
        self.host_width = host_width;
        self.host_height = host_height;
        self.recalculate();
    }

    /// Update guest resolution and recalculate derived values.
    pub fn update_guest(&mut self, guest_width: i32, guest_height: i32) {
        self.guest_width = guest_width;
        self.guest_height = guest_height;
        self.recalculate();
    }
}

/// Thread-safe shared reference to display metrics.
pub type SharedDisplayMetrics = Arc<Mutex<DisplayMetrics>>;

/// Create a new shared display metrics instance with default values.
#[must_use]
pub fn shared_display_metrics() -> SharedDisplayMetrics {
    Arc::new(Mutex::new(DisplayMetrics::default()))
}

/// Map host surface coordinates to Android guest screen coordinates.
///
/// Subtracts letterbox offsets, divides by scale factor, and clamps
/// to guest resolution bounds.
#[must_use]
pub fn map_host_to_guest(host_x: f64, host_y: f64, metrics: &DisplayMetrics) -> (i32, i32) {
    // Subtract letterbox offset
    let adjusted_x = host_x - metrics.letterbox_x;
    let adjusted_y = host_y - metrics.letterbox_y;

    // Divide by scale factor
    let guest_x = adjusted_x / metrics.scale;
    let guest_y = adjusted_y / metrics.scale;

    // Clamp to guest bounds
    #[allow(clippy::cast_possible_truncation)]
    let clamped_x = guest_x.round() as i32;
    #[allow(clippy::cast_possible_truncation)]
    let clamped_y = guest_y.round() as i32;

    let clamped_x = clamped_x.clamp(0, metrics.guest_width - 1);
    let clamped_y = clamped_y.clamp(0, metrics.guest_height - 1);

    (clamped_x, clamped_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_scaling_no_letterbox() {
        // Guest 1920x1080, host 960x540 → scale 0.5, no letterbox
        let m = DisplayMetrics::new(1920, 1080, 960.0, 540.0);
        assert!((m.scale - 0.5).abs() < f64::EPSILON);
        assert!((m.letterbox_x).abs() < f64::EPSILON);
        assert!((m.letterbox_y).abs() < f64::EPSILON);

        let (gx, gy) = map_host_to_guest(480.0, 270.0, &m);
        assert_eq!(gx, 960);
        assert_eq!(gy, 540);
    }

    #[test]
    fn letterboxing_wider_host() {
        // Guest 1920x1080, host 2000x1080 → scale 1.0, letterbox_x = 40
        let m = DisplayMetrics::new(1920, 1080, 2000.0, 1080.0);
        assert!((m.scale - 1.0).abs() < f64::EPSILON);
        assert!((m.letterbox_x - 40.0).abs() < f64::EPSILON);
        assert!((m.letterbox_y).abs() < f64::EPSILON);

        // Click in the letterbox area → clamps to 0
        let (gx, _) = map_host_to_guest(10.0, 540.0, &m);
        assert_eq!(gx, 0);

        // Click at center of rendered area
        let (gx, gy) = map_host_to_guest(1000.0, 540.0, &m);
        assert_eq!(gx, 960);
        assert_eq!(gy, 540);
    }

    #[test]
    fn letterboxing_taller_host() {
        // Guest 1920x1080, host 1920x1200 → scale 1.0, letterbox_y = 60
        let m = DisplayMetrics::new(1920, 1080, 1920.0, 1200.0);
        assert!((m.scale - 1.0).abs() < f64::EPSILON);
        assert!((m.letterbox_y - 60.0).abs() < f64::EPSILON);

        // Click in top letterbox → clamps to 0
        let (_, gy) = map_host_to_guest(960.0, 10.0, &m);
        assert_eq!(gy, 0);
    }

    #[test]
    fn combined_scaling_and_letterbox() {
        // Guest 1920x1080, host 1000x1000
        // scale_x = 1000/1920 ≈ 0.5208, scale_y = 1000/1080 ≈ 0.9259
        // scale = min = 0.5208..., rendered_w = 1000, rendered_h ≈ 562.5
        // letterbox_x = 0, letterbox_y ≈ 218.75
        let m = DisplayMetrics::new(1920, 1080, 1000.0, 1000.0);
        assert!(m.letterbox_x.abs() < 0.01);
        assert!((m.letterbox_y - 218.75).abs() < 0.01);

        // Center of host → center of guest
        let (gx, gy) = map_host_to_guest(500.0, 500.0, &m);
        assert_eq!(gx, 960);
        assert_eq!(gy, 540);
    }

    #[test]
    fn edge_clamping() {
        let m = DisplayMetrics::new(1920, 1080, 1920.0, 1080.0);

        // Negative coordinates clamp to 0
        let (gx, gy) = map_host_to_guest(-10.0, -10.0, &m);
        assert_eq!(gx, 0);
        assert_eq!(gy, 0);

        // Beyond bounds clamp to max
        let (gx, gy) = map_host_to_guest(2000.0, 1200.0, &m);
        assert_eq!(gx, 1919);
        assert_eq!(gy, 1079);
    }

    #[test]
    fn shared_metrics_thread_safe() {
        let shared = shared_display_metrics();
        let metrics = shared.lock().expect("lock poisoned");
        assert_eq!(metrics.guest_width, 1920);
        assert_eq!(metrics.guest_height, 1080);
    }

    #[test]
    fn update_host_recalculates() {
        let mut m = DisplayMetrics::new(1920, 1080, 1920.0, 1080.0);
        assert!((m.scale - 1.0).abs() < f64::EPSILON);

        m.update_host(960.0, 540.0);
        assert!((m.scale - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn update_guest_recalculates() {
        let mut m = DisplayMetrics::new(1920, 1080, 1920.0, 1080.0);
        m.update_guest(3840, 2160);
        assert!((m.scale - 0.5).abs() < f64::EPSILON);
    }
}
