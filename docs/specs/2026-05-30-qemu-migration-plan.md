# QEMU Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace crosvm with stock QEMU for reliable persistent storage, zero-latency input, and native gfxstream GPU rendering.

**Architecture:** `qemu-system-x86_64` with `-kernel` direct boot, `virtio-gpu-rutabaga` for GPU (gfxstream/Vulkan), `virtio-input` for keyboard/mouse, raw disk images for persistence. nux-ui becomes a thin control panel that launches QEMU and communicates via ADB.

**Tech Stack:** Rust (nux-ui GTK4), QEMU 11.0.1, KVM, virtio-gpu-rutabaga, AOSP 16

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `nux-ui/src/vm_launcher.rs` | Rewrite | QEMU process management (start/stop) |
| `nux-ui/src/vm_bootstrap.rs` | Modify | Create ramdisk + userdata for QEMU |
| `nux-ui/src/window.rs` | Simplify | Remove Wayland compositor, simplify boot flow |
| `nux-ui/src/main.rs` | Modify | Remove dead modules |
| `nux-ui/src/display.rs` | Remove | No longer needed (QEMU handles display) |
| `nux-ui/src/wayland_compositor.rs` | Remove | No longer needed |
| `nux-ui/src/wayland_protocol.rs` | Remove | No longer needed |
| `nux-ui/src/x11_input.rs` | Remove | Replaced by virtio-input |
| `nux-ui/src/scrcpy/` | Remove | No longer needed |
| `nux-ui/Cargo.toml` | Modify | Remove wayland/scrcpy dependencies |

---

### Task 1: Rewrite vm_bootstrap.rs for QEMU

**Files:**
- Modify: `nux-ui/src/vm_bootstrap.rs`

- [ ] **Step 1: Rewrite bootstrap for QEMU disk layout**

The bootstrap creates:
- `combined_ramdisk.img` (vendor + generic ramdisk — already exists)
- `userdata.img` (65GB sparse f2fs)
- `cache.img` (2GB sparse ext4)

Remove references to crosvm-specific images (metadata.img, misc.img).

```rust
//! Prepares disk images for QEMU direct kernel boot.

use std::path::PathBuf;
use std::process::Command;

/// Returns the persistent data directory: ~/.local/share/nux-emulator/
pub fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".local/share/nux-emulator")
}

/// Returns the AOSP product output directory.
pub fn product_out() -> PathBuf {
    PathBuf::from("/build2/nux-emulator/nux-android-image/aosp/out/target/product/vsoc_x86_64")
}

/// Returns the AOSP host tools output directory.
pub fn host_out() -> PathBuf {
    PathBuf::from("/build2/nux-emulator/nux-android-image/aosp/out/host/linux-x86")
}

/// Checks whether bootstrap has been performed.
pub fn is_bootstrapped() -> bool {
    data_dir().join("userdata.img").exists()
}

/// Creates persistent disk images and combined ramdisk.
pub fn bootstrap() -> Result<(), String> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;

    create_combined_ramdisk(&dir.join("combined_ramdisk.img"))?;
    create_sparse_image(&dir.join("userdata.img"), "65G", "f2fs", "data")?;
    create_sparse_image(&dir.join("cache.img"), "2G", "ext4", "")?;

    log::info!("bootstrap: complete — images ready in {}", dir.display());
    Ok(())
}

fn create_combined_ramdisk(path: &std::path::Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    log::info!("bootstrap: creating combined ramdisk...");

    let product = product_out();
    let host = host_out();
    let tmp = std::env::temp_dir().join("nux_ramdisk_tmp");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| format!("mkdir tmp: {e}"))?;

    // Extract vendor ramdisk
    let output = Command::new(host.join("bin/unpack_bootimg"))
        .args(["--boot_img", &product.join("vendor_boot.img").to_string_lossy(),
               "--out", &tmp.to_string_lossy()])
        .output()
        .map_err(|e| format!("unpack_bootimg: {e}"))?;
    if !output.status.success() {
        return Err(format!("unpack_bootimg failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    // Concatenate vendor + generic ramdisk
    let vendor_rd = tmp.join("vendor_ramdisk00");
    let generic_rd = product.join("ramdisk.img");
    let output = Command::new("sh")
        .args(["-c", &format!("cat '{}' '{}' > '{}'",
            vendor_rd.display(), generic_rd.display(), path.display())])
        .output()
        .map_err(|e| format!("cat: {e}"))?;
    if !output.status.success() {
        return Err("cat ramdisks failed".into());
    }

    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}

fn create_sparse_image(path: &std::path::Path, size: &str, fs: &str, label: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    log::info!("bootstrap: creating {} ({} {})...", path.display(), size, fs);

    // Create sparse file
    let s = Command::new("truncate")
        .args(["-s", size, &path.to_string_lossy()])
        .status()
        .map_err(|e| format!("truncate: {e}"))?;
    if !s.success() {
        return Err(format!("truncate {} failed", path.display()));
    }

    // Format
    match fs {
        "f2fs" => {
            let mut args = vec!["-f".to_string()];
            if !label.is_empty() {
                args.push("-l".to_string());
                args.push(label.to_string());
            }
            args.push(path.to_string_lossy().to_string());
            let s = Command::new("mkfs.f2fs").args(&args).status()
                .map_err(|e| format!("mkfs.f2fs: {e}"))?;
            if !s.success() {
                return Err(format!("mkfs.f2fs {} failed", path.display()));
            }
        }
        "ext4" => {
            let s = Command::new("mkfs.ext4")
                .args(["-F", &path.to_string_lossy()])
                .status()
                .map_err(|e| format!("mkfs.ext4: {e}"))?;
            if !s.success() {
                return Err(format!("mkfs.ext4 {} failed", path.display()));
            }
        }
        _ => {}
    }
    Ok(())
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /home/kk/dev/emulator && cargo check -p nux-ui`

