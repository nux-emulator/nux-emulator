## 1. Module Structure and Types

- [ ] 1.1 Create `nux-core/src/keymap/mod.rs` with the module skeleton: declare submodules (`format`, `binding`, `scaling`, `overlay`), re-export public API types (`KeymapEngine`, `Keymap`, `OverlayData`). Verify the crate compiles with empty submodules.
- [ ] 1.2 Add `toml` crate to `[workspace.dependencies]` in root `Cargo.toml` if not already present. Add `toml` as a dependency in `nux-core/Cargo.toml` using workspace inheritance. Verify `cargo check -p nux-core` passes.
- [ ] 1.3 Define core types in `nux-core/src/keymap/format.rs`: `KeymapMeta` (name, game_package, resolution), `Binding` enum (Tap, LongPress, Swipe, Joystick, Aim) with per-variant fields, and the top-level `Keymap` struct. Derive `serde::Deserialize` for all types. Verify with a unit test that deserializes a minimal TOML string into a `Keymap`.

## 2. Keymap Parsing and Validation

- [ ] 2.1 Implement TOML deserialization in `format.rs`: `pub fn parse_keymap(toml_str: &str) -> Result<Keymap, KeymapError>`. Handle missing fields, unknown binding types, and malformed TOML with descriptive errors. Write unit tests for valid input, missing meta fields, and unknown binding type.
- [ ] 2.2 Implement validation in `format.rs`: `pub fn validate_keymap(keymap: &Keymap) -> Result<(), KeymapError>`. Check duplicate key assignments, invalid key names (against a known key name set), joystick must have exactly 4 keys, radius/duration must be positive, aim region must be valid (x1 < x2, y1 < y2). Write unit tests for each validation rule.
- [ ] 2.3 Implement `pub fn load_keymap(path: &Path) -> Result<Keymap, KeymapError>` in `mod.rs` that reads a file, calls `parse_keymap`, then `validate_keymap`. Write tests for file-not-found and invalid-TOML-syntax error paths.

## 3. Coordinate Scaling

- [ ] 3.1 Implement `ScaleFactors` struct in `nux-core/src/keymap/scaling.rs` with `fn new(keymap_res: (u32, u32), display_res: (u32, u32)) -> Self` and `fn scale(&self, x: i32, y: i32) -> (i32, i32)`. Cache the ratio. Write unit tests: identity (same resolution), 2x scaling, non-uniform scaling.
- [ ] 3.2 Add `fn update_resolution(&mut self, display_res: (u32, u32))` to recompute cached scale factors. Write a test that changes resolution and verifies subsequent `scale()` calls use the new factors.

## 4. Binding Translation — Stateless Types

