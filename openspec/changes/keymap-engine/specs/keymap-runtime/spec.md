## ADDED Requirements

### Requirement: Load keymap from file path
The keymap engine SHALL provide a function to load a keymap from a TOML file path. The function SHALL read the file, parse it, validate all fields and bindings, and return a `Keymap` struct or an error.

#### Scenario: Successful load from valid file
- **WHEN** `load_keymap("/path/to/pubg.toml")` is called with a valid keymap file
- **THEN** the function SHALL return a fully populated `Keymap` struct

#### Scenario: File not found
- **WHEN** `load_keymap("/nonexistent/path.toml")` is called
- **THEN** the function SHALL return an error indicating the file was not found

#### Scenario: File contains invalid TOML
- **WHEN** `load_keymap` is called with a file containing malformed TOML syntax
- **THEN** the function SHALL return a parse error with the line number of the syntax error

### Requirement: KeymapEngine holds active keymap
The `KeymapEngine` struct SHALL hold the currently active keymap behind a shared, thread-safe reference (`Arc<RwLock<Keymap>>`). It SHALL be constructible with an initial keymap or in an empty state (no active keymap).

#### Scenario: Create engine with initial keymap
- **WHEN** a `KeymapEngine` is constructed with a loaded `Keymap`
- **THEN** the engine SHALL have that keymap as the active keymap and be ready to translate events

#### Scenario: Create engine with no keymap
- **WHEN** a `KeymapEngine` is constructed without a keymap
- **THEN** the engine SHALL pass through all input events untranslated

### Requirement: Hot-switch keymap at runtime
The keymap engine SHALL support replacing the active keymap at runtime without restarting the VM. The switch SHALL be atomic — no events are lost or partially translated during the swap.

#### Scenario: Switch keymap while engine is active
- **WHEN** `engine.switch_keymap(new_keymap)` is called while the engine is processing events
- **THEN** the engine SHALL atomically replace the active keymap and all subsequent events SHALL be translated using the new keymap

#### Scenario: Switch to no keymap (disable)
- **WHEN** `engine.clear_keymap()` is called
- **THEN** the engine SHALL stop translating events and pass all input through untranslated

#### Scenario: Binding state reset on switch
- **WHEN** a keymap switch occurs while a joystick binding has active directional state
- **THEN** the engine SHALL release all active touch contacts from the previous keymap before activating the new one

### Requirement: Coordinate scaling on resolution change
The keymap engine SHALL scale binding coordinates when the display resolution differs from the keymap's authored resolution. Scale factors SHALL be computed as `(display_width / keymap_width, display_height / keymap_height)` and cached until the resolution changes again.

#### Scenario: Display matches keymap resolution
- **WHEN** the display resolution is `1080x1920` and the keymap's `resolution = [1080, 1920]`
- **THEN** binding coordinates SHALL be used without modification

#### Scenario: Display resolution is doubled
- **WHEN** the display resolution is `2160x3840` and the keymap's `resolution = [1080, 1920]`
- **THEN** a tap binding at `(540, 1400)` SHALL produce a touch event at `(1080, 2800)`

#### Scenario: Resolution changes at runtime
- **WHEN** the display resolution changes from `1080x1920` to `1440x2560` while a keymap is active
- **THEN** the engine SHALL recompute scale factors and apply them to all subsequent translations

### Requirement: Keymap engine exposes overlay data
The keymap engine SHALL provide a method to retrieve overlay data for the active keymap. Overlay data SHALL be a list of `(label: String, x: f64, y: f64)` tuples representing key hint positions, already scaled to the current display resolution.

#### Scenario: Retrieve overlay data for active keymap
- **WHEN** `engine.overlay_data()` is called with an active keymap containing 5 bindings
- **THEN** the method SHALL return 5 overlay entries with labels and scaled coordinates

#### Scenario: Overlay data with no active keymap
- **WHEN** `engine.overlay_data()` is called with no active keymap
- **THEN** the method SHALL return an empty list
