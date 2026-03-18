//! Configuration system for Nux Emulator.
//!
//! Provides typed config structs, XDG-compliant path resolution,
//! global/instance merge, validation, save, and schema migration.

mod migration;
mod paths;
mod schema;
mod validation;

pub use migration::{CURRENT_SCHEMA_VERSION, migrate};
pub use paths::{global_config_path, instance_config_path};
pub use schema::{
    DeviceConfig, DisplayConfig, GAppsSource, GoogleServicesConfig, GoogleServicesProvider,
    GpuBackend, GpuConfig, HardwareConfig, InstanceConfigOverlay, InstanceMeta, NetworkConfig,
    NetworkMode, NuxConfig, RootConfig, RootMode,
};
pub use validation::{ConfigError, validate};

use anyhow::{Context, Result};
use std::path::Path;

/// Load the resolved config for a given instance.
///
/// Reads the global config, overlays instance-specific overrides,
/// and runs schema migration on both before merging.
///
/// # Errors
///
/// Returns an error if config files exist but contain invalid TOML,
/// or if schema migration fails.
pub fn load(instance_name: &str) -> Result<NuxConfig> {
    let global_path = global_config_path();
    let instance_path = instance_config_path(instance_name);

    let global = load_and_migrate(&global_path).unwrap_or_default();
    let overlay = load_overlay(&instance_path).unwrap_or_default();

    Ok(merge(global, &overlay))
}

/// Save a config to the given path, creating parent directories as needed.
///
/// # Errors
///
/// Returns an error if directory creation or file writing fails.
pub fn save(config: &NuxConfig, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory: {}", parent.display()))?;
    }
    let toml_str = toml::to_string_pretty(config).context("failed to serialize config to TOML")?;
    std::fs::write(path, toml_str)
        .with_context(|| format!("failed to write config to {}", path.display()))?;
    Ok(())
}

/// Merge a global config with an instance overlay.
pub fn merge(global: NuxConfig, overlay: &InstanceConfigOverlay) -> NuxConfig {
    NuxConfig {
        schema_version: global.schema_version,
        instance: InstanceMeta {
            name: overlay.instance_name().unwrap_or(global.instance.name),
        },
        hardware: HardwareConfig {
            cpu_cores: overlay.cpu_cores().unwrap_or(global.hardware.cpu_cores),
            ram_mb: overlay.ram_mb().unwrap_or(global.hardware.ram_mb),
        },
        display: DisplayConfig {
            width: overlay.display_width().unwrap_or(global.display.width),
            height: overlay.display_height().unwrap_or(global.display.height),
            dpi: overlay.display_dpi().unwrap_or(global.display.dpi),
        },
        gpu: GpuConfig {
            backend: overlay.gpu_backend().unwrap_or(global.gpu.backend),
        },
        root: RootConfig {
            mode: overlay.root_mode().unwrap_or(global.root.mode),
        },
        google_services: GoogleServicesConfig {
            provider: overlay
                .google_services_provider()
                .unwrap_or(global.google_services.provider),
            provider_version: overlay
                .google_services_provider_version()
                .or(global.google_services.provider_version),
            gapps_source: overlay
                .google_services_gapps_source()
                .unwrap_or(global.google_services.gapps_source),
        },
        network: NetworkConfig {
            mode: overlay.network_mode().unwrap_or(global.network.mode),
        },
        device: DeviceConfig {
            model: overlay.device_model().unwrap_or(global.device.model),
            manufacturer: overlay
                .device_manufacturer()
                .unwrap_or(global.device.manufacturer),
        },
    }
}

fn load_and_migrate(path: &Path) -> Result<NuxConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config: {}", path.display()))?;
    let raw: toml::Value = content.parse().context("failed to parse TOML")?;
    let migrated = migrate(raw)?;
    let config: NuxConfig = migrated
        .try_into()
        .context("failed to deserialize config")?;
    Ok(config)
}

fn load_overlay(path: &Path) -> Result<InstanceConfigOverlay> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read instance config: {}", path.display()))?;
    let overlay: InstanceConfigOverlay =
        toml::from_str(&content).context("failed to parse instance config")?;
    Ok(overlay)
}

#[cfg(test)]
mod tests;
