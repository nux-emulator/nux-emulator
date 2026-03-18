//! Config validation.

use super::NuxConfig;

/// A config validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// CPU cores must be >= 1.
    CpuCoresTooLow(u32),
    /// RAM must be >= 512 MB.
    RamTooLow(u32),
    /// Display width must be >= 1.
    DisplayWidthTooLow(u32),
    /// Display height must be >= 1.
    DisplayHeightTooLow(u32),
    /// DPI must be >= 1.
    DpiTooLow(u32),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CpuCoresTooLow(v) => {
                write!(f, "cpu_cores must be >= 1, got {v}")
            }
            Self::RamTooLow(v) => {
                write!(f, "ram_mb must be >= 512, got {v}")
            }
            Self::DisplayWidthTooLow(v) => {
                write!(f, "display.width must be >= 1, got {v}")
            }
            Self::DisplayHeightTooLow(v) => {
                write!(f, "display.height must be >= 1, got {v}")
            }
            Self::DpiTooLow(v) => {
                write!(f, "display.dpi must be >= 1, got {v}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// Validate a resolved config, returning all errors found.
pub fn validate(config: &NuxConfig) -> Vec<ConfigError> {
    let mut errors = Vec::new();

    if config.hardware.cpu_cores < 1 {
        errors.push(ConfigError::CpuCoresTooLow(config.hardware.cpu_cores));
    }
    if config.hardware.ram_mb < 512 {
        errors.push(ConfigError::RamTooLow(config.hardware.ram_mb));
    }
    if config.display.width < 1 {
        errors.push(ConfigError::DisplayWidthTooLow(config.display.width));
    }
    if config.display.height < 1 {
        errors.push(ConfigError::DisplayHeightTooLow(config.display.height));
    }
    if config.display.dpi < 1 {
        errors.push(ConfigError::DpiTooLow(config.display.dpi));
    }

    errors
}
