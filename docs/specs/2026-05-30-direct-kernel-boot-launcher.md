# Direct Kernel Boot VM Launcher

## Problem

Cuttlefish's disk architecture (QCOW2 overlays on composite disks referencing AOSP build files) makes persistent storage impossible. The overlay gets invalidated on every `launch_cvd` restart, wiping user data.

## Solution

Replace Cuttlefish entirely with a direct crosvm launcher that:
- Boots the kernel directly (no u-boot, no BIOS)
- Uses raw block devices (no QCOW2, no composites)
- Stores userdata as a plain file that persists across reboots

## Architecture

```
crosvm run \
  --kernel ~/.local/share/nux-emulator/kernel \
  --initrd ~/.local/share/nux-emulator/ramdisk.img \
  --params "<kernel cmdline>" \
  --block path=<aosp>/super.img,ro \
  --block path=~/.local/share/nux-emulator/userdata.img \
  --block path=~/.local/share/nux-emulator/metadata.img \
  --block path=~/.local/share/nux-emulator/misc.img \
  --gpu=<gfxstream config> \
  --wayland-sock=<our compositor> \
  --net tap-name=cvd-mtap-01 \
  --vsock cid=3 \
  --mem=8192 --cpus=8
```

## File Layout

```
~/.local/share/nux-emulator/
├── kernel              # Extracted from boot.img (or KernelSU-patched)
├── ramdisk.img         # Combined ramdisk (init_boot + vendor_boot)
├── userdata.img        # 65GB f2fs, persistent, raw block device
├── metadata.img        # 64MB ext4, persistent (encryption metadata)
├── misc.img            # 1MB, persistent (bootloader messages)
├── config.json         # VM config (root method, memory, cpus, etc.)
└── sdcard.img          # 2GB vfat, persistent (shared storage)

Read-only (referenced from AOSP build dir, never modified):
  <aosp>/out/target/product/vsoc_x86_64/super.img
```

## First Run (Bootstrap)

1. Extract kernel from `boot.img` using `unpack_bootimg`
2. Extract ramdisks from `init_boot.img` + `vendor_boot.img`, combine them
3. Create `userdata.img` (65GB sparse f2fs)
4. Create `metadata.img` (64MB ext4)
5. Create `misc.img` (1MB zeroed)
6. Create `sdcard.img` (2GB vfat)
7. Write `config.json` with default settings
8. Boot crosvm

## Subsequent Runs

1. Read `config.json`
2. Boot crosvm with existing images (no assembly, no modification)
3. All writes go directly to raw files — no overlay invalidation possible

## Kernel Command Line

Derived from Cuttlefish's bootconfig, adapted for direct boot:

```
androidboot.hardware=cutf_cvm
androidboot.serialno=CUTTLEFISHCVD01
androidboot.boot_devices=4010000000.pci
androidboot.fstab_suffix=cf.f2fs.hctr2
androidboot.verifiedbootstate=orange
androidboot.vbmeta.device=PARTUUID=...
androidboot.slot_suffix=_a
ro.boot.hardware.sku=
console=hvc0
panic=-1
noefi
loglevel=4
```

Note: The exact params will be extracted from the running VM's `/proc/cmdline` during first successful boot, then saved to `config.json` for reuse.

## ADB Connectivity

- vsock CID 3 → `adb_connector --addresses=vsock:3:5555 --adb_port=6520`
- Or: virtio-console serial port for ADB (simpler, no vsock needed)
- Fallback: `--net` tap device + TCP ADB inside guest

Recommended: Use the same vsock approach as Cuttlefish but run `adb_connector` ourselves.

## Shutdown

1. `adb shell sync` — flush guest filesystem
2. `adb shell reboot -p` — guest powers off, unmounts cleanly
3. crosvm exits naturally when guest halts (no process_restarter to fight)
4. Done. Raw files are consistent.

No QCOW2 = no cache flush issues. No composite = no invalidation. No overlay = no corruption.

## Rooting Support

| Method | How |
|--------|-----|
| KernelSU | Replace `kernel` with patched version |
| Magisk | Replace `ramdisk.img` with magisk-patched version |
| APatch | Replace `kernel` with patched version |
| None | Use stock kernel + ramdisk |

Config in `config.json`:
```json
{
  "root_method": "none",
  "kernel_path": "kernel",
  "ramdisk_path": "ramdisk.img"
}
```

## Networking

Same TAP setup as current implementation:
- `cvd-mtap-01` for mobile data (NAT to host)
- `cvd-etap-01` for ethernet (optional)
- iptables masquerade rules

## GPU / Display

Unchanged from current implementation:
- `--gpu=gfxstream` with our X11Presenter
- `--wayland-sock` pointing to our Wayland compositor
- X11 window with deferred mapping

## What We Remove

- `launch_cvd` / `assemble_cvd` / `run_cvd` / `process_restarter`
- QCOW2 overlays
- Composite disk assembly
- `disk_builder.cpp` patches
- All the persistence hacks (--resume, --data_policy, checkpoint removal)
- `stop_cvd`

## What We Keep

- `crosvm` binary (from AOSP build)
- `adb_connector` binary (for vsock→TCP bridge)
- Our Wayland compositor
- X11Presenter (gfxstream patches)
- scrcpy control
- Keymap engine
- All nux-ui GTK code

## Migration Path

1. Implement `start_direct_kernel()` in `vm_launcher.rs`
2. Add bootstrap logic (extract kernel/ramdisk, create images)
3. Remove `start()` (launch_cvd path) and all persistence hacks
4. Test boot + persistence + shutdown cycle
5. Remove dead code (stop_cvd, disk_builder patches, etc.)

## Success Criteria

- First boot: < 30s to home screen
- Subsequent boots: < 15s to home screen (no assembly overhead)
- Data persists across unlimited reboots
- Clean shutdown in < 5s
- No I/O errors, no corruption, no rollback
