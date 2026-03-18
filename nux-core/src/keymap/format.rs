//! Keymap TOML schema types, deserialization, and validation.
//!
//! Defines the strongly-typed structs that map to the keymap TOML format
//! and provides parsing + validation functions.

use std::collections::HashSet;

use serde::Deserialize;

use crate::keymap::error::KeymapError;

/// Metadata section of a keymap file.
#[derive(Debug, Clone, Deserialize)]
pub struct KeymapMeta {
    /// Human-readable name for this keymap.
    pub name: String,
    /// Android package name of the target game.
    pub game_package: String,
    /// Authored resolution as `[width, height]`.
    pub resolution: [u32; 2],
}

/// A point coordinate in authored resolution space.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Point {
    /// X coordinate.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
}

/// A rectangular region in authored resolution space.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Region {
    /// Left edge X.
    pub x1: i32,
    /// Top edge Y.
    pub y1: i32,
    /// Right edge X.
    pub x2: i32,
    /// Bottom edge Y.
    pub y2: i32,
}

/// A single binding entry from the TOML `[[bindings]]` array.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Binding {
    /// Single tap at a fixed point.
    Tap {
        /// Key name (e.g. "f", "space").
        key: String,
        /// Screen coordinate to tap.
        point: Point,
    },
    /// Long press (touch-down on press, touch-up on release).
    LongPress {
        /// Key name.
        key: String,
        /// Screen coordinate.
        point: Point,
        /// Informational hold duration in milliseconds.
        duration_ms: u64,
    },
    /// Swipe gesture from one point to another.
    Swipe {
        /// Key name.
        key: String,
        /// Start point.
        from: Point,
        /// End point.
        to: Point,
        /// Swipe duration in milliseconds.
        duration_ms: u64,
    },
    /// Virtual joystick mapped to WASD (or similar 4-key set).
    Joystick {
        /// Four directional keys: `[up, left, down, right]`.
        keys: [String; 4],
        /// Center of the virtual stick.
        center: Point,
        /// Radius of the stick circle in pixels.
        radius: u32,
    },
    /// Mouse-aim: mouse deltas map to touch drags within a region.
    Aim {
        /// Activation key (e.g. right-click toggle key).
        key: String,
        /// Bounding region for aim movement.
        region: Region,
        /// Sensitivity multiplier.
        sensitivity: f64,
    },
}

/// Top-level keymap structure deserialized from TOML.
#[derive(Debug, Clone, Deserialize)]
pub struct Keymap {
    /// Keymap metadata.
    pub meta: KeymapMeta,
    /// Binding definitions.
    #[serde(rename = "bindings")]
    pub bindings: Vec<Binding>,
}

/// Parse a TOML string into a [`Keymap`].
///
/// # Errors
///
/// Returns [`KeymapError::ParseError`] if the TOML is malformed or
/// missing required fields.
pub fn parse_keymap(toml_str: &str) -> Result<Keymap, KeymapError> {
    toml::from_str(toml_str).map_err(|e| KeymapError::ParseError(e.to_string()))
}

/// Known key names accepted in keymap bindings.
const KNOWN_KEYS: &[&str] = &[
    "a",
    "b",
    "c",
    "d",
    "e",
    "f",
    "g",
    "h",
    "i",
    "j",
    "k",
    "l",
    "m",
    "n",
    "o",
    "p",
    "q",
    "r",
    "s",
    "t",
    "u",
    "v",
    "w",
    "x",
    "y",
    "z",
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "space",
    "shift",
    "ctrl",
    "alt",
    "tab",
    "escape",
    "enter",
    "backspace",
    "capslock",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
    "up",
    "down",
    "left",
    "right",
    "mouse1",
    "mouse2",
    "mouse3",
    "mouse4",
    "mouse5",
    "grave",
    "minus",
    "equal",
    "leftbracket",
    "rightbracket",
    "backslash",
    "semicolon",
    "apostrophe",
    "comma",
    "period",
    "slash",
    "insert",
    "delete",
    "home",
    "end",
    "pageup",
    "pagedown",
];

