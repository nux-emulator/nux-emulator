//! VM launcher — manages the QEMU process lifecycle.
//!
//! Uses qemu-system-x86_64 with direct kernel boot, virtio-gpu-rutabaga,
//! and user-mode networking with ADB port forwarding.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, Mutex};

const ADB_SERIAL: &str = "127.0.0.1:5555";

pub struct VmLaunchConfig {
    pub aosp_root: PathBuf,
    pub gpu_mode: String,
    pub cpus: u32,
    pub memory_mb: u32,
}

impl Default for VmLaunchConfig {
    fn default() -> Self {
        Self {
            aosp_root: PathBuf::from("/build2/nux-emulator/nux-android-image/aosp"),
            gpu_mode: "gfxstream".to_owned(),
            cpus: 8,
            memory_mb: 8192,
        }
    }
}

impl std::fmt::Debug for VmLaunchConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmLaunchConfig")
            .field("aosp_root", &self.aosp_root)
            .field("gpu_mode", &self.gpu_mode)
            .field("cpus", &self.cpus)
            .field("memory_mb", &self.memory_mb)
            .finish()
    }
}

/// Manages the QEMU VM lifecycle.
#[derive(Debug)]
pub struct VmLauncher {
    pub config: VmLaunchConfig,
    process: Arc<Mutex<Option<Child>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BootStatus {
    NotConnected,
    Booting,
    Booted,
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
                Ok(Some(_)) => {
                    *guard = None;
                    false
                }
                Ok(None) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// Start the VM with QEMU direct kernel boot.
    pub fn start_kernel(&self) -> Result<(), String> {
        if self.is_running() {
            return Err("VM is already running".to_owned());
        }

        // Bootstrap if needed
        if !crate::vm_bootstrap::is_bootstrapped() {
            log::info!("vm: first run — bootstrapping disk images");
            crate::vm_bootstrap::bootstrap()?;
        }

        self.setup_networking().ok();

        let data_dir = crate::vm_bootstrap::data_dir();
        let sysimg = crate::vm_bootstrap::sysimg_dir();
        let data_dir = crate::vm_bootstrap::data_dir();
        let monitor_sock = data_dir.join("qemu-monitor.sock");

        // Remove stale monitor socket
        let _ = std::fs::remove_file(&monitor_sock);

        let kernel = sysimg.join("kernel-ranchu");
        let ramdisk = sysimg.join("ramdisk.img");
        let system_img = sysimg.join("system.img");
        let vendor_img = sysimg.join("vendor.img");
        let userdata_img = data_dir.join("userdata.img");
        let cache_img = data_dir.join("cache.img");

        let cmdline = "qemu=1 \
            androidboot.hardware=ranchu \
            androidboot.serialno=EMULATOR35X0X0X0 \
            console=ttyS0 \
            androidboot.console=ttyS0 \
            androidboot.verifiedbootstate=orange \
            qemu.gles=1 \
            clocksource=pit \
            8250.nr_uarts=1";

        let mut cmd = Command::new("qemu-system-x86_64");
        cmd.args(["-enable-kvm", "-cpu", "host"]);
        cmd.args(["-smp", &self.config.cpus.to_string()]);
        cmd.args(["-m", &self.config.memory_mb.to_string()]);
        cmd.args(["-machine", "q35"]);

        // Kernel + initrd
        cmd.args(["-kernel", &kernel.to_string_lossy()]);
        cmd.args(["-initrd", &ramdisk.to_string_lossy()]);
        cmd.args(["-append", cmdline]);

        // Drives — system and vendor are read-only, userdata is persistent
        cmd.args([
            "-drive", &format!("file={},format=raw,if=none,id=system,readonly=on", system_img.display()),
            "-device", "virtio-blk-pci,drive=system",
        ]);
        cmd.args([
            "-drive", &format!("file={},format=raw,if=none,id=vendor,readonly=on", vendor_img.display()),
            "-device", "virtio-blk-pci,drive=vendor",
        ]);
        cmd.args([
            "-drive", &format!("file={},format=raw,if=none,id=userdata", userdata_img.display()),
            "-device", "virtio-blk-pci,drive=userdata",
        ]);
        cmd.args([
            "-drive", &format!("file={},format=raw,if=none,id=cache", cache_img.display()),
            "-device", "virtio-blk-pci,drive=cache",
        ]);

        // GPU
        cmd.args(["-device", "virtio-gpu-rutabaga,gfxstream-vulkan=on,hostmem=2G"]);
        cmd.args(["-display", "gtk,gl=on"]);

        // Input
        cmd.args(["-device", "virtio-keyboard-pci"]);
        cmd.args(["-device", "virtio-mouse-pci"]);

        // Serial/console (ttyS0 for ranchu kernel)
        cmd.args(["-serial", "file:/tmp/nux-kernel.log"]);

        // Network (user-mode with ADB port forward)
        cmd.args([
            "-netdev", "user,id=net0,hostfwd=tcp::5555-:5555",
            "-device", "virtio-net-pci,netdev=net0",
        ]);

        // Monitor socket for clean shutdown
        cmd.args([
            "-monitor", &format!("unix:{},server,nowait", monitor_sock.display()),
        ]);

        // Environment
        cmd.env("DISPLAY", std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_owned()));
        for (key, val) in Self::gpu_env() {
            cmd.env(&key, &val);
        }

        cmd.stdout(Stdio::null()).stderr(Stdio::null());

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start qemu-system-x86_64: {e}"))?;

        log::info!("vm: QEMU started (pid={})", child.id());
        *self.process.lock().unwrap() = Some(child);
        Ok(())
    }

