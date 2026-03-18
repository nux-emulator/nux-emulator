## Why

Mobile games are designed for touch input, but emulator users have a keyboard and mouse. Without a translation layer that maps physical inputs to precise screen coordinates, games are unplayable. A keymap engine lets users bind keys to taps, swipes, joystick movements, and mouse-aim regions — making touch-only games fully controllable from a desktop. This is the core feature that separates a gaming emulator from a general-purpose one.

## What Changes

- New `nux-core::keymap` module that loads TOML keymap files, parses binding definitions, and translates keyboard/mouse events into Android touch events at specified screen coordinates.
- Support for five binding types: **tap**, **long-press**, **swipe**, **joystick** (WASD → virtual stick), and **aim** (mouse movement → touch drag within a region).
- Coordinate scaling system that adapts bindings when the VM display resolution differs from the keymap's authored resolution.
- Runtime keymap loading and hot-switching (change keymaps without restarting the VM).
- Pre-built keymap files shipped in `keymaps/` for popular games.
- GTK overlay layer in `nux-ui` that renders semi-transparent key hints on top of the game display, showing active bindings.
- Integration with the input-system change: the keymap engine consumes raw input events and produces translated touch events routed back through the input pipeline.

## Capabilities

### New Capabilities
- `keymap-format`: TOML keymap file schema — meta fields, binding type definitions, validation rules, and resolution metadata.
- `keymap-runtime`: Loading, parsing, validating, hot-switching keymaps at runtime; coordinate scaling when resolution changes.
- `binding-translation`: Translating keyboard/mouse events into Android touch events for each binding type (tap, long-press, swipe, joystick, aim).
- `keymap-overlay`: GTK4 overlay rendering of key hints on the game display surface.
- `builtin-keymaps`: Pre-built keymap files for popular games, shipped in `keymaps/`.

### Modified Capabilities
<!-- No existing spec-level requirements are changing. -->

## Non-goals

- **Visual keymap editor** — drag-and-drop UI for creating/editing keymaps is deferred to v2. V1 is TOML-only.
- **Per-game auto-detection** — automatically selecting a keymap based on the running game package is out of scope.
- **Cloud keymap sharing** — uploading/downloading community keymaps from a server is not planned.
- **Gamepad/controller input** — this change covers keyboard and mouse only.

## Impact

- **nux-core**: New `keymap` module added. Depends on the `input-system` change for raw event consumption and touch event injection.
- **nux-ui**: New overlay widget for rendering key hints. Requires access to the active keymap state from `nux-core`.
- **keymaps/**: New directory with shipped TOML keymap files.
- **Dependencies**: `toml` crate for parsing (likely already in workspace for config). No new external dependencies expected.
- **Testing**: Each binding type needs unit tests with synthetic input sequences. Integration tests require a mock input pipeline.
