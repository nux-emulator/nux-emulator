//! Keymap system — maps keyboard/mouse input to Android touch events.
//!
//! Compatible with QtScrcpy keymap JSON format. Supports:
//! - KMT_CLICK: key → tap at fixed position
//! - KMT_CLICK_TWICE: key → toggle touch (hold/release)
//! - KMT_CLICK_MULTI: key → sequential taps with delays
//! - KMT_STEER_WHEEL: WASD → virtual joystick drag
//! - KMT_DRAG: key → swipe from A to B
//! - mouseMoveMap: mouse delta → touch drag (FPS aim/camera)

pub mod engine;
pub mod overlay;

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

// ── JSON types (QtScrcpy-compatible) ──

/// Normalized position [0,1] relative to display.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Pos {
    pub x: f64,
    pub y: f64,
}

/// Root keymap configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeymapConfig {
    /// Key to toggle keymap on/off (e.g., "Key_QuoteLeft" for backtick).
    pub switch_key: String,
    /// Mouse movement → touch drag mapping (FPS aim).
    #[serde(default)]
    pub mouse_move_map: Option<MouseMoveMap>,
    /// List of key → touch mappings.
    #[serde(default)]
    pub key_map_nodes: Vec<KeyMapNode>,
}

/// Mouse movement mapping for FPS camera control.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MouseMoveMap {
    /// Center position where touch starts (normalized).
    pub start_pos: Pos,
    /// Horizontal speed multiplier.
    #[serde(default = "default_speed")]
    pub speed_ratio_x: f64,
    /// Vertical speed multiplier.
    #[serde(default = "default_speed")]
    pub speed_ratio_y: f64,
    /// Legacy single speed ratio (used if X/Y not specified).
    #[serde(default)]
    pub speed_ratio: f64,
    /// "Small eyes" — key to temporarily look around without moving.
    #[serde(default)]
    pub small_eyes: Option<ClickNode>,
    /// Sensitivity curve: "linear", "exponential", "scurve".
    #[serde(default = "default_curve")]
    pub sensitivity_curve: String,
    /// ADS (aim-down-sight) sensitivity multiplier.
    #[serde(default = "default_ads")]
    pub ads_sensitivity: f64,
    /// Key to activate ADS sensitivity.
    #[serde(default)]
    pub ads_key: Option<String>,
}

fn default_speed() -> f64 {
    1.0
}
fn default_curve() -> String {
    "linear".into()
}
fn default_ads() -> f64 {
    0.4
}

/// A single click node (used in smallEyes and click_multi).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickNode {
    #[serde(default)]
    pub comment: String,
    #[serde(rename = "type", default)]
    pub node_type: String,
    #[serde(default)]
    pub key: String,
    pub pos: Pos,
    #[serde(default)]
    pub switch_map: bool,
}

/// A delayed click for KMT_CLICK_MULTI.
#[derive(Debug, Clone, Deserialize)]
pub struct DelayedClick {
    /// Delay in milliseconds before this click.
    pub delay: u64,
    pub pos: Pos,
}

/// Key mapping node — one of several types.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
#[allow(non_camel_case_types)]
pub enum KeyMapNode {
    /// Single tap at a fixed position.
    KMT_CLICK {
        #[serde(default)]
        comment: String,
        key: String,
        pos: Pos,
        #[serde(default)]
        switch_map: bool,
    },
    /// Toggle touch — first press = down, second press = up.
    KMT_CLICK_TWICE {
        #[serde(default)]
        comment: String,
        key: String,
        pos: Pos,
    },
    /// Sequential taps with delays.
    KMT_CLICK_MULTI {
        #[serde(default)]
        comment: String,
        key: String,
        #[serde(rename = "clickNodes")]
        click_nodes: Vec<DelayedClick>,
    },
    /// Virtual joystick (WASD).
    KMT_STEER_WHEEL {
        #[serde(default)]
        comment: String,
        #[serde(rename = "centerPos")]
        center_pos: Pos,
        #[serde(rename = "leftOffset")]
        left_offset: f64,
        #[serde(rename = "rightOffset")]
        right_offset: f64,
        #[serde(rename = "upOffset")]
        up_offset: f64,
        #[serde(rename = "downOffset")]
        down_offset: f64,
        #[serde(rename = "leftKey")]
        left_key: String,
        #[serde(rename = "rightKey")]
        right_key: String,
        #[serde(rename = "upKey")]
        up_key: String,
        #[serde(rename = "downKey")]
        down_key: String,
    },
    /// Swipe from start to end position.
    KMT_DRAG {
        #[serde(default)]
        comment: String,
        key: String,
        #[serde(rename = "startPos")]
        start_pos: Pos,
        #[serde(rename = "endPos")]
        end_pos: Pos,
    },
}

// ── Loading ──

/// Load a keymap from a JSON file.
pub fn load_keymap(path: &Path) -> anyhow::Result<KeymapConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: KeymapConfig = serde_json::from_str(&content)?;
    log::info!(
        "keymap: loaded {} nodes from {}",
        config.key_map_nodes.len(),
        path.display()
    );
    Ok(config)
}

