//! Input grab state management.
//!
//! Tracks whether the mouse cursor is captured (grabbed) by the emulator
//! window, and computes relative motion deltas when in grabbed mode.

/// Whether input is currently grabbed (cursor captured) or free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrabMode {
    /// Cursor is free — absolute coordinates are used.
    Free,
    /// Cursor is grabbed — relative deltas are used.
    Grabbed,
}

/// Manages input grab state and relative motion computation.
#[derive(Debug)]
pub struct InputGrabState {
    /// Current grab mode.
    mode: GrabMode,
    /// Last known absolute X position (for delta computation in grabbed mode).
    last_x: f64,
    /// Last known absolute Y position (for delta computation in grabbed mode).
    last_y: f64,
    /// Whether we have a valid last position.
    has_last_position: bool,
}

impl Default for InputGrabState {
    fn default() -> Self {
        Self::new()
    }
}

impl InputGrabState {
    /// Create a new grab state, initially free (ungrabbed).
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: GrabMode::Free,
            last_x: 0.0,
            last_y: 0.0,
            has_last_position: false,
        }
    }

    /// Get the current grab mode.
    #[must_use]
    pub fn mode(&self) -> GrabMode {
        self.mode
    }

    /// Check if input is currently grabbed.
    #[must_use]
    pub fn is_grabbed(&self) -> bool {
        self.mode == GrabMode::Grabbed
    }

    /// Toggle between grabbed and free modes.
    ///
    /// Returns the new mode after toggling.
    pub fn toggle(&mut self) -> GrabMode {
        self.mode = match self.mode {
            GrabMode::Free => GrabMode::Grabbed,
            GrabMode::Grabbed => GrabMode::Free,
        };
        // Reset position tracking on mode change
        self.has_last_position = false;
        self.mode
    }

    /// Compute relative motion deltas from absolute positions.
    ///
    /// When grabbed, consecutive absolute positions are differenced to
    /// produce relative deltas. The first call after a grab or toggle
    /// returns `(0, 0)` since there is no previous position.
    ///
    /// Returns `(dx, dy)` as integer deltas.
    #[allow(clippy::cast_possible_truncation)]
    pub fn compute_delta(&mut self, abs_x: f64, abs_y: f64) -> (i32, i32) {
        if !self.has_last_position {
            self.last_x = abs_x;
            self.last_y = abs_y;
            self.has_last_position = true;
            return (0, 0);
        }

        let dx = (abs_x - self.last_x).round() as i32;
        let dy = (abs_y - self.last_y).round() as i32;

        self.last_x = abs_x;
        self.last_y = abs_y;

        (dx, dy)
    }

    /// Reset position tracking (e.g., when cursor re-enters the window).
    pub fn reset_position(&mut self) {
        self.has_last_position = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_free() {
        let state = InputGrabState::new();
        assert_eq!(state.mode(), GrabMode::Free);
        assert!(!state.is_grabbed());
    }

    #[test]
    fn toggle_switches_modes() {
        let mut state = InputGrabState::new();

        let mode = state.toggle();
        assert_eq!(mode, GrabMode::Grabbed);
        assert!(state.is_grabbed());

        let mode = state.toggle();
        assert_eq!(mode, GrabMode::Free);
        assert!(!state.is_grabbed());
    }

    #[test]
    fn first_delta_is_zero() {
        let mut state = InputGrabState::new();
        state.toggle(); // Enter grabbed mode

        let (dx, dy) = state.compute_delta(100.0, 200.0);
        assert_eq!(dx, 0);
        assert_eq!(dy, 0);
    }

    #[test]
    fn subsequent_deltas_are_correct() {
        let mut state = InputGrabState::new();
        state.toggle();

        // First call establishes baseline
        state.compute_delta(100.0, 200.0);

        // Second call produces delta
        let (dx, dy) = state.compute_delta(110.0, 195.0);
        assert_eq!(dx, 10);
        assert_eq!(dy, -5);

        // Third call from new position
        let (dx, dy) = state.compute_delta(110.0, 195.0);
        assert_eq!(dx, 0);
        assert_eq!(dy, 0);
    }

    #[test]
    fn toggle_resets_position() {
        let mut state = InputGrabState::new();
        state.toggle(); // Grabbed

        state.compute_delta(100.0, 200.0);
        state.compute_delta(150.0, 250.0);

        // Toggle back to free, then to grabbed again
        state.toggle();
        state.toggle();

        // First delta after re-grab should be zero
        let (dx, dy) = state.compute_delta(300.0, 400.0);
        assert_eq!(dx, 0);
        assert_eq!(dy, 0);
    }

    #[test]
    fn reset_position_clears_tracking() {
        let mut state = InputGrabState::new();
        state.toggle();

        state.compute_delta(100.0, 200.0);
        state.reset_position();

        // After reset, first delta is zero again
        let (dx, dy) = state.compute_delta(500.0, 600.0);
        assert_eq!(dx, 0);
        assert_eq!(dy, 0);
    }
}
