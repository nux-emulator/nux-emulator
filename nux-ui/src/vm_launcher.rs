//! VM launcher — manages the crosvm process lifecycle.
//!
//! Launches crosvm with direct kernel boot (no Cuttlefish launch_cvd).
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
            home_dir: crate::vm_bootstrap::data_dir(),
            gpu_mode: "gfxstream".to_owned(),
            cpus: 8,
            memory_mb: 8192,
        }
    }
}

/// Manages the crosvm VM lifecycle.
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

    /// Start the VM with direct kernel boot.
    ///
    /// `wayland_sock` is the path to the Wayland compositor socket (created before calling this).
    pub fn start_kernel(&self, wayland_sock: &str) -> Result<(), String> {
        if self.is_running() {
            return Err("VM is already running".to_owned());
        }

        // Bootstrap disk images if needed
        if !crate::vm_bootstrap::is_bootstrapped() {
            log::info!("vm: first run — bootstrapping disk images...");
            crate::vm_bootstrap::bootstrap()?;
        }

        // Kill any previous crosvm instances
        let _ = Command::new("sudo")
            .args(["pkill", "-9", "-f", "crosvm"])
            .output();
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Clean signal files
        let _ = std::fs::remove_file("/tmp/nux-x11-ready");
        let _ = std::fs::remove_file("/tmp/nux-x11-orientation");

        // Setup networking
        self.setup_networking().ok();

        let product_out = crate::vm_bootstrap::product_out();
        let data_dir = crate::vm_bootstrap::data_dir();
        let host_out = crate::vm_bootstrap::host_out();
        let crosvm_bin = host_out.join("bin/crosvm");

        // Verify kernel exists
        let kernel = product_out.join("kernel");
        if !kernel.exists() {
            return Err(format!("Kernel not found: {}", kernel.display()));
        }

        // Create log files
        let crosvm_log = data_dir.join("crosvm.log");
        let crosvm_err = data_dir.join("crosvm_err.log");
        let kernel_log = data_dir.join("kernel.log");
        for f in [&crosvm_log, &crosvm_err, &kernel_log] {
            let _ = std::fs::File::create(f);
        }

        // Remove stale control socket
        let control_sock = data_dir.join("crosvm_control.sock");
        let _ = std::fs::remove_file(&control_sock);

        // Build crosvm command
        let mut cmd = Command::new("sudo");
        cmd.arg("-E").arg(&crosvm_bin);
        cmd.arg("run");

        // Core settings
        cmd.args(["--no-smt", "--no-usb", "--core-scheduling=false"]);
        cmd.arg(format!("--mem={}", self.config.memory_mb));
        cmd.arg(format!("--cpus={}", self.config.cpus));
        cmd.arg("--disable-sandbox");

        // Kernel initrd and params
        cmd.arg("--initrd").arg(data_dir.join("combined_ramdisk.img"));

        // Kernel command line
        cmd.arg("--params").arg(
            "androidboot.hardware=cutf_cvm \
             androidboot.fstab_suffix=cf.f2fs.hctr2 \
             androidboot.boot_devices=4010000000.pci \
             androidboot.verifiedbootstate=orange \
             androidboot.slot_suffix=_a \
             console=hvc0 panic=-1 noefi loglevel=4 \
             printk.devkmsg=on \
             firmware_class.path=/vendor/etc/ \
             init=/init",
        );

        // Block devices
        cmd.arg(format!("--block=path={},ro", product_out.join("super.img").display()));
        cmd.arg(format!("--block=path={}", data_dir.join("userdata.img").display()));
        cmd.arg(format!("--block=path={}", data_dir.join("metadata.img").display()));
        cmd.arg(format!("--block=path={}", data_dir.join("misc.img").display()));
        cmd.arg(format!("--block=path={}", data_dir.join("sdcard.img").display()));

        // GPU + Wayland display
        cmd.arg(
            "--gpu=displays=[[mode=windowed[720,1280],dpi=[320,320],refresh-rate=60]],\
             context-types=gfxstream-gles:gfxstream-vulkan:gfxstream-composer,\
             egl=true,surfaceless=true,glx=false,gles=true,\
             udmabuf=true,\
             renderer-features=\"GlProgramBinaryLinkStatus:enabled\"",
        );
        cmd.arg(format!("--wayland-sock={wayland_sock}"));

        // Network
        cmd.arg("--net=tap-name=cvd-mtap-01,mac=00:1a:11:e0:cf:00");

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

        // Control socket
        cmd.arg(format!("--socket={}", control_sock.display()));

        // Environment variables
        for (key, val) in Self::gpu_env() {
            cmd.env(&key, &val);
        }
        cmd.env(
            "DISPLAY",
            std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_owned()),
        );

        // Kernel binary (positional argument — must be last)
        cmd.arg(&kernel);

        // Redirect output to log files
        let stdout_file = std::fs::File::create(&crosvm_log)
            .map_err(|e| format!("create crosvm.log: {e}"))?;
        let stderr_file = std::fs::File::create(&crosvm_err)
            .map_err(|e| format!("create crosvm_err.log: {e}"))?;
        cmd.stdout(stdout_file).stderr(stderr_file);

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start crosvm: {e}"))?;

        log::info!("vm: crosvm started (pid={})", child.id());
        *self.process.lock().unwrap() = Some(child);

        // Start adb_connector to bridge vsock to TCP port 6520
        let adb_connector = host_out.join("bin/adb_connector");
        let home_dir2 = self.config.home_dir.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            log::info!("vm: starting adb_connector (vsock:3:5555 → TCP 6520)...");
            let child = Command::new("sudo")
                .arg("-E")
                .arg(&adb_connector)
                .arg("--addresses=vsock:3:5555")
                .arg("--adb_port=6520")
                .env("HOME", &home_dir2)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            match child {
                Ok(_) => log::info!("vm: adb_connector started"),
                Err(e) => log::error!("vm: adb_connector failed: {e}"),
            }
        });

        Ok(())
    }

    /// Stop the VM gracefully, then force-kill if needed.
    pub fn stop(&self) -> Result<(), String> {
        let _ = self.adb_shell(&["sync"]);
        std::thread::sleep(std::time::Duration::from_secs(2));
        log::info!("vm: stopping crosvm...");
        let _ = Command::new("sudo")
            .args(["pkill", "-TERM", "-f", "crosvm"])
            .output();
        std::thread::sleep(std::time::Duration::from_secs(3));
        let _ = Command::new("sudo")
            .args(["pkill", "-9", "-f", "crosvm"])
            .output();
        *self.process.lock().unwrap() = None;
        log::info!("vm: stopped");
        Ok(())
    }

    /// Set up TAP networking for the VM.
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

    /// Connect WiFi after boot.
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

        // Push ALL Google's ARM64 guest libs
        let arm64_dir = prebuilts.join("lib64/arm64");
        if arm64_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&arm64_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.ends_with(".so") {
                        let _ = Command::new("adb")
                            .args(["-s", "127.0.0.1:6520", "push"])
                            .arg(entry.path())
                            .arg(format!("/system/lib64/arm64/{name_str}"))
                            .output();
                    }
                }
            }
            log::info!("vm: pushed all Google ARM64 guest libs to /system/lib64/arm64/");
        }

        // Push ARM64 binaries
        let arm64_bin = prebuilts.join("bin/arm64");
        if arm64_bin.exists() {
            let _ = Command::new("adb")
                .args(["-s", "127.0.0.1:6520", "push"])
                .arg(arm64_bin.join("app_process64"))
                .arg("/system/bin/arm64/app_process64")
                .output();
            let _ = Command::new("adb")
                .args(["-s", "127.0.0.1:6520", "push"])
                .arg(arm64_bin.join("linker64"))
                .arg("/system/bin/arm64/linker64")
                .output();
            log::info!("vm: pushed ARM64 binaries");
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

        // Regenerate linkerconfig with native bridge paths
        let _ = adb(&["shell", "su", "0", "rm", "-rf", "/linkerconfig"]);
        let _ = adb(&[
            "shell",
            "su",
            "0",
            "/apex/com.android.runtime/bin/linkerconfig",
            "--target",
            "/linkerconfig",
        ]);

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
        // Try TCP first
        let _ = Command::new("adb")
            .args(["connect", "127.0.0.1:6520"])
            .output();

        let serial = self.find_adb_device();
        let serial = match serial {
            Some(s) => s,
            None => return BootStatus::NotConnected,
        };

        let output = Command::new("adb")
            .args(["-s", &serial, "shell", "getprop", "sys.boot_completed"])
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

    /// Run an ADB shell command, auto-detecting the device serial.
    fn adb_shell(&self, args: &[&str]) -> Result<std::process::Output, String> {
        let serial = self
            .find_adb_device()
            .ok_or_else(|| "No ADB device found".to_string())?;
        Command::new("adb")
            .args(["-s", &serial, "shell"])
            .args(args)
            .output()
            .map_err(|e| format!("adb shell failed: {e}"))
    }

    /// Run an ADB command (non-shell), auto-detecting the device serial.
    fn adb_cmd(&self, args: &[&str]) -> Result<std::process::Output, String> {
        let serial = self
            .find_adb_device()
            .ok_or_else(|| "No ADB device found".to_string())?;
        Command::new("adb")
            .args(["-s", &serial])
            .args(args)
            .output()
            .map_err(|e| format!("adb failed: {e}"))
    }

    /// Find a working ADB device serial (TCP or USB/vsock).
    fn find_adb_device(&self) -> Option<String> {
        let output = Command::new("adb").args(["devices"]).output().ok()?;
        let devices = String::from_utf8_lossy(&output.stdout);
        // Prefer 127.0.0.1:6520 if available
        for line in devices.lines() {
            if line.contains("127.0.0.1:6520") && line.contains("device") {
                return Some("127.0.0.1:6520".to_string());
            }
        }
        // Fall back to any connected device (vsock/USB)
        for line in devices.lines() {
            if line.ends_with("\tdevice") && !line.starts_with("List") {
                return Some(line.split('\t').next()?.to_string());
            }
        }
        None
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