- [ ] **Step 3: Commit**

```bash
git add nux-ui/src/vm_bootstrap.rs
git commit -m "Rewrite vm_bootstrap for QEMU disk layout"
```

---

### Task 2: Rewrite vm_launcher.rs for QEMU

**Files:**
- Rewrite: `nux-ui/src/vm_launcher.rs`

- [ ] **Step 1: Replace entire vm_launcher with QEMU launcher**

The new launcher:
- Calls bootstrap if needed
- Sets up networking (TAP device)
- Launches `qemu-system-x86_64` with direct kernel boot
- Provides `stop()` via QEMU monitor socket
- Provides `check_boot_status()` via ADB
- Provides `adb_shell()`, `install_apk()`, etc.

Key QEMU arguments:
```
qemu-system-x86_64 \
  -enable-kvm -cpu host -smp 8 -m 8192 -machine q35 \
  -kernel <product_out>/kernel \
  -initrd <data_dir>/combined_ramdisk.img \
  -append "<kernel cmdline>" \
  -drive file=<product_out>/super.img,format=raw,if=none,id=system,readonly=on \
  -device virtio-blk-pci,drive=system \
  -drive file=<data_dir>/userdata.img,format=raw,if=none,id=userdata \
  -device virtio-blk-pci,drive=userdata \
  -drive file=<data_dir>/cache.img,format=raw,if=none,id=cache \
  -device virtio-blk-pci,drive=cache \
  -device virtio-gpu-rutabaga,gfxstream-vulkan=on,hostmem=2G \
  -display gtk,gl=on \
  -device virtio-keyboard-pci \
  -device virtio-mouse-pci \
  -netdev user,id=net0,hostfwd=tcp::5555-:5555 \
  -device virtio-net-pci,netdev=net0 \
  -serial mon:stdio \
  -monitor unix:<data_dir>/qemu-monitor.sock,server,nowait
```

Stop via monitor: `echo "quit" | socat - UNIX-CONNECT:<data_dir>/qemu-monitor.sock`

ADB: `adb connect 127.0.0.1:5555`

- [ ] **Step 2: Verify it compiles**

Run: `cd /home/kk/dev/emulator && cargo check -p nux-ui`

- [ ] **Step 3: Commit**

```bash
git add nux-ui/src/vm_launcher.rs
git commit -m "Rewrite VM launcher for QEMU with virtio-gpu-rutabaga"
```

---

### Task 3: Simplify window.rs

**Files:**
- Modify: `nux-ui/src/window.rs`

- [ ] **Step 1: Remove Wayland compositor and display code from boot flow**

