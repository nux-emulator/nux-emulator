//! Keymap visual overlay — renders key labels on the X11 window.
//!
//! Shows semi-transparent labels at each mapped touch position so the user
//! can see which keys map to which game buttons. Toggle with F1.

use crate::keymap::{KeyMapNode, KeymapConfig, Pos};

/// Generate overlay label data from a keymap config.
/// Returns (label, normalized_x, normalized_y) tuples.
pub fn generate_labels(config: &KeymapConfig) -> Vec<(String, f64, f64)> {
    let mut labels = Vec::new();

    for node in &config.key_map_nodes {
        match node {
            KeyMapNode::KMT_CLICK {
                key, pos, comment, ..
            }
            | KeyMapNode::KMT_CLICK_TWICE {
                key, pos, comment, ..
            } => {
                let label = if comment.is_empty() {
                    short_key_name(key)
                } else {
                    format!("[{}] {}", short_key_name(key), comment)
                };
                labels.push((label, pos.x, pos.y));
            }
            KeyMapNode::KMT_STEER_WHEEL {
                center_pos,
                left_key,
                right_key,
                up_key,
                down_key,
                left_offset,
                right_offset,
                up_offset,
                down_offset,
                ..
            } => {
                // Center label
                labels.push(("WASD".into(), center_pos.x, center_pos.y));
                // Direction labels
                labels.push((
                    short_key_name(up_key),
                    center_pos.x,
                    center_pos.y - up_offset,
                ));
                labels.push((
                    short_key_name(down_key),
                    center_pos.x,
                    center_pos.y + down_offset,
                ));
                labels.push((
                    short_key_name(left_key),
                    center_pos.x - left_offset,
                    center_pos.y,
                ));
                labels.push((
                    short_key_name(right_key),
                    center_pos.x + right_offset,
                    center_pos.y,
                ));
            }
            KeyMapNode::KMT_DRAG {
                key,
                start_pos,
                comment,
                ..
            } => {
                let label = if comment.is_empty() {
                    short_key_name(key)
                } else {
                    format!("[{}] {}", short_key_name(key), comment)
                };
                labels.push((label, start_pos.x, start_pos.y));
            }
            KeyMapNode::KMT_CLICK_MULTI {
                key,
                comment,
                click_nodes,
                ..
            } => {
                if let Some(first) = click_nodes.first() {
                    let label = if comment.is_empty() {
                        short_key_name(key)
                    } else {
                        format!("[{}] {}", short_key_name(key), comment)
                    };
                    labels.push((label, first.pos.x, first.pos.y));
                }
            }
        }
    }

    // Mouse aim center
    if let Some(ref mm) = config.mouse_move_map {
        labels.push(("Mouse Aim".into(), mm.start_pos.x, mm.start_pos.y));
    }

    labels
}

/// Shorten a key name for display (e.g., "Key_Space" → "Space").
fn short_key_name(key: &str) -> String {
    key.strip_prefix("Key_")
        .unwrap_or(key)
        .replace("Mouse_Left", "LMB")
        .replace("Mouse_Right", "RMB")
        .to_string()
}
