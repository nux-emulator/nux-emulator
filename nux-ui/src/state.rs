//! Shared UI state for cross-widget communication.

use std::cell::Cell;
use std::sync::Arc;

use crate::vm_launcher::{VmLaunchConfig, VmLauncher};

/// Lightweight shared state for the UI layer.
#[derive(Debug)]
pub struct UiState {
    /// Whether the VM is currently running (mirrors `VmState::Running`).
    pub vm_running: Cell<bool>,
    /// Whether boot has completed.
    pub vm_booted: Cell<bool>,
    /// Whether an APK install is in progress.
    pub apk_installing: Cell<bool>,
    /// Whether fullscreen mode is active.
    pub fullscreen: Cell<bool>,
    /// Pre-fullscreen window width (for state persistence).
    pub pre_fs_width: Cell<i32>,
    /// Pre-fullscreen window height (for state persistence).
    pub pre_fs_height: Cell<i32>,
    /// VM launcher instance.
    pub launcher: Arc<VmLauncher>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            vm_running: Cell::new(false),
            vm_booted: Cell::new(false),
            apk_installing: Cell::new(false),
            fullscreen: Cell::new(false),
            pre_fs_width: Cell::new(1024),
            pre_fs_height: Cell::new(768),
            launcher: Arc::new(VmLauncher::new(VmLaunchConfig::default())),
        }
    }
}
