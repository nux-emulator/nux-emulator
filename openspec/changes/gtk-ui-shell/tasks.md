## 1. Crate Scaffolding and Application Bootstrap

- [x] 1.1 Create `nux-ui/` crate directory with `Cargo.toml` (depends on `gtk4`, `libadwaita`, `nux-core`) and `src/main.rs` stub. Add `nux-ui` to workspace `Cargo.toml` members. Verify `cargo check -p nux-ui` passes.
- [x] 1.2 Implement `AdwApplication` bootstrap in `main.rs`: create `adw::Application` with app ID `com.nuxemu.Nux`, connect `activate` signal, call `application.run()`. Verify the app launches and shows an empty window.
- [x] 1.3 Add single-instance enforcement: verify that launching a second process activates the existing window via D-Bus. Add `open` signal handler for CLI file arguments. Verify second launch raises existing window.

## 2. Main Window Layout

- [x] 2.1 Create `window.rs` module with `NuxWindow` subclass of `AdwApplicationWindow`. Set minimum size 800x600. Add `AdwHeaderBar` with placeholder title. Verify window displays with header bar.
- [x] 2.2 Build the horizontal layout: `GtkBox` (horizontal) containing a central `GtkOverlay` (expanding) and a right-side vertical `GtkBox` toolbar (fixed width ~48px). Verify layout renders correctly and resizes properly.
- [x] 2.3 Add `AdwToastOverlay` wrapping the main content area. Verify toasts can be displayed programmatically.
- [x] 2.4 Implement header bar VM status label (left side) and FPS counter label (right side). Wire `app.toggle-fps` action to show/hide FPS label. Verify status label updates and FPS toggles.

## 3. Display Area

- [x] 3.1 Create `display.rs` module with a `GtkGLArea` widget placed inside the central `GtkOverlay`. Implement `render` signal handler as a placeholder (clear to black). Verify GL area renders black.
- [x] 3.2 Implement aspect-ratio-preserving layout: compute letterbox bars based on Android surface dimensions vs widget allocation. Verify correct letterboxing when window is resized to various aspect ratios.
- [x] 3.3 Add placeholder/logo display when VM is stopped (draw Nux logo or "Start VM" text in the GL area). Verify placeholder shows when no frames are available.

## 4. Sidebar Toolbar

- [x] 4.1 Create `toolbar.rs` module. Build vertical `GtkBox` with icon buttons: Screenshot, Volume Up, Volume Down, Shake, Rotate, Install APK, Keymap Overlay toggle, Settings, Fullscreen. Use appropriate icon names. Verify all buttons render in the sidebar.
- [x] 4.2 Register `gio::SimpleAction` for each toolbar action on the window (`win.screenshot`, `win.volume-up`, `win.volume-down`, `win.shake`, `win.rotate`, `win.install-apk`, `win.toggle-keymap-overlay`, `win.open-settings`, `win.toggle-fullscreen`). Wire buttons to actions. Verify actions fire on click (log to stdout).
- [x] 4.3 Implement VM-state-dependent action sensitivity: disable screenshot, volume, shake, rotate actions when VM is stopped. Enable when VM is running. Verify buttons gray out/enable based on mock VM state.

## 5. Fullscreen Mode

- [x] 5.1 Implement `win.toggle-fullscreen` action: call `window.fullscreen()` / `window.unfullscreen()`. Hide header bar and sidebar on enter, restore on exit. Verify fullscreen toggle works with `Ctrl+F`, `F11`, and toolbar button.
- [x] 5.2 Implement sidebar reveal-on-hover in fullscreen: add a `GtkEventControllerMotion` on the right edge that temporarily shows the toolbar as an overlay. Verify hover reveals toolbar in fullscreen.
- [x] 5.3 Implement `Escape` key to exit fullscreen. Verify pressing Escape in fullscreen returns to windowed mode.

## 6. Keyboard Shortcuts

- [x] 6.1 Register application-level keyboard shortcuts: `Ctrl+Q` (quit), `Ctrl+F` / `F11` (fullscreen), and any other shortcuts defined in the app-shell spec. Verify each shortcut triggers the correct action.
- [x] 6.2 Add application menu (hamburger or primary menu button in header bar) with entries: About, Keyboard Shortcuts, Quit. Wire to corresponding actions. Verify menu items work.