/// Validate a parsed keymap for semantic correctness.
///
/// Checks:
/// - No duplicate key assignments
/// - All key names are recognized
/// - Joystick bindings have exactly 4 keys (enforced by type)
/// - Positive radius and duration values
/// - Valid aim regions (`x1 < x2`, `y1 < y2`)
///
/// # Errors
///
/// Returns [`KeymapError::ValidationError`] describing the first rule violation found.
pub fn validate_keymap(keymap: &Keymap) -> Result<(), KeymapError> {
    let known: HashSet<&str> = KNOWN_KEYS.iter().copied().collect();
    let mut used_keys: HashSet<String> = HashSet::new();

    for binding in &keymap.bindings {
        let keys = binding_keys(binding);
        for key in &keys {
            if !known.contains(key.as_str()) {
                return Err(KeymapError::ValidationError(format!(
                    "unknown key name: {key}"
                )));
            }
            if !used_keys.insert(key.clone()) {
                return Err(KeymapError::ValidationError(format!(
                    "duplicate key assignment: {key}"
                )));
            }
        }

        match binding {
            Binding::LongPress { duration_ms, .. } | Binding::Swipe { duration_ms, .. } => {
                if *duration_ms == 0 {
                    return Err(KeymapError::ValidationError(
                        "duration_ms must be positive".to_owned(),
                    ));
                }
            }
            Binding::Joystick { radius, .. } => {
                if *radius == 0 {
                    return Err(KeymapError::ValidationError(
                        "joystick radius must be positive".to_owned(),
                    ));
                }
            }
            Binding::Aim {
                region,
                sensitivity,
                ..
            } => {
                if region.x1 >= region.x2 || region.y1 >= region.y2 {
                    return Err(KeymapError::ValidationError(
                        "aim region must have x1 < x2 and y1 < y2".to_owned(),
                    ));
                }
                if *sensitivity <= 0.0 {
                    return Err(KeymapError::ValidationError(
                        "aim sensitivity must be positive".to_owned(),
                    ));
                }
            }
            Binding::Tap { .. } => {}
        }
    }

    Ok(())
}

