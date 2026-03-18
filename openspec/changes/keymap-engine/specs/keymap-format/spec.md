## ADDED Requirements

### Requirement: Keymap file uses TOML format with meta section
The keymap file SHALL be a valid TOML document. It SHALL contain a `[meta]` table with required fields: `name` (string), `game_package` (string), and `resolution` (array of two positive integers representing width and height).

#### Scenario: Valid keymap with complete meta section
- **WHEN** a TOML file contains `[meta]` with `name = "PUBG Mobile"`, `game_package = "com.tencent.ig"`, and `resolution = [1080, 1920]`
- **THEN** the parser SHALL accept the file and produce a `Keymap` struct with the meta fields populated

#### Scenario: Missing required meta field
- **WHEN** a TOML file is missing the `game_package` field in `[meta]`
- **THEN** the parser SHALL return an error identifying the missing field

### Requirement: Keymap file defines bindings as an array of tables
The keymap file SHALL contain a `[[bindings]]` array. Each entry SHALL have a `type` field (string) that identifies the binding type. Additional fields depend on the binding type.

#### Scenario: Multiple bindings parsed in order
- **WHEN** a keymap file contains three `[[bindings]]` entries of types `tap`, `joystick`, and `swipe`
- **THEN** the parser SHALL produce a `Keymap` with three bindings in the same order as defined in the file

#### Scenario: Unknown binding type
- **WHEN** a `[[bindings]]` entry has `type = "unknown_type"`
- **THEN** the parser SHALL return an error indicating an unrecognized binding type

### Requirement: Tap binding schema
A tap binding SHALL have fields: `type = "tap"`, `key` (string, single key name), `x` (integer), `y` (integer).

#### Scenario: Valid tap binding
- **WHEN** a binding has `type = "tap"`, `key = "W"`, `x = 540`, `y = 1400`
- **THEN** the parser SHALL produce a tap binding with the specified key and coordinates

#### Scenario: Tap binding missing coordinate
- **WHEN** a tap binding is missing the `y` field
- **THEN** the parser SHALL return a validation error

### Requirement: Long-press binding schema
A long-press binding SHALL have fields: `type = "long-press"`, `key` (string), `x` (integer), `y` (integer), and `duration_ms` (positive integer, milliseconds to hold before activation).

#### Scenario: Valid long-press binding
- **WHEN** a binding has `type = "long-press"`, `key = "F"`, `x = 300`, `y = 800`, `duration_ms = 500`
- **THEN** the parser SHALL produce a long-press binding with the specified parameters

### Requirement: Swipe binding schema
A swipe binding SHALL have fields: `type = "swipe"`, `key` (string), `from` (array of two integers), `to` (array of two integers), and `duration_ms` (positive integer).

#### Scenario: Valid swipe binding
- **WHEN** a binding has `type = "swipe"`, `key = "Q"`, `from = [100, 500]`, `to = [400, 500]`, `duration_ms = 200`
- **THEN** the parser SHALL produce a swipe binding with start/end coordinates and duration

### Requirement: Joystick binding schema
A joystick binding SHALL have fields: `type = "joystick"`, `keys` (array of exactly 4 strings representing up/left/down/right), `center_x` (integer), `center_y` (integer), and `radius` (positive integer).

#### Scenario: Valid joystick binding
- **WHEN** a binding has `type = "joystick"`, `keys = ["W", "A", "S", "D"]`, `center_x = 200`, `center_y = 1400`, `radius = 150`
- **THEN** the parser SHALL produce a joystick binding with center, radius, and directional keys

#### Scenario: Joystick with wrong number of keys
- **WHEN** a joystick binding has `keys = ["W", "A", "S"]` (only 3 keys)
- **THEN** the parser SHALL return a validation error indicating exactly 4 keys are required

### Requirement: Aim binding schema
An aim binding SHALL have fields: `type = "aim"`, `sensitivity` (positive float), and `region` (array of 4 integers representing `[x1, y1, x2, y2]` bounding box).

#### Scenario: Valid aim binding
- **WHEN** a binding has `type = "aim"`, `sensitivity = 1.5`, `region = [540, 960, 1080, 1920]`
- **THEN** the parser SHALL produce an aim binding with the sensitivity and bounding region

### Requirement: Binding key names are validated
All `key` and `keys` values SHALL be validated against a known set of key names (matching physical keyboard keys). Duplicate key assignments within a single keymap SHALL be rejected.

#### Scenario: Duplicate key assignment
- **WHEN** two bindings in the same keymap both use `key = "W"`
- **THEN** the parser SHALL return a validation error indicating the duplicate key

#### Scenario: Invalid key name
- **WHEN** a binding uses `key = "INVALID_KEY_NAME"`
- **THEN** the parser SHALL return a validation error indicating the unrecognized key name
