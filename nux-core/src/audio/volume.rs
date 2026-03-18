//! Volume control interface for crosvm virtio-snd.

use super::config::AudioConfig;
use super::error::{AudioError, AudioResult};

/// Maximum volume level.
pub const MAX_VOLUME: u8 = 100;

/// Volume control state.
#[derive(Debug, Clone)]
pub struct VolumeState {
    /// Current volume level (0–100).
    pub volume: u8,
    /// Whether audio is muted.
    pub muted: bool,
}

impl VolumeState {
    /// Create a new volume state from an audio config.
    pub fn from_config(config: &AudioConfig) -> Self {
        Self {
            volume: config.volume.min(MAX_VOLUME),
            muted: config.muted,
        }
    }

    /// Set the volume level.
    ///
    /// # Errors
    ///
    /// Returns `AudioError::VolumeOutOfRange` if `level` exceeds 100.
    pub fn set_volume(&mut self, level: u8) -> AudioResult<()> {
        if level > MAX_VOLUME {
            return Err(AudioError::VolumeOutOfRange(level));
        }
        self.volume = level;
        Ok(())
    }

    /// Toggle mute state. Returns the new mute state.
    pub fn toggle_mute(&mut self) -> bool {
        self.muted = !self.muted;
        self.muted
    }

    /// Get the effective volume (0 if muted).
    pub fn effective_volume(&self) -> u8 {
        if self.muted { 0 } else { self.volume }
    }

    /// Build the crosvm control socket command for setting volume.
    ///
    /// Returns the command string to send to the control socket.
    pub fn volume_command(&self) -> String {
        let effective = self.effective_volume();
        format!("volume {effective}")
    }

    /// Apply the current state back to an `AudioConfig` for persistence.
    pub fn apply_to_config(&self, config: &mut AudioConfig) {
        config.volume = self.volume;
        config.muted = self.muted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_config_defaults() {
        let config = AudioConfig::default();
        let state = VolumeState::from_config(&config);
        assert_eq!(state.volume, 80);
        assert!(!state.muted);
    }

    #[test]
    fn set_volume_valid() {
        let mut state = VolumeState::from_config(&AudioConfig::default());
        assert!(state.set_volume(50).is_ok());
        assert_eq!(state.volume, 50);
    }

    #[test]
    fn set_volume_max() {
        let mut state = VolumeState::from_config(&AudioConfig::default());
        assert!(state.set_volume(100).is_ok());
        assert_eq!(state.volume, 100);
    }

    #[test]
    fn set_volume_zero() {
        let mut state = VolumeState::from_config(&AudioConfig::default());
        assert!(state.set_volume(0).is_ok());
        assert_eq!(state.volume, 0);
    }

    #[test]
    fn set_volume_out_of_range() {
        let mut state = VolumeState::from_config(&AudioConfig::default());
        // u8 max is 255, but our limit is 100
        let result = state.set_volume(101);
        assert!(result.is_err());
    }

    #[test]
    fn toggle_mute() {
        let mut state = VolumeState::from_config(&AudioConfig::default());
        assert!(!state.muted);
        assert!(state.toggle_mute()); // now muted
        assert!(state.muted);
        assert!(!state.toggle_mute()); // now unmuted
        assert!(!state.muted);
    }

    #[test]
    fn effective_volume_when_muted() {
        let mut state = VolumeState::from_config(&AudioConfig::default());
        state.set_volume(75).unwrap();
        assert_eq!(state.effective_volume(), 75);
        state.toggle_mute();
        assert_eq!(state.effective_volume(), 0);
    }

    #[test]
    fn volume_command_format() {
        let state = VolumeState {
            volume: 60,
            muted: false,
        };
        assert_eq!(state.volume_command(), "volume 60");
    }

    #[test]
    fn volume_command_when_muted() {
        let state = VolumeState {
            volume: 60,
            muted: true,
        };
        assert_eq!(state.volume_command(), "volume 0");
    }

    #[test]
    fn apply_to_config_persists_state() {
        let mut config = AudioConfig::default();
        let mut state = VolumeState::from_config(&config);
        state.set_volume(42).unwrap();
        state.toggle_mute();
        state.apply_to_config(&mut config);
        assert_eq!(config.volume, 42);
        assert!(config.muted);
    }

    #[test]
    fn config_volume_clamped_to_max() {
        let config = AudioConfig {
            volume: 255, // u8 max, but should be clamped
            ..AudioConfig::default()
        };
        let state = VolumeState::from_config(&config);
        assert_eq!(state.volume, 100);
    }
}
