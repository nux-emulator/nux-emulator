## 1. Module Structure and evdev Types

- [x] 1.1 Create `nux-core/src/input/mod.rs` module with submodule declarations (`evdev`, `translate`, `grab`, `coordinate`, `manager`) and add `pub mod input;` to `nux-core/src/lib.rs`
- [x] 1.2 Define `#[repr(C)]` `InputEvent` struct in `nux-core/src/input/evdev.rs` matching Linux `input_event` layout (16 bytes on x86_64), with `to_bytes()` serialization and constants for `EV_KEY`, `EV_ABS`, `EV_REL`, `EV_SYN`, `SYN_REPORT`, `BTN_TOUCH`, `KEY_BACK`, `ABS_X`, `ABS_Y`, `ABS_MT_*`, `REL_X`, `REL_Y`, `REL_WHEEL`, `REL_HWHEEL`
- [x] 1.3 Write unit tests verifying `InputEvent` byte layout matches Linux ABI (size = 16, field offsets correct)

## 2. Coordinate Mapping

- [x] 2.1 Implement `DisplayMetrics` struct in `nux-core/src/input/coordinate.rs` with guest/host dimensions, scale factor, and letterbox offsets. Wrap in `Arc<Mutex<>>` for shared access
- [x] 2.2 Implement `map_host_to_guest(host_x, host_y, metrics) -> (i32, i32)` that subtracts letterbox offsets, divides by scale factor, and clamps to guest bounds
- [x] 2.3 Write unit tests for coordinate mapping: uniform scaling, letterboxing, combined scaling+letterbox, edge clamping

## 3. Input Translation

- [x] 3.1 Implement keyboard translation in `nux-core/src/input/translate.rs`: GTK4 hardware keycode → `EV_KEY` event batch (key event + `SYN_REPORT`)
- [x] 3.2 Implement mouse motion translation: host coordinates → `map_host_to_guest` → `EV_ABS/ABS_X` + `ABS_Y` + `SYN_REPORT`
- [x] 3.3 Implement left-click translation: button press → `BTN_TOUCH` down + position + `SYN_REPORT`; release → `BTN_TOUCH` up + `SYN_REPORT`
- [x] 3.4 Implement right-click translation: button press → `KEY_BACK` press + `SYN_REPORT` + `KEY_BACK` release + `SYN_REPORT`
- [x] 3.5 Implement scroll translation: vertical → `REL_WHEEL` + `SYN_REPORT`; horizontal → `REL_HWHEEL` + `SYN_REPORT`
- [x] 3.6 Write unit tests for each translation function verifying correct evdev event sequences

## 4. Multi-Touch Synthesis

- [x] 4.1 Implement multi-touch state machine in `nux-core/src/input/translate.rs`: track active slots, tracking IDs, and pinch-gesture state (idle, active)
- [x] 4.2 Implement Ctrl+click → dual-slot touch-down: slot 0 at click position, slot 1 at mirrored position, using `ABS_MT_SLOT`, `ABS_MT_TRACKING_ID`, `ABS_MT_POSITION_X/Y`
- [x] 4.3 Implement Ctrl+drag → symmetric movement of both slots around screen center
- [x] 4.4 Implement gesture end (Ctrl release or mouse release) → `ABS_MT_TRACKING_ID = -1` for both slots
- [x] 4.5 Write unit tests for pinch-zoom event sequences: initiation, drag, release, and slot cleanup

## 5. Input Grab

- [x] 5.1 Implement `InputGrabState` in `nux-core/src/input/grab.rs` with `Grabbed`/`Free` enum and toggle logic
- [x] 5.2 Implement relative motion computation: when grabbed, convert absolute positions to deltas → `EV_REL/REL_X` + `REL_Y` events
- [x] 5.3 Write unit tests for grab state transitions and relative delta calculation

## 6. Input Manager and Socket Injection

- [x] 6.1 Implement `InputManager` in `nux-core/src/input/manager.rs`: holds `UnixStream` to virtio-input socket, `DisplayMetrics` ref, `InputGrabState`, and multi-touch state
- [x] 6.2 Implement `InputManager::inject(events: &[InputEvent])` that serializes and writes event batches to the socket, logging errors on write failure without panicking
- [x] 6.3 Implement public API methods on `InputManager`: `handle_key`, `handle_motion`, `handle_click`, `handle_scroll`, `toggle_grab`
- [x] 6.4 Write integration test with a mock Unix socket verifying end-to-end event flow (key press → socket bytes)

## 7. GTK4 Event Controller Hookup (nux-ui)

- [ ] 7.1 In `nux-ui`, attach `EventControllerKey` to the drawing surface and wire key press/release callbacks to `InputManager::handle_key`
- [ ] 7.2 Attach `EventControllerMotion` and wire motion callback to `InputManager::handle_motion`
- [ ] 7.3 Attach `GestureClick` for button 1 (left) and button 3 (right), wire to `InputManager::handle_click`
- [ ] 7.4 Attach `EventControllerScroll` and wire scroll callback to `InputManager::handle_scroll`
- [ ] 7.5 Wire grab hotkey (Ctrl+Alt) detection in the key controller to `InputManager::toggle_grab`, and implement cursor hide/show and pointer confinement request on the GTK4 surface
- [ ] 7.6 Verify all controllers only fire when the emulator window has focus (GTK4 default behavior — add a manual test confirming no events when unfocused)
