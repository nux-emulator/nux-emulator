//! VM launcher — manages the crosvm process lifecycle.
//!
//! Two-phase approach:
//! - First run: `launch_cvd` creates disk images (composite + overlay with GPT)
//! - Subsequent runs: boots crosvm directly from existing overlay (no rebuild)

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

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

impl std::fmt::Debug for VmLaunchConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmLaunchConfig")
            .field("aosp_root", &self.aosp_root)
            .field("home_dir", &self.home_dir)
            .finish()
    }
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

    /// Check if this is a first run (no instance exists yet).
    fn needs_bootstrap(&self) -> bool {
        !self.config.home_dir
            .join("cuttlefish/instances/cvd-1/overlay.img")
            .exists()
    }

    /// Start the VM. Uses launch_cvd on first run, direct crosvm on subsequent runs.
    pub fn start_kernel(&self, wayland_sock: &str) -> Result<(), String> {
        if self.is_running() {
            return Err("VM is already running".to_owned());
        }

        // Clean signal files
        let _ = std::fs::remove_file("/tmp/nux-x11-ready");
        let _ = std::fs::remove_file("/tmp/nux-x11-orientation");

        if self.needs_bootstrap() {
            log::info!("vm: first run — using launch_cvd to create disk images");
            self.start_with_launch_cvd(wayland_sock)
        } else {
            log::info!("vm: existing instance — booting crosvm directly (persistent data)");
            self.start_crosvm_direct(wayland_sock)
        }
    }

    /// First run: use launch_cvd to create all disk images.
    fn start_with_launch_cvd(&self, _wayland_sock: &str) -> Result<(), String> {
        // Kill any previous instances
        let _ = Command::new("sudo")
            .args(["pkill", "-9", "-f", "launch_cvd|run_cvd|crosvm|process_restarter|secure_env"])
            .output();
        std::thread::sleep(std::time::Duration::from_secs(2));

        let _ = Command::new("sudo")
            .args(["rm", "-rf", "/tmp/cf_avd_0", "/tmp/cf_env_0"])
            .output();

        std::fs::create_dir_all(&self.config.home_dir).ok();

        self.setup_networking().ok();

        let product_out = self.config.aosp_root.join("out/target/product/vsoc_x86_64");
        let host_out = self.config.aosp_root.join("out/host/linux-x86");
        let launch_cvd = host_out.join("bin/launch_cvd");

        let mut cmd = Command::new("sudo");
        cmd.arg("-E").arg(&launch_cvd).args([
            "--daemon=false",
            &format!("--gpu_mode={}", self.config.gpu_mode),
            &format!("--cpus={}", self.config.cpus),
            &format!("--memory_mb={}", self.config.memory_mb),
            "--report_anonymous_usage_stats=n",
            "--enable_sandbox=false",
            "--netsim=false",
            "--enable_gpu_udmabuf=true",
            "--blank_data_image_mb=65536",
        ]);

        cmd.env("DISPLAY", std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_owned()))
            .env("HOME", &self.config.home_dir)
            .env("ANDROID_PRODUCT_OUT", &product_out)
            .env("ANDROID_HOST_OUT", &host_out)
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        for (key, val) in Self::gpu_env() {
            cmd.env(&key, &val);
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start launch_cvd: {e}"))?;

        *self.process.lock().unwrap() = Some(child);
        Ok(())
    }

    /// Subsequent runs: boot crosvm directly from existing overlay.
    fn start_crosvm_direct(&self, wayland_sock: &str) -> Result<(), String> {
        // Kill any previous instances
        let _ = Command::new("sudo")
            .args(["pkill", "-9", "-f", "crosvm|process_restarter|secure_env"])
            .output();
        std::thread::sleep(std::time::Duration::from_secs(1));

        self.setup_networking().ok();

        let instance_dir = self.config.home_dir.join("cuttlefish/instances/cvd-1");
        let internal_dir = instance_dir.join("internal");
        let host_out = self.config.aosp_root.join("out/host/linux-x86");
        let crosvm_bin = host_out.join("bin/crosvm");

        // Verify overlay exists
        let overlay = instance_dir.join("overlay.img");
        if !overlay.exists() {
            return Err(format!("overlay.img not found: {}", overlay.display()));
        }

        // Create internal directory
        let _ = Command::new("sudo")
            .args(["mkdir", "-p", &internal_dir.to_string_lossy()])
            .output();
        let _ = Command::new("sudo")
            .args(["chmod", "777", &internal_dir.to_string_lossy()])
            .output();

        // Log files
        let kernel_log = internal_dir.join("kernel.log");
        let crosvm_log = internal_dir.join("crosvm.log");
        let crosvm_err = internal_dir.join("crosvm_err.log");
        for f in [&kernel_log, &crosvm_log, &crosvm_err] {
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

        // Control socket
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

        // GPU + Wayland display
        cmd.arg(format!("--wayland-sock={wayland_sock}"));
        cmd.arg(
            "--gpu=displays=[[mode=windowed[720,1280],dpi=[320,320],refresh-rate=60]],\
             context-types=gfxstream-gles:gfxstream-vulkan:gfxstream-composer,\
             pci-address=00:02.0,egl=true,surfaceless=true,glx=false,gles=true,\
             udmabuf=true,\
             renderer-features=\"GlProgramBinaryLinkStatus:enabled\"",
        );

        // Disk images — overlay has all partitions (system, userdata, etc.)
        cmd.arg(format!("--block=path={}", overlay.display()));
        // Persistent composite (misc data)
        let persistent = instance_dir.join("persistent_composite.img");
        if persistent.exists() {
            cmd.arg(format!("--block=path={}", persistent.display()));
        }
        // SD card
        let sdcard = instance_dir.join("sdcard.img");
        if sdcard.exists() {
            cmd.arg(format!("--block=path={}", sdcard.display()));
        }

        // BIOS (u-boot — reads GPT from overlay, loads kernel)
        cmd.arg(format!(
            "--bios={}",
            host_out.join("etc/bootloader_x86_64/bootloader.crosvm").display()
        ));

        // pflash + pmem
        let pflash = instance_dir.join("pflash.img");
        if pflash.exists() {
            cmd.arg(format!("--pflash={}", pflash.display()));
        }
        let hwcomposer_pmem = instance_dir.join("hwcomposer-pmem");
        if hwcomposer_pmem.exists() {
            cmd.arg(format!("--pmem=path={}", hwcomposer_pmem.display()));
        }
        let access_kregistry = instance_dir.join("access-kregistry");
        if access_kregistry.exists() {
            cmd.arg(format!("--pmem=path={}", access_kregistry.display()));
        }
        let pstore = instance_dir.join("pstore");
        if pstore.exists() {
            cmd.arg(format!("--pstore=path={},size=2097152", pstore.display()));
        }

        // Network
        cmd.arg("--net=tap-name=cvd-mtap-01,mac=00:1a:11:e0:cf:00,pci-address=00:01.1");

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
        // Sink remaining serial ports (HAL expects them)
        for i in 2..=10 {
            cmd.arg(format!("--serial=hardware=virtio-console,num={i},type=sink"));
        }

        // Environment
        for (key, val) in Self::gpu_env() {
            cmd.env(&key, &val);
        }
        cmd.env("DISPLAY", std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_owned()));

        // Redirect output
        let stdout_file = std::fs::OpenOptions::new()
            .write(true).truncate(true).open(&crosvm_log)
            .or_else(|_| std::fs::File::create(&crosvm_log))
            .map_err(|e| format!("crosvm.log: {e}"))?;
        let stderr_file = std::fs::OpenOptions::new()
            .write(true).truncate(true).open(&crosvm_err)
            .or_else(|_| std::fs::File::create(&crosvm_err))
            .map_err(|e| format!("crosvm_err.log: {e}"))?;
        cmd.stdout(stdout_file).stderr(stderr_file);

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start crosvm: {e}"))?;

        log::info!("vm: crosvm started directly (pid={})", child.id());
        *self.process.lock().unwrap() = Some(child);

        // Start adb_connector for vsock→TCP bridge
        let adb_connector = host_out.join("bin/adb_connector");
        let home = self.config.home_dir.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            log::info!("vm: starting adb_connector...");
            let _ = Command::new("sudo")
                .arg("-E")
                .arg(&adb_connector)
                .arg("--addresses=vsock:3:5555")
                .arg("--adb_port=6520")
                .env("HOME", &home)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
        });

        Ok(())
    }

    /// Stop the VM gracefully.
    pub fn stop(&self) -> Result<(), String> {
        // Sync guest filesystem
        let _ = self.adb_shell(&["sync"]);
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Kill crosvm + all infrastructure
        log::info!("vm: stopping...");
        let _ = Command::new("sudo")
            .args(["pkill", "-TERM", "-f", "crosvm"])
            .output();
        std::thread::sleep(std::time::Duration::from_secs(3));
        let _ = Command::new("sudo")
            .args(["pkill", "-9", "-f",
                "launch_cvd|run_cvd|crosvm|process_restarter|secure_env|log_tee|adb_connector"])
            .output();

        std::thread::sleep(std::time::Duration::from_secs(1));
        *self.process.lock().unwrap() = None;
        log::info!("vm: stopped");
        Ok(())
    }

    // ── Networking ──

    pub fn setup_networking(&self) -> Result<(), String> {
        // Create TAP devices
        let _ = Command::new("sudo").args(["ip", "tuntap", "add", "dev", "cvd-mtap-01", "mode", "tap"]).output();
        let _ = Command::new("sudo").args(["ip", "link", "set", "cvd-mtap-01", "up"]).output();
        let _ = Command::new("sudo").args(["ip", "addr", "add", "192.168.96.1/24", "dev", "cvd-mtap-01"]).output();

        // NAT
        let _ = Command::new("sudo").args(["sysctl", "-w", "net.ipv4.ip_forward=1"]).output();
        let _ = Command::new("sudo").args(["iptables", "-t", "nat", "-A", "POSTROUTING", "-s", "192.168.96.0/24", "-j", "MASQUERADE"]).output();
        let _ = Command::new("sudo").args(["iptables", "-A", "FORWARD", "-i", "cvd-mtap-01", "-j", "ACCEPT"]).output();
        let _ = Command::new("sudo").args(["iptables", "-A", "FORWARD", "-o", "cvd-mtap-01", "-m", "state", "--state", "RELATED,ESTABLISHED", "-j", "ACCEPT"]).output();

        Ok(())
    }

    // ── ADB helpers ──

    pub fn check_boot_status(&self) -> BootStatus {
        let _ = Command::new("adb").args(["connect", "127.0.0.1:6520"]).output();

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

    fn find_adb_device(&self) -> Option<String> {
        let output = Command::new("adb").args(["devices"]).output().ok()?;
        let devices = String::from_utf8_lossy(&output.stdout);
        for line in devices.lines() {
            if line.contains("127.0.0.1:6520") && line.contains("device") {
                return Some("127.0.0.1:6520".to_string());
            }
        }
        for line in devices.lines() {
            if line.ends_with("\tdevice") && !line.starts_with("List") {
                let serial = line.split('\t').next()?.to_string();
                // Skip physical devices (not our emulator)
                if !serial.contains("127.0.0.1") && !serial.contains("0.0.0.0") {
                    continue;
                }
                return Some(serial);
            }
        }
        None
    }

    pub fn adb_shell(&self, args: &[&str]) -> Result<std::process::Output, String> {
        let serial = self.find_adb_device()
            .ok_or_else(|| "No ADB device found".to_string())?;
        Command::new("adb")
            .args(["-s", &serial, "shell"])
            .args(args)
            .output()
            .map_err(|e| format!("adb shell failed: {e}"))
    }

    // ── WiFi + ARM translation ──

    pub fn enable_wifi(&self) -> Result<(), String> {
        let _ = self.adb_shell(&["cmd", "wifi", "set-wifi-enabled", "enabled"]);
        std::thread::sleep(std::time::Duration::from_secs(3));
        let _ = self.adb_shell(&["cmd", "wifi", "connect-network", "VirtWifi", "open"]);
        Ok(())
    }

    pub fn setup_arm_translation(&self) -> Result<(), String> {
        let serial = self.find_adb_device()
            .ok_or_else(|| "No ADB device".to_string())?;

        // Push Google ARM64 libs
        let arm_dir = self.config.aosp_root.join("vendor/nux/arm-translation/prebuilts");
        let lib_dir = arm_dir.join("lib64/arm64");
        let bin_dir = arm_dir.join("bin/arm64");

        if lib_dir.exists() {
            let _ = Command::new("adb")
                .args(["-s", &serial, "push"])
                .arg(&lib_dir)
                .arg("/system/lib64/arm64/")
                .output();
            log::info!("vm: pushed all Google ARM64 guest libs to /system/lib64/arm64/");
        }

        if bin_dir.exists() {
            for entry in std::fs::read_dir(&bin_dir).into_iter().flatten().flatten() {
                let _ = Command::new("adb")
                    .args(["-s", &serial, "push"])
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
        log::info!("vm: ARM64 native bridge initialized (SELinux permissive + zygote restart)");
        Ok(())
    }

    // ── APK install ──

    pub fn install_apk(&self, path: &std::path::Path) -> Result<String, String> {
        let serial = self.find_adb_device()
            .ok_or_else(|| "No ADB device".to_string())?;
        let output = Command::new("adb")
            .args(["-s", &serial, "install", "-r"])
            .arg(path)
            .output()
            .map_err(|e| format!("adb install: {e}"))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    // ── GPU env ──

    fn gpu_env() -> Vec<(String, String)> {
        vec![
            ("MESA_LOADER_DRIVER_OVERRIDE".into(), "zink".into()),
            ("GALLIUM_DRIVER".into(), "zink".into()),
            ("__GLX_VENDOR_LIBRARY_NAME".into(), "mesa".into()),
        ]
    }

    // ── Misc ──

    pub fn screenshot(&self, path: &std::path::Path) -> Result<(), String> {
        let serial = self.find_adb_device().ok_or("No ADB device")?;
        let _ = Command::new("adb")
            .args(["-s", &serial, "shell", "screencap", "-p", "/sdcard/screenshot.png"])
            .output();
        let output = Command::new("adb")
            .args(["-s", &serial, "pull", "/sdcard/screenshot.png"])
            .arg(path)
            .output()
            .map_err(|e| format!("screenshot: {e}"))?;
        if output.status.success() { Ok(()) } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    pub fn volume_up(&self) {
        let _ = self.adb_shell(&["input", "keyevent", "24"]);
    }

    pub fn volume_down(&self) {
        let _ = self.adb_shell(&["input", "keyevent", "25"]);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BootStatus {
    NotConnected,
    Booting,
    Booted,
}
