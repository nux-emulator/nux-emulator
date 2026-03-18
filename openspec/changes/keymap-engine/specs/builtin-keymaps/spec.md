## ADDED Requirements

### Requirement: Ship pre-built keymaps in keymaps/ directory
The project SHALL include pre-built TOML keymap files in the `keymaps/` directory at the workspace root. Each file SHALL be a valid keymap conforming to the keymap-format specification.

#### Scenario: Pre-built keymaps are valid
- **WHEN** the build or test suite validates all files in `keymaps/*.toml`
- **THEN** every file SHALL parse and validate successfully against the keymap schema

### Requirement: Pre-built keymaps cover popular games
The `keymaps/` directory SHALL include keymap files for at least 3 popular mobile games at initial release. Each keymap SHALL include bindings for the game's core controls.

#### Scenario: PUBG Mobile keymap exists
- **WHEN** the user looks for a PUBG Mobile keymap
- **THEN** a file `keymaps/pubg-mobile.toml` SHALL exist with `game_package = "com.tencent.ig"` and bindings for movement (joystick), shooting (tap), aiming (aim), and common actions (tap/swipe)

#### Scenario: Keymap file naming convention
- **WHEN** a new pre-built keymap is added
- **THEN** the file SHALL be named using kebab-case matching the game name (e.g., `call-of-duty-mobile.toml`)

### Requirement: Pre-built keymaps are discoverable at runtime
The keymap engine SHALL provide a function to list all available pre-built keymaps from the `keymaps/` directory. The function SHALL return the keymap metadata (name, game package) without fully loading all bindings.

#### Scenario: List available keymaps
- **WHEN** `list_builtin_keymaps()` is called and the `keymaps/` directory contains 3 TOML files
- **THEN** the function SHALL return 3 entries with their `name` and `game_package` from the `[meta]` section

#### Scenario: Keymaps directory is empty
- **WHEN** `list_builtin_keymaps()` is called and the `keymaps/` directory contains no TOML files
- **THEN** the function SHALL return an empty list without error
