## 1. Network Module Scaffolding

- [x] 1.1 Create `nux-core/src/network.rs` module with `NetworkBackend` enum (Tap, Passt) and `NetworkConfig` struct; register module in `nux-core/src/lib.rs`
- [x] 1.2 Add network-related fields to the Nux TOML config struct (backend preference, bridge name, guest IP, ADB port) with serde defaults

## 2. TAP Network Backend

- [x] 2.1 Implement bridge detection: check if `nux-br0` interface exists on the host (read from `/sys/class/net/`)
- [x] 2.2 Implement TAP device creation and attachment to `nux-br0`; return the TAP fd/name for crosvm CLI args
- [x] 2.3 Wire TAP backend into crosvm argument builder: append `--tap` with the device name when TAP is selected
- [x] 2.4 Test: verify crosvm launches with correct `--tap` argument when `nux-br0` exists

## 3. Passt Fallback Backend

- [x] 3.1 Implement passt binary detection (`which passt` or `$PATH` lookup)
- [x] 3.2 Implement passt process spawning with correct flags (socket path, `--forward 5555:5555` for ADB)
- [x] 3.3 Wire passt backend into crosvm argument builder: append passt socket path argument
- [x] 3.4 Implement passt process lifecycle management (start before crosvm, kill on VM shutdown)
- [x] 3.5 Test: verify crosvm launches with passt socket arg when no bridge exists but passt is available

## 4. Network Error Handling and Backend Selection

- [x] 4.1 Implement backend selection logic: TAP if bridge exists → passt if binary found → error with actionable message
- [x] 4.2 Write unit tests for backend selection logic covering all three paths (TAP, passt, error)

## 5. Network Setup Script

- [x] 5.1 Create `scripts/setup-network.sh`: create `nux-br0` bridge, configure subnet `192.168.100.0/24`, enable NAT/IP forwarding
- [x] 5.2 Add systemd-networkd drop-in persistence to the script so config survives reboots
- [x] 5.3 Add idempotency checks (skip if bridge already exists and is correctly configured)
- [x] 5.4 Test: run script twice, verify bridge is created on first run and no-op on second

## 6. DNS Configuration

- [x] 6.1 Configure DHCP on `nux-br0` (via dnsmasq or systemd-networkd) to hand out DNS to the guest in TAP mode
- [x] 6.2 Verify passt auto-forwards host DNS to guest (no extra config needed; add integration note)
- [x] 6.3 Test: verify guest can resolve `dns.google` on both TAP and passt backends

## 7. Port Forwarding for ADB

- [x] 7.1 Assign static guest IP `192.168.100.2` in TAP mode (via DHCP reservation or guest-side config)
- [x] 7.2 Configure passt `--forward` flag for port 5555 mapping in passt mode
- [x] 7.3 Test: verify `adb connect` works on both TAP (direct IP) and passt (`localhost:5555`) backends

## 8. Audio Module Scaffolding

- [x] 8.1 Create `nux-core/src/audio.rs` module with `AudioConfig` struct (`enabled`, `volume`, `muted`); register module in `nux-core/src/lib.rs`
- [x] 8.2 Add `[audio]` section to TOML config struct with defaults (`enabled = true`, `volume = 80`, `muted = false`)
- [x] 8.3 Test: verify config deserialization with and without `[audio]` section

## 9. Virtio-snd Integration

- [x] 9.1 Wire `--virtio-snd` into crosvm argument builder when `audio.enabled` is true; omit when false
- [x] 9.2 Configure ALSA backend parameters: period size 256, period count 2 in crosvm args
- [x] 9.3 Implement audio initialization error handling: detect crosvm audio failure from stderr, report to UI, allow VM to continue without audio
- [x] 9.4 Test: verify crosvm args include `--virtio-snd` with correct buffer params when audio enabled; verify args omit it when disabled

## 10. Audio Latency Measurement

- [x] 10.1 Implement latency measurement at audio init (timestamp-based probe via crosvm control socket or log parsing)
- [x] 10.2 Log measured latency at info level; emit warning if >80ms
- [x] 10.3 Expose latency warning event to UI layer (callback or channel message)
- [x] 10.4 Test: verify warning is emitted when simulated latency exceeds threshold

## 11. Volume Control — Core

- [x] 11.1 Implement volume command interface in nux-core: `set_volume(level: u8)` and `toggle_mute()` that send commands to crosvm control socket
- [x] 11.2 Persist volume/mute state to TOML config on change
- [x] 11.3 Restore volume/mute state from config on VM startup
- [x] 11.4 Test: verify volume commands are sent to control socket; verify config round-trip

## 12. Volume Control — UI

- [ ] 12.1 Add volume toolbar button with mute/unmute icon toggle to `nux-ui` toolbar
- [ ] 12.2 Add volume slider popover attached to the volume button
- [ ] 12.3 Wire slider and button to nux-core volume control interface
- [ ] 12.4 Update button icon to reflect mute state (speaker vs speaker-muted)
- [ ] 12.5 Test: verify UI controls update core state and icon reflects mute/unmute correctly
