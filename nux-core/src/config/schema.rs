//! Typed config structs and enums for all Nux configuration domains.

use serde::{Deserialize, Serialize};

// ── Enums ──

/// GPU rendering backend.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuBackend {
    #[default]
    Gfxstream,
    Virglrenderer,
    Software,
}

/// Root manager mode.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RootMode {
    #[default]
    None,
    Magisk,
    Kernelsu,
    Apatch,
}

/// Google Services provider.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GoogleServicesProvider {
    None,
    #[default]
    Microg,
    Gapps,
}

/// `GApps` package source.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GAppsSource {
    #[default]
    Opengapps,
    Mindthegapps,
}

/// Network mode.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkMode {
    #[default]
    Nat,
    Bridged,
}

// ── Section structs ──

/// Instance identity metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstanceMeta {
    pub name: String,
}

impl Default for InstanceMeta {
    fn default() -> Self {
        Self {
            name: "default".to_owned(),
        }
    }
}

/// Hardware allocation settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HardwareConfig {
    pub cpu_cores: u32,
    pub ram_mb: u32,
}

impl Default for HardwareConfig {
    fn default() -> Self {
        Self {
            cpu_cores: 2,
            ram_mb: 2048,
        }
    }
}

/// Display resolution and density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            width: 1080,
            height: 1920,
            dpi: 320,
        }
    }
}

/// GPU backend selection.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GpuConfig {
    pub backend: GpuBackend,
}

/// Root manager configuration.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RootConfig {
    pub mode: RootMode,
}

/// Google Services provider configuration.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GoogleServicesConfig {
    pub provider: GoogleServicesProvider,
    pub provider_version: Option<String>,
    pub gapps_source: GAppsSource,
}

/// Network mode configuration.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    pub mode: NetworkMode,
}

/// Device identity spoofing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceConfig {
    pub model: String,
    pub manufacturer: String,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            model: "Pixel 9".to_owned(),
            manufacturer: "Google".to_owned(),
        }
    }
}

// ── Top-level config ──

/// Complete Nux instance configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NuxConfig {
    pub schema_version: u32,
    pub instance: InstanceMeta,
    pub hardware: HardwareConfig,
    pub display: DisplayConfig,
    pub gpu: GpuConfig,
    pub root: RootConfig,
    pub google_services: GoogleServicesConfig,
    pub network: NetworkConfig,
    pub device: DeviceConfig,
}

impl Default for NuxConfig {
    fn default() -> Self {
        Self {
            schema_version: super::migration::CURRENT_SCHEMA_VERSION,
            instance: InstanceMeta::default(),
            hardware: HardwareConfig::default(),
            display: DisplayConfig::default(),
            gpu: GpuConfig::default(),
            root: RootConfig::default(),
            google_services: GoogleServicesConfig::default(),
            network: NetworkConfig::default(),
            device: DeviceConfig::default(),
        }
    }
}

// ── Instance overlay (all fields optional for merge) ──

/// Partial config for instance-level overrides.
///
/// Every field is `Option<T>` — `None` means "inherit from global."
/// Uses a flat structure to keep instance TOML files minimal.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct InstanceConfigOverlay {
    #[serde(rename = "instance")]
    overlay_instance: Option<OverlayInstance>,
    #[serde(rename = "hardware")]
    overlay_hardware: Option<OverlayHardware>,
    #[serde(rename = "display")]
    overlay_display: Option<OverlayDisplay>,
    #[serde(rename = "gpu")]
    overlay_gpu: Option<OverlayGpu>,
    #[serde(rename = "root")]
    overlay_root: Option<OverlayRoot>,
    #[serde(rename = "google_services")]
    overlay_google_services: Option<OverlayGoogleServices>,
    #[serde(rename = "network")]
    overlay_network: Option<OverlayNetwork>,
    #[serde(rename = "device")]
    overlay_device: Option<OverlayDevice>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct OverlayInstance {
    name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct OverlayHardware {
    cpu_cores: Option<u32>,
    ram_mb: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct OverlayDisplay {
    width: Option<u32>,
    height: Option<u32>,
    dpi: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct OverlayGpu {
    backend: Option<GpuBackend>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct OverlayRoot {
    mode: Option<RootMode>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct OverlayGoogleServices {
    provider: Option<GoogleServicesProvider>,
    provider_version: Option<String>,
    gapps_source: Option<GAppsSource>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct OverlayNetwork {
    mode: Option<NetworkMode>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct OverlayDevice {
    model: Option<String>,
    manufacturer: Option<String>,
}

// Accessor helpers for the overlay — flattens the nested Option<Option<T>> into Option<T>.
impl InstanceConfigOverlay {
    pub fn instance_name(&self) -> Option<String> {
        self.overlay_instance.as_ref()?.name.clone()
    }
    pub fn cpu_cores(&self) -> Option<u32> {
        self.overlay_hardware.as_ref()?.cpu_cores
    }
    pub fn ram_mb(&self) -> Option<u32> {
        self.overlay_hardware.as_ref()?.ram_mb
    }
    pub fn display_width(&self) -> Option<u32> {
        self.overlay_display.as_ref()?.width
    }
    pub fn display_height(&self) -> Option<u32> {
        self.overlay_display.as_ref()?.height
    }
    pub fn display_dpi(&self) -> Option<u32> {
        self.overlay_display.as_ref()?.dpi
    }
    pub fn gpu_backend(&self) -> Option<GpuBackend> {
        self.overlay_gpu.as_ref()?.backend
    }
    pub fn root_mode(&self) -> Option<RootMode> {
        self.overlay_root.as_ref()?.mode
    }
    pub fn google_services_provider(&self) -> Option<GoogleServicesProvider> {
        self.overlay_google_services.as_ref()?.provider
    }
    pub fn google_services_provider_version(&self) -> Option<String> {
        self.overlay_google_services
            .as_ref()?
            .provider_version
            .clone()
    }
    pub fn google_services_gapps_source(&self) -> Option<GAppsSource> {
        self.overlay_google_services.as_ref()?.gapps_source
    }
    pub fn network_mode(&self) -> Option<NetworkMode> {
        self.overlay_network.as_ref()?.mode
    }
    pub fn device_model(&self) -> Option<String> {
        self.overlay_device.as_ref()?.model.clone()
    }
    pub fn device_manufacturer(&self) -> Option<String> {
        self.overlay_device.as_ref()?.manufacturer.clone()
    }
}
