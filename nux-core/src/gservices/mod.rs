//! Google Services manager for Nux Emulator.
//!
//! Manages the Google Services provider (`MicroG`, `GApps`, or `None`) for
//! each instance. Handles provider detection via ADB, `GApps` download
//! and verification, overlay-based system image modification, and
//! provider switching with rollback support.

pub mod detection;
pub mod download;
pub mod overlay;
pub mod status;
pub mod switching;
pub mod types;

pub use detection::{detect_provider, detect_version, parse_provider_from_packages};
pub use download::{download_gapps, gapps_cache_dir, resolve_package_info, verify_hash};
pub use overlay::{
    apply_gapps_overlay, apply_removal_overlay, backup_overlay, instance_overlay_dir,
    reset_overlay_to_base, restore_overlay,
};
pub use status::{query_status, update_config_from_status};
pub use switching::{SwitchResult, VmState, switch_provider, updated_config};
pub use types::{
    AdbShell, Freshness, GAppsPackageInfo, GServicesError, GServicesResult, GoogleServicesStatus,
};
