# QEMU Migration: virtio-gpu-rutabaga + virtio-input

## Problem

crosvm's disk architecture (QCOW2 overlays on composite disks) makes persistent storage impossible. After 20+ attempts, no combination of flags, patches, or shutdown sequences reliably preserves user data across reboots.

## Solution

Replace crosvm with stock QEMU (`qemu-system-x86_64` 11.0.1) which:
- Has native gfxstream support via `virtio-gpu-rutabaga` (Vulkan + OpenGL)
- Provides `virtio-input` for zero-latency keyboard/mouse (no scrcpy)
- Uses raw disk images directly (trivial persistence)
- Supports direct kernel boot (`-kernel` + `-initrd`)
- Has built-in GTK display with GL rendering

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                  nux-ui (GTK4)                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │
│  │ Control  │  │ Keymap   │  │ APK Install /    │  │
│  │ Panel    │  │ Engine   │  │ Settings         │  │
│  └──────────┘  └──────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────┐
│              qemu-system-x86_64                      │
│  ┌──────────────┐  ┌────────────┐  ┌────────────┐  │
│  │ virtio-gpu   │  │ virtio-    │  │ virtio-blk │  │
│  │ rutabaga     │  │ input      │  │ (raw imgs) │  │
│  │ (gfxstream)  │  │ (kb+mouse) │  │            │  │
│  └──────────────┘  └────────────┘  └────────────┘  │
│  ┌──────────────┐  ┌────────────┐                   │
│  │ GTK display  │  │ KVM accel  │                   │
│  │ (GL render)  │  │ (host CPU) │                   │
│  └──────────────┘  └────────────┘                   │
└─────────────────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────┐
│              Android (AOSP 16)                        │
│  kernel → init → zygote → launcher                   │
│  GPU: gfxstream guest driver → host Vulkan (RTX3060)│
│  Input: virtio-input → /dev/input/eventX            │
│  Storage: /dev/vdX → userdata.img (persistent)      │
└─────────────────────────────────────────────────────┘
```

## QEMU Command Line

```bash
qemu-system-x86_64 \
  -enable-kvm \
  -cpu host \
  -smp 8 \
  -m 8192 \
  -machine q35 \
  \
  # Direct kernel boot
  -kernel <product_out>/kernel \
  -initrd ~/.local/share/nux-emulator/combined_ramdisk.img \
  -append "androidboot.hardware=ranchu androidboot.serialno=EMULATOR30X0X0X0 \
           console=ttyS0 androidboot.console=ttyS0 \
           androidboot.verifiedbootstate=orange \
           qemu=1 qemu.gles=1 \
           androidboot.vbmeta.size=0 \
           androidboot.vbmeta.hash_alg=sha256 \
           androidboot.vbmeta.invalidate_on_error=yes \
           androidboot.logcat=*:V \
           clocksource=pit" \
  \
  # Block devices (raw, no overlay)
  -drive file=<product_out>/super.img,format=raw,if=none,id=system,readonly=on \
  -device virtio-blk-pci,drive=system \
  -drive file=~/.local/share/nux-emulator/userdata.img,format=raw,if=none,id=userdata \
  -device virtio-blk-pci,drive=userdata \
  -drive file=~/.local/share/nux-emulator/cache.img,format=raw,if=none,id=cache \
  -device virtio-blk-pci,drive=cache \
  \
  # GPU: gfxstream via rutabaga (Vulkan + OpenGL)
  -device virtio-gpu-rutabaga,gfxstream-vulkan=on,hostmem=2G \
  -display gtk,gl=on \
  \
  # Input: zero-latency virtio-input
  -device virtio-keyboard-pci \
  -device virtio-mouse-pci \
  \
  # Network: user-mode with ADB port forward
  -netdev user,id=net0,hostfwd=tcp::5555-:5555 \
  -device virtio-net-pci,netdev=net0 \
  \
  # Serial console (for kernel logs)
  -serial stdio \
  \
  # Monitor (for QEMU control)
  -monitor unix:/tmp/nux-qemu-monitor.sock,server,nowait
```

## File Layout

```
~/.local/share/nux-emulator/
├── combined_ramdisk.img    # vendor + generic ramdisk (created once)
├── userdata.img            # 65GB sparse, persistent (raw f2fs)
├── cache.img               # 2GB, persistent
├── sdcard.img              # 2GB vfat, persistent
└── config.json             # VM settings

