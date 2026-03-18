## Why

Nux Emulator can start a crosvm VM (via `crosvm-integration`), but has no way to show what's happening inside it. The display pipeline bridges crosvm's gfxstream GPU output to the GTK4 window — without it, the emulator is headless. This is the critical visual path that makes Nux usable as a graphical Android emulator.

## What Changes

- New `nux-core::display` module that captures rendered frames from crosvm's gfxstream backend
- dmabuf-based zero-copy frame import: crosvm exports dmabuf FDs, Nux imports them via `GdkDmabufTextureBuilder` for GPU-to-GPU transfer with no CPU copies
- Fallback shared-memory path for environments without dmabuf support (upload frames as GL textures in a `GtkGLArea`)
- Frame presenter widget that handles scaling, aspect-ratio letterboxing, and resolution changes
- VSync synchronization to avoid tearing and limit unnecessary GPU work
- Fullscreen mode toggle with proper resolution and scaling adaptation
- Optional FPS counter overlay for performance debugging

## Non-goals

- Input routing (keyboard, mouse, gamepad → virtio-input) — handled by `input-system` change
- UI chrome (toolbar, settings, status bar) — handled by `gtk-ui-shell` change
- Keymap overlays — handled by `keymap-engine` change
- Audio pipeline — handled by `networking-audio` change
- Multi-display or multi-window support (deferred to v2)
- HDR or color management (future enhancement)

## Capabilities

### New Capabilities
- `frame-capture`: Acquire rendered frames from crosvm via dmabuf FDs or shared memory fallback
- `frame-presentation`: Display frames in the GTK4 window with correct scaling, aspect ratio, and resolution handling
- `display-fullscreen`: Toggle fullscreen mode with proper scaling and resolution adaptation
- `display-sync`: VSync synchronization and optional FPS counter overlay

### Modified Capabilities
_(none — no existing specs to modify)_

## Impact

- **Code**: New `nux-core::display` module with submodules for frame capture, presentation, sync, and fullscreen
- **Dependencies**: `gtk4` (GdkDmabufTextureBuilder, GtkGLArea), `wayland-client` (dmabuf protocol), `nix` (FD passing), `libc` (mmap for shared memory fallback)
- **System requirements**: GPU with dmabuf export support (NVIDIA 515+, Mesa 22.0+) for zero-copy path; any GL-capable GPU for fallback
- **Integration points**: Consumes crosvm process handle from `nux-core::vm` (depends on `crosvm-integration`); provides a display widget consumed by `nux-ui`
- **Config**: New `[display]` section in Nux TOML config for resolution, scaling mode, vsync, and FPS overlay toggle
