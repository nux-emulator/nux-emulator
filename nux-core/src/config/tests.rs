//! Tests for the config module.

use super::*;

// ── Schema round-trip tests ──

#[test]
fn round_trip_default_config() {
    let config = NuxConfig::default();
    let toml_str = toml::to_string_pretty(&config).unwrap();
    let parsed: NuxConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(config, parsed);
}

#[test]
fn deserialize_complete_toml() {
    let toml_str = r#"
schema_version = 1

[instance]
name = "gaming"

[hardware]
cpu_cores = 8
ram_mb = 8192

[display]
width = 2560
height = 1440
dpi = 480

[gpu]
backend = "gfxstream"

[root]
mode = "magisk"

[google_services]
provider = "gapps"

[network]
mode = "bridged"

[device]
model = "Pixel 9 Pro"
manufacturer = "Google"
"#;
    let config: NuxConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.instance.name, "gaming");
    assert_eq!(config.hardware.cpu_cores, 8);
    assert_eq!(config.hardware.ram_mb, 8192);
    assert_eq!(config.display.width, 2560);
    assert_eq!(config.display.height, 1440);
    assert_eq!(config.display.dpi, 480);
    assert_eq!(config.gpu.backend, GpuBackend::Gfxstream);
    assert_eq!(config.root.mode, RootMode::Magisk);
    assert_eq!(
        config.google_services.provider,
        GoogleServicesProvider::Gapps
    );
    assert_eq!(config.network.mode, NetworkMode::Bridged);
    assert_eq!(config.device.model, "Pixel 9 Pro");
    assert_eq!(config.device.manufacturer, "Google");
}

#[test]
fn deserialize_minimal_toml_uses_defaults() {
    let toml_str = "schema_version = 1\n";
    let config: NuxConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.hardware.cpu_cores, 2);
    assert_eq!(config.hardware.ram_mb, 2048);
    assert_eq!(config.display.width, 1080);
    assert_eq!(config.display.height, 1920);
    assert_eq!(config.display.dpi, 320);
    assert_eq!(config.gpu.backend, GpuBackend::Gfxstream);
    assert_eq!(config.root.mode, RootMode::None);
    assert_eq!(
        config.google_services.provider,
        GoogleServicesProvider::Microg
    );
    assert_eq!(config.network.mode, NetworkMode::Nat);
}

#[test]
fn reject_unknown_gpu_backend() {
    let toml_str = r#"
[gpu]
backend = "invalid"
"#;
    let result: Result<NuxConfig, _> = toml::from_str(toml_str);
    assert!(result.is_err());
}

#[test]
fn reject_unknown_root_mode() {
    let toml_str = r#"
[root]
mode = "supersu"
"#;
    let result: Result<NuxConfig, _> = toml::from_str(toml_str);
    assert!(result.is_err());
}

#[test]
fn all_enum_variants_serialize() {
    for backend in [
        GpuBackend::Gfxstream,
        GpuBackend::Virglrenderer,
        GpuBackend::Software,
    ] {
        let s = toml::to_string(&GpuConfig { backend }).unwrap();
        let parsed: GpuConfig = toml::from_str(&s).unwrap();
        assert_eq!(parsed.backend, backend);
    }
    for mode in [
        RootMode::None,
        RootMode::Magisk,
        RootMode::Kernelsu,
        RootMode::Apatch,
    ] {
        let s = toml::to_string(&RootConfig { mode }).unwrap();
        let parsed: RootConfig = toml::from_str(&s).unwrap();
        assert_eq!(parsed.mode, mode);
    }
    for provider in [
        GoogleServicesProvider::None,
        GoogleServicesProvider::Microg,
        GoogleServicesProvider::Gapps,
    ] {
        let cfg = GoogleServicesConfig {
            provider,
            ..GoogleServicesConfig::default()
        };
        let s = toml::to_string(&cfg).unwrap();
        let parsed: GoogleServicesConfig = toml::from_str(&s).unwrap();
        assert_eq!(parsed.provider, provider);
    }
    for mode in [NetworkMode::Nat, NetworkMode::Bridged] {
        let s = toml::to_string(&NetworkConfig { mode }).unwrap();
        let parsed: NetworkConfig = toml::from_str(&s).unwrap();
        assert_eq!(parsed.mode, mode);
    }
}

// ── Path resolution tests ──

#[test]
fn global_config_path_ends_correctly() {
    let path = global_config_path();
    assert!(path.ends_with("nux/config.toml"));
}

