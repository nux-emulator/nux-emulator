//! VM launcher — manages the `launch_cvd` process lifecycle.
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

/// Configuration for launching the VM.
#[derive(Debug, Clone)]
pub struct VmLaunchConfig {
    pub aosp_root: PathBuf,
    pub home_dir: PathBuf,
    pub gpu_mode: String,
    pub cpus: u32,
    pub memory_mb: u32,
}

impl Default for VmLaunchConfig {
    fn default() -> Self {
        Self {
            aosp_root: PathBuf::from("/build2/nux-emulator/nux-android-image/aosp"),
            home_dir: PathBuf::from("/tmp/nux-cf"),
            gpu_mode: "gfxstream".to_owned(),
            cpus: 4,
            memory_mb: 4096,
        }
    }
}

/// Manages the crosvm VM lifecycle via `launch_cvd`.
#[derive(Debug)]
pub struct VmLauncher {
    config: VmLaunchConfig,
    process: Arc<Mutex<Option<Child>>>,
}

impl VmLauncher {
    pub fn new(config: VmLaunchConfig) -> Self {
        Self {
            config,
            process: Arc::new(Mutex::new(None)),
        }
    }

    /// Check if the VM is currently running.
    pub fn is_running(&self) -> bool {
        let mut guard = self.process.lock().unwrap();
        if let Some(child) = guard.as_mut() {
            match child.try_wait() {
                Ok(None) => true, // Still running
                Ok(Some(_)) => {
                    *guard = None; // Exited
                    false
                }
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// Get the process handle for async monitoring.
    pub fn process_handle(&self) -> Arc<Mutex<Option<Child>>> {
        self.process.clone()
    }

    /// Configuration for launching the VM.
    pub fn setup_networking(&self) -> Result<(), String> {
        // Create TAP devices
        for tap in &["cvd-mtap-01", "cvd-etap-01", "cvd-wtap-01"] {
            let _ = Command::new("sudo")
                .args([
                    "ip",
                    "tuntap",
                    "add",
                    "dev",
                    tap,
                    "mode",
                    "tap",
                    "user",
                    &whoami(),
                ])
                .output();
            let _ = Command::new("sudo")
                .args(["ip", "link", "set", tap, "up"])
                .output();
        }

        // Set gateway IP on OpenWrt WAN TAP
        let _ = Command::new("sudo")
            .args(["ip", "addr", "add", "192.168.96.1/24", "dev", "cvd-wtap-01"])
            .output();

        // Enable IP forwarding
        let _ = Command::new("sudo")
            .args(["sysctl", "-qw", "net.ipv4.ip_forward=1"])
            .output();

        // Get main interface
        let main_if = get_main_interface().unwrap_or_else(|| "eth0".to_owned());

        // NAT rules
        for subnet in &["192.168.96.0/24", "192.168.99.0/24"] {
            let _ = Command::new("sudo")
                .args([
                    "iptables",
                    "-t",
                    "nat",
                    "-C",
                    "POSTROUTING",
                    "-s",
                    subnet,
                    "-o",
                    &main_if,
                    "-j",
                    "MASQUERADE",
                ])
                .output()
                .and_then(|o| {
                    if !o.status.success() {
                        Command::new("sudo")
                            .args([
                                "iptables",
                                "-t",
                                "nat",
                                "-A",
                                "POSTROUTING",
                                "-s",
                                subnet,
                                "-o",
                                &main_if,
                                "-j",
                                "MASQUERADE",
                            ])
                            .output()
                    } else {
                        Ok(o)
                    }
                });
        }

        // iptables FORWARD rules — INSERT at top (before Docker DROP policy)
        let _ = Command::new("sudo")
            .args([
                "iptables",
                "-I",
                "FORWARD",
                "1",
                "-i",
                "cvd-wtap-01",
                "-o",
                &main_if,
                "-j",
                "ACCEPT",
            ])
            .output();
        let _ = Command::new("sudo")
            .args([
                "iptables",
                "-I",
                "FORWARD",
                "2",
                "-i",
                &main_if,
                "-o",
                "cvd-wtap-01",
                "-m",
                "state",
                "--state",
                "RELATED,ESTABLISHED",
                "-j",
                "ACCEPT",
            ])
            .output();

        // nftables forwarding (Arch Linux uses nftables by default)
        let _ = Command::new("sudo")
            .args([
                "nft", "add", "rule", "inet", "filter", "forward", "iifname", "cvd-*", "accept",
            ])
            .output();
        let _ = Command::new("sudo")
            .args([
                "nft",
                "add",
                "rule",
                "inet",
                "filter",
                "forward",
                "oifname",
                "cvd-*",
                "ct",
                "state",
                "established,related",
                "accept",
            ])
            .output();

        Ok(())
    }

    /// Set GPU environment variables based on detected GPU.
    fn gpu_env() -> Vec<(String, String)> {
        let mut env = Vec::new();

        // Check for NVIDIA
        if let Ok(output) = Command::new("lspci").output() {
            let lspci = String::from_utf8_lossy(&output.stdout).to_lowercase();
            if lspci.contains("nvidia") {
                env.push((
                    "__EGL_VENDOR_LIBRARY_FILENAMES".to_owned(),
                    "/usr/share/glvnd/egl_vendor.d/50_mesa.json".to_owned(),
                ));
                env.push(("MESA_LOADER_DRIVER_OVERRIDE".to_owned(), "zink".to_owned()));
            }
        }

        env
    }

    /// Start the VM via `launch_cvd`.
    pub fn start(&self) -> Result<(), String> {
        if self.is_running() {
            return Err("VM is already running".to_owned());
        }

        // Clean previous instance
        let _ = Command::new("sudo")
            .args([
                "pkill",
                "-9",
                "-f",
                "launch_cvd|run_cvd|crosvm|process_restarter|secure_env",
            ])
            .output();
        std::thread::sleep(std::time::Duration::from_secs(2));

        let _ = Command::new("sudo")
            .args([
                "rm",
                "-rf",
                &format!("{}/cuttlefish", self.config.home_dir.display()),
                "/tmp/cf_avd_0",
                "/tmp/cf_env_0",
            ])
            .output();
        std::fs::create_dir_all(&self.config.home_dir).ok();

        // Setup networking
        self.setup_networking().ok();

        let product_out = self.config.aosp_root.join("out/target/product/vsoc_x86_64");
        let host_out = self.config.aosp_root.join("out/host/linux-x86");
        let launch_cvd = host_out.join("bin/launch_cvd");

        let mut cmd = Command::new("sudo");
        cmd.arg("-E")
            .arg(&launch_cvd)
            .args([
                "--daemon=false",
                &format!("--gpu_mode={}", self.config.gpu_mode),
                &format!("--cpus={}", self.config.cpus),
                &format!("--memory_mb={}", self.config.memory_mb),
                "--report_anonymous_usage_stats=n",
                "--enable_sandbox=false",
                "--netsim=false",
            ])
            .env(
                "DISPLAY",
                std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_owned()),
            )
            .env("HOME", &self.config.home_dir)
            .env("ANDROID_PRODUCT_OUT", &product_out)
            .env("ANDROID_HOST_OUT", &host_out)
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        // Add GPU-specific env vars
        for (key, val) in Self::gpu_env() {
            cmd.env(&key, &val);
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start launch_cvd: {e}"))?;

        *self.process.lock().unwrap() = Some(child);
        Ok(())
    }

    /// Stop the VM.
    pub fn stop(&self) -> Result<(), String> {
        // Kill all crosvm-related processes
        let _ = Command::new("sudo")
            .args(["pkill", "-9", "-f",
                "launch_cvd|run_cvd|crosvm|process_restarter|secure_env|log_tee|netsimd|wmediumd|webrtc|webRTC|casimir|modem_sim"])
            .output();

        *self.process.lock().unwrap() = None;
        Ok(())
    }

    /// Connect `WiFi` after boot.
    pub fn enable_wifi(&self) -> Result<(), String> {
        let _ = Command::new("adb")
            .args([
                "-s",
                "127.0.0.1:6520",
                "shell",
                "cmd",
                "wifi",
                "set-wifi-enabled",
                "enabled",
            ])
            .output();
        std::thread::sleep(std::time::Duration::from_secs(3));
        let _ = Command::new("adb")
            .args([
                "-s",
                "127.0.0.1:6520",
                "shell",
                "cmd",
                "wifi",
                "connect-network",
                "VirtWifi",
                "open",
            ])
            .output();
        Ok(())
    }

    /// Check if ADB is connected and boot is complete.
    pub fn check_boot_status(&self) -> BootStatus {
        let connect = Command::new("adb")
            .args(["connect", "127.0.0.1:6520"])
            .output();

        if connect.is_err() {
            return BootStatus::NotConnected;
        }

        let output = Command::new("adb")
            .args([
                "-s",
                "127.0.0.1:6520",
                "shell",
                "getprop",
                "sys.boot_completed",
            ])
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                if stdout == "1" {
                    BootStatus::Booted
                } else {
                    BootStatus::Booting
                }
            }
            Err(_) => BootStatus::NotConnected,
        }
    }

    /// Install an APK via ADB.
    pub fn install_apk(&self, path: &std::path::Path) -> Result<String, String> {
        let output = Command::new("adb")
            .args(["-s", "127.0.0.1:6520", "install", "-r"])
            .arg(path)
            .output()
            .map_err(|e| format!("ADB install failed: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if stdout.contains("Success") {
            Ok("APK installed successfully".to_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(format!("Install failed: {stdout} {stderr}"))
        }
    }

    /// Take a screenshot via ADB.
    pub fn screenshot(&self, save_path: &std::path::Path) -> Result<(), String> {
        let _ = Command::new("adb")
            .args([
                "-s",
                "127.0.0.1:6520",
                "shell",
                "screencap",
                "-p",
                "/sdcard/screenshot.png",
            ])
            .output()
            .map_err(|e| format!("Screenshot failed: {e}"))?;

        let _ = Command::new("adb")
            .args(["-s", "127.0.0.1:6520", "pull", "/sdcard/screenshot.png"])
            .arg(save_path)
            .output()
            .map_err(|e| format!("Pull screenshot failed: {e}"))?;

        Ok(())
    }

    /// Send volume key via ADB.
    pub fn volume_up(&self) {
        let _ = Command::new("adb")
            .args(["-s", "127.0.0.1:6520", "shell", "input", "keyevent", "24"])
            .output();
    }

    pub fn volume_down(&self) {
        let _ = Command::new("adb")
            .args(["-s", "127.0.0.1:6520", "shell", "input", "keyevent", "25"])
            .output();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootStatus {
    NotConnected,
    Booting,
    Booted,
}

fn whoami() -> String {
    Command::new("whoami").output().map_or_else(
        |_| "user".to_owned(),
        |o| String::from_utf8_lossy(&o.stdout).trim().to_owned(),
    )
}

fn get_main_interface() -> Option<String> {
    let output = Command::new("ip")
        .args(["route", "show", "default"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.split_whitespace().nth(4).map(str::to_owned)
}