- [ ] 4.1 Define the `Binding` trait in `nux-core/src/keymap/binding.rs`: `fn handle(&mut self, event: &InputEvent, scale: &ScaleFactors) -> Vec<TouchEvent>` and `fn reset(&mut self) -> Vec<TouchEvent>` (for releasing active touches on keymap switch). Define `InputEvent` and `TouchEvent` structs (or reference input-system types).
- [ ] 4.2 Implement `TapHandler`: on key-down emit touch-down at scaled coords, on key-up emit touch-up. Write unit tests with synthetic key press/release events verifying correct touch events and coordinates.
- [ ] 4.3 Implement `LongPressHandler`: same as tap (touch-down on press, touch-up on release). The `duration_ms` is stored but not enforced by the engine (it's informational). Write unit tests verifying hold behavior.

## 5. Binding Translation — Stateful Types

- [ ] 5.1 Implement `SwipeHandler`: on key-down, start a swipe animation from `from` to `to` over `duration_ms`. Expose `fn tick(&mut self, elapsed_ms: u64) -> Vec<TouchEvent>` to produce interpolated touch-move events. Emit touch-up at `to` when complete. Write unit tests for full swipe, and verify fire-and-forget (early key release doesn't cancel).
- [ ] 5.2 Implement `JoystickHandler`: track which of the 4 directional keys are held, compute the target point on the circle edge (single direction or diagonal at 45°). Emit touch-down at center on first key, touch-move on direction change, touch-up when all released. Write unit tests for single direction, diagonal, direction change, and full release.
- [ ] 5.3 Implement `AimHandler`: accumulate mouse deltas scaled by sensitivity, clamp to region bounds, emit touch-move events. Handle activation/deactivation (touch-down/touch-up). Write unit tests for basic movement, clamping at region boundary, and deactivation.

## 6. Touch Slot Allocator

- [ ] 6.1 Implement `SlotAllocator` in `binding.rs` (or a new `slots.rs`): pool of 10 slots, `fn allocate() -> Option<u32>`, `fn release(slot: u32)`. Write unit tests for allocate/release cycle, exhaustion (returns `None` after 10), and reuse after release.

## 7. KeymapEngine Core

- [ ] 7.1 Implement `KeymapEngine` struct in `mod.rs`: holds `Arc<RwLock<Option<Keymap>>>`, `ScaleFactors`, `SlotAllocator`, and a `HashMap<BindingId, Box<dyn BindingHandler>>`. Constructor takes optional `Keymap` and initial display resolution.
- [ ] 7.2 Implement `fn translate(&mut self, event: InputEvent) -> Vec<TouchEvent>`: look up binding for the event's key, dispatch to the handler, assign/release slots. Pass through unbound keys. Write unit tests with a loaded keymap: bound key produces touch events, unbound key passes through.
- [ ] 7.3 Implement `fn switch_keymap(&mut self, keymap: Option<Keymap>)`: reset all active handlers (call `reset()` to release touches), replace the keymap, rebuild handler map. Write test verifying active touches are released on switch.
- [ ] 7.4 Implement `fn update_resolution(&mut self, width: u32, height: u32)`: update `ScaleFactors`. Write test verifying translations use new scale after resolution change.

## 8. Overlay Data

- [ ] 8.1 Define `OverlayEntry` struct in `nux-core/src/keymap/overlay.rs`: `label: String`, `x: f64`, `y: f64`, `kind: OverlayKind` (enum: Point, Circle { radius }, Region { x2, y2 }). Implement `fn overlay_data(&self) -> Vec<OverlayEntry>` on `KeymapEngine` that iterates bindings and produces scaled overlay entries. Write unit tests for each binding type's overlay representation.

## 9. Keymap Overlay Widget (nux-ui)

- [ ] 9.1 Create `nux-ui/src/overlay/keymap_overlay.rs`: a GTK4 `DrawingArea` subclass (or wrapper) that takes a `Vec<OverlayEntry>` and draws semi-transparent key labels using Cairo. Render point labels for tap/long-press/swipe, circle + directional labels for joystick, rectangle for aim region.
- [ ] 9.2 Implement overlay visibility toggle: add a toolbar button and keyboard shortcut in the main window that shows/hides the overlay widget. Wire the toggle to the overlay's `set_visible()`.
- [ ] 9.3 Wire overlay updates: subscribe to keymap-switch and resolution-change signals. On either event, call `engine.overlay_data()` and pass the result to the overlay widget for redraw. Verify overlay updates when switching keymaps.

## 10. Built-in Keymaps

- [ ] 10.1 Create `keymaps/pubg-mobile.toml` with meta (`name`, `game_package = "com.tencent.ig"`, `resolution = [1080, 1920]`) and bindings for movement (joystick WASD), shooting (tap), aiming (aim), and common actions (tap/swipe for reload, crouch, etc.).
- [ ] 10.2 Create at least 2 more keymap files for popular games (e.g., `call-of-duty-mobile.toml`, `genshin-impact.toml`) with appropriate bindings.
- [ ] 10.3 Implement `pub fn list_builtin_keymaps(keymaps_dir: &Path) -> Result<Vec<KeymapMeta>, KeymapError>` that reads all `.toml` files in the directory, parses only the `[meta]` section, and returns the list. Write unit tests with a temp directory containing sample files.

## 11. Integration and Verification

- [ ] 11.1 Write an integration test that loads a built-in keymap, creates a `KeymapEngine`, sends a sequence of synthetic input events (tap, joystick combo, aim movement), and asserts the correct touch event output including slot IDs and scaled coordinates.
- [ ] 11.2 Write a validation test that runs `load_keymap` on every file in `keymaps/` and asserts they all parse and validate successfully.
- [ ] 11.3 Run `cargo clippy -p nux-core -p nux-ui` and `cargo test --workspace` — fix any warnings or failures. Verify the full workspace builds cleanly.
