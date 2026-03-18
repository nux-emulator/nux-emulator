## Context

Nux Emulator uses crosvm with gfxstream as its GPU backend. crosvm renders Android frames on the host GPU via gfxstream, which translates guest Vulkan/GLES calls into host Vulkan/GL calls. The rendered output needs to reach the GTK4 window.

Currently, `nux-core::vm` (from `crosvm-integration`) spawns and manages the crosvm process. The display pipeline sits between crosvm's rendered output and the GTK4 UI — it captures frames and presents them in the application window. This module lives in `nux-core::display` (capture/sync logic) with a thin presentation layer in `nux-ui` (GTK widget).

crosvm supports multiple display backends. For our use case, the relevant ones are:
- **gpu display mode `surfaceless`** with dmabuf export: crosvm renders to GPU buffers and exports them as dmabuf FDs via a Wayland protocol or virtio-gpu resource export
- **gpu display mode `x` or `wayland`**: crosvm creates its own window surface — we'd need to embed or redirect it

The GTK4 stack (4.14+) has native dmabuf import support via `GdkDmabufTextureBuilder`, making zero-copy display possible on modern systems.

## Goals / Non-Goals

**Goals:**
- Zero-copy frame path from crosvm GPU output to GTK4 display on supported hardware
- Functional fallback for systems without dmabuf support
- Correct aspect ratio preservation at all window sizes
- Smooth fullscreen transitions without frame drops
- VSync-aligned presentation to prevent tearing
- Minimal latency between frame render and display (target: <1 frame of latency)

**Non-Goals:**
- Input event routing (separate `input-system` change)
- Window chrome, toolbar, or settings UI (separate `gtk-ui-shell` change)
- Audio synchronization with video frames
- HDR, wide color gamut, or color management
- Multi-display / multi-window
- Recording or screenshot capture (future enhancement)

## Decisions

### 1. Primary path: dmabuf zero-copy via `GdkDmabufTextureBuilder`

**Choice:** Run crosvm with `--gpu=gfxstream,surfaceless` and configure it to export rendered frames as dmabuf FDs. Import these into GTK4 using `GdkDmabufTextureBuilder` to create `GdkTexture` objects with no CPU-side copies.

