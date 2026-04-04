//! Keymap engine — processes keyboard/mouse events and emits touch events.
//!
//! Maintains state for each active mapping (steer wheel position, toggle state,
//! mouse accumulator) and converts X11 input events into scrcpy touch commands.

use crate::keymap::{self, KeyMapNode, KeymapConfig, MouseMoveMap, Pos};
use crate::scrcpy::control::ControlSocket;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

// Pointer IDs for multi-touch
const PTR_STEER: u64 = 0;
const PTR_AIM: u64 = 1;
const PTR_FIRE: u64 = 2;
const PTR_BASE: u64 = 3; // keys use 3, 4, 5, ...

/// Mouse button constants (X11 button numbers, not keysyms).
pub const MOUSE_LEFT: u32 = 1;
pub const MOUSE_RIGHT: u32 = 3;

/// Keymap engine state.
pub struct KeymapEngine {
    config: KeymapConfig,
    /// KeySym → node indices lookup.
    key_lookup: HashMap<u64, Vec<usize>>,
    /// Switch key keysym.
    switch_keysym: u64,
    /// Whether keymap is active (toggled by switch key).
    pub active: bool,
    /// Display dimensions (landscape-aware).
    disp_w: u32,
    disp_h: u32,

    // ── Steer wheel state ──
    steer_active: bool,
    steer_keys: HashSet<u64>, // currently pressed WASD keys
    steer_center: Pos,
    steer_offsets: (f64, f64, f64, f64), // left, right, up, down

    // ── Click-twice toggle state ──
    /// node index → whether touch is currently held down
    toggle_state: HashMap<usize, bool>,

    // ── Mouse aim state ──
    mouse_active: bool,
    mouse_center_x: f64,
    mouse_center_y: f64,
    mouse_accum_x: f64,
    mouse_accum_y: f64,
    mouse_speed_x: f64,
    mouse_speed_y: f64,
    ads_active: bool,
    ads_keysym: Option<u64>,
    ads_sensitivity: f64,

    // ── Per-node pointer ID assignment ──
    node_pointer: HashMap<usize, u64>,
    next_pointer: u64,
}

impl KeymapEngine {
    pub fn new(config: KeymapConfig, disp_w: u32, disp_h: u32) -> Self {
        let key_lookup = keymap::build_key_lookup(&config);
        let switch_keysym = keymap::key_name_to_keysym(&config.switch_key).unwrap_or(0x0060);

        // Parse steer wheel config
        let mut steer_center = Pos { x: 0.0, y: 0.0 };
        let mut steer_offsets = (0.0, 0.0, 0.0, 0.0);
        for node in &config.key_map_nodes {
            if let KeyMapNode::KMT_STEER_WHEEL {
                center_pos,
                left_offset,
                right_offset,
                up_offset,
                down_offset,
                ..
            } = node
            {
                steer_center = *center_pos;
                steer_offsets = (*left_offset, *right_offset, *up_offset, *down_offset);
                break;
            }
        }

        // Parse mouse move config
        let (mouse_cx, mouse_cy, mouse_sx, mouse_sy, ads_sens, ads_ks) =
            if let Some(ref mm) = config.mouse_move_map {
                let sx = if mm.speed_ratio_x != 1.0 {
                    mm.speed_ratio_x
                } else if mm.speed_ratio != 0.0 {
                    mm.speed_ratio
                } else {
                    1.0
                };
                let sy = if mm.speed_ratio_y != 1.0 {
                    mm.speed_ratio_y
                } else if mm.speed_ratio != 0.0 {
                    mm.speed_ratio
                } else {
                    1.0
                };
                let ads_ks = mm
                    .ads_key
                    .as_ref()
                    .and_then(|k| keymap::key_name_to_keysym(k));
                (
                    mm.start_pos.x,
                    mm.start_pos.y,
                    sx,
                    sy,
                    mm.ads_sensitivity,
                    ads_ks,
                )
            } else {
                (0.5, 0.5, 1.0, 1.0, 0.4, None)
            };

        Self {
            config,
            key_lookup,
            switch_keysym,
            active: false,
            disp_w,
            disp_h,
            steer_active: false,
            steer_keys: HashSet::new(),
            steer_center,
            steer_offsets,
            toggle_state: HashMap::new(),
            mouse_active: false,
            mouse_center_x: mouse_cx,
            mouse_center_y: mouse_cy,
            mouse_accum_x: 0.0,
            mouse_accum_y: 0.0,
            mouse_speed_x: mouse_sx,
            mouse_speed_y: mouse_sy,
            ads_active: false,
            ads_keysym: ads_ks,
            ads_sensitivity: ads_sens,
            node_pointer: HashMap::new(),
            next_pointer: PTR_BASE,
        }
    }