/// Extract all key names from a binding.
fn binding_keys(binding: &Binding) -> Vec<String> {
    match binding {
        Binding::Tap { key, .. }
        | Binding::LongPress { key, .. }
        | Binding::Swipe { key, .. }
        | Binding::Aim { key, .. } => vec![key.clone()],
        Binding::Joystick { keys, .. } => keys.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_TOML: &str = r#"
[meta]
name = "Test"
game_package = "com.test.game"
resolution = [1080, 1920]

[[bindings]]
type = "tap"
key = "f"
point = { x = 500, y = 500 }
"#;

    #[test]
    fn parse_minimal_keymap() {
        let km = parse_keymap(MINIMAL_TOML).expect("should parse");
        assert_eq!(km.meta.name, "Test");
        assert_eq!(km.meta.game_package, "com.test.game");
        assert_eq!(km.meta.resolution, [1080, 1920]);
        assert_eq!(km.bindings.len(), 1);
    }

    #[test]
    fn parse_missing_meta_fails() {
        let toml = r#"
[[bindings]]
type = "tap"
key = "f"
point = { x = 0, y = 0 }
"#;
        assert!(parse_keymap(toml).is_err());
    }

    #[test]
    fn parse_unknown_binding_type_fails() {
        let toml = r#"
[meta]
name = "Test"
game_package = "com.test"
resolution = [1080, 1920]

[[bindings]]
type = "teleport"
key = "t"
"#;
        assert!(parse_keymap(toml).is_err());
    }

    #[test]
    fn parse_malformed_toml_fails() {
        assert!(parse_keymap("this is not valid toml {{{").is_err());
    }

    #[test]
    fn validate_valid_keymap() {
        let km = parse_keymap(MINIMAL_TOML).unwrap();
        assert!(validate_keymap(&km).is_ok());
    }

    #[test]
    fn validate_duplicate_key() {
        let toml = r#"
[meta]
name = "Test"
game_package = "com.test"
resolution = [1080, 1920]

[[bindings]]
type = "tap"
key = "f"
point = { x = 100, y = 100 }

[[bindings]]
type = "tap"
key = "f"
point = { x = 200, y = 200 }
"#;
        let km = parse_keymap(toml).unwrap();
        let err = validate_keymap(&km).unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn validate_unknown_key_name() {
        let toml = r#"
[meta]
name = "Test"
game_package = "com.test"
resolution = [1080, 1920]

[[bindings]]
type = "tap"
key = "nonexistent_key"
point = { x = 100, y = 100 }
"#;
        let km = parse_keymap(toml).unwrap();
        let err = validate_keymap(&km).unwrap_err();
        assert!(err.to_string().contains("unknown key"));
    }

    #[test]
    fn validate_zero_duration() {
        let toml = r#"
[meta]
name = "Test"
game_package = "com.test"
resolution = [1080, 1920]

[[bindings]]
type = "long_press"
key = "f"
point = { x = 100, y = 100 }
duration_ms = 0
"#;
        let km = parse_keymap(toml).unwrap();
        let err = validate_keymap(&km).unwrap_err();
        assert!(err.to_string().contains("duration_ms"));
    }

    #[test]
    fn validate_zero_radius() {
        let toml = r#"
[meta]
name = "Test"
game_package = "com.test"
resolution = [1080, 1920]

[[bindings]]
type = "joystick"
keys = ["w", "a", "s", "d"]
center = { x = 200, y = 800 }
radius = 0
"#;
        let km = parse_keymap(toml).unwrap();
        let err = validate_keymap(&km).unwrap_err();
        assert!(err.to_string().contains("radius"));
    }

    #[test]
    fn validate_invalid_aim_region() {
        let toml = r#"
[meta]
name = "Test"
game_package = "com.test"
resolution = [1080, 1920]

[[bindings]]
type = "aim"
key = "mouse2"
region = { x1 = 500, y1 = 500, x2 = 100, y2 = 100 }
sensitivity = 1.0
"#;
        let km = parse_keymap(toml).unwrap();
        let err = validate_keymap(&km).unwrap_err();
        assert!(err.to_string().contains("aim region"));
    }

    #[test]
    fn validate_negative_sensitivity() {
        let toml = r#"
[meta]
name = "Test"
game_package = "com.test"
resolution = [1080, 1920]

[[bindings]]
type = "aim"
key = "mouse2"
region = { x1 = 100, y1 = 100, x2 = 500, y2 = 500 }
sensitivity = -1.0
"#;
        let km = parse_keymap(toml).unwrap();
        let err = validate_keymap(&km).unwrap_err();
        assert!(err.to_string().contains("sensitivity"));
    }

    #[test]
    fn parse_all_binding_types() {
        let toml = r#"
[meta]
name = "Full"
game_package = "com.test"
resolution = [1080, 1920]

[[bindings]]
type = "tap"
key = "f"
point = { x = 100, y = 100 }

[[bindings]]
type = "long_press"
key = "g"
point = { x = 200, y = 200 }
duration_ms = 500

[[bindings]]
type = "swipe"
key = "h"
from = { x = 100, y = 100 }
to = { x = 300, y = 300 }
duration_ms = 200

[[bindings]]
type = "joystick"
keys = ["w", "a", "s", "d"]
center = { x = 200, y = 800 }
radius = 100

[[bindings]]
type = "aim"
key = "mouse2"
region = { x1 = 400, y1 = 200, x2 = 1000, y2 = 800 }
sensitivity = 1.5
"#;
        let km = parse_keymap(toml).unwrap();
        assert_eq!(km.bindings.len(), 5);
        assert!(validate_keymap(&km).is_ok());
    }
}