Read-only (from AOSP build dir):
  <aosp>/out/target/product/vsoc_x86_64/kernel
  <aosp>/out/target/product/vsoc_x86_64/super.img
```

## Persistence

Trivial. `userdata.img` is a raw file. QEMU writes directly to it. On shutdown:
1. `adb shell sync` (flush guest buffers)
2. `quit` via QEMU monitor socket (clean QEMU exit)
3. Done. File is consistent.

No QCOW2, no overlays, no composites, no checkpoint rollback.

## Input

QEMU's GTK window captures keyboard/mouse natively via `virtio-input`. Events go directly to the Android kernel's input subsystem — no scrcpy, no TCP, no Java InputManager.

For the keymap engine: we intercept QEMU's input events at the GTK level (before they reach virtio-input) and translate mapped keys to touch events via ADB. Unmapped keys pass through to virtio-input normally.

## Display

QEMU's built-in GTK display with `gl=on` renders gfxstream output directly. The window is managed by QEMU — we don't need X11Presenter, Wayland compositor, or any custom display code.

Our GTK control panel runs as a separate window alongside QEMU's display window.

## ADB

`-netdev user,hostfwd=tcp::5555-:5555` forwards host port 5555 to guest port 5555 (adbd). Connect with `adb connect 127.0.0.1:5555`.

## AOSP Image Compatibility

The kernel command line uses `androidboot.hardware=ranchu` which is the standard QEMU/goldfish machine type. This requires:
- A ranchu-compatible fstab (maps virtio-blk devices to partitions)
- Or: keep using `cutf_cvm` hardware with a custom fstab that maps `/dev/vdX` devices

Recommended: Use `ranchu` hardware — it's what the Android Emulator uses and has well-tested fstab support for virtio-blk.

## What We Keep

- GTK4 control panel (nux-ui binary)
- Keymap engine (JSON keymaps, steer wheel, mouse aim)
- APK install via ADB
- Landscape orientation detection
- Navigation shortcuts (Escape→Back, F11→Home, F12→Recent)
- Persistent storage at ~/.local/share/nux-emulator/

## What We Remove

- crosvm binary and all crosvm-specific code
- X11Presenter (AOSP gfxstream patches to crosvm)
- Wayland compositor (wayland_compositor.rs)
- scrcpy (server, control, decoder)
- display_wl.c patches
- disk_builder.cpp patches
- sed.f2fs patches
- All launch_cvd / assemble_cvd / stop_cvd code
- x11_input.rs (replaced by QEMU's native input)
- vm_bootstrap.rs (simplified — just create raw images)

## What Changes

| Component | Before (crosvm) | After (QEMU) |
|-----------|----------------|--------------|
| VMM | crosvm (AOSP build) | qemu-system-x86_64 (system package) |
| GPU | gfxstream (custom patches) | virtio-gpu-rutabaga (stock) |
| Display | X11Presenter + Wayland compositor | QEMU GTK display (built-in) |
| Input | scrcpy control socket (5ms) | virtio-input (0ms) |
| Disk | QCOW2 overlay on composite | Raw images (direct) |
| Boot | u-boot + GPT composite | -kernel + -initrd (direct) |
| ADB | vsock + adb_connector | TCP port forward (built-in) |
| Shutdown | Complex (sync+stop_cvd+SIGTERM) | Simple (sync + monitor quit) |
| Persistence | Broken (overlay invalidation) | Trivial (raw file) |

## Risks

1. **Ranchu fstab**: May need AOSP rebuild with ranchu device config instead of Cuttlefish
2. **gfxstream version mismatch**: System rutabaga lib may not match AOSP guest drivers — need to verify
3. **virtio-input keymap**: Android may need input device configuration for virtio-keyboard
4. **Performance**: Need to benchmark QEMU vs crosvm (should be similar with KVM)

## Success Criteria

- Boot to home screen in < 20s
- Data persists across unlimited reboots (install app → reboot → app still there)
- GPU rendering at 30+ fps (gfxstream Vulkan)
- Input latency < 5ms (virtio-input)
- Clean shutdown in < 3s
