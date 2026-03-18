//! Keymap engine for translating keyboard/mouse bindings into Android touch events.
//!
//! Provides TOML-based keymap parsing, validation, coordinate scaling,
//! touch slot allocation, and runtime keymap switching.

pub mod error;
pub mod format;
pub mod scaling;
pub mod slots;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

pub use error::KeymapError;
pub use format::{Binding, Keymap, KeymapMeta, Point, Region, parse_keymap, validate_keymap};
pub use scaling::ScaleFactors;
pub use slots::SlotAllocator;

/// Runtime keymap manager supporting hot-swap and key lookup.
#[derive(Debug)]
pub struct KeymapEngine {
    active: Arc<RwLock<Option<ActiveKeymap>>>,
}

/// An active keymap with precomputed lookup tables.
#[derive(Debug, Clone)]
struct ActiveKeymap {
    keymap: Keymap,
    scale: ScaleFactors,
    /// Maps key name → binding index for O(1) lookup.
    key_index: HashMap<String, usize>,
}

impl KeymapEngine {
    /// Create a new keymap engine with no active keymap.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: Arc::new(RwLock::new(None)),
        }
    }

    /// Load a keymap from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns `KeymapError` if the file can't be read, parsed, or fails validation.
    pub fn load_file(&self, path: &Path, display_res: (u32, u32)) -> Result<(), KeymapError> {
        let content = std::fs::read_to_string(path)?;
        self.load_str(&content, display_res)
    }

    /// Load a keymap from a TOML string.
    ///
    /// # Errors
    ///
    /// Returns `KeymapError` if parsing or validation fails.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn load_str(&self, toml_str: &str, display_res: (u32, u32)) -> Result<(), KeymapError> {
        let keymap = parse_keymap(toml_str)?;
        validate_keymap(&keymap)?;

        let scale = ScaleFactors::new(
            (keymap.meta.resolution[0], keymap.meta.resolution[1]),
            display_res,
        );

        let mut key_index = HashMap::new();
        for (i, binding) in keymap.bindings.iter().enumerate() {
            for key in binding_keys(binding) {
                key_index.insert(key, i);
            }
        }

        let active = ActiveKeymap {
            keymap,
            scale,
            key_index,
        };

        *self.active.write().unwrap() = Some(active);
        Ok(())
    }

    /// Clear the active keymap.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn unload(&self) {
        *self.active.write().unwrap() = None;
    }

    /// Check if a keymap is currently loaded.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn is_loaded(&self) -> bool {
        self.active.read().unwrap().is_some()
    }

    /// Look up the binding for a key name, returning the binding and scaled coordinates.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn lookup(&self, key: &str) -> Option<ScaledBinding> {
        let guard = self.active.read().unwrap();
        let active = guard.as_ref()?;
        let &idx = active.key_index.get(key)?;
        let binding = &active.keymap.bindings[idx];
        Some(scale_binding(binding, &active.scale))
    }

    /// Update the display resolution, rescaling all coordinates.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn update_resolution(&self, display_res: (u32, u32)) {
        if let Some(active) = self.active.write().unwrap().as_mut() {
            active.scale.update_resolution(display_res);
        }
    }

    /// Get the name of the currently loaded keymap.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn active_name(&self) -> Option<String> {
        let guard = self.active.read().unwrap();
        guard.as_ref().map(|a| a.keymap.meta.name.clone())
    }

    /// Get the game package of the currently loaded keymap.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn active_game_package(&self) -> Option<String> {
        let guard = self.active.read().unwrap();
        guard.as_ref().map(|a| a.keymap.meta.game_package.clone())
    }

    /// Get all key bindings for overlay rendering.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn overlay_hints(&self) -> Vec<OverlayHint> {
        let guard = self.active.read().unwrap();
        let Some(active) = guard.as_ref() else {
            return Vec::new();
        };
        active
            .keymap
            .bindings
            .iter()
            .flat_map(|b| {
                let keys = binding_keys(b);
                let scaled = scale_binding(b, &active.scale);
                keys.into_iter().map(move |key| OverlayHint {
                    key,
                    position: scaled.primary_position(),
                    binding_type: scaled.type_name().to_owned(),
                })
            })
            .collect()
    }
}

