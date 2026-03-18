//! VM configuration types for crosvm.

use super::error::{VmError, VmResult};
use serde::Deserialize;
use std::path::PathBuf;

/// GPU configuration for the VM.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GpuVmConfig {
    pub enabled: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl Default for GpuVmConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            width: None,
            height: None,
        }
    }
}

/// A disk image to attach to the VM.
#[derive(Debug, Clone, Deserialize)]
pub struct DiskConfig {
    pub path: PathBuf,
    #[serde(default)]
    pub readonly: bool,
}

/// Complete VM configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct VmConfig {
    pub cpus: u32,
    pub ram_mb: u32,
    pub gpu: GpuVmConfig,
    pub disks: Vec<DiskConfig>,
    pub kernel: PathBuf,
    pub boot_image: PathBuf,
    pub audio_enabled: bool,
    pub network_tap: Option<String>,
    pub input_devices: Vec<String>,
    pub control_socket_path: Option<PathBuf>,
    pub crosvm_binary: PathBuf,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            cpus: 2,
            ram_mb: 2048,
            gpu: GpuVmConfig::default(),
            disks: Vec::new(),
            kernel: PathBuf::new(),
            boot_image: PathBuf::new(),
            audio_enabled: true,
            network_tap: None,
            input_devices: Vec::new(),
            control_socket_path: None,
            crosvm_binary: PathBuf::from("crosvm"),
        }
    }
}

impl VmConfig {
    /// Validate the configuration, returning errors for invalid fields.
    ///
    /// # Errors
    ///
    /// Returns `VmError::ConfigValidation` if any field is invalid.
    pub fn validate(&self) -> VmResult<()> {
        let mut errors = Vec::new();

        if self.cpus < 1 {
            errors.push("cpu count must be at least 1".to_owned());
        }
        if self.ram_mb < 512 {
            errors.push("RAM must be at least 512 MB".to_owned());
        }
        if self.kernel.as_os_str().is_empty() {
            errors.push("kernel path is required".to_owned());
        } else if !self.kernel.exists() {
            errors.push(format!("kernel not found: {}", self.kernel.display()));
        }
        if self.boot_image.as_os_str().is_empty() {
            errors.push("boot image path is required".to_owned());
        } else if !self.boot_image.exists() {
            errors.push(format!(
                "boot image not found: {}",
                self.boot_image.display()
            ));
        }
        for disk in &self.disks {
            if !disk.path.exists() {
                errors.push(format!("disk image not found: {}", disk.path.display()));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(VmError::ConfigValidation(errors.join("; ")))
        }
    }

    /// Get the control socket path, using a default if not configured.
    pub fn effective_socket_path(&self) -> PathBuf {
        self.control_socket_path.clone().unwrap_or_else(|| {
            let uid = nix::unistd::getuid();
            PathBuf::from(format!("/run/user/{uid}/nux/control.sock"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config(dir: &std::path::Path) -> VmConfig {
        let kernel = dir.join("kernel");
        let boot = dir.join("boot.img");
        let system = dir.join("system.img");
        std::fs::write(&kernel, "").unwrap();
        std::fs::write(&boot, "").unwrap();
        std::fs::write(&system, "").unwrap();
        VmConfig {
            cpus: 4,
            ram_mb: 4096,
            kernel,
            boot_image: boot,
            disks: vec![DiskConfig {
                path: system,
                readonly: true,
            }],
            ..VmConfig::default()
        }
    }

    #[test]
    fn valid_config_passes() {
        let dir = tempfile::tempdir().unwrap();
        let config = valid_config(dir.path());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn zero_cpus_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = valid_config(dir.path());
        config.cpus = 0;
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("cpu count"));
    }

    #[test]
    fn low_ram_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = valid_config(dir.path());
        config.ram_mb = 256;
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("RAM"));
    }

    #[test]
    fn missing_kernel_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = valid_config(dir.path());
        config.kernel = PathBuf::from("/nonexistent/kernel");
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("kernel"));
    }

    #[test]
    fn missing_boot_image_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = valid_config(dir.path());
        config.boot_image = PathBuf::from("/nonexistent/boot.img");
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("boot image"));
    }

    #[test]
    fn missing_disk_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = valid_config(dir.path());
        config.disks.push(DiskConfig {
            path: PathBuf::from("/nonexistent/data.img"),
            readonly: false,
        });
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("disk image"));
    }

    #[test]
    fn default_socket_path_contains_uid() {
        let config = VmConfig::default();
        let path = config.effective_socket_path();
        let uid = nix::unistd::getuid();
        assert!(path.to_str().unwrap().contains(&uid.to_string()));
        assert!(path.to_str().unwrap().contains("nux/control.sock"));
    }

    #[test]
    fn custom_socket_path_used() {
        let config = VmConfig {
            control_socket_path: Some(PathBuf::from("/tmp/my.sock")),
            ..VmConfig::default()
        };
        assert_eq!(
            config.effective_socket_path(),
            PathBuf::from("/tmp/my.sock")
        );
    }
}