## 7. Settings Dialog

- [x] 7.1 Create `settings.rs` module with `AdwPreferencesWindow` subclass. Add `win.open-settings` action that presents it as modal. Verify dialog opens and closes from toolbar and menu.
- [x] 7.2 Implement Performance page: `AdwPreferencesPage` with `AdwSpinRow` for CPU cores (1–host max), `AdwComboRow` for RAM (2/4/6/8 GB), `AdwSpinRow` for DPI (120–640, step 20). Wire to config read/write. Verify values load from and save to config.
- [x] 7.3 Implement Root page: `AdwComboRow` for root mode (None/Magisk/KernelSU/APatch), `AdwActionRow` showing boot.img patch status. Wire to config. Verify mode selection persists and status displays.
- [x] 7.4 Implement Google Services page: `AdwComboRow` for MicroG/GApps/None. Wire to config. Verify selection persists.
- [x] 7.5 Implement Device page: `AdwComboRow` for device model (Pixel 9, Samsung S24, Custom), `AdwComboRow` for default orientation (Portrait/Landscape). Wire to config. Verify selections persist.
- [x] 7.6 Implement Display page: `AdwComboRow` for resolution presets (720x1280, 1080x1920, 1440x2560, Custom), conditional width/height `AdwSpinRow` entries for custom resolution (min 320x480, max 3840x2160). Wire to config. Verify preset and custom resolution persist.
- [x] 7.7 Implement About page: `AdwPreferencesPage` with app name, version, license (GPLv3), project links. Verify page displays correct information.
- [x] 7.8 Implement restart-required `AdwBanner`: show banner when CPU, RAM, DPI, resolution, or Google Services settings change while VM is running. Hide after VM restart. Verify banner appears and dismisses correctly.

## 8. APK Drag-and-Drop

- [x] 8.1 Register `GtkDropTarget` on the main window accepting `text/uri-list`. Filter for `.apk` file extensions. On drop, call ADB install (stub/trait for now). Verify drop of `.apk` triggers install path and non-APK files are rejected with toast.
- [x] 8.2 Implement drag-over visual feedback: show an overlay with "Drop APK to install" text and icon during drag-over, hide on drag-leave. Verify overlay appears/disappears during drag.
- [x] 8.3 Implement install progress indication: show spinner toast during install, success/failure toast on completion. Reject drops when VM is stopped with appropriate toast. Verify progress and result toasts display.

## 9. Keymap Overlay

- [x] 9.1 Create `overlay.rs` module with a `GtkOverlay` child widget that renders keymap hint labels at configured positions. Wire `win.toggle-keymap-overlay` to toggle visibility. Verify overlay shows/hides on toggle.

## 10. Window State Persistence

- [x] 10.1 Implement window state save: on `close-request`, write width, height, and maximized state to `[ui.window]` in the TOML config via `nux-core::config`. If fullscreen, save pre-fullscreen dimensions. Verify config file is written on close.
- [x] 10.2 Implement window state restore: on startup, read `[ui.window]` from config, apply dimensions (default 1024x768 if absent), validate against display bounds. Verify window restores saved size and falls back to defaults.

## 11. Graceful Shutdown

- [x] 11.1 Implement `close-request` handler: if an APK install is in progress, show a confirmation `AdwAlertDialog` before closing. Otherwise, save state and exit. Verify confirmation dialog appears during active install and clean exit otherwise.

## 12. Integration Wiring

- [x] 12.1 Wire display widget to `nux-core` display pipeline trait (consume frames from the pipeline, render via GL). Verify frames from a mock display source render in the GL area.
- [x] 12.2 Wire toolbar actions to `nux-core` APIs: screenshot → display pipeline capture, volume/shake/rotate → input system, APK install → ADB bridge. Verify each action calls the correct `nux-core` interface.
- [x] 12.3 Wire settings dialog to `nux-core::config`: ensure all preference changes read/write through the config system. Verify round-trip: change setting → close → reopen → value persisted.