The new boot flow:
1. Call `launcher.start_kernel()` (no wayland_sock needed — QEMU handles display)
2. Poll `check_boot_status()` until booted
3. Run ARM translation setup
4. Done — QEMU's GTK window is the display

Remove:
- `wl_tx`/`wl_rx` channels
- `wayland_compositor::start_compositor_at_path()`
- Socket swap logic
- `display::start_input_only()`
- `x11_input::start_x11_input_bridge()`
- All `wayland_frame_slot` / `wayland_input` state

- [ ] **Step 2: Verify it compiles**

Run: `cd /home/kk/dev/emulator && cargo check -p nux-ui`

- [ ] **Step 3: Commit**

```bash
git add nux-ui/src/window.rs
git commit -m "Simplify window.rs: remove Wayland/display code (QEMU handles it)"
```

---

### Task 4: Remove Dead Code

**Files:**
- Remove: `nux-ui/src/display.rs`
- Remove: `nux-ui/src/wayland_compositor.rs`
- Remove: `nux-ui/src/wayland_protocol.rs`
- Remove: `nux-ui/src/x11_input.rs`
- Remove: `nux-ui/src/scrcpy/` (entire directory)
- Modify: `nux-ui/src/main.rs`
- Modify: `nux-ui/src/state.rs`
- Modify: `nux-ui/Cargo.toml`

- [ ] **Step 1: Remove module declarations from main.rs**

Remove these lines:
```rust
mod display;
mod wayland_compositor;
mod wayland_protocol;
mod x11_input;
pub mod scrcpy;
```

- [ ] **Step 2: Remove dead files**

```bash
rm nux-ui/src/display.rs
rm nux-ui/src/wayland_compositor.rs
rm nux-ui/src/wayland_protocol.rs
rm nux-ui/src/x11_input.rs
rm -rf nux-ui/src/scrcpy/
```

- [ ] **Step 3: Clean up state.rs**

Remove `wayland_frame_slot`, `wayland_input`, `scrcpy` fields from `UiState`.

- [ ] **Step 4: Remove unused dependencies from Cargo.toml**

Remove: `wayland-server`, `wayland-backend`, `wayland-protocols`, `wayland-scanner`, `nix`

- [ ] **Step 5: Verify it compiles**

Run: `cd /home/kk/dev/emulator && cargo check -p nux-ui`

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "Remove dead code: Wayland compositor, scrcpy, X11 input, display"
```

---

### Task 5: Integration Test — Boot + Persistence

- [ ] **Step 1: Clean start**

```bash
sudo pkill -9 -f qemu
rm -rf ~/.local/share/nux-emulator
xhost +local:
cd /home/kk/dev/emulator && RUST_LOG=info cargo run --release -p nux-ui
```

Verify: QEMU window appears with Android booting, reaches home screen.

- [ ] **Step 2: Check ADB**

```bash
adb connect 127.0.0.1:5555
adb -s 127.0.0.1:5555 shell getprop sys.boot_completed
```

Expected: `1`

- [ ] **Step 3: Install an app**

Install any APK via the control panel toolbar.

- [ ] **Step 4: Shutdown**

Close the control panel window. Verify QEMU exits cleanly.

- [ ] **Step 5: Second boot — verify persistence**

```bash
cd /home/kk/dev/emulator && RUST_LOG=info cargo run --release -p nux-ui
```

Verify: App is still installed.

- [ ] **Step 6: Third boot — verify reliability**

Close and relaunch. App still there.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "QEMU migration complete: verified persistent storage"
git push origin master
```

---

### Task 6: Keymap Integration (Post-boot)

**Files:**
- Modify: `nux-ui/src/keymap/engine.rs`

- [ ] **Step 1: Adapt keymap to work with ADB input instead of scrcpy**

Since QEMU handles input natively via virtio-input, the keymap engine needs to send touch events via `adb shell input` instead of scrcpy control socket. For gaming keymaps (steer wheel, mouse aim), use ADB input commands.

Note: This is a follow-up task. Basic keyboard/mouse works immediately via QEMU's virtio-input. The keymap engine (for gaming) needs adaptation.

- [ ] **Step 2: Commit**

```bash
git add nux-ui/src/keymap/
git commit -m "Adapt keymap engine for ADB input (post-QEMU migration)"
```