**Alternatives considered:**
- **Embed crosvm's own Wayland/X11 surface as a subsurface:** Would avoid frame capture entirely, but GTK4 doesn't support foreign subsurface embedding reliably. Compositor-dependent, breaks fullscreen/scaling control, and makes overlays (FPS counter, keymaps) impossible to layer correctly.
- **Shared memory with CPU upload:** Works everywhere but involves a full-frame CPU copy per frame (at 1080p60 that's ~370 MB/s of memcpy). Unacceptable as the primary path for a gaming emulator.

**Rationale:** dmabuf is the standard Linux zero-copy buffer sharing mechanism. GTK4 4.14+ supports it natively. gfxstream on crosvm can export dmabufs. This gives us GPU→GPU transfer with no CPU involvement — critical for gaming performance.

### 2. Fallback path: shared memory upload to `GtkGLArea`

**Choice:** When dmabuf import fails (older GPU drivers, missing kernel support, or proprietary driver quirks), fall back to crosvm rendering into a shared memory region. Map it with `mmap`, then upload to a GL texture inside a `GtkGLArea` widget each frame.

**Rationale:** Not every system supports dmabuf. The shared memory path is slower but universally compatible. Auto-detection at startup: attempt dmabuf first, fall back on failure. Log which path is active so users can diagnose performance issues.

### 3. Module structure under `nux-core::display`

```
nux-core/src/display/
├── mod.rs           // DisplayPipeline: top-level orchestrator, public API
├── capture.rs       // FrameCapture trait + DmabufCapture / ShmCapture impls
├── sync.rs          // VSync timing, frame pacing, FPS counter
├── config.rs        // DisplayConfig: typed config from [display] TOML section
└── error.rs         // DisplayError enum
```

The presentation widget lives in `nux-ui`:
```
nux-ui/src/display_widget.rs  // NuxDisplayWidget: GtkWidget that consumes frames
```

**Rationale:** Frame capture and sync are core logic with no UI dependency — they belong in `nux-core`. The GTK widget that actually renders textures to screen belongs in `nux-ui`. This keeps the crate boundary clean: `nux-core` produces frames, `nux-ui` presents them.

### 4. Frame delivery via channel

**Choice:** `FrameCapture` sends frames through a `tokio::sync::watch` channel. The `nux-ui` display widget polls the latest frame on each GTK frame clock tick via `add_tick_callback`.

Using `watch` (not `mpsc`) because the display widget only cares about the latest frame — if the VM produces frames faster than the display refreshes, intermediate frames are silently dropped. This is correct behavior for real-time display.

**Rationale:** Decouples capture rate from display rate. No backpressure needed — dropping stale frames is the right thing to do. `watch` has minimal overhead and is lock-free for the receiver.

### 5. Scaling and aspect ratio via `GtkPicture`

**Choice:** Wrap the frame texture in a `GtkPicture` widget with `content-fit = contain`. GTK handles letterboxing/pillarboxing automatically. For integer scaling (pixel-perfect mode), calculate the largest integer multiple that fits and set explicit size requests.

**Alternatives considered:**
- Custom `GtkGLArea` with manual viewport math: More control but reimplements what GTK already does well. Only needed for the shared-memory fallback path.
- CSS-based scaling: Not precise enough for pixel-perfect rendering.

**Rationale:** `GtkPicture` with `GdkTexture` is the idiomatic GTK4 approach. It handles HiDPI, scaling, and aspect ratio correctly with no custom code.

### 6. Fullscreen via `GtkWindow::fullscreen()`

**Choice:** Use GTK4's native fullscreen API. On entering fullscreen, hide UI chrome and let the display widget fill the window. The scaling logic adapts automatically since it's already responsive to container size.

**Rationale:** GTK4 handles the compositor negotiation (Wayland `xdg_toplevel.set_fullscreen`, X11 `_NET_WM_STATE_FULLSCREEN`). No custom fullscreen logic needed.

### 7. VSync via GTK frame clock

**Choice:** Tie frame presentation to GTK's `FrameClock` via `add_tick_callback`. This naturally syncs to the compositor's VSync. For the VM side, configure crosvm's virtio-gpu to signal frame completion, and pace frame capture accordingly.

**Rationale:** GTK's frame clock is already VSync-aligned by the compositor. Presenting one frame per tick gives us tear-free display without manual VSync implementation.

## Risks / Trade-offs

- **dmabuf support varies across drivers** → Auto-detect at startup with a test import. Fall back to shared memory gracefully. Log the active path clearly.
- **gfxstream dmabuf export may require specific crosvm flags** → Document required crosvm build flags and GPU driver versions. Test on NVIDIA (proprietary), AMD (Mesa), and Intel (Mesa).
- **Frame latency from channel + frame clock** → Worst case is ~1 frame of latency (capture happens between ticks). Acceptable for an emulator. Could optimize later with `wl_surface.frame` callbacks if needed.
- **Shared memory fallback performance** → At 1080p60, each frame is ~8MB uncompressed BGRA. `memcpy` + GL upload will consume CPU and bus bandwidth. Acceptable as a fallback, not as the primary path. Consider reducing resolution or frame rate in this mode.
- **GTK4 version requirements** → `GdkDmabufTextureBuilder` requires GTK 4.14+. Most current distros ship 4.14+ (Ubuntu 24.04, Fedora 40+). Older distros fall back to the shared memory path.
- **`unsafe` code for dmabuf FD handling** → FD import requires `unsafe` for `from_raw_fd`. Minimize the unsafe surface area, wrap in safe abstractions, and document invariants. Note: workspace forbids `unsafe_code` — will need a scoped `#[allow(unsafe_code)]` on the specific FD wrapper functions with justification comments.

## Open Questions

1. **crosvm dmabuf export mechanism**: Does gfxstream's surfaceless mode export dmabufs via virtio-gpu resource sharing, or do we need to use a custom Wayland compositor stub? Need to prototype this.
2. **Frame format negotiation**: What pixel formats does gfxstream export (ARGB8888, NV12, etc.)? `GdkDmabufTextureBuilder` supports a specific set — need to verify overlap.
3. **Resolution change signaling**: When the Android guest changes display resolution, how does crosvm signal this to the host? Need to check if virtio-gpu config changes are propagated.
4. **FPS overlay implementation**: Should the FPS counter be a GTK overlay widget or rendered directly into the frame texture? Overlay widget is simpler and doesn't affect the frame path.
