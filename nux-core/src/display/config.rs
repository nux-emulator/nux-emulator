//! Display configuration types for the display pipeline.

use super::error::{DisplayError, DisplayResult};
use serde::Deserialize;

/// Scaling mode for frame presentation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScalingMode {
    /// Scale to fit within the widget, preserving aspect ratio (letterbox/pillarbox).
    #[default]
    Contain,
    /// Scale to the largest integer multiple of native resolution that fits.
    Integer,
}

/// Display pipeline configuration, read from the `[display]` TOML section.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct DisplayPipelineConfig {
    /// Guest display width in pixels.
    pub width: u32,
    /// Guest display height in pixels.
    pub height: u32,
    /// Scaling mode for frame presentation.
    pub scaling_mode: ScalingMode,
    /// Whether to synchronize frame presentation with the compositor `VSync`.
    pub vsync: bool,
    /// Whether to show the FPS counter overlay.
    pub fps_overlay: bool,
}

impl Default for DisplayPipelineConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            scaling_mode: ScalingMode::Contain,
            vsync: true,
            fps_overlay: false,
        }
    }
}

impl DisplayPipelineConfig {
    /// Validate the configuration, returning an error for invalid fields.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::ConfigValidation` if any field is invalid.
    pub fn validate(&self) -> DisplayResult<()> {
        let mut errors = Vec::new();

        if self.width == 0 {
            errors.push("width must be greater than 0".to_owned());
        }
        if self.height == 0 {
            errors.push("height must be greater than 0".to_owned());
        }
        if self.width > 7680 {
            errors.push("width must not exceed 7680".to_owned());
        }
        if self.height > 4320 {
            errors.push("height must not exceed 4320".to_owned());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(DisplayError::ConfigValidation(errors.join("; ")))
        }
    }

    /// Compute the aspect ratio as a floating-point value (width / height).
    pub fn aspect_ratio(&self) -> f64 {
        f64::from(self.width) / f64::from(self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = DisplayPipelineConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
        assert_eq!(config.scaling_mode, ScalingMode::Contain);
        assert!(config.vsync);
        assert!(!config.fps_overlay);
    }

    #[test]
    fn zero_width_rejected() {
        let config = DisplayPipelineConfig {
            width: 0,
            ..Default::default()
        };
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("width"));
    }

    #[test]
    fn zero_height_rejected() {
        let config = DisplayPipelineConfig {
            height: 0,
            ..Default::default()
        };
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("height"));
    }

    #[test]
    fn excessive_resolution_rejected() {
        let config = DisplayPipelineConfig {
            width: 10000,
            height: 5000,
            ..Default::default()
        };
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("width"));
        assert!(err.contains("height"));
    }

    #[test]
    fn aspect_ratio_calculation() {
        let config = DisplayPipelineConfig::default();
        let ratio = config.aspect_ratio();
        let expected = 1920.0 / 1080.0;
        assert!((ratio - expected).abs() < f64::EPSILON);
    }

    #[test]
    fn deserialize_from_toml() {
        let toml_str = r#"
            width = 2560
            height = 1440
            scaling_mode = "integer"
            vsync = false
            fps_overlay = true
        "#;
        let config: DisplayPipelineConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.width, 2560);
        assert_eq!(config.height, 1440);
        assert_eq!(config.scaling_mode, ScalingMode::Integer);
        assert!(!config.vsync);
        assert!(config.fps_overlay);
    }

    #[test]
    fn deserialize_defaults_for_missing_fields() {
        let toml_str = "";
        let config: DisplayPipelineConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config, DisplayPipelineConfig::default());
    }
}
