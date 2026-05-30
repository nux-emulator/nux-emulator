# Direct Kernel Boot VM Launcher — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Cuttlefish's launch_cvd with a direct crosvm launcher that boots the kernel directly, uses raw block devices, and provides reliable persistent storage.

**Architecture:** crosvm boots a bzImage kernel directly with an initrd ramdisk. System partitions come from AOSP's super.img (read-only). Userdata is a raw f2fs file that persists across reboots. No QCOW2 overlays, no composite disks, no assembly step.

**Tech Stack:** Rust (nux-ui), crosvm, AOSP build outputs, f2fs-tools

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `nux-ui/src/vm_launcher.rs` | Rewrite | New `start_kernel()` replacing `start()` + `start_direct()` |
| `nux-ui/src/vm_bootstrap.rs` | Create | First-run image preparation (extract kernel, create userdata) |
| `nux-ui/src/window.rs` | Modify | Use new launcher, simplify boot flow |

---

### Task 1: Create Bootstrap Module

**Files:**
- Create: `nux-ui/src/vm_bootstrap.rs`
- Modify: `nux-ui/src/main.rs`

- [ ] **Step 1: Create vm_bootstrap.rs with image preparation logic**

```rust
//! VM bootstrap — prepares disk images for direct kernel boot.
//!
//! First run: creates persistent userdata.img, metadata.img, misc.img.
//! Subsequent runs: verifies images exist.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Persistent VM data directory.
pub fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/share/nux-emulator")
}

/// AOSP product output directory.
pub fn product_out() -> PathBuf {
    PathBuf::from("/build2/nux-emulator/nux-android-image/aosp/out/target/product/vsoc_x86_64")
}

/// AOSP host tools directory.
pub fn host_out() -> PathBuf {
    PathBuf::from("/build2/nux-emulator/nux-android-image/aosp/out/host/linux-x86")
}

/// Check if bootstrap has been completed (persistent images exist).
pub fn is_bootstrapped() -> bool {
    let dir = data_dir();
    dir.join("userdata.img").exists()
}

/// Run first-time bootstrap: create persistent disk images.
pub fn bootstrap() -> Result<(), String> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;

    log::info!("bootstrap: creating persistent disk images...");

    // Create 65GB sparse userdata.img (f2fs)
    let userdata = dir.join("userdata.img");
    if !userdata.exists() {
        log::info!("bootstrap: creating 65GB userdata.img (f2fs)...");
        // Create sparse file
        let status = Command::new("truncate")
            .args(["-s", "65G", &userdata.to_string_lossy()])
            .status()
            .map_err(|e| format!("truncate: {e}"))?;
        if !status.success() {
            return Err("truncate userdata.img failed".into());
        }
        // Format as f2fs (no checkpoint)
        let status = Command::new("mkfs.f2fs")
            .args(["-f", "-l", "data", "-O", "encrypt,quota,verity",
                   &userdata.to_string_lossy()])
            .status()
            .map_err(|e| format!("mkfs.f2fs: {e}"))?;
        if !status.success() {
            return Err("mkfs.f2fs userdata.img failed".into());
        }
    }

    // Create 64MB metadata.img (ext4)
    let metadata = dir.join("metadata.img");
    if !metadata.exists() {
        log::info!("bootstrap: creating metadata.img...");
        let status = Command::new("truncate")
            .args(["-s", "64M", &metadata.to_string_lossy()])
            .status()
            .map_err(|e| format!("truncate: {e}"))?;
        if !status.success() {
            return Err("truncate metadata.img failed".into());
        }
        let status = Command::new("mkfs.ext4")
            .args(["-F", &metadata.to_string_lossy()])
            .status()
            .map_err(|e| format!("mkfs.ext4: {e}"))?;
        if !status.success() {
            return Err("mkfs.ext4 metadata.img failed".into());
        }
    }

    // Create 1MB misc.img (zeroed)
    let misc = dir.join("misc.img");
    if !misc.exists() {
        log::info!("bootstrap: creating misc.img...");
        let status = Command::new("truncate")
            .args(["-s", "1M", &misc.to_string_lossy()])
            .status()
            .map_err(|e| format!("truncate: {e}"))?;
        if !status.success() {
            return Err("truncate misc.img failed".into());
        }
    }

    // Create 2GB sdcard.img (vfat) — optional shared storage
    let sdcard = dir.join("sdcard.img");
    if !sdcard.exists() {
        log::info!("bootstrap: creating sdcard.img...");
        let status = Command::new("truncate")
            .args(["-s", "2G", &sdcard.to_string_lossy()])
            .status()
            .map_err(|e| format!("truncate: {e}"))?;
        if !status.success() {
            return Err("truncate sdcard.img failed".into());
        }
        let _ = Command::new("mkfs.vfat")
            .args([&sdcard.to_string_lossy().to_string()])
            .status();
    }

    log::info!("bootstrap: complete");
    Ok(())
}
```

