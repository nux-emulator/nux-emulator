//! Provider detection via ADB package queries.

use crate::config::GoogleServicesProvider;
use crate::gservices::types::{AdbShell, Freshness, GServicesResult};

/// Package identifier for Google Play Services (`GApps`).
const GAPPS_GMS_PACKAGE: &str = "com.google.android.gms";
/// Package identifier for Google Services Framework (`GApps`).
const GAPPS_GSF_PACKAGE: &str = "com.google.android.gsf";
/// Package identifier for `MicroG` `DroidGuard`.
const MICROG_PACKAGE: &str = "org.microg.gms.droidguard";

/// Detect the active provider by querying installed packages via ADB.
///
/// Returns the detected provider and `Freshness::Live`.
///
/// # Errors
///
/// Returns `GServicesError::AdbError` if the shell command fails.
pub async fn detect_provider(
    adb: &mut dyn AdbShell,
) -> GServicesResult<(GoogleServicesProvider, Freshness)> {
    let output = adb
        .shell_exec("pm list packages")
        .await
        .map_err(crate::gservices::types::GServicesError::AdbError)?;

    let provider = parse_provider_from_packages(&output);
    Ok((provider, Freshness::Live))
}

/// Detect the version of the active provider via ADB.
///
/// Returns `None` for the `None` provider.
///
/// # Errors
///
/// Returns `GServicesError::AdbError` if the shell command fails.
pub async fn detect_version(
    adb: &mut dyn AdbShell,
    provider: GoogleServicesProvider,
) -> GServicesResult<Option<String>> {
    let package = match provider {
        GoogleServicesProvider::Gapps => GAPPS_GMS_PACKAGE,
        GoogleServicesProvider::Microg => MICROG_PACKAGE,
        GoogleServicesProvider::None => return Ok(None),
    };

    let cmd = format!("dumpsys package {package}");
    let output = adb
        .shell_exec(&cmd)
        .await
        .map_err(crate::gservices::types::GServicesError::AdbError)?;

    Ok(parse_version_name(&output))
}

/// Parse `pm list packages` output to determine the active provider.
pub fn parse_provider_from_packages(output: &str) -> GoogleServicesProvider {
    let has_gapps_gms = output.lines().any(|l| {
        l.trim()
            .strip_prefix("package:")
            .is_some_and(|p| p == GAPPS_GMS_PACKAGE)
    });
    let has_gapps_gsf = output.lines().any(|l| {
        l.trim()
            .strip_prefix("package:")
            .is_some_and(|p| p == GAPPS_GSF_PACKAGE)
    });
    let has_microg = output.lines().any(|l| {
        l.trim()
            .strip_prefix("package:")
            .is_some_and(|p| p == MICROG_PACKAGE)
    });

    if has_gapps_gms && has_gapps_gsf {
        GoogleServicesProvider::Gapps
    } else if has_microg {
        GoogleServicesProvider::Microg
    } else {
        GoogleServicesProvider::None
    }
}

/// Parse `versionName` from `dumpsys package` output.
pub fn parse_version_name(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("versionName=") {
            let version = rest.trim();
            if !version.is_empty() {
                return Some(version.to_owned());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_microg_from_packages() {
        let output = "\
package:com.android.settings\n\
package:org.microg.gms.droidguard\n\
package:com.android.launcher3\n";
        assert_eq!(
            parse_provider_from_packages(output),
            GoogleServicesProvider::Microg
        );
    }

    #[test]
    fn detect_gapps_from_packages() {
        let output = "\
package:com.google.android.gms\n\
package:com.google.android.gsf\n\
package:com.android.settings\n";
        assert_eq!(
            parse_provider_from_packages(output),
            GoogleServicesProvider::Gapps
        );
    }

    #[test]
    fn detect_none_from_packages() {
        let output = "\
package:com.android.settings\n\
package:com.android.launcher3\n";
        assert_eq!(
            parse_provider_from_packages(output),
            GoogleServicesProvider::None
        );
    }

    #[test]
    fn detect_gapps_takes_priority_over_microg() {
        // If both are present (shouldn't happen, but be safe), GApps wins.
        let output = "\
package:com.google.android.gms\n\
package:com.google.android.gsf\n\
package:org.microg.gms.droidguard\n";
        assert_eq!(
            parse_provider_from_packages(output),
            GoogleServicesProvider::Gapps
        );
    }

    #[test]
    fn parse_version_from_dumpsys() {
        let output = "\
    Packages:\n\
      Package [com.google.android.gms] (abc123):\n\
        userId=10001\n\
        versionCode=240913000\n\
        versionName=24.09.13\n\
        targetSdk=34\n";
        assert_eq!(parse_version_name(output), Some("24.09.13".to_owned()));
    }

    #[test]
    fn parse_version_missing() {
        let output = "some random output\nno version here\n";
        assert_eq!(parse_version_name(output), None);
    }

    #[test]
    fn parse_version_empty_value() {
        let output = "versionName=\n";
        assert_eq!(parse_version_name(output), None);
    }
}
