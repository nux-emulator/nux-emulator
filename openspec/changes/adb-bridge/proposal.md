## Why

Nux needs a way to communicate with the Android VM for essential emulator operations: installing and managing apps, transferring files, capturing screenshots, and querying device state. ADB (Android Debug Bridge) is the standard protocol for this. Without an integrated ADB bridge, users would need to manually manage an external ADB binary and connection, which breaks the seamless desktop-native experience Nux aims for.

This change depends on `crosvm-integration` since ADB connectivity requires a running VM with a reachable guest network or virtio-serial channel.

## What Changes

- Add `nux-core::adb` module providing a Rust ADB client that connects to the Android guest
- Support two transport modes: TCP (over guest virtual network) and virtio-serial (direct host-guest channel)
- Expose app management operations: install APK (by file path), uninstall by package name, list installed packages, launch app by package name
- Expose file transfer operations: push file to VM, pull file from VM
- Provide internal shell command execution for: device property queries, screen resolution, input injection fallback (`adb shell input`), screenshot capture (`adb shell screencap`)
- Expose device info queries: Android version, API level, device model, available ABIs

## Capabilities

### New Capabilities
- `adb-connection`: ADB transport layer — TCP and virtio-serial connection management, handshake, reconnection, and connection health monitoring
- `adb-app-management`: App lifecycle operations — install APK, uninstall, list packages, launch by package name
- `adb-file-transfer`: File push/pull between host and Android guest
- `adb-shell`: Internal shell command execution, screenshot capture, device info queries, and input injection fallback

### Modified Capabilities
_(none — no existing spec-level requirements change)_

## Impact

- **Code**: New `nux-core/src/adb/` module tree. `nux-ui` will depend on these APIs for drag-and-drop APK install, file manager integration, and device info display.
- **Dependencies**: Depends on `crosvm-integration` for VM lifecycle and guest networking/virtio-serial setup. No new external crate dependencies expected — the ADB protocol will be implemented directly in Rust.
- **Systems**: Requires the Android guest to have `adbd` running and reachable. TCP mode needs the virtual network from `networking-audio`; virtio-serial mode needs a configured virtio-serial port from crosvm.

## Non-goals

- ADB over network to external physical devices — Nux only talks to its own VM
- Wireless ADB or ADB pairing workflows
- ADB server management for multiple devices — single-instance VM only
- Exposing a general-purpose ADB server that external tools can connect to
- GUI for ADB shell — shell access is internal API only, not user-facing