- [ ] **Step 2: Add module to main.rs**

Add `mod vm_bootstrap;` to `nux-ui/src/main.rs`.

- [ ] **Step 3: Verify it compiles**

Run: `cd /home/kk/dev/emulator && cargo check -p nux-ui`

- [ ] **Step 4: Commit**

```bash
git add nux-ui/src/vm_bootstrap.rs nux-ui/src/main.rs
git commit -m "Add vm_bootstrap module for direct kernel boot image creation"
```

---

### Task 2: Rewrite VM Launcher for Direct Kernel Boot

**Files:**
- Modify: `nux-ui/src/vm_launcher.rs`

- [ ] **Step 1: Replace `start()` with `start_kernel()`**

Replace the `start()` and `start_direct()` methods with a single `start_kernel()` that:
- Calls bootstrap if needed
- Sets up networking
- Launches crosvm with `--kernel` + `--initrd` + raw block devices
- No launch_cvd, no composites, no overlays

Key crosvm arguments:
```
crosvm run \
  --no-smt --no-usb --core-scheduling=false \
  --mem=8192 --cpus=8 --disable-sandbox \
  --kernel <product_out>/kernel \
  --initrd <product_out>/ramdisk.img \
  --params "androidboot.hardware=cutf_cvm androidboot.fstab_suffix=cf.f2fs.hctr2 androidboot.boot_devices=4010000000.pci androidboot.verifiedbootstate=orange androidboot.slot_suffix=_a console=hvc0 panic=-1 noefi loglevel=4 printk.devkmsg=on firmware_class.path=/vendor/etc/ init=/init" \
  --block path=<product_out>/super.img,ro \
  --block path=<data_dir>/userdata.img \
  --block path=<data_dir>/metadata.img \
  --block path=<data_dir>/misc.img \
  --block path=<data_dir>/sdcard.img \
  --gpu=displays=[[mode=windowed[720,1280],dpi=[320,320],refresh-rate=60]],context-types=gfxstream-gles:gfxstream-vulkan:gfxstream-composer,egl=true,surfaceless=true,glx=false,gles=true,udmabuf=true \
  --wayland-sock=<wayland_sock> \
  --net=tap-name=cvd-mtap-01,mac=00:1a:11:e0:cf:00 \
  --vsock=cid=3 \
  --serial=hardware=virtio-console,num=1,type=file,path=<data_dir>/kernel.log,console=true \
  --serial=hardware=serial,num=1,type=file,path=<data_dir>/kernel.log,earlycon=true \
  --socket=<data_dir>/crosvm_control.sock
```

- [ ] **Step 2: Simplify stop() — just kill crosvm**

With raw block devices (no QCOW2), killing crosvm is safe after `sync`:
```rust
pub fn stop(&self) -> Result<(), String> {
    let _ = self.adb_shell(&["sync"]);
    std::thread::sleep(std::time::Duration::from_secs(2));
    log::info!("vm: stopping crosvm...");
    let _ = Command::new("sudo")
        .args(["pkill", "-TERM", "-f", "crosvm"])
        .output();
    std::thread::sleep(std::time::Duration::from_secs(3));
    // Force kill if still alive
    let _ = Command::new("sudo")
        .args(["pkill", "-9", "-f", "crosvm"])
        .output();
    *self.process.lock().unwrap() = None;
    log::info!("vm: stopped");
    Ok(())
}
```

- [ ] **Step 3: Remove all Cuttlefish-specific code**