#[test]
fn instance_config_path_contains_name() {
    let path = instance_config_path("gaming");
    assert!(path.ends_with("nux/instances/gaming/config.toml"));
}

#[test]
fn instance_config_path_default() {
    let path = instance_config_path("default");
    assert!(path.ends_with("nux/instances/default/config.toml"));
}

// ── Merge tests ──

#[test]
fn merge_with_empty_overlay_returns_global() {
    let global = NuxConfig {
        hardware: HardwareConfig {
            cpu_cores: 4,
            ram_mb: 4096,
        },
        ..NuxConfig::default()
    };
    let overlay = InstanceConfigOverlay::default();
    let merged = merge(global.clone(), &overlay);
    assert_eq!(merged.hardware.cpu_cores, 4);
    assert_eq!(merged.hardware.ram_mb, 4096);
    assert_eq!(merged.display, global.display);
}

#[test]
fn merge_with_partial_overlay() {
    let global = NuxConfig::default();
    let overlay_toml = r#"
[hardware]
ram_mb = 8192
"#;
    let overlay: InstanceConfigOverlay = toml::from_str(overlay_toml).unwrap();
    let merged = merge(global, &overlay);
    assert_eq!(merged.hardware.ram_mb, 8192);
    assert_eq!(merged.hardware.cpu_cores, 2); // inherited from global default
}

#[test]
fn merge_with_full_overlay() {
    let global = NuxConfig::default();
    let overlay_toml = r#"
[instance]
name = "test"

[hardware]
cpu_cores = 8
ram_mb = 16384

[display]
width = 2560
height = 1440
dpi = 480

[gpu]
backend = "virglrenderer"

[root]
mode = "kernelsu"

[google_services]
provider = "none"

[network]
mode = "bridged"

[device]
model = "SM-S928B"
manufacturer = "Samsung"
"#;
    let overlay: InstanceConfigOverlay = toml::from_str(overlay_toml).unwrap();
    let merged = merge(global, &overlay);
    assert_eq!(merged.instance.name, "test");
    assert_eq!(merged.hardware.cpu_cores, 8);
    assert_eq!(merged.hardware.ram_mb, 16384);
    assert_eq!(merged.display.width, 2560);
    assert_eq!(merged.gpu.backend, GpuBackend::Virglrenderer);
    assert_eq!(merged.root.mode, RootMode::Kernelsu);
    assert_eq!(
        merged.google_services.provider,
        GoogleServicesProvider::None
    );
    assert_eq!(merged.network.mode, NetworkMode::Bridged);
    assert_eq!(merged.device.model, "SM-S928B");
    assert_eq!(merged.device.manufacturer, "Samsung");
}

#[test]
fn merge_multiple_sections_partial() {
    let global = NuxConfig::default();
    let overlay_toml = r#"
[hardware]
cpu_cores = 6

[display]
dpi = 160
"#;
    let overlay: InstanceConfigOverlay = toml::from_str(overlay_toml).unwrap();
    let merged = merge(global, &overlay);
    assert_eq!(merged.hardware.cpu_cores, 6);
    assert_eq!(merged.hardware.ram_mb, 2048); // inherited
    assert_eq!(merged.display.dpi, 160);
    assert_eq!(merged.display.width, 1080); // inherited
}

// ── Validation tests ──

#[test]
fn valid_config_passes_validation() {
    let config = NuxConfig::default();
    let errors = validate(&config);
    assert!(errors.is_empty());
}

#[test]
fn invalid_cpu_cores() {
    let config = NuxConfig {
        hardware: HardwareConfig {
            cpu_cores: 0,
            ..HardwareConfig::default()
        },
        ..NuxConfig::default()
    };
    let errors = validate(&config);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0], ConfigError::CpuCoresTooLow(0));
}

#[test]
fn invalid_ram() {
    let config = NuxConfig {
        hardware: HardwareConfig {
            ram_mb: 128,
            ..HardwareConfig::default()
        },
        ..NuxConfig::default()
    };
    let errors = validate(&config);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0], ConfigError::RamTooLow(128));
}

#[test]
fn invalid_display_dimensions() {
    let config = NuxConfig {
        display: DisplayConfig {
            width: 0,
            height: 0,
            dpi: 0,
        },
        ..NuxConfig::default()
    };
    let errors = validate(&config);
    assert_eq!(errors.len(), 3);
    assert!(errors.contains(&ConfigError::DisplayWidthTooLow(0)));
    assert!(errors.contains(&ConfigError::DisplayHeightTooLow(0)));
    assert!(errors.contains(&ConfigError::DpiTooLow(0)));
}