impl Default for KeymapEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// A binding with coordinates scaled to the current display resolution.
#[derive(Debug, Clone)]
pub enum ScaledBinding {
    /// Tap at a scaled point.
    Tap { x: i32, y: i32 },
    /// Long press at a scaled point.
    LongPress { x: i32, y: i32, duration_ms: u64 },
    /// Swipe between two scaled points.
    Swipe {
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
        duration_ms: u64,
    },
    /// Joystick with scaled center and radius.
    Joystick {
        center_x: i32,
        center_y: i32,
        radius: i32,
        keys: [String; 4],
    },
    /// Aim with scaled region.
    Aim {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        sensitivity: f64,
    },
}

impl ScaledBinding {
    /// Get the primary position for overlay rendering.
    #[must_use]
    pub fn primary_position(&self) -> (i32, i32) {
        match self {
            Self::Tap { x, y } | Self::LongPress { x, y, .. } => (*x, *y),
            Self::Swipe { from_x, from_y, .. } => (*from_x, *from_y),
            Self::Joystick {
                center_x, center_y, ..
            } => (*center_x, *center_y),
            Self::Aim { x1, y1, x2, y2, .. } => ((x1 + x2) / 2, (y1 + y2) / 2),
        }
    }

    /// Get the binding type name.
    #[must_use]
    pub fn type_name(&self) -> &str {
        match self {
            Self::Tap { .. } => "tap",
            Self::LongPress { .. } => "long_press",
            Self::Swipe { .. } => "swipe",
            Self::Joystick { .. } => "joystick",
            Self::Aim { .. } => "aim",
        }
    }
}

