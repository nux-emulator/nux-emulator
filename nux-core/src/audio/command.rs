//! crosvm argument building for virtio-snd audio.

use super::config::AudioConfig;
use std::ffi::OsString;

/// Build crosvm CLI arguments for virtio-snd audio.
///
/// When audio is enabled, appends `--virtio-snd` with ALSA backend
/// configuration including period size and period count for low latency.
/// Returns an empty vec when audio is disabled.
pub fn build_audio_args(config: &AudioConfig) -> Vec<OsString> {
    if !config.enabled {
        return Vec::new();
    }

    vec![
        "--virtio-snd".into(),
        format!(
            "backend=alsa,period-size={},period-count={}",
            config.period_size, config.period_count
        )
        .into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_enabled_produces_virtio_snd_args() {
        let config = AudioConfig::default();
        let args = build_audio_args(&config);
        let strings: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(strings.len(), 2);
        assert_eq!(strings[0], "--virtio-snd");
        assert_eq!(strings[1], "backend=alsa,period-size=256,period-count=2");
    }

    #[test]
    fn audio_disabled_produces_no_args() {
        let config = AudioConfig {
            enabled: false,
            ..AudioConfig::default()
        };
        let args = build_audio_args(&config);
        assert!(args.is_empty());
    }

    #[test]
    fn custom_period_params() {
        let config = AudioConfig {
            period_size: 512,
            period_count: 4,
            ..AudioConfig::default()
        };
        let args = build_audio_args(&config);
        let strings: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(strings[1], "backend=alsa,period-size=512,period-count=4");
    }
}
