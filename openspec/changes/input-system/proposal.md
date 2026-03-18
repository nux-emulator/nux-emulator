## Why

Nux Emulator needs to route host input (keyboard, mouse, scroll) into the Android VM so users can actually interact with apps and games. Without an input system, the emulator renders frames but is completely non-interactive. This is a prerequisite for any usable release and blocks all gameplay and app-testing workflows.

## What Changes

- New `nux-core::input` module that captures GTK4 input events and translates them to Linux input events for crosvm's virtio-input device.
- Keyboard input: host key press/release mapped to Android key events via virtio-input.
- Mouse input: pointer movement mapped to Android absolute pointer coordinates; left-click mapped to Android touch; right-click mapped to Android back button.
- Scroll input: mouse wheel events mapped to Android scroll events.
- Multi-touch support: keyboard modifier combos (e.g., Ctrl+click) synthesize pinch-zoom gestures via multi-touch protocol.
- Input grab/release toggle: capture mouse within the emulator window or release to free cursor (toggled via hotkey).
- Coordinate mapping: translate host window coordinates to Android screen coordinates, accounting for display scaling and letterboxing.

## Non-goals

- **Keymap engine**: Custom key remapping and game-specific profiles are a separate change (`keymap-engine`).
- **UI toolbar**: Toolbar buttons and on-screen controls belong to `gtk-ui-shell`.
- **Display rendering**: The display pipeline is handled by `display-pipeline`.
- **Gamepad/controller input**: Deferred to a future change.

## Capabilities

### New Capabilities
- `input-event-capture`: Capture keyboard, mouse, and scroll events from the GTK4 surface.
- `input-translation`: Translate host input events to Linux evdev events for virtio-input injection into the VM.
- `coordinate-mapping`: Map host window coordinates to Android screen coordinates with scaling and letterbox correction.
- `multi-touch`: Synthesize multi-touch gestures (pinch-zoom) from keyboard+mouse combos.
- `input-grab`: Toggle mouse capture/release within the emulator window.

### Modified Capabilities
<!-- No existing spec-level requirements are changing. -->

## Impact

- **Code**: New `nux-core/src/input/` module tree; new GTK4 event controller hookup in `nux-ui`.
- **Dependencies**: Depends on `crosvm-integration` for virtio-input device socket/API. Uses GTK4 `EventControllerKey`, `EventControllerMotion`, `EventControllerScroll`, `GestureClick`.
- **APIs**: Exposes `InputManager` from `nux-core` consumed by `nux-ui` window setup.
- **Systems**: Requires KVM + crosvm with virtio-input enabled. Coordinate mapping must stay in sync with display pipeline's scaling parameters.
