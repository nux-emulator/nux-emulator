## Context

Nux Emulator needs a GTK4 + libadwaita application shell (`nux-ui` crate) that serves as the primary user interface. The UI wraps the Android display surface provided by the display pipeline and exposes emulator controls (toolbar, settings, APK install, etc.).

The `nux-core` crate provides all backend functionality — VM management, display pipeline, input routing, ADB bridge, and config. The UI crate is a thin presentation layer that calls into `nux-core` APIs and renders the Android framebuffer.

GTK4 and libadwaita are accessed via `gtk-rs` bindings (gtk4 0.9.x, libadwaita 0.7.x), which provide safe Rust APIs without C++ FFI. The application targets GNOME desktops primarily but works on any Linux DE via GTK4's backend abstraction (Wayland primary, X11 supported).

## Goals / Non-Goals

**Goals:**
- Provide a complete, polished application shell that feels native on GNOME
- Clean separation between UI and core logic — `nux-ui` depends on `nux-core` but never the reverse
- Responsive layout that handles window resizing, fullscreen, and orientation changes
- Persistent window state and user preferences
- Accessible toolbar and settings via standard GTK4 widget patterns

**Non-Goals:**
- Custom rendering engine or widget toolkit — we use stock GTK4/libadwaita widgets everywhere except the display area
- Keymap visual editor (v2)
- Multi-instance window management (v2)
- VM lifecycle logic — that lives in `nux-core`; the UI only calls start/stop/pause

## Decisions

### 1. Application bootstrap: `AdwApplication` with single-instance activation

Use `adw::Application` (wraps `gtk::Application`) as the entry point. GTK4's `GApplication` provides single-instance semantics via D-Bus automatically — a second launch activates the existing window. This avoids custom IPC for single-instance enforcement.

**Alternative considered:** Manual PID file locking. Rejected — `GApplication` handles this natively and also forwards CLI args and file opens to the running instance.

### 2. Main window layout: `AdwApplicationWindow` with horizontal box

```
┌─────────────────────────────────────────────┐
│ AdwHeaderBar [VM status] [FPS]              │
├──────────────────────────────────┬──────────┤
│                                  │ Toolbar  │
│         GtkOverlay               │ (GtkBox  │
│         ├─ Display widget        │  vertical│
│         └─ Keymap hints overlay  │  buttons)│
│                                  │          │
├──────────────────────────────────┴──────────┤
│ (status bar — optional, future)             │
└─────────────────────────────────────────────┘
```

The main content is a horizontal `GtkBox`: display area (expanding) + sidebar toolbar (fixed width). The display area is wrapped in a `GtkOverlay` to layer keymap hints on top. The sidebar sits on the right edge, matching the Bluestacks convention users expect from Android emulators.

**Alternative considered:** `AdwToolbarView` with bottom toolbar. Rejected — a vertical sidebar is more space-efficient and familiar for emulator users. Horizontal bottom bars waste vertical space that the Android display needs.

### 3. Display widget: `GtkGLArea` initially, dmabuf path later

Start with `GtkGLArea` receiving frames from the display pipeline via OpenGL texture sharing. This is the simplest integration path with gfxstream. A dmabuf-based `GtkWidget` subclass can be added later for zero-copy display on Wayland compositors.

The display widget maintains aspect ratio of the Android surface and centers within available space (letterboxing with black bars).

**Alternative considered:** Custom `GdkPaintable` implementation. Viable but `GtkGLArea` gives direct GL context control needed for gfxstream texture consumption.

### 4. Settings dialog: `AdwPreferencesWindow` with `AdwPreferencesPage` per category

Each settings category (Performance, Root, Google Services, Device, Display, About) is an `AdwPreferencesPage` containing `AdwPreferencesGroup` and standard Adwaita preference rows (`AdwSpinRow`, `AdwComboRow`, `AdwSwitchRow`, `AdwActionRow`). This gives us a native GNOME settings look with zero custom styling.

Settings read from and write to the config system (`nux-core::config`). Changes apply immediately where possible (e.g., volume) or prompt for VM restart when required (e.g., RAM, CPU cores).

### 5. Toolbar actions: GIO `ActionMap` on the application window

Each toolbar button maps to a `gio::SimpleAction` registered on the `AdwApplicationWindow`. Actions are also exposed as application-level actions for keyboard shortcuts (`app.fullscreen`, `win.screenshot`, etc.). This follows GTK4 best practices and enables keyboard shortcut customization.

### 6. APK drag-and-drop: `GtkDropTarget` on the main window

Register a `GtkDropTarget` controller on the main window accepting `text/uri-list` MIME type. On drop, filter for `.apk` files and invoke `nux-core::adb::install_apk()`. Show progress via an `AdwToast` in the window's `AdwToastOverlay`.

### 7. Window state persistence: TOML config file

Store window geometry (width, height, x, y, maximized) in the existing Nux TOML config file managed by `nux-core::config`. Load on startup, save on `close-request` signal. This avoids a GSettings schema dependency (which requires system-level installation).

**Alternative considered:** GSettings. Rejected — requires `.gschema.xml` compilation and installation into system paths, complicating packaging. TOML config is already the project standard.

## Risks / Trade-offs

- **GtkGLArea performance on X11** → GtkGLArea may have higher latency on X11 due to GLX sync. Mitigation: profile early on X11; the dmabuf path can be prioritized if needed.
- **Sidebar toolbar discoverability** → Users unfamiliar with emulators may not recognize icon-only buttons. Mitigation: use tooltips on all buttons; add optional labels in a future iteration.
- **Settings requiring VM restart** → Changing CPU/RAM while VM is running requires restart. Mitigation: clearly label these settings and show a restart-required banner via `AdwBanner`.
- **libadwaita version coupling** → libadwaita 0.7.x maps to GNOME 48+. Older distros may not have it. Mitigation: document minimum versions; Flatpak bundles the runtime.
- **Display aspect ratio edge cases** → Rotation changes (portrait ↔ landscape) require dynamic resize of the display widget. Mitigation: handle `notify::resolution` from the display pipeline and trigger `queue_resize()`.

## Open Questions

1. Should the toolbar be on the right or left side? Right matches Bluestacks/LDPlayer convention — going with right unless user testing says otherwise.
2. Should settings changes that require VM restart auto-restart or just notify? Starting with notify + manual restart for safety.
3. FPS counter display — always visible or toggle-able? Leaning toward toggle via header bar button or `app.toggle-fps` action.
