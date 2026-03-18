## Context

Nux Emulator runs Android games inside a KVM-accelerated VM (crosvm) with GPU passthrough via gfxstream. Users interact through keyboard and mouse, but Android games expect multi-touch input at specific screen coordinates. The input-system change (dependency) establishes the raw event capture and touch injection pipeline. This design covers the keymap engine that sits between those two layers — consuming raw keyboard/mouse events and producing synthetic Android touch events.

Currently there is no keymap infrastructure. The `keymaps/` directory exists in the workspace but is empty. The `nux-core` crate has no `keymap` module yet.

## Goals / Non-Goals

**Goals:**
- Define a clean, extensible architecture for the keymap engine within `nux-core::keymap`
- Support all five binding types (tap, long-press, swipe, joystick, aim) with correct multi-touch semantics
- Enable runtime keymap loading and hot-switching without VM restart
- Render key hint overlays in the GTK4 display surface
- Ship pre-built keymaps for popular games

**Non-Goals:**
- Visual drag-and-drop keymap editor (v2)
- Gamepad/controller binding support
- Per-game auto-detection or cloud sharing
- Android-side input injection (handled by input-system change)

## Decisions

### 1. Keymap file format: TOML with strict schema validation

**Choice:** TOML with serde deserialization into strongly-typed Rust structs, validated at load time.

**Alternatives considered:**
- JSON: More verbose, no comments, worse for hand-editing.
- YAML: Rust ecosystem support is weaker; indentation-sensitive format is error-prone for users.
- Custom DSL: Unnecessary complexity for a configuration file.

**Rationale:** TOML is already the project's config format, the `toml` crate is likely already a workspace dependency, and it supports comments — important since v1 users will hand-edit keymaps.

### 2. Architecture: `KeymapEngine` as a stateful event transformer

**Choice:** A `KeymapEngine` struct in `nux-core::keymap` that holds the active keymap, maintains binding state machines (e.g., joystick direction tracking, long-press timers), and exposes a `fn translate(&mut self, event: InputEvent) -> Vec<TouchEvent>` method.

**Alternatives considered:**
- Stateless function mapping: Cannot handle joystick (continuous directional state), long-press (timer-based), or aim (accumulated delta tracking).
- Trait-based polymorphic bindings: Each binding type implements a `Binding` trait with `fn handle(&mut self, event) -> Vec<TouchEvent>`. This is the internal design — `KeymapEngine` dispatches to trait objects.

**Rationale:** Binding types have fundamentally different state requirements. A trait-based dispatch inside a stateful engine gives extensibility (new binding types) without leaking complexity to callers.

### 3. Coordinate scaling: Linear scaling at translation time

**Choice:** Store bindings in authored resolution coordinates. Scale at event translation time using the ratio `(display_width / keymap_width, display_height / keymap_height)`. Recompute scale factors only when resolution changes.

**Alternatives considered:**
- Pre-scale on load: Requires reloading the keymap on every resolution change.
- Normalized coordinates (0.0–1.0): Loses precision for users authoring keymaps; harder to reason about.

**Rationale:** Linear scaling is simple, correct for uniform aspect ratios, and defers the aspect-ratio mismatch problem (letterboxing) to a future enhancement. Scale factors are cached and cheap to apply per-event.

### 4. Hot-switching: Atomic swap via `Arc<RwLock<Keymap>>`

**Choice:** The active keymap is held behind `Arc<RwLock<Keymap>>`. Hot-switch replaces the inner value under a write lock. The overlay widget holds a read handle.

**Alternatives considered:**
- Channel-based message passing: More complex, unnecessary when reads vastly outnumber writes.
- `ArcSwap`: Good fit but adds a dependency for a simple use case.

**Rationale:** `RwLock` is zero-dependency, allows concurrent reads from the translation thread and overlay renderer, and write contention is negligible (switches are rare user actions).

### 5. Overlay rendering: GTK4 `DrawingArea` overlay with Cairo

**Choice:** A transparent `gtk4::DrawingArea` layered on top of the game display surface in `nux-ui`. It reads the active keymap's binding positions and draws semi-transparent key labels using Cairo.

**Alternatives considered:**
- Snapshot-based custom widget: More idiomatic GTK4 but heavier for simple text labels.
- CSS overlay with GTK labels: Positioning absolute labels over a dynamic surface is fragile.

**Rationale:** `DrawingArea` with Cairo gives pixel-precise control over label placement, matches the coordinate system of the keymap bindings, and is lightweight. The overlay redraws only on keymap switch or resolution change.

### 6. Module structure

```
nux-core/src/keymap/
├── mod.rs          // public API: KeymapEngine, load/switch functions
├── format.rs       // TOML schema types, serde deserialization, validation
├── binding.rs      // Binding trait + implementations (tap, long_press, swipe, joystick, aim)
├── scaling.rs      // Coordinate scaling logic
└── overlay.rs      // OverlayData struct (shared with nux-ui, no GTK deps)

nux-ui/src/overlay/
└── keymap_overlay.rs  // GTK4 DrawingArea widget that renders key hints
```

`nux-core` exposes an `OverlayData` struct (label + position pairs) so the UI crate can render without depending on keymap internals.

## Risks / Trade-offs

- **[Aspect ratio mismatch]** → Linear scaling assumes matching aspect ratios. If the display aspect ratio differs from the keymap's authored ratio, coordinates will be distorted. **Mitigation:** Document that keymaps should match the VM display ratio. Letterbox-aware scaling can be added later without breaking the format.

- **[Input-system dependency]** → The keymap engine depends on event types and the touch injection API from the input-system change. If that API changes, the keymap engine must adapt. **Mitigation:** Define a minimal `InputEvent`/`TouchEvent` interface in the design; coordinate with input-system on type stability.

- **[Long-press timing accuracy]** → Long-press detection requires a timer. Using `std::time::Instant` polling in the translation loop may have jitter. **Mitigation:** Acceptable for v1. If precision matters, move to a dedicated async timer (tokio/glib timeout) in a future iteration.

- **[Multi-touch slot management]** → Android multi-touch protocol uses numbered slots. The keymap engine must allocate and release slots correctly when multiple bindings are active simultaneously (e.g., joystick + tap). **Mitigation:** Implement a simple slot allocator inside `KeymapEngine` with a fixed pool (10 slots matches Android's typical limit).

## Open Questions

1. Should the keymap engine own its own thread, or run synchronously in the input-system's event loop? (Leaning toward synchronous — translation is fast and avoids cross-thread latency.)
2. What is the exact `TouchEvent` struct shape from the input-system change? Need to coordinate on the shared type.
3. Should the overlay toggle be a global hotkey or a toolbar button? (Likely both, but UX decision needed.)