    /// Update display dimensions (on orientation change).
    pub fn set_display_size(&mut self, w: u32, h: u32) {
        self.disp_w = w;
        self.disp_h = h;
    }

    /// Convert normalized position to pixel coordinates.
    fn to_px(&self, pos: &Pos) -> (u32, u32) {
        let x = (pos.x * self.disp_w as f64).round() as u32;
        let y = (pos.y * self.disp_h as f64).round() as u32;
        (x.min(self.disp_w - 1), y.min(self.disp_h - 1))
    }

    /// Get or assign a pointer ID for a node.
    fn pointer_for(&mut self, node_idx: usize) -> u64 {
        *self.node_pointer.entry(node_idx).or_insert_with(|| {
            let id = self.next_pointer;
            self.next_pointer += 1;
            id
        })
    }

    /// Handle a key press. Returns true if the event was consumed by the keymap.
    pub fn on_key_down(
        &mut self,
        keysym: u64,
        control: &Arc<Mutex<Option<ControlSocket>>>,
    ) -> bool {
        // Toggle keymap
        if keysym == self.switch_keysym {
            self.active = !self.active;
            if self.active {
                log::info!("keymap: === ACTIVATED (press ` to deactivate) ===");
                for node in &self.config.key_map_nodes {
                    match node {
                        KeyMapNode::KMT_CLICK {
                            key, comment, pos, ..
                        } => {
                            log::info!(
                                "  [{}] {} → tap({:.0}%, {:.0}%)",
                                key,
                                comment,
                                pos.x * 100.0,
                                pos.y * 100.0
                            );
                        }
                        KeyMapNode::KMT_CLICK_TWICE {
                            key, comment, pos, ..
                        } => {
                            log::info!(
                                "  [{}] {} → toggle({:.0}%, {:.0}%)",
                                key,
                                comment,
                                pos.x * 100.0,
                                pos.y * 100.0
                            );
                        }
                        KeyMapNode::KMT_STEER_WHEEL {
                            left_key,
                            right_key,
                            up_key,
                            down_key,
                            ..
                        } => {
                            log::info!(
                                "  [{}/{}/{}/{}] Movement joystick",
                                up_key,
                                left_key,
                                down_key,
                                right_key
                            );
                        }
                        KeyMapNode::KMT_DRAG { key, comment, .. } => {
                            log::info!("  [{}] {} → drag", key, comment);
                        }
                        KeyMapNode::KMT_CLICK_MULTI { key, comment, .. } => {
                            log::info!("  [{}] {} → multi-tap", key, comment);
                        }
                    }
                }
                if self.config.mouse_move_map.is_some() {
                    log::info!("  [Mouse] Camera/aim control");
                }
                log::info!("  [Mouse_Left] Fire");
            } else {
                log::info!("keymap: === DEACTIVATED ===");
                self.release_all(control);
            }
            return true;
        }

        if !self.active {
            return false;
        }

        // Check ADS key
        if self.ads_keysym == Some(keysym) {
            self.ads_active = true;
            return true;
        }

        let indices = match self.key_lookup.get(&keysym) {
            Some(v) => v.clone(),
            None => return false,
        };

        for idx in indices {
            let node = self.config.key_map_nodes[idx].clone();
            match node {
                KeyMapNode::KMT_CLICK {
                    pos, switch_map, ..
                } => {
                    let (px, py) = self.to_px(&pos);
                    let ptr = self.pointer_for(idx);
                    if let Ok(mut g) = control.lock() {
                        if let Some(cs) = g.as_mut() {
                            cs.touch_down_id(ptr, px, py);
                            // Brief hold then release
                            let ctrl = control.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(50));
                                if let Ok(mut g) = ctrl.lock() {
                                    if let Some(cs) = g.as_mut() {
                                        cs.touch_up_id(ptr, px, py);
                                    }
                                }
                            });
                        }
                    }
                    if switch_map {
                        self.active = false;
                        log::info!("keymap: deactivated (switchMap)");
                    }
                }
                KeyMapNode::KMT_CLICK_TWICE { pos, .. } => {
                    let (px, py) = self.to_px(&pos);
                    let ptr = self.pointer_for(idx);
                    let held = self.toggle_state.entry(idx).or_insert(false);
                    if let Ok(mut g) = control.lock() {
                        if let Some(cs) = g.as_mut() {
                            if *held {
                                cs.touch_up_id(ptr, px, py);
                                *held = false;
                            } else {
                                cs.touch_down_id(ptr, px, py);
                                *held = true;
                            }
                        }
                    }
                }
                KeyMapNode::KMT_CLICK_MULTI { click_nodes, .. } => {
                    let ctrl = control.clone();
                    let dw = self.disp_w;
                    let dh = self.disp_h;
                    std::thread::spawn(move || {
                        for cn in &click_nodes {
                            std::thread::sleep(std::time::Duration::from_millis(cn.delay));
                            let px = (cn.pos.x * dw as f64).round().min(dw as f64 - 1.0) as u32;
                            let py = (cn.pos.y * dh as f64).round().min(dh as f64 - 1.0) as u32;
                            if let Ok(mut g) = ctrl.lock() {
                                if let Some(cs) = g.as_mut() {
                                    cs.tap(px, py);
                                }
                            }
                        }
                    });
                }
                KeyMapNode::KMT_STEER_WHEEL {
                    left_key,
                    right_key,
                    up_key,
                    down_key,
                    ..
                } => {
                    // Determine which direction this key maps to
                    let key_name = keysym_to_key_name(keysym);
                    if [&left_key, &right_key, &up_key, &down_key]
                        .iter()
                        .any(|k| **k == key_name)
                    {
                        self.steer_keys.insert(keysym);
                        self.update_steer_wheel(control, &left_key, &right_key, &up_key, &down_key);
                    }
                }
                KeyMapNode::KMT_DRAG {
                    start_pos, end_pos, ..
                } => {
                    let (sx, sy) = self.to_px(&start_pos);
                    let (ex, ey) = self.to_px(&end_pos);
                    let ptr = self.pointer_for(idx);
                    let ctrl = control.clone();
                    std::thread::spawn(move || {
                        if let Ok(mut g) = ctrl.lock() {
                            if let Some(cs) = g.as_mut() {
                                cs.touch_down_id(ptr, sx, sy);
                            }
                        }
                        // Smooth drag over 150ms
                        let steps: i32 = 5;
                        for i in 1..=steps {
                            std::thread::sleep(std::time::Duration::from_millis(30));
                            let cx = sx as i32 + (ex as i32 - sx as i32) * i / steps;
                            let cy = sy as i32 + (ey as i32 - sy as i32) * i / steps;
                            if let Ok(mut g) = ctrl.lock() {
                                if let Some(cs) = g.as_mut() {
                                    cs.touch_move_id(ptr, cx as u32, cy as u32);
                                }
                            }
                        }
                        if let Ok(mut g) = ctrl.lock() {
                            if let Some(cs) = g.as_mut() {
                                cs.touch_up_id(ptr, ex, ey);
                            }
                        }
                    });
                }
            }
        }
        true
    }

    /// Handle a key release. Returns true if consumed.
    pub fn on_key_up(&mut self, keysym: u64, control: &Arc<Mutex<Option<ControlSocket>>>) -> bool {
        if keysym == self.switch_keysym {
            return true;
        }
        if !self.active {
            return false;
        }

        // ADS key release
        if self.ads_keysym == Some(keysym) {
            self.ads_active = false;
            return true;
        }

        let indices = match self.key_lookup.get(&keysym) {
            Some(v) => v.clone(),
            None => return false,
        };

        for idx in indices {
            let node = self.config.key_map_nodes[idx].clone();
            if let KeyMapNode::KMT_STEER_WHEEL {
                left_key,
                right_key,
                up_key,
                down_key,
                ..
            } = &node
            {
                self.steer_keys.remove(&keysym);
                self.update_steer_wheel(control, left_key, right_key, up_key, down_key);
            }
            // KMT_CLICK_TWICE: release is handled on next press (toggle)
            // KMT_CLICK: already released via timer
            // KMT_DRAG: already completed via thread
        }
        true
    }

    /// Handle mouse button press. Returns true if consumed.
    pub fn on_mouse_down(
        &mut self,
        button: u32,
        control: &Arc<Mutex<Option<ControlSocket>>>,
    ) -> bool {
        if !self.active {
            return false;
        }

        if button == MOUSE_LEFT {
            // Check if Mouse_Left is mapped to a click node
            let mut fire_pos = None;
            for node in &self.config.key_map_nodes {
                if let KeyMapNode::KMT_CLICK { key, pos, .. } = node {
                    if key == "Mouse_Left" {
                        fire_pos = Some(*pos);
                        break;
                    }
                }
            }
            if let Some(pos) = fire_pos {
                let (px, py) = self.to_px(&pos);
                if let Ok(mut g) = control.lock() {
                    if let Some(cs) = g.as_mut() {
                        cs.touch_down_id(PTR_FIRE, px, py);
                    }
                }
                return true;
            }
        }
        false
    }

    /// Handle mouse button release. Returns true if consumed.
    pub fn on_mouse_up(
        &mut self,
        button: u32,
        control: &Arc<Mutex<Option<ControlSocket>>>,
    ) -> bool {
        if !self.active {
            return false;
        }

        if button == MOUSE_LEFT {
            let mut fire_pos = None;
            for node in &self.config.key_map_nodes {
                if let KeyMapNode::KMT_CLICK { key, pos, .. } = node {
                    if key == "Mouse_Left" {
                        fire_pos = Some(*pos);
                        break;
                    }
                }
            }
            if let Some(pos) = fire_pos {
                let (px, py) = self.to_px(&pos);
                if let Ok(mut g) = control.lock() {
                    if let Some(cs) = g.as_mut() {
                        cs.touch_up_id(PTR_FIRE, px, py);
                    }
                }
                return true;
            }
        }
        false
    }

    /// Handle relative mouse motion (dx, dy in pixels). Returns true if consumed.
    pub fn on_mouse_move(
        &mut self,
        dx: i32,
        dy: i32,
        control: &Arc<Mutex<Option<ControlSocket>>>,
    ) -> bool {
        if !self.active || self.config.mouse_move_map.is_none() {
            return false;
        }

        let sens_mult = if self.ads_active {
            self.ads_sensitivity
        } else {
            1.0
        };

        // Accumulate mouse delta (scaled by speed ratios)
        self.mouse_accum_x += dx as f64 * self.mouse_speed_x * sens_mult / self.disp_w as f64;
        self.mouse_accum_y += dy as f64 * self.mouse_speed_y * sens_mult / self.disp_h as f64;

        // Compute touch position
        let tx = self.mouse_center_x + self.mouse_accum_x;
        let ty = self.mouse_center_y + self.mouse_accum_y;

        // Clamp to display bounds
        let px = (tx * self.disp_w as f64)
            .round()
            .clamp(0.0, self.disp_w as f64 - 1.0) as u32;
        let py = (ty * self.disp_h as f64)
            .round()
            .clamp(0.0, self.disp_h as f64 - 1.0) as u32;

        if let Ok(mut g) = control.lock() {
            if let Some(cs) = g.as_mut() {
                if !self.mouse_active {
                    // First move — touch down at center
                    let cx = (self.mouse_center_x * self.disp_w as f64).round() as u32;
                    let cy = (self.mouse_center_y * self.disp_h as f64).round() as u32;
                    cs.touch_down_id(PTR_AIM, cx, cy);
                    self.mouse_active = true;
                }
                cs.touch_move_id(PTR_AIM, px, py);
            }
        }

        // Reset accumulator when it gets too far from center (wrap around)
        if self.mouse_accum_x.abs() > 0.3 || self.mouse_accum_y.abs() > 0.3 {
            // Lift and re-center
            if let Ok(mut g) = control.lock() {
                if let Some(cs) = g.as_mut() {
                    cs.touch_up_id(PTR_AIM, px, py);
                    self.mouse_active = false;
                }
            }
            self.mouse_accum_x = 0.0;
            self.mouse_accum_y = 0.0;
        }

        true
    }

    /// Update steer wheel touch based on currently pressed keys.
    fn update_steer_wheel(
        &mut self,
        control: &Arc<Mutex<Option<ControlSocket>>>,
        left_key: &str,
        right_key: &str,
        up_key: &str,
        down_key: &str,
    ) {
        let left_sym = keymap::key_name_to_keysym(left_key).unwrap_or(0);
        let right_sym = keymap::key_name_to_keysym(right_key).unwrap_or(0);
        let up_sym = keymap::key_name_to_keysym(up_key).unwrap_or(0);
        let down_sym = keymap::key_name_to_keysym(down_key).unwrap_or(0);

        let left = self.steer_keys.contains(&left_sym);
        let right = self.steer_keys.contains(&right_sym);
        let up = self.steer_keys.contains(&up_sym);
        let down = self.steer_keys.contains(&down_sym);

        let any = left || right || up || down;

        if !any && self.steer_active {
            // Release steer wheel
            let (cx, cy) = self.to_px(&self.steer_center);
            if let Ok(mut g) = control.lock() {
                if let Some(cs) = g.as_mut() {
                    cs.touch_up_id(PTR_STEER, cx, cy);
                }
            }
            self.steer_active = false;
            return;
        }

        if !any {
            return;
        }

        // Compute target offset from center
        let (lo, ro, uo, do_) = self.steer_offsets;
        let mut ox: f64 = 0.0;
        let mut oy: f64 = 0.0;
        if left {
            ox -= lo;
        }
        if right {
            ox += ro;
        }
        if up {
            oy -= uo;
        }
        if down {
            oy += do_;
        }

        // Normalize diagonal
        if ox != 0.0 && oy != 0.0 {
            let len = (ox * ox + oy * oy).sqrt();
            let max_len = lo.max(ro).max(uo).max(do_);
            if len > max_len {
                ox = ox / len * max_len;
                oy = oy / len * max_len;
            }
        }

        let target = Pos {
            x: self.steer_center.x + ox,
            y: self.steer_center.y + oy,
        };
        let (tx, ty) = self.to_px(&target);

        if let Ok(mut g) = control.lock() {
            if let Some(cs) = g.as_mut() {
                if !self.steer_active {
                    let (cx, cy) = self.to_px(&self.steer_center);
                    cs.touch_down_id(PTR_STEER, cx, cy);
                    self.steer_active = true;
                }
                cs.touch_move_id(PTR_STEER, tx, ty);
            }
        }
    }

    /// Release all active touches (on deactivation).
    fn release_all(&mut self, control: &Arc<Mutex<Option<ControlSocket>>>) {
        if let Ok(mut g) = control.lock() {
            if let Some(cs) = g.as_mut() {
                // Release steer wheel
                if self.steer_active {
                    let (cx, cy) = self.to_px(&self.steer_center);
                    cs.touch_up_id(PTR_STEER, cx, cy);
                }
                // Release mouse aim
                if self.mouse_active {
                    let cx = (self.mouse_center_x * self.disp_w as f64).round() as u32;
                    let cy = (self.mouse_center_y * self.disp_h as f64).round() as u32;
                    cs.touch_up_id(PTR_AIM, cx, cy);
                }
                // Release all toggles
                for (&idx, &held) in &self.toggle_state {
                    if held {
                        if let Some(node) = self.config.key_map_nodes.get(idx) {
                            if let KeyMapNode::KMT_CLICK_TWICE { pos, .. } = node {
                                let (px, py) = self.to_px(pos);
                                let ptr = self.node_pointer.get(&idx).copied().unwrap_or(PTR_BASE);
                                cs.touch_up_id(ptr, px, py);
                            }
                        }
                    }
                }
            }
        }
        self.steer_active = false;
        self.steer_keys.clear();
        self.mouse_active = false;
        self.mouse_accum_x = 0.0;
        self.mouse_accum_y = 0.0;
        self.toggle_state.clear();
        self.ads_active = false;
    }
}

/// Reverse lookup: keysym → QtScrcpy key name (for steer wheel matching).
fn keysym_to_key_name(keysym: u64) -> String {
    let name = match keysym {
        0x0061..=0x007a => format!("Key_{}", (keysym as u8 - 0x61 + b'A') as char),
        0x0030..=0x0039 => format!("Key_{}", (keysym as u8) as char),
        0x0020 => "Key_Space".into(),
        0xff0d => "Key_Return".into(),
        0xff1b => "Key_Escape".into(),
        0xff09 => "Key_Tab".into(),
        0xff08 => "Key_BackSpace".into(),
        0xffe1 => "Key_Shift".into(),
        0xffe3 => "Key_Control".into(),
        0xffe9 => "Key_Alt".into(),
        0x0060 => "Key_QuoteLeft".into(),
        _ => format!("0x{keysym:04x}"),
    };
    name
}