Remove: `start()`, `start_direct()`, all `launch_cvd`/`stop_cvd`/`assemble_cvd` references, `--resume`, `--data_policy`, overlay/composite logic, `disk_builder.cpp` patches.

- [ ] **Step 4: Verify it compiles**

Run: `cd /home/kk/dev/emulator && cargo check -p nux-ui`

- [ ] **Step 5: Commit**

```bash
git add nux-ui/src/vm_launcher.rs
git commit -m "Rewrite VM launcher for direct kernel boot (no Cuttlefish)"
```

---

### Task 3: Update Window Boot Flow

**Files:**
- Modify: `nux-ui/src/window.rs`

- [ ] **Step 1: Simplify the VM start thread**

Replace the complex launch_cvd + crosvm detection + Wayland socket swap with:
1. Start our Wayland compositor FIRST (we control the socket path)
2. Call `launcher.start_kernel(wayland_sock_path)`
3. Wait for ADB connection
4. Done

The key simplification: we pass our Wayland socket directly to crosvm (no swap needed).

- [ ] **Step 2: Remove Wayland socket swap logic**

No more waiting for crosvm to appear, no chmod, no socket replacement. crosvm connects to our compositor directly on startup.

- [ ] **Step 3: Verify it compiles**

Run: `cd /home/kk/dev/emulator && cargo check -p nux-ui`

- [ ] **Step 4: Commit**

```bash
git add nux-ui/src/window.rs
git commit -m "Simplify boot flow for direct kernel boot"
```

---

### Task 4: ADB Connectivity

**Files:**
- Modify: `nux-ui/src/vm_launcher.rs`

- [ ] **Step 1: Start adb_connector after crosvm launches**

```rust
// After crosvm starts, launch adb_connector to bridge vsock→TCP
let adb_connector = host_out().join("bin/adb_connector");
std::thread::spawn(move || {
    std::thread::sleep(std::time::Duration::from_secs(5));
    let _ = Command::new(&adb_connector)
        .args(["--addresses=vsock:3:5555", "--adb_port=6520"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
});
```

- [ ] **Step 2: Update check_boot_status to use 127.0.0.1:6520**

The adb_connector bridges vsock to TCP, so `adb -s 127.0.0.1:6520` works.

- [ ] **Step 3: Test ADB connectivity**

Run the emulator, verify `adb -s 127.0.0.1:6520 shell echo ok` works after boot.

- [ ] **Step 4: Commit**

```bash
git add nux-ui/src/vm_launcher.rs
git commit -m "Add ADB connectivity via adb_connector for direct boot"
```

---

### Task 5: Integration Test — Full Boot Cycle

- [ ] **Step 1: Clean start**

```bash
sudo pkill -9 -f crosvm
rm -rf ~/.local/share/nux-emulator
xhost +local:
cd /home/kk/dev/emulator && RUST_LOG=info cargo run --release -p nux-ui
```

Verify: VM boots, X11 window appears, input works.

- [ ] **Step 2: Install an app**

Install any APK via the toolbar.

- [ ] **Step 3: Shutdown**

Close the control panel window. Verify clean exit.

- [ ] **Step 4: Second boot — verify persistence**

```bash
cd /home/kk/dev/emulator && RUST_LOG=info cargo run --release -p nux-ui
```

Verify: VM boots, previously installed app is still there.

- [ ] **Step 5: Third boot — verify reliability**

Close and relaunch again. App must still be there.

- [ ] **Step 6: Commit final state**

```bash
git add -A
git commit -m "Direct kernel boot: verified persistent storage across reboots"
```

---

### Task 6: Cleanup Dead Code

**Files:**
- Remove: All Cuttlefish-specific patches and references

- [ ] **Step 1: Remove disk_builder.cpp patches**

Revert changes to `/build2/.../disk_builder.cpp` (no longer needed).

- [ ] **Step 2: Remove sed.f2fs patch**

Revert `/build2/.../sed.f2fs` (no longer needed — we create our own f2fs without checkpoint).

- [ ] **Step 3: Remove unused vm_launcher code**

Remove `start()`, `start_direct()`, all `launch_cvd`/`stop_cvd` references, `--resume`/`--data_policy` logic.

- [ ] **Step 4: Update project memory**

Update the project memory block to reflect the new architecture.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "Remove Cuttlefish dependencies and dead code"
```
