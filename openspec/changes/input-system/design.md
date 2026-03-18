## Context

Nux Emulator currently spawns a crosvm VM with display output via gfxstream, but has no mechanism to send user input back into the guest. The `crosvm-integration` change provides the virtio-input device socket that this design builds on. GTK4's event controller API gives us typed, Wayland/X11-agnostic input events on the drawing surface.

The input pipeline is: GTK4 event → `nux-core::input` translation → Linux evdev event → Unix socket write → crosvm virtio-input device → Android `InputFlinger`.

Key constraints:
- All Rust, no C FFI. We write raw evdev structs (`input_event`) directly — these are simple fixed-size C structs we can define in Rust.
- `unsafe_code` is forbidden at the workspace level, so socket I/O must use safe abstractions (std `UnixStream` or a thin safe wrapper).
- Coordinate mapping must track the display pipeline's scaling/letterbox state, which may change on window resize.

## Goals / Non-Goals

**Goals:**
- Deliver low-latency keyboard, mouse, and scroll input from host to Android guest.
- Support multi-touch synthesis for pinch-zoom via keyboard+mouse combos.
- Provide input grab/release toggle so the cursor can be captured for games or freed for desktop use.
- Correctly map host window coordinates to Android screen coordinates under any scaling or letterbox configuration.
- Clean separation: `nux-core` owns translation and injection logic; `nux-ui` only hooks GTK4 controllers and forwards raw events.

**Non-Goals:**
- Custom key remapping or game profiles (handled by `keymap-engine`).
- Gamepad/controller input (future change).
- On-screen virtual controls (part of `gtk-ui-shell`).
- Display rendering or frame delivery.

## Decisions

### 1. evdev struct definition in Rust (no bindgen)

Define `InputEvent` as a `#[repr(C)]` struct matching Linux `input_event` (16 bytes on x86_64: `timeval` + `u16 type` + `u16 code` + `i32 value`). This avoids a bindgen/libc dependency and keeps the crate pure Rust. The struct is trivial and stable across kernel versions.

**Alternative considered:** Using the `evdev` crate — rejected because we only need to *write* events to a socket, not read from real devices, and adding a full crate for a 16-byte struct is unnecessary.

### 2. Unix socket write to crosvm virtio-input

crosvm's `--input` flag accepts a Unix socket path. We open a `UnixStream`, write serialized `InputEvent` sequences, and crosvm's virtio-input device forwards them to the guest. Each logical action (e.g., a key press) is a batch of events terminated by `EV_SYN/SYN_REPORT`.

**Alternative considered:** Using crosvm's D-Bus or gRPC control API — rejected because the socket protocol is simpler, lower-latency, and already the standard interface for virtio-input.

### 3. Coordinate mapping via shared `DisplayMetrics`

A `DisplayMetrics` struct (guest width/height, host surface width/height, scale factor, letterbox offsets) is shared between the display pipeline and input system. On window resize, the display pipeline updates these metrics; the input module reads them to transform coordinates. This is a simple `Arc<Mutex<DisplayMetrics>>` — resize events are infrequent so contention is negligible.

**Alternative considered:** Having the input module query the GTK widget directly — rejected because it couples `nux-core` to GTK types and breaks the core/UI separation.

### 4. Multi-touch via type-B protocol

Use the Linux multi-touch type-B (slot-based) protocol with `ABS_MT_SLOT`, `ABS_MT_TRACKING_ID`, `ABS_MT_POSITION_X/Y`. Two slots are sufficient for pinch-zoom. Ctrl+click initiates a second touch point; mouse movement with Ctrl held moves both points symmetrically around center for zoom gestures.

### 5. Input grab via GTK4 pointer lock

Use `GdkSurface` pointer confinement (Wayland `zwp_pointer_constraints`) or XInput2 grab (X11). Toggle via a configurable hotkey (default: Ctrl+Alt). When grabbed, mouse motion is reported as relative deltas; when released, absolute coordinates resume.

## Risks / Trade-offs

- **[Latency]** Writing evdev events over a Unix socket adds a small overhead vs. shared memory. → Mitigation: Unix socket writes for small payloads (48-96 bytes per action) are sub-microsecond; this is negligible compared to virtio transport latency.
- **[Pointer lock portability]** Wayland pointer constraints require compositor support; some compositors may not implement `zwp_pointer_constraints`. → Mitigation: Fall back to relative motion calculation from absolute coordinates. Log a warning if confinement fails.
- **[Coordinate drift]** If `DisplayMetrics` update races with an input event, a frame of incorrect coordinates could be sent. → Mitigation: Acceptable — one frame of slight offset is imperceptible. The mutex ensures no torn reads.
- **[Multi-touch fidelity]** Synthesized pinch-zoom from keyboard+mouse is approximate and won't match real touchscreen precision. → Mitigation: Sufficient for map zoom and photo pinch; precision gaming touch is out of scope for v1.

## Open Questions

1. Should the input grab hotkey be user-configurable from day one, or hardcoded with config support added in `config-system`?
2. Does crosvm's virtio-input socket need any handshake or capability negotiation, or is it pure fire-and-forget evdev writes?