#[test]
fn multiple_validation_errors() {
    let config = NuxConfig {
        hardware: HardwareConfig {
            cpu_cores: 0,
            ram_mb: 0,
        },
        ..NuxConfig::default()
    };
    let errors = validate(&config);
    assert_eq!(errors.len(), 2);
    assert!(errors.contains(&ConfigError::CpuCoresTooLow(0)));
    assert!(errors.contains(&ConfigError::RamTooLow(0)));
}

// ── Save tests ──

#[test]
fn save_creates_directories_and_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deep").join("nested").join("config.toml");

    let config = NuxConfig::default();
    save(&config, &path).unwrap();

    assert!(path.exists());
    let content = std::fs::read_to_string(&path).unwrap();
    let loaded: NuxConfig = toml::from_str(&content).unwrap();
    assert_eq!(config, loaded);
}

#[test]
fn save_overwrites_existing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");

    let config1 = NuxConfig::default();
    save(&config1, &path).unwrap();

    let config2 = NuxConfig {
        hardware: HardwareConfig {
            cpu_cores: 16,
            ram_mb: 32768,
        },
        ..NuxConfig::default()
    };
    save(&config2, &path).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    let loaded: NuxConfig = toml::from_str(&content).unwrap();
    assert_eq!(loaded.hardware.cpu_cores, 16);
    assert_eq!(loaded.hardware.ram_mb, 32768);
}

// ── Migration tests ──

#[test]
fn current_version_skips_migration() {
    let toml_str = format!("schema_version = {CURRENT_SCHEMA_VERSION}\n");
    let raw: toml::Value = toml_str.parse().unwrap();
    let result = migrate(raw.clone()).unwrap();
    assert_eq!(
        result.get("schema_version").unwrap().as_integer().unwrap(),
        i64::from(CURRENT_SCHEMA_VERSION)
    );
}

#[test]
fn missing_version_treated_as_v1() {
    let raw: toml::Value = "[hardware]\ncpu_cores = 4\n".parse().unwrap();
    let result = migrate(raw).unwrap();
    assert_eq!(
        result.get("schema_version").unwrap().as_integer().unwrap(),
        i64::from(CURRENT_SCHEMA_VERSION)
    );
}

#[test]
fn future_version_rejected() {
    let raw: toml::Value = "schema_version = 99\n".parse().unwrap();
    let result = migrate(raw);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("newer version"));
}

// ── Integration test: full load/merge/validate/save round-trip ──

#[test]
fn integration_load_merge_validate_save() {
    let dir = tempfile::tempdir().unwrap();
    let global_path = dir.path().join("global.toml");
    let instance_path = dir.path().join("instance.toml");
    let output_path = dir.path().join("output.toml");

    // Write global config
    let global = NuxConfig {
        hardware: HardwareConfig {
            cpu_cores: 4,
            ram_mb: 4096,
        },
        display: DisplayConfig {
            width: 1920,
            height: 1080,
            dpi: 240,
        },
        ..NuxConfig::default()
    };
    save(&global, &global_path).unwrap();

    // Write instance overlay
    std::fs::write(
        &instance_path,
        "[hardware]\nram_mb = 8192\n\n[gpu]\nbackend = \"virglrenderer\"\n",
    )
    .unwrap();

    // Load and migrate global
    let loaded_global: NuxConfig = {
        let content = std::fs::read_to_string(&global_path).unwrap();
        let raw: toml::Value = content.parse().unwrap();
        let migrated = migrate(raw).unwrap();
        migrated.try_into().unwrap()
    };

    // Load overlay
    let overlay: InstanceConfigOverlay = {
        let content = std::fs::read_to_string(&instance_path).unwrap();
        toml::from_str(&content).unwrap()
    };

    // Merge
    let merged = merge(loaded_global, &overlay);
    assert_eq!(merged.hardware.cpu_cores, 4); // from global
    assert_eq!(merged.hardware.ram_mb, 8192); // from instance
    assert_eq!(merged.gpu.backend, GpuBackend::Virglrenderer); // from instance
    assert_eq!(merged.display.width, 1920); // from global

    // Validate
    let errors = validate(&merged);
    assert!(errors.is_empty());

    // Save and re-read
    save(&merged, &output_path).unwrap();
    let final_content = std::fs::read_to_string(&output_path).unwrap();
    let final_config: NuxConfig = toml::from_str(&final_content).unwrap();
    assert_eq!(merged, final_config);
}
