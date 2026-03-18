//! Shared UI state for cross-widget communication.

use std::cell::Cell;

/// Lightweight shared state for the UI layer.
///
/// This is stored in the `NuxWindow` and passed to child widgets that need
/// to query VM status or APK install progress. The actual VM state comes
/// from `nux-core`; this is a UI-side mirror updated via signals.
#[derive(Debug)]
pub struct UiState {
    /// Whether the VM is currently running (mirrors `VmState::Running`).
    pub vm_running: Cell<bool>,
    /// Whether an APK install is in progress.
    pub apk_installing: Cell<bool>,
    /// Whether fullscreen mode is active.
    pub fullscreen: Cell<bool>,
    /// Pre-fullscreen window width (for state persistence).
    pub pre_fs_width: Cell<i32>,
    /// Pre-fullscreen window height (for state persistence).
    pub pre_fs_height: Cell<i32>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            vm_running: Cell::new(false),
            apk_installing: Cell::new(false),
            fullscreen: Cell::new(false),
            pre_fs_width: Cell::new(1024),
            pre_fs_height: Cell::new(768),
        }
    }
}