/// Find keymap for a package name in the keymaps directory.
pub fn find_keymap_for_package(package: &str) -> Option<KeymapConfig> {
    let dirs = [
        dirs_path("keymaps"),
        std::path::PathBuf::from("keymaps"), // relative to CWD
    ];
    for dir in &dirs {
        // Try exact package name match
        let path = dir.join(format!("{package}.json"));
        if path.exists() {
            if let Ok(config) = load_keymap(&path) {
                return Some(config);
            }
        }
    }
    None
}

/// Load the default/active keymap (first .json in keymaps dir).
pub fn load_active_keymap() -> Option<KeymapConfig> {
    let dirs = [dirs_path("keymaps"), std::path::PathBuf::from("keymaps")];
    for dir in &dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json") {
                    if let Ok(config) = load_keymap(&path) {
                        return Some(config);
                    }
                }
            }
        }
    }
    None
}

fn dirs_path(subdir: &str) -> std::path::PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home)
            .join(".config/nux-emulator")
            .join(subdir)
    } else {
        std::path::PathBuf::from(format!(".config/nux-emulator/{subdir}"))
    }
}

// ── Key name resolution ──

/// Convert a QtScrcpy key name (e.g., "Key_W", "Mouse_Left") to an X11 KeySym.
pub fn key_name_to_keysym(name: &str) -> Option<u64> {
    // Strip "Key_" prefix
    let name = name.strip_prefix("Key_").unwrap_or(name);
    Some(match name {
        // Letters
        "A" | "a" => 0x0061,
        "B" | "b" => 0x0062,
        "C" | "c" => 0x0063,
        "D" | "d" => 0x0064,
        "E" | "e" => 0x0065,
        "F" | "f" => 0x0066,
        "G" | "g" => 0x0067,
        "H" | "h" => 0x0068,
        "I" | "i" => 0x0069,
        "J" | "j" => 0x006a,
        "K" | "k" => 0x006b,
        "L" | "l" => 0x006c,
        "M" | "m" => 0x006d,
        "N" | "n" => 0x006e,
        "O" | "o" => 0x006f,
        "P" | "p" => 0x0070,
        "Q" | "q" => 0x0071,
        "R" | "r" => 0x0072,
        "S" | "s" => 0x0073,
        "T" | "t" => 0x0074,
        "U" | "u" => 0x0075,
        "V" | "v" => 0x0076,
        "W" | "w" => 0x0077,
        "X" | "x" => 0x0078,
        "Y" | "y" => 0x0079,
        "Z" | "z" => 0x007a,
        // Digits
        "0" => 0x0030,
        "1" => 0x0031,
        "2" => 0x0032,
        "3" => 0x0033,
        "4" => 0x0034,
        "5" => 0x0035,
        "6" => 0x0036,
        "7" => 0x0037,
        "8" => 0x0038,
        "9" => 0x0039,
        // Special keys
        "Space" => 0x0020,
        "Return" | "Enter" => 0xff0d,
        "Escape" => 0xff1b,
        "Tab" => 0xff09,
        "BackSpace" => 0xff08,
        "Delete" => 0xffff,
        "Shift" | "Shift_L" => 0xffe1,
        "Shift_R" => 0xffe2,
        "Control" | "Control_L" | "Ctrl" => 0xffe3,
        "Control_R" => 0xffe4,
        "Alt" | "Alt_L" => 0xffe9,
        "Alt_R" => 0xffea,
        "Up" => 0xff52,
        "Down" => 0xff54,
        "Left" => 0xff51,
        "Right" => 0xff53,
        "QuoteLeft" | "Grave" => 0x0060, // backtick
        "Minus" => 0x002d,
        "Equal" => 0x003d,
        "BracketLeft" => 0x005b,
        "BracketRight" => 0x005d,
        "Semicolon" => 0x003b,
        "Apostrophe" => 0x0027,
        "Comma" => 0x002c,
        "Period" => 0x002e,
        "Slash" => 0x002f,
        "Backslash" => 0x005c,
        // F-keys
        "F1" => 0xffbe,
        "F2" => 0xffbf,
        "F3" => 0xffc0,
        "F4" => 0xffc1,
        "F5" => 0xffc2,
        "F6" => 0xffc3,
        "F7" => 0xffc4,
        "F8" => 0xffc5,
        "F9" => 0xffc6,
        "F10" => 0xffc7,
        "F11" => 0xffc8,
        "F12" => 0xffc9,
        _ => return None,
    })
}

/// Build a lookup table: X11 KeySym → index into key_map_nodes.
pub fn build_key_lookup(config: &KeymapConfig) -> HashMap<u64, Vec<usize>> {
    let mut map: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, node) in config.key_map_nodes.iter().enumerate() {
        let keys = match node {
            KeyMapNode::KMT_CLICK { key, .. }
            | KeyMapNode::KMT_CLICK_TWICE { key, .. }
            | KeyMapNode::KMT_CLICK_MULTI { key, .. }
            | KeyMapNode::KMT_DRAG { key, .. } => vec![key.clone()],
            KeyMapNode::KMT_STEER_WHEEL {
                left_key,
                right_key,
                up_key,
                down_key,
                ..
            } => vec![
                left_key.clone(),
                right_key.clone(),
                up_key.clone(),
                down_key.clone(),
            ],
        };
        for key_name in keys {
            if let Some(keysym) = key_name_to_keysym(&key_name) {
                map.entry(keysym).or_default().push(i);
            }
        }
    }
    map
}