/// Hint for rendering a key overlay on the display.
#[derive(Debug, Clone)]
pub struct OverlayHint {
    /// Key name to display.
    pub key: String,
    /// Position on screen (scaled).
    pub position: (i32, i32),
    /// Binding type name.
    pub binding_type: String,
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

/// Scale a binding's coordinates using the given scale factors.
fn scale_binding(binding: &Binding, scale: &ScaleFactors) -> ScaledBinding {
    match binding {
        Binding::Tap { point, .. } => {
            let (x, y) = scale.scale(point.x, point.y);
            ScaledBinding::Tap { x, y }
        }
        Binding::LongPress {
            point, duration_ms, ..
        } => {
            let (x, y) = scale.scale(point.x, point.y);
            ScaledBinding::LongPress {
                x,
                y,
                duration_ms: *duration_ms,
            }
        }
        Binding::Swipe {
            from,
            to,
            duration_ms,
            ..
        } => {
            let (fx, fy) = scale.scale(from.x, from.y);
            let (tx, ty) = scale.scale(to.x, to.y);
            ScaledBinding::Swipe {
                from_x: fx,
                from_y: fy,
                to_x: tx,
                to_y: ty,
                duration_ms: *duration_ms,
            }
        }
        Binding::Joystick {
            keys,
            center,
            radius,
        } => {
            let (cx, cy) = scale.scale(center.x, center.y);
            #[allow(clippy::cast_possible_wrap)]
            let (rx, _) = scale.scale(*radius as i32, 0);
            ScaledBinding::Joystick {
                center_x: cx,
                center_y: cy,
                radius: rx,
                keys: keys.clone(),
            }
        }
        Binding::Aim {
            region,
            sensitivity,
            ..
        } => {
            let (x1, y1) = scale.scale(region.x1, region.y1);
            let (x2, y2) = scale.scale(region.x2, region.y2);
            ScaledBinding::Aim {
                x1,
                y1,
                x2,
                y2,
                sensitivity: *sensitivity,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_TOML: &str = r#"
[meta]
name = "Test Game"
game_package = "com.test.game"
resolution = [1080, 1920]

[[bindings]]
type = "tap"
key = "f"
point = { x = 500, y = 500 }

[[bindings]]
type = "joystick"
keys = ["w", "a", "s", "d"]
center = { x = 200, y = 1400 }
radius = 150

[[bindings]]
type = "aim"
key = "mouse2"
region = { x1 = 540, y1 = 960, x2 = 1080, y2 = 1920 }
sensitivity = 1.5
"#;

    #[test]
    fn engine_load_and_lookup() {
        let engine = KeymapEngine::new();
        assert!(!engine.is_loaded());

        engine.load_str(FULL_TOML, (1080, 1920)).unwrap();
        assert!(engine.is_loaded());
        assert_eq!(engine.active_name().unwrap(), "Test Game");

        let binding = engine.lookup("f").unwrap();
        assert_eq!(binding.type_name(), "tap");
        assert_eq!(binding.primary_position(), (500, 500));
    }

    #[test]
    fn engine_joystick_keys_lookup() {
        let engine = KeymapEngine::new();
        engine.load_str(FULL_TOML, (1080, 1920)).unwrap();

        for key in ["w", "a", "s", "d"] {
            let binding = engine.lookup(key).unwrap();
            assert_eq!(binding.type_name(), "joystick");
        }
    }

    #[test]
    fn engine_unknown_key_returns_none() {
        let engine = KeymapEngine::new();
        engine.load_str(FULL_TOML, (1080, 1920)).unwrap();
        assert!(engine.lookup("z").is_none());
    }

    #[test]
    fn engine_unload() {
        let engine = KeymapEngine::new();
        engine.load_str(FULL_TOML, (1080, 1920)).unwrap();
        assert!(engine.is_loaded());

        engine.unload();
        assert!(!engine.is_loaded());
        assert!(engine.lookup("f").is_none());
    }

    #[test]
    fn engine_resolution_scaling() {
        let engine = KeymapEngine::new();
        engine.load_str(FULL_TOML, (2160, 3840)).unwrap();

        // Tap at (500, 500) in 1080x1920 → (1000, 1000) in 2160x3840
        let binding = engine.lookup("f").unwrap();
        assert_eq!(binding.primary_position(), (1000, 1000));
    }

    #[test]
    fn engine_update_resolution() {
        let engine = KeymapEngine::new();
        engine.load_str(FULL_TOML, (1080, 1920)).unwrap();

        let binding = engine.lookup("f").unwrap();
        assert_eq!(binding.primary_position(), (500, 500));

        engine.update_resolution((2160, 3840));
        let binding = engine.lookup("f").unwrap();
        assert_eq!(binding.primary_position(), (1000, 1000));
    }

    #[test]
    fn engine_overlay_hints() {
        let engine = KeymapEngine::new();
        engine.load_str(FULL_TOML, (1080, 1920)).unwrap();

        let hints = engine.overlay_hints();
        // f + w,a,s,d + mouse2 = 6 hints
        assert_eq!(hints.len(), 6);
        assert!(hints.iter().any(|h| h.key == "f"));
        assert!(hints.iter().any(|h| h.key == "w"));
    }

    #[test]
    fn engine_load_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.toml");
        std::fs::write(&path, FULL_TOML).unwrap();

        let engine = KeymapEngine::new();
        engine.load_file(&path, (1080, 1920)).unwrap();
        assert!(engine.is_loaded());
    }

    #[test]
    fn engine_hot_swap() {
        let engine = KeymapEngine::new();
        engine.load_str(FULL_TOML, (1080, 1920)).unwrap();
        assert_eq!(engine.active_name().unwrap(), "Test Game");

        let other_toml = r#"
[meta]
name = "Other Game"
game_package = "com.other"
resolution = [1080, 1920]

[[bindings]]
type = "tap"
key = "g"
point = { x = 100, y = 100 }
"#;
        engine.load_str(other_toml, (1080, 1920)).unwrap();
        assert_eq!(engine.active_name().unwrap(), "Other Game");
        assert!(engine.lookup("f").is_none());
        assert!(engine.lookup("g").is_some());
    }
}