    /// Stop the VM gracefully via QEMU monitor socket.
    pub fn stop(&self) -> Result<(), String> {
        // Sync guest filesystem
        let _ = Command::new("adb")
            .args(["-s", ADB_SERIAL, "shell", "sync"])
            .output();
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Send quit to QEMU monitor
        let monitor_sock = crate::vm_bootstrap::data_dir().join("qemu-monitor.sock");
        let _ = Command::new("sh")
            .args(["-c", &format!(
                "echo quit | socat - UNIX-CONNECT:{}",
                monitor_sock.display()
            )])
            .output();

        // Wait up to 5s for QEMU to exit
        let start = std::time::Instant::now();
        loop {
            if !self.is_running() {
                break;
            }
            if start.elapsed() > std::time::Duration::from_secs(5) {
                // Force kill
                let mut guard = self.process.lock().unwrap();
                if let Some(child) = guard.as_mut() {
                    let _ = child.kill();
                }
                *guard = None;
                log::warn!("vm: QEMU force-killed after 5s timeout");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        log::info!("vm: stopped");
        Ok(())
    }

    // ── Boot status ──

    pub fn check_boot_status(&self) -> BootStatus {
        let _ = Command::new("adb")
            .args(["connect", ADB_SERIAL])
            .output();

        let output = Command::new("adb")
            .args(["-s", ADB_SERIAL, "shell", "getprop", "sys.boot_completed"])
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                if stdout == "1" {
                    BootStatus::Booted
                } else if out.status.success() {
                    BootStatus::Booting
                } else {
                    BootStatus::NotConnected
                }
            }
            Err(_) => BootStatus::NotConnected,
        }
    }

    // ── ADB helpers ──

    pub fn adb_shell(&self, args: &[&str]) -> Result<Output, String> {
        Command::new("adb")
            .args(["-s", ADB_SERIAL, "shell"])
            .args(args)
            .output()
            .map_err(|e| format!("adb shell failed: {e}"))
    }

    pub fn install_apk(&self, path: &Path) -> Result<String, String> {
        let output = Command::new("adb")
            .args(["-s", ADB_SERIAL, "install", "-r"])
            .arg(path)
            .output()
            .map_err(|e| format!("adb install: {e}"))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    // ── WiFi + ARM translation ──

    pub fn enable_wifi(&self) -> Result<(), String> {
        let _ = self.adb_shell(&["cmd", "wifi", "set-wifi-enabled", "enabled"]);
        std::thread::sleep(std::time::Duration::from_secs(3));
        let _ = self.adb_shell(&["cmd", "wifi", "connect-network", "VirtWifi", "open"]);
        Ok(())
    }

    pub fn setup_arm_translation(&self) -> Result<(), String> {
        let arm_dir = self.config.aosp_root.join("vendor/nux/arm-translation/prebuilts");
        let lib_dir = arm_dir.join("lib64/arm64");
        let bin_dir = arm_dir.join("bin/arm64");

        if lib_dir.exists() {
            let _ = Command::new("adb")
                .args(["-s", ADB_SERIAL, "push"])
                .arg(&lib_dir)
                .arg("/system/lib64/arm64/")
                .output();
            log::info!("vm: pushed ARM64 guest libs");
        }

        if bin_dir.exists() {
            for entry in std::fs::read_dir(&bin_dir).into_iter().flatten().flatten() {
                let _ = Command::new("adb")
                    .args(["-s", ADB_SERIAL, "push"])
                    .arg(entry.path())
                    .arg(format!("/system/bin/arm64/{}", entry.file_name().to_string_lossy()))
                    .output();
            }
            log::info!("vm: pushed ARM64 binaries");
        }

        // SELinux permissive + restart zygote
        log::info!("vm: restarting zygote for native bridge initialization...");
        let _ = self.adb_shell(&["setenforce", "0"]);
        let _ = self.adb_shell(&["setprop", "ctl.restart", "zygote"]);
        std::thread::sleep(std::time::Duration::from_secs(30));
        log::info!("vm: ARM64 native bridge initialized");
        Ok(())
    }

    // ── Networking ──

    pub fn setup_networking(&self) -> Result<(), String> {
        // TAP device for host-side networking (optional, QEMU user-mode works without it)
        let _ = Command::new("sudo")
            .args(["ip", "tuntap", "add", "dev", "nux-tap0", "mode", "tap"])
            .output();
        let _ = Command::new("sudo")
            .args(["ip", "link", "set", "nux-tap0", "up"])
            .output();

        // Enable IP forwarding
        let _ = Command::new("sudo")
            .args(["sysctl", "-w", "net.ipv4.ip_forward=1"])
            .output();

        Ok(())
    }

    // ── Misc ──

    pub fn screenshot(&self, path: &Path) -> Result<(), String> {
        let _ = self.adb_shell(&["screencap", "-p", "/sdcard/screenshot.png"]);
        let output = Command::new("adb")
            .args(["-s", ADB_SERIAL, "pull", "/sdcard/screenshot.png"])
            .arg(path)
            .output()
            .map_err(|e| format!("screenshot: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    pub fn volume_up(&self) {
        let _ = self.adb_shell(&["input", "keyevent", "24"]);
    }

    pub fn volume_down(&self) {
        let _ = self.adb_shell(&["input", "keyevent", "25"]);
    }

    // ── GPU env ──

    fn gpu_env() -> Vec<(String, String)> {
        vec![
            ("MESA_LOADER_DRIVER_OVERRIDE".into(), "zink".into()),
            ("GALLIUM_DRIVER".into(), "zink".into()),
            ("__GLX_VENDOR_LIBRARY_NAME".into(), "mesa".into()),
        ]
    }
}
