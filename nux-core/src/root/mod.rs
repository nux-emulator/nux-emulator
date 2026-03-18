//! Root manager for Nux Emulator.
//!
//! Provides boot image management, root mode switching, and patching workflow
//! orchestration for Magisk, `KernelSU`, and `APatch` root managers.
//!
//! # Architecture
//!
//! - [`store::BootImageStore`] — manages boot image files on disk per instance
//! - [`switching`] — root mode switching and active boot image resolution
//! - [`manager::RootManager`] — orchestrates the ADB-based patching workflow
//! - [`apk`] — APK resource paths and VM-side patched output paths
//! - [`error`] — error types for root operations

pub mod apk;
pub mod error;
pub mod manager;
pub mod store;
pub mod switching;

// Re-export key types at module level for convenience.
pub use error::{RootError, RootResult};
pub use manager::{AdbBridge, RootManager};
pub use store::BootImageStore;
pub use switching::{active_boot_image_path, restart_required, set_root_mode, unroot};
