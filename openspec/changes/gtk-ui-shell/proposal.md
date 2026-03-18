## Why

Nux needs a polished, GNOME-native application shell that wraps the Android display surface and exposes emulator controls. Without a proper GTK4/libadwaita UI, users have no way to interact with VM settings, install APKs, take screenshots, or control the emulator beyond the raw Android screen. The shell is the primary user-facing surface and must feel like a first-class Linux desktop application.

## What Changes

- New `nux-ui` binary crate providing the full GTK4 + libadwaita application
- `AdwApplicationWindow` as the main window with:
  - Central display area (`GtkGLArea` or dmabuf widget) rendering the Android screen from the display pipeline
  - Sidebar toolbar (Bluestacks-style vertical strip) with action buttons: Screenshot, Volume up/down, Shake device, Rotate, Install APK, Toggle keymap overlay, Settings, Fullscreen
  - `GtkOverlay` layer for keymap hint visualization on top of the display
  - `AdwHeaderBar` showing VM status indicator and FPS counter
- `AdwPreferencesWindow`-based settings dialog with pages: Performance, Root, Google Services, Device, Display, About
- APK drag-and-drop install: file drop on the main window triggers ADB sideload
- Window state persistence (size, position, maximized state) across sessions via GSettings or config file
- Application menu, keyboard shortcuts (`Ctrl+F` fullscreen, `Ctrl+Q` quit, etc.)
- Dark mode support inherited automatically from libadwaita / system preference

## Non-goals

- Keymap visual editor UI (deferred to v2)
- Multi-instance window manager (deferred to v2)
- Actual VM lifecycle management logic (belongs in nux-core)
- Android image building or flashing UI
- Custom theming beyond libadwaita defaults

## Capabilities

### New Capabilities
- `app-shell`: AdwApplication bootstrap, single-instance activation, CLI arg handling, application menu
- `main-window`: AdwApplicationWindow layout — display area, sidebar toolbar, header bar, overlay stack
- `settings-dialog`: AdwPreferencesWindow with all preference pages (Performance, Root, Google Services, Device, Display, About)
- `toolbar-actions`: Sidebar toolbar button definitions and action dispatch (screenshot, volume, shake, rotate, APK install, keymap toggle, settings, fullscreen)
- `apk-drag-drop`: Drag-and-drop file handling on the main window for APK installation via ADB
- `window-state`: Window geometry and state persistence across sessions

### Modified Capabilities
<!-- No existing specs are being modified by this change. -->

## Impact

- **New crate**: `nux-ui/` binary crate added to the Cargo workspace
- **Dependencies**: `gtk4 0.9.x`, `libadwaita 0.7.x`, plus `nux-core` for VM control, config, and ADB interfaces
- **Depends on**: `display-pipeline` (provides the renderable surface), `input-system` (forwards touch/key events), `config-system` (reads/writes VM settings)
- **Platform**: Wayland primary, X11 supported (GTK4 handles both transparently)
- **Build**: `cargo build -p nux-ui` / `cargo run -p nux-ui`
