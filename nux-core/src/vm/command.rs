//! crosvm command builder.

use super::config::VmConfig;
use std::ffi::OsString;
use std::fmt::Write;

/// Build the crosvm CLI arguments from a `VmConfig`.
pub fn build_command(config: &VmConfig) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        config.crosvm_binary.clone().into(),
        "run".into(),
        "--cpus".into(),
        config.cpus.to_string().into(),
        "--mem".into(),
        config.ram_mb.to_string().into(),
    ];

    // GPU
    if config.gpu.enabled {
        let mut gpu_arg = String::from("backend=gfxstream");
        if let Some(w) = config.gpu.width {
            let _ = write!(gpu_arg, ",width={w}");
        }
        if let Some(h) = config.gpu.height {
            let _ = write!(gpu_arg, ",height={h}");
        }
        args.push("--gpu".into());
        args.push(gpu_arg.into());
    }

    // Block devices
    for disk in &config.disks {
        let mut block_arg = format!("path={}", disk.path.display());
        if disk.readonly {
            block_arg.push_str(",ro");
        }
        args.push("--block".into());
        args.push(block_arg.into());
    }

    // Audio
    if config.audio_enabled {
        args.push("--sound".into());
    }

    // Network
    if let Some(tap) = &config.network_tap {
        args.push("--net".into());
        args.push(format!("tap-name={tap}").into());
    }

    // Input devices
    for dev in &config.input_devices {
        args.push("--input-ev".into());
        args.push(dev.into());
    }

    // Control socket
    let socket_path = config.effective_socket_path();
    args.push("--socket".into());
    args.push(socket_path.into());

    // Boot image and kernel (must be last)
    args.push("--boot".into());
    args.push(config.boot_image.clone().into());
    args.push(config.kernel.clone().into());

    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::config::{DiskConfig, GpuVmConfig};
    use std::path::PathBuf;

    fn base_config() -> VmConfig {
        VmConfig {
            cpus: 4,
            ram_mb: 4096,
            gpu: GpuVmConfig {
                enabled: false,
                width: None,
                height: None,
            },
            disks: vec![],
            kernel: PathBuf::from("/images/kernel"),
            boot_image: PathBuf::from("/images/boot.img"),
            audio_enabled: false,
            network_tap: None,
            input_devices: vec![],
            control_socket_path: Some(PathBuf::from("/tmp/test.sock")),
            crosvm_binary: PathBuf::from("crosvm"),
        }
    }

    fn args_to_strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn minimal_config_produces_base_args() {
        let config = base_config();
        let args = build_command(&config);
        let s = args_to_strings(&args);

        assert_eq!(s[0], "crosvm");
        assert_eq!(s[1], "run");
        assert_eq!(s[2], "--cpus");
        assert_eq!(s[3], "4");
        assert_eq!(s[4], "--mem");
        assert_eq!(s[5], "4096");
        let len = s.len();
        assert_eq!(s[len - 3], "--boot");
        assert_eq!(s[len - 2], "/images/boot.img");
        assert_eq!(s[len - 1], "/images/kernel");
    }

    #[test]
    fn gpu_enabled_with_resolution() {
        let mut config = base_config();
        config.gpu = GpuVmConfig {
            enabled: true,
            width: Some(1920),
            height: Some(1080),
        };
        let args = build_command(&config);
        let s = args_to_strings(&args);
        let gpu_idx = s.iter().position(|a| a == "--gpu").unwrap();
        assert_eq!(s[gpu_idx + 1], "backend=gfxstream,width=1920,height=1080");
    }

    #[test]
    fn gpu_enabled_default_resolution() {
        let mut config = base_config();
        config.gpu = GpuVmConfig {
            enabled: true,
            width: None,
            height: None,
        };
        let args = build_command(&config);
        let s = args_to_strings(&args);
        let gpu_idx = s.iter().position(|a| a == "--gpu").unwrap();
        assert_eq!(s[gpu_idx + 1], "backend=gfxstream");
    }

    #[test]
    fn gpu_disabled_omits_flag() {
        let config = base_config();
        let args = build_command(&config);
        let s = args_to_strings(&args);
        assert!(!s.contains(&"--gpu".to_owned()));
    }

    #[test]
    fn block_devices_readonly_and_readwrite() {
        let mut config = base_config();
        config.disks = vec![
            DiskConfig {
                path: PathBuf::from("/images/system.img"),
                readonly: true,
            },
            DiskConfig {
                path: PathBuf::from("/images/userdata.img"),
                readonly: false,
            },
        ];
        let args = build_command(&config);
        let s = args_to_strings(&args);

        let block_indices: Vec<usize> = s
            .iter()
            .enumerate()
            .filter(|(_, a)| *a == "--block")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(block_indices.len(), 2);
        assert_eq!(s[block_indices[0] + 1], "path=/images/system.img,ro");
        assert_eq!(s[block_indices[1] + 1], "path=/images/userdata.img");
    }

    #[test]
    fn audio_and_network_flags() {
        let mut config = base_config();
        config.audio_enabled = true;
        config.network_tap = Some("nux0".to_owned());
        let args = build_command(&config);
        let s = args_to_strings(&args);
        assert!(s.contains(&"--sound".to_owned()));
        let net_idx = s.iter().position(|a| a == "--net").unwrap();
        assert_eq!(s[net_idx + 1], "tap-name=nux0");
    }

    #[test]
    fn input_devices() {
        let mut config = base_config();
        config.input_devices = vec!["keyboard".to_owned(), "mouse".to_owned()];
        let args = build_command(&config);
        let s = args_to_strings(&args);
        let input_indices: Vec<usize> = s
            .iter()
            .enumerate()
            .filter(|(_, a)| *a == "--input-ev")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(input_indices.len(), 2);
        assert_eq!(s[input_indices[0] + 1], "keyboard");
        assert_eq!(s[input_indices[1] + 1], "mouse");
    }

    #[test]
    fn custom_socket_path() {
        let config = base_config();
        let args = build_command(&config);
        let s = args_to_strings(&args);
        let sock_idx = s.iter().position(|a| a == "--socket").unwrap();
        assert_eq!(s[sock_idx + 1], "/tmp/test.sock");
    }

    #[test]
    fn default_socket_path_uses_uid() {
        let mut config = base_config();
        config.control_socket_path = None;
        let args = build_command(&config);
        let s = args_to_strings(&args);
        let sock_idx = s.iter().position(|a| a == "--socket").unwrap();
        let uid = nix::unistd::getuid();
        assert!(s[sock_idx + 1].contains(&uid.to_string()));
    }

    #[test]
    fn full_config_all_flags() {
        let mut config = base_config();
        config.gpu = GpuVmConfig {
            enabled: true,
            width: Some(1080),
            height: Some(1920),
        };
        config.disks = vec![
            DiskConfig {
                path: PathBuf::from("/images/system.img"),
                readonly: true,
            },
            DiskConfig {
                path: PathBuf::from("/images/userdata.img"),
                readonly: false,
            },
        ];
        config.audio_enabled = true;
        config.network_tap = Some("nux0".to_owned());
        config.input_devices = vec!["keyboard".to_owned(), "mouse".to_owned()];

        let args = build_command(&config);
        let s = args_to_strings(&args);

        assert!(s.contains(&"--gpu".to_owned()));
        assert!(s.contains(&"--sound".to_owned()));
        assert!(s.contains(&"--net".to_owned()));
        assert!(s.contains(&"--socket".to_owned()));
        assert_eq!(s.iter().filter(|a| *a == "--block").count(), 2);
        assert_eq!(s.iter().filter(|a| *a == "--input-ev").count(), 2);
    }
}
