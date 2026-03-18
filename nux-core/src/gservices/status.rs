//! Status query API for Google Services.

use crate::config::GoogleServicesConfig;
use crate::gservices::detection::{detect_provider, detect_version};
use crate::gservices::types::{AdbShell, Freshness, GServicesResult, GoogleServicesStatus};

/// Query the current Google Services status for an instance.
///
/// Attempts live detection via ADB first. If ADB is unavailable,
/// falls back to the persisted config state with `Cached` freshness.
///
/// `restart_required` is passed through from the caller (tracked
/// externally, set after provider switches, cleared on VM start).
///
/// # Errors
///
/// Returns `GServicesError` if live detection fails and no fallback
/// is possible (in practice this always succeeds via the cached path).
pub async fn query_status(
    adb: &mut dyn AdbShell,
    config: &GoogleServicesConfig,
    restart_required: bool,
) -> GServicesResult<GoogleServicesStatus> {
    if adb.is_connected() {
        match live_status(adb, restart_required).await {
            Ok(status) => return Ok(status),
            Err(e) => {
                log::warn!("live detection failed, falling back to cache: {e}");
            }
        }
    }

    Ok(cached_status(config, restart_required))
}

/// Perform live detection and build a status.
async fn live_status(
    adb: &mut dyn AdbShell,
    restart_required: bool,
) -> GServicesResult<GoogleServicesStatus> {
    let (provider, freshness) = detect_provider(adb).await?;
    let version = detect_version(adb, provider).await?;

    Ok(GoogleServicesStatus {
        provider,
        version,
        freshness,
        restart_required,
    })
}

/// Build a status from cached config.
fn cached_status(config: &GoogleServicesConfig, restart_required: bool) -> GoogleServicesStatus {
    GoogleServicesStatus {
        provider: config.provider,
        version: config.provider_version.clone(),
        freshness: Freshness::Cached,
        restart_required,
    }
}

/// Persist updated provider and version to the config after detection.
pub fn update_config_from_status(config: &mut GoogleServicesConfig, status: &GoogleServicesStatus) {
    if status.freshness == Freshness::Live {
        config.provider = status.provider;
        config.provider_version.clone_from(&status.version);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GoogleServicesProvider;

    /// Mock ADB that returns canned responses.
    struct MockAdb {
        connected: bool,
        packages: String,
        dumpsys: String,
    }

    impl AdbShell for MockAdb {
        fn shell_exec(
            &mut self,
            command: &str,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + '_>>
        {
            let result = if command.contains("pm list packages") {
                Ok(self.packages.clone())
            } else if command.contains("dumpsys package") {
                Ok(self.dumpsys.clone())
            } else {
                Err(format!("unknown command: {command}"))
            };
            Box::pin(async move { result })
        }

        fn is_connected(&self) -> bool {
            self.connected
        }
    }

    #[tokio::test]
    async fn live_status_microg() {
        let mut adb = MockAdb {
            connected: true,
            packages: "package:org.microg.gms.droidguard\npackage:com.android.settings\n"
                .to_owned(),
            dumpsys: "versionName=0.3.1.4\n".to_owned(),
        };
        let config = GoogleServicesConfig::default();
        let status = query_status(&mut adb, &config, false).await.unwrap();
        assert_eq!(status.provider, GoogleServicesProvider::Microg);
        assert_eq!(status.version.as_deref(), Some("0.3.1.4"));
        assert_eq!(status.freshness, Freshness::Live);
        assert!(!status.restart_required);
    }

    #[tokio::test]
    async fn live_status_gapps() {
        let mut adb = MockAdb {
            connected: true,
            packages: "package:com.google.android.gms\npackage:com.google.android.gsf\n".to_owned(),
            dumpsys: "versionName=24.09.13\n".to_owned(),
        };
        let config = GoogleServicesConfig::default();
        let status = query_status(&mut adb, &config, true).await.unwrap();
        assert_eq!(status.provider, GoogleServicesProvider::Gapps);
        assert_eq!(status.version.as_deref(), Some("24.09.13"));
        assert_eq!(status.freshness, Freshness::Live);
        assert!(status.restart_required);
    }

    #[tokio::test]
    async fn cached_fallback_when_disconnected() {
        let mut adb = MockAdb {
            connected: false,
            packages: String::new(),
            dumpsys: String::new(),
        };
        let config = GoogleServicesConfig {
            provider: GoogleServicesProvider::Gapps,
            provider_version: Some("24.09.13".to_owned()),
            ..GoogleServicesConfig::default()
        };
        let status = query_status(&mut adb, &config, false).await.unwrap();
        assert_eq!(status.provider, GoogleServicesProvider::Gapps);
        assert_eq!(status.version.as_deref(), Some("24.09.13"));
        assert_eq!(status.freshness, Freshness::Cached);
    }

    #[tokio::test]
    async fn none_provider_live() {
        let mut adb = MockAdb {
            connected: true,
            packages: "package:com.android.settings\n".to_owned(),
            dumpsys: String::new(),
        };
        let config = GoogleServicesConfig::default();
        let status = query_status(&mut adb, &config, false).await.unwrap();
        assert_eq!(status.provider, GoogleServicesProvider::None);
        assert_eq!(status.version, None);
        assert_eq!(status.freshness, Freshness::Live);
    }

    #[test]
    fn update_config_from_live_status() {
        let mut config = GoogleServicesConfig::default();
        let status = GoogleServicesStatus {
            provider: GoogleServicesProvider::Gapps,
            version: Some("24.09.13".to_owned()),
            freshness: Freshness::Live,
            restart_required: false,
        };
        update_config_from_status(&mut config, &status);
        assert_eq!(config.provider, GoogleServicesProvider::Gapps);
        assert_eq!(config.provider_version.as_deref(), Some("24.09.13"));
    }

    #[test]
    fn update_config_skips_cached_status() {
        let mut config = GoogleServicesConfig {
            provider: GoogleServicesProvider::Microg,
            provider_version: Some("0.3.0".to_owned()),
            ..GoogleServicesConfig::default()
        };
        let status = GoogleServicesStatus {
            provider: GoogleServicesProvider::Gapps,
            version: Some("24.09.13".to_owned()),
            freshness: Freshness::Cached,
            restart_required: false,
        };
        update_config_from_status(&mut config, &status);
        // Should NOT update from cached
        assert_eq!(config.provider, GoogleServicesProvider::Microg);
        assert_eq!(config.provider_version.as_deref(), Some("0.3.0"));
    }
}
