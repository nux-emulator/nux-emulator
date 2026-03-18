## 1. Module scaffolding and configuration

- [x] 1.1 Create `nux-core/src/display/` module structure: `mod.rs`, `capture.rs`, `sync.rs`, `config.rs`, `error.rs`. Wire `pub mod display` into `nux-core/src/lib.rs`. Verify it compiles with empty modules.
- [x] 1.2 Define `DisplayConfig` struct in `config.rs` with fields: resolution (width/height), scaling_mode (Contain/Integer enum), vsync (bool), fps_overlay (bool). Implement `Default` (1080p, Contain, vsync=true, fps_overlay=false). Add serde `Deserialize`. Add `[display]` section to the Nux TOML config schema.
- [x] 1.3 Define `DisplayError` enum in `error.rs` covering: config validation errors, dmabuf import failure, shared memory map failure, frame channel closed. Implement `std::fmt::Display` and `std::error::Error`.
- [x] 1.4 Add workspace dependencies needed: `wayland-client`, `nix` (features: mman, fcntl), `libc`. Verify workspace builds cleanly.

## 2. Frame capture — dmabuf path

- [x] 2.1 Define `FrameCapture` trait in `capture.rs` with an async method to start capture that sends frames into a `tokio::sync::watch::Sender`. Define a `Frame` enum with variants `Dmabuf { fd, width, height, stride, fourcc }` and `Shm { data: Vec<u8>, width, height, stride }`.
- [x] 2.2 Implement `DmabufCapture` struct that receives dmabuf FDs from crosvm's gfxstream surfaceless output. Wrap raw FDs in an `OwnedFd` safe abstraction with scoped `#[allow(unsafe_code)]` and drop cleanup. Send `Frame::Dmabuf` into the watch channel.
- [x] 2.3 Implement dmabuf capability detection: attempt a test dmabuf import at startup, return `Ok(())` if supported or `Err(DisplayError)` if not. Log the result at info level.
- [x] 2.4 Write unit tests for `DmabufCapture`: test FD ownership/drop, test watch channel delivery replaces stale frames, test detection returns error on mock failure.

## 3. Frame capture — shared memory fallback

- [x] 3.1 Implement `ShmCapture` struct that maps crosvm's shared memory rendering region via `mmap` (using `nix::sys::mman`). Read frame data on each render completion and send `Frame::Shm` into the watch channel.
- [x] 3.2 Implement auto-detection logic in `capture.rs`: try `DmabufCapture::detect()` first; on failure, fall back to `ShmCapture`. Expose a `CaptureBackend` enum (Dmabuf/Shm) queryable at runtime. Log the selected path at info level.
- [x] 3.3 Write unit tests for `ShmCapture`: test mmap/munmap lifecycle, test frame delivery through watch channel, test fallback selection logic.

## 4. Frame presentation widget

- [ ] 4.1 Create `nux-ui/src/display_widget.rs`. Define `NuxDisplayWidget` as a GTK4 composite widget (subclass of `gtk::Widget`). Wire it into `nux-ui/src/main.rs` or the app module. Verify it renders an empty black surface.
- [ ] 4.2 Implement dmabuf frame presentation: receive `Frame::Dmabuf` from the watch channel receiver, create `GdkDmabufTextureBuilder`, build a `GdkTexture`, and set it on an internal `GtkPicture` with `content-fit = contain`.
- [ ] 4.3 Implement shared memory frame presentation: receive `Frame::Shm`, upload pixel data to a GL texture inside a `GtkGLArea`, render to screen. Switch between `GtkPicture` and `GtkGLArea` based on active capture backend.
- [ ] 4.4 Implement aspect ratio preservation: configure `GtkPicture` content-fit for contain mode. Verify letterboxing (tall window) and pillarboxing (wide window) with black fill.
- [ ] 4.5 Implement integer scaling mode: calculate largest integer multiple of native resolution that fits the widget, set explicit size request, center within the container. Fall back to contain if widget is smaller than native resolution.
- [ ] 4.6 Implement resolution change handling: detect when incoming frame dimensions differ from the current resolution, update scaling calculations dynamically without restarting the pipeline. Test with a simulated resolution switch.

## 5. VSync and frame pacing

- [x] 5.1 Implement VSync-aligned presentation in `sync.rs`: register a `add_tick_callback` on the display widget. On each tick, check the watch channel for a new frame; if available, present it. If no new frame, skip redraw.
- [x] 5.2 Implement VSync-disabled mode: when `vsync = false` in config, call `queue_draw` immediately on frame receipt instead of waiting for the tick callback. Wire the toggle to `DisplayConfig.vsync`.
- [x] 5.3 Write integration test: verify that with VSync enabled, frames are only presented on tick boundaries; verify that intermediate frames are dropped when capture outpaces display.

## 6. FPS counter overlay

- [ ] 6.1 Implement FPS counter as a `GtkLabel` overlay positioned top-left over the display widget with semi-transparent background. Track unique frames presented in a rolling one-second window. Update the label text once per second.
- [ ] 6.2 Wire FPS overlay visibility to `DisplayConfig.fps_overlay`. Implement runtime toggle via a public method on `NuxDisplayWidget` that shows/hides the overlay without restarting the pipeline.
- [ ] 6.3 Test FPS counter accuracy: verify displayed value matches actual presented frame count, not tick rate.

## 7. Fullscreen support

- [ ] 7.1 Implement fullscreen toggle: call `GtkWindow::fullscreen()` / `unfullscreen()`. On enter, hide UI chrome (toolbar, status bar) and let display widget fill the window. On exit, restore chrome.
- [ ] 7.2 Wire F11 keybinding (or configured shortcut) to the fullscreen toggle action.
- [ ] 7.3 Verify scaling adaptation: confirm that scaling recalculates correctly for full monitor resolution on enter and restored window size on exit, using the active scaling mode.
- [ ] 7.4 Verify no frame drop during transitions: test that frames continue presenting without a visible black flash or gap when toggling fullscreen.

## 8. Integration and wiring

- [x] 8.1 Wire `DisplayPipeline` orchestrator in `display/mod.rs`: initialize `DisplayConfig`, run capture path detection, start the selected `FrameCapture`, create the watch channel, and expose the receiver for the UI layer.
- [ ] 8.2 Integrate with `nux-core::vm`: after `VmManager` starts crosvm, initialize `DisplayPipeline` with the crosvm process handle to access dmabuf FDs or shared memory region.
- [ ] 8.3 End-to-end smoke test: start crosvm with a test Android image, verify frames appear in the GTK4 window via dmabuf path (or shm fallback), toggle fullscreen, toggle FPS overlay, resize window and confirm aspect ratio preservation.
