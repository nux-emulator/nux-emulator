//! Audio configuration types.

use serde::{Deserialize, Serialize};

/// Audio configuration for a Nux VM instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Whether audio output is enabled.
    pub enabled: bool,
    /// Volume level (0–100).
    pub volume: u8,
    /// Whether audio is muted.
    pub muted: bool,
    /// ALSA period size in frames for latency tuning.
    pub period_size: u32,
    /// Number of ALSA periods for latency tuning.
    pub period_count: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 80,
            muted: false,
            period_size: 256,
            period_count: 2,
        }
    }
}

/// Maximum acceptable audio round-trip latency in milliseconds.
pub const LATENCY_WARNING_THRESHOLD_MS: u64 = 80;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = AudioConfig::default();
        assert!(config.enabled);
        assert_eq!(config.volume, 80);
        assert!(!config.muted);
        assert_eq!(config.period_size, 256);
        assert_eq!(config.period_count, 2);
    }

    #[test]
    fn deserialize_with_defaults() {
        let toml_str = "";
        let config: AudioConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config, AudioConfig::default());
    }

    #[test]
    fn deserialize_partial_override() {
        let toml_str = "enabled = false\nvolume = 50";
        let config: AudioConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.enabled);
        assert_eq!(config.volume, 50);
        assert!(!config.muted); // default
    }

    #[test]
    fn deserialize_full() {
        let toml_str = r#"
enabled = true
volume = 100
muted = true
period_size = 512
period_count = 4
"#;
        let config: AudioConfig = toml::from_str(toml_str).unwrap();
        assert!(config.enabled);
        assert_eq!(config.volume, 100);
        assert!(config.muted);
        assert_eq!(config.period_size, 512);
        assert_eq!(config.period_count, 4);
    }

    #[test]
    fn serialize_roundtrip() {
        let config = AudioConfig {
            enabled: false,
            volume: 42,
            muted: true,
            period_size: 128,
            period_count: 3,
        };
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: AudioConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, deserialized);
    }
}
