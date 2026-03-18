//! Coordinate scaling between keymap-authored resolution and display resolution.

/// Cached scale factors for translating keymap coordinates to display coordinates.
#[derive(Debug, Clone)]
pub struct ScaleFactors {
    /// Keymap authored resolution width.
    keymap_w: f64,
    /// Keymap authored resolution height.
    keymap_h: f64,
    /// Current display resolution width.
    display_w: f64,
    /// Current display resolution height.
    display_h: f64,
    /// Horizontal scale ratio (display / keymap).
    ratio_x: f64,
    /// Vertical scale ratio (display / keymap).
    ratio_y: f64,
}

impl ScaleFactors {
    /// Create new scale factors from keymap and display resolutions.
    #[must_use]
    pub fn new(keymap_res: (u32, u32), display_res: (u32, u32)) -> Self {
        let keymap_w = f64::from(keymap_res.0);
        let keymap_h = f64::from(keymap_res.1);
        let display_w = f64::from(display_res.0);
        let display_h = f64::from(display_res.1);
        Self {
            keymap_w,
            keymap_h,
            display_w,
            display_h,
            ratio_x: display_w / keymap_w,
            ratio_y: display_h / keymap_h,
        }
    }

    /// Scale a coordinate from keymap space to display space.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn scale(&self, x: i32, y: i32) -> (i32, i32) {
        let sx = (f64::from(x) * self.ratio_x).round() as i32;
        let sy = (f64::from(y) * self.ratio_y).round() as i32;
        (sx, sy)
    }

    /// Scale a floating-point coordinate from keymap space to display space.
    #[must_use]
    pub fn scale_f64(&self, x: f64, y: f64) -> (f64, f64) {
        (x * self.ratio_x, y * self.ratio_y)
    }

    /// Update the display resolution and recompute cached ratios.
    pub fn update_resolution(&mut self, display_res: (u32, u32)) {
        self.display_w = f64::from(display_res.0);
        self.display_h = f64::from(display_res.1);
        self.ratio_x = self.display_w / self.keymap_w;
        self.ratio_y = self.display_h / self.keymap_h;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_scaling() {
        let sf = ScaleFactors::new((1080, 1920), (1080, 1920));
        assert_eq!(sf.scale(500, 800), (500, 800));
    }

    #[test]
    fn double_scaling() {
        let sf = ScaleFactors::new((1080, 1920), (2160, 3840));
        assert_eq!(sf.scale(100, 200), (200, 400));
    }

    #[test]
    fn non_uniform_scaling() {
        let sf = ScaleFactors::new((1080, 1920), (2160, 1920));
        // x doubles, y stays same
        assert_eq!(sf.scale(100, 200), (200, 200));
    }

    #[test]
    fn update_resolution_changes_scale() {
        let mut sf = ScaleFactors::new((1080, 1920), (1080, 1920));
        assert_eq!(sf.scale(100, 200), (100, 200));

        sf.update_resolution((2160, 3840));
        assert_eq!(sf.scale(100, 200), (200, 400));
    }

    #[test]
    fn fractional_scaling() {
        // 1080 -> 720: ratio = 0.6667
        let sf = ScaleFactors::new((1080, 1920), (720, 1280));
        let (sx, sy) = sf.scale(1080, 1920);
        assert_eq!(sx, 720);
        assert_eq!(sy, 1280);
    }
}
