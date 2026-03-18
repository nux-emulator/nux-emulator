//! `GApps` package download and SHA-256 verification.

use crate::config::GAppsSource;
use crate::gservices::types::{GAppsPackageInfo, GServicesError, GServicesResult};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// Return the XDG cache directory for `GApps` packages.
///
/// Resolves to `$XDG_DATA_HOME/nux/cache/gapps/`, falling back to
/// `~/.local/share/nux/cache/gapps/`.
///
/// # Panics
///
/// Panics if the home directory cannot be determined.
pub fn gapps_cache_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| {
        let home = dirs::home_dir().expect("cannot determine home directory");
        home.join(".local").join("share")
    });
    base.join("nux").join("cache").join("gapps")
}

/// Resolve download metadata for a `GApps` source.
///
/// In a real implementation these URLs would come from a remote manifest
/// or be configurable. For now we return well-known URL patterns.
pub fn resolve_package_info(source: GAppsSource) -> GAppsPackageInfo {
    match source {
        GAppsSource::Opengapps => GAppsPackageInfo {
            url: "https://github.com/opengapps/opengapps/releases/download/latest/open_gapps-x86_64-16.0-pico.zip".to_owned(),
            sha256: String::new(), // populated from manifest at runtime
            source,
            filename: "open_gapps-x86_64-16.0-pico.zip".to_owned(),
        },
        GAppsSource::Mindthegapps => GAppsPackageInfo {
            url: "https://github.com/AuroraOSS/AuroraMindTheGapps/releases/download/latest/MindTheGapps-16.0-x86_64.zip".to_owned(),
            sha256: String::new(),
            source,
            filename: "MindTheGapps-16.0-x86_64.zip".to_owned(),
        },
    }
}

/// Check whether a verified cached package already exists.
pub fn cached_package_path(info: &GAppsPackageInfo) -> Option<PathBuf> {
    let path = gapps_cache_dir().join(&info.filename);
    if path.is_file() { Some(path) } else { None }
}

/// Download a `GApps` package to the cache directory.
///
/// If a cached copy already exists, returns its path without re-downloading.
/// Reports progress via the callback `on_progress(bytes_downloaded, total_bytes)`.
///
/// # Errors
///
/// Returns `GServicesError::DownloadFailed` on HTTP errors or I/O failures.
pub async fn download_gapps<F>(info: &GAppsPackageInfo, on_progress: F) -> GServicesResult<PathBuf>
where
    F: Fn(u64, u64),
{
    // Check cache first
    if let Some(cached) = cached_package_path(info) {
        log::info!("using cached GApps package: {}", cached.display());
        return Ok(cached);
    }

    let cache_dir = gapps_cache_dir();
    tokio::fs::create_dir_all(&cache_dir).await?;

    let dest = cache_dir.join(&info.filename);
    let tmp = cache_dir.join(format!("{}.part", &info.filename));

    let mut response = reqwest::get(&info.url)
        .await
        .map_err(|e| GServicesError::DownloadFailed(format!("HTTP request failed: {e}")))?;

    if !response.status().is_success() {
        return Err(GServicesError::DownloadFailed(format!(
            "HTTP {}",
            response.status()
        )));
    }

    let total = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(&tmp).await?;

    // Read chunks without requiring tokio-stream
    loop {
        let chunk = response
            .chunk()
            .await
            .map_err(|e| GServicesError::DownloadFailed(format!("stream error: {e}")))?;
        let Some(chunk) = chunk else { break };
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total);
    }

    file.flush().await?;
    drop(file);

    // Move temp to final location
    tokio::fs::rename(&tmp, &dest).await?;

    log::info!("downloaded GApps package: {}", dest.display());
    Ok(dest)
}

/// Verify the SHA-256 hash of a downloaded file.
///
/// If the hash is empty (not yet known), verification is skipped.
///
/// # Errors
///
/// Returns `GServicesError::HashMismatch` if the hash doesn't match,
/// and deletes the file.
pub async fn verify_hash(path: &Path, expected_hex: &str) -> GServicesResult<()> {
    if expected_hex.is_empty() {
        log::warn!("no expected hash provided, skipping verification");
        return Ok(());
    }

    let data = tokio::fs::read(path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let actual = format!("{:x}", hasher.finalize());

    if actual != expected_hex {
        // Delete corrupted file
        let _ = tokio::fs::remove_file(path).await;
        return Err(GServicesError::HashMismatch {
            expected: expected_hex.to_owned(),
            actual,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_dir_ends_correctly() {
        let dir = gapps_cache_dir();
        assert!(dir.ends_with("nux/cache/gapps"));
    }

    #[test]
    fn resolve_opengapps_info() {
        let info = resolve_package_info(GAppsSource::Opengapps);
        assert_eq!(info.source, GAppsSource::Opengapps);
        assert!(info.filename.contains("open_gapps"));
        assert!(info.url.contains("opengapps"));
    }

    #[test]
    fn resolve_mindthegapps_info() {
        let info = resolve_package_info(GAppsSource::Mindthegapps);
        assert_eq!(info.source, GAppsSource::Mindthegapps);
        assert!(info.filename.contains("MindTheGapps"));
    }

    #[test]
    fn cached_package_path_returns_none_for_missing() {
        let info = GAppsPackageInfo {
            url: String::new(),
            sha256: String::new(),
            source: GAppsSource::Opengapps,
            filename: "nonexistent-test-file.zip".to_owned(),
        };
        assert!(cached_package_path(&info).is_none());
    }

    #[tokio::test]
    async fn verify_hash_empty_skips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        tokio::fs::write(&path, b"hello").await.unwrap();
        // Empty hash => skip
        verify_hash(&path, "").await.unwrap();
        assert!(path.exists());
    }

    #[tokio::test]
    async fn verify_hash_correct() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        tokio::fs::write(&path, b"hello").await.unwrap();
        // SHA-256 of "hello"
        let expected = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        verify_hash(&path, expected).await.unwrap();
    }

    #[tokio::test]
    async fn verify_hash_mismatch_deletes_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        tokio::fs::write(&path, b"hello").await.unwrap();
        let result = verify_hash(
            &path,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .await;
        assert!(result.is_err());
        assert!(!path.exists(), "file should be deleted on hash mismatch");
    }
}
