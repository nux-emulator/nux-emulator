//! VM launcher — manages the crosvm process lifecycle.
//!
//! Supports two modes:
//! - `start()`: Uses `launch_cvd` (full Cuttlefish stack, scrcpy display)
//! - `start_direct()`: Runs crosvm directly (native Wayland display, 60fps)
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
    pub config: VmLaunchConfig,
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
    /// `pre_launch` runs after cleanup but before launch_cvd starts —
    /// use this to bind the Wayland compositor socket.
    pub fn start_with_hook<F: FnOnce()>(&self, pre_launch: F) -> Result<(), String> {
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

        // Run pre-launch hook (bind Wayland compositor here)
        pre_launch();

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
                "--blank_data_image_mb=16384",
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

    /// Start the VM via `launch_cvd` (no pre-launch hook).
    pub fn start(&self) -> Result<(), String> {
        self.start_with_hook(|| {})
    }

    /// Start the VM by running crosvm directly (bypasses launch_cvd).
    /// Uses our Wayland compositor for native display at 60fps.
    /// Requires disk images from a previous `launch_cvd` run.
    pub fn start_direct(&self, wayland_sock: &str) -> Result<(), String> {
        if self.is_running() {
            return Err("VM is already running".to_owned());
        }

        // Kill any previous crosvm instances
        let _ = Command::new("sudo")
            .args(["pkill", "-9", "-f", "crosvm|process_restarter|secure_env"])
            .output();
        std::thread::sleep(std::time::Duration::from_secs(1));

        self.setup_networking().ok();

        let instance_dir = self.config.home_dir.join("cuttlefish/instances/cvd-1");
        let internal_dir = instance_dir.join("internal");
        let host_out = self.config.aosp_root.join("out/host/linux-x86");
        let crosvm_bin = host_out.join("bin/crosvm");

        // Verify disk images exist
        let overlay = instance_dir.join("overlay.img");
        if !overlay.exists() {
            return Err(format!(
                "Disk images not found at {}. Run launch_cvd once first to create them.",
                instance_dir.display()
            ));
        }

        // Create internal directory and log files (needs sudo since dir is root-owned)
        let _ = Command::new("sudo")
            .args(["mkdir", "-p", &internal_dir.to_string_lossy()])
            .output();

        // Serial ports — essential ones use files, HAL ports use sink
        let kernel_log = internal_dir.join("kernel.log");
        let logcat_log = internal_dir.join("logcat.log");
        let crosvm_log = internal_dir.join("crosvm.log");
        let crosvm_err = internal_dir.join("crosvm_err.log");

        // Create log files as root
        for f in [&kernel_log, &logcat_log, &crosvm_log, &crosvm_err] {
            let _ = Command::new("sudo")
                .args(["touch", &f.to_string_lossy()])
                .output();
            let _ = Command::new("sudo")
                .args(["chmod", "666", &f.to_string_lossy()])
                .output();
        }

        // Build crosvm command
        let mut cmd = Command::new("sudo");
        cmd.arg("-E").arg(&crosvm_bin);
        cmd.args(["--extended-status", "run"]);

        // Control socket — remove stale one
        let control_sock = internal_dir.join("crosvm_control.sock");
        let _ = Command::new("sudo")
            .args(["rm", "-f", &control_sock.to_string_lossy()])
            .output();
        cmd.arg(format!("--socket={}", control_sock.display()));

        // Core settings
        cmd.args(["--no-smt", "--no-usb", "--core-scheduling=false"]);
        cmd.arg(format!("--mem={}", self.config.memory_mb));
        cmd.arg(format!("--cpus={}", self.config.cpus));
        cmd.arg("--disable-sandbox");

        // GPU + Wayland display (our compositor)
        cmd.arg(format!("--wayland-sock={wayland_sock}"));
        cmd.arg(
            "--gpu=displays=[[mode=windowed[720,1280],dpi=[320,320],refresh-rate=60]],\
             context-types=gfxstream-gles:gfxstream-vulkan:gfxstream-composer,\
             pci-address=00:02.0,egl=true,surfaceless=true,glx=false,gles=true,\
             renderer-features=\"GlProgramBinaryLinkStatus:enabled\"",
        );

        // Disk images
        cmd.arg(format!(
            "--block=path={}",
            instance_dir.join("overlay.img").display()
        ));
        cmd.arg(format!(
            "--block=path={}",
            instance_dir.join("persistent_composite.img").display()
        ));
        cmd.arg(format!(
            "--block=path={}",
            instance_dir.join("sdcard.img").display()
        ));

        // BIOS
        cmd.arg(format!(
            "--bios={}",
            host_out
                .join("etc/bootloader_x86_64/bootloader.crosvm")
                .display()
        ));

        // pflash + pmem
        cmd.arg(format!(
            "--pflash={}",
            instance_dir.join("pflash.img").display()
        ));
        cmd.arg(format!(
            "--pmem=path={}",
            instance_dir.join("hwcomposer-pmem").display()
        ));
        cmd.arg(format!(
            "--pmem=path={}",
            instance_dir.join("access-kregistry").display()
        ));
        cmd.arg(format!(
            "--pstore=path={},size=2097152",
            instance_dir.join("pstore").display()
        ));

        // Network
        cmd.arg("--net=tap-name=cvd-mtap-01,mac=00:1a:11:e0:cf:00,pci-address=00:01.1");
        cmd.arg("--net=tap-name=cvd-etap-01,mac=00:1a:11:e1:cf:00,pci-address=00:01.2");

        // vsock
        cmd.arg("--vsock=cid=3");

        // Serial ports
        cmd.arg(format!(
            "--serial=hardware=virtio-console,num=1,type=file,path={},console=true",
            kernel_log.display()
        ));
        cmd.arg(format!(
            "--serial=hardware=serial,num=1,type=file,path={},earlycon=true",
            kernel_log.display()
        ));
        cmd.arg("--serial=hardware=virtio-console,num=2,type=sink");
        cmd.arg(format!(
            "--serial=hardware=virtio-console,num=3,type=file,path={}",
            logcat_log.display()
        ));

        // HAL serial ports — all sinks (no HAL services running)
        for num in 4..=17 {
            cmd.arg(format!(
                "--serial=hardware=virtio-console,num={num},type=sink"
            ));
        }

        // GPU environment
        for (key, val) in Self::gpu_env() {
            cmd.env(&key, &val);
        }
        cmd.env(
            "DISPLAY",
            std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_owned()),
        );

        // Redirect output to log files
        let stdout_file = std::fs::File::create(&crosvm_log)
            .or_else(|_| {
                std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&crosvm_log)
            })
            .map_err(|e| format!("create crosvm.log: {e}"))?;
        let stderr_file = std::fs::File::create(&crosvm_err)
            .or_else(|_| {
                std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&crosvm_err)
            })
            .map_err(|e| format!("create crosvm_err.log: {e}"))?;
        cmd.stdout(stdout_file).stderr(stderr_file);

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start crosvm: {e}"))?;

        log::info!("vm: crosvm started directly (pid={})", child.id());

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

    /// Set up ARM64 native bridge (binfmt_misc) after boot.
    /// SELinux blocks the init.rc trigger, so we:
    /// 1. Set SELinux permissive
    /// 2. Set blocked properties
    /// 3. Mount binfmt_misc and register entries
    /// 4. Restart zygote so native bridge reinitializes
    pub fn setup_arm_translation(&self) -> Result<(), String> {
        let adb = |args: &[&str]| -> Result<std::process::Output, String> {
            Command::new("adb")
                .args(["-s", "127.0.0.1:6520"])
                .args(args)
                .output()
                .map_err(|e| format!("adb: {e}"))
        };

        let prebuilts = self
            .config
            .aosp_root
            .join("vendor/nux/arm-translation/prebuilts");

        // Enable root and remount to push missing ARM64 bionic libs
        let _ = adb(&["root"]);
        std::thread::sleep(std::time::Duration::from_secs(2));
        let _ = adb(&["remount"]);
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Push Google's ARM64 libc.so (build system filter excludes it from image)
        let arm64_lib = prebuilts.join("lib64/arm64/libc.so");
        if arm64_lib.exists() {
            let _ = Command::new("adb")
                .args(["-s", "127.0.0.1:6520", "push"])
                .arg(&arm64_lib)
                .arg("/system/lib64/arm64/libc.so")
                .output();
            log::info!("vm: pushed ARM64 libc.so to /system/lib64/arm64/");
        }

        // Delete the 6GB scratch image that adb remount creates
        let _ = adb(&[
            "shell",
            "su",
            "0",
            "rm",
            "-f",
            "/data/gsi/remount/scratch.img.0000",
        ]);

        // Set SELinux permissive
        let _ = adb(&["shell", "su", "0", "setenforce", "0"]);

        // Mount binfmt_misc and register ARM translation entries
        let _ = adb(&[
            "shell",
            "su",
            "0",
            "mount",
            "-t",
            "binfmt_misc",
            "binfmt_misc",
            "/proc/sys/fs/binfmt_misc",
        ]);
        for entry in &["arm64_exe", "arm64_dyn", "arm_exe", "arm_dyn"] {
            let src = format!("/system/etc/binfmt_misc/{entry}");
            let _ = adb(&[
                "shell",
                "su",
                "0",
                "cp",
                &src,
                "/proc/sys/fs/binfmt_misc/register",
            ]);
        }

        // Restart zygote so native bridge reinitializes
        log::info!("vm: restarting zygote for native bridge initialization...");
        let _ = adb(&["shell", "su", "0", "setprop", "ctl.restart", "zygote"]);

        // Wait for framework to come back
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if let Ok(out) = adb(&["shell", "getprop", "sys.boot_completed"]) {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                if s == "1" {
                    break;
                }
            }
        }

        log::info!("vm: ARM64 native bridge initialized (SELinux permissive + zygote restart)");
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
