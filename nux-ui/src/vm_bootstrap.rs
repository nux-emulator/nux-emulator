//! Prepares disk images for QEMU direct kernel boot with Android Emulator images.

use std::path::PathBuf;
use std::process::Command;

/// Returns the persistent data directory: ~/.local/share/nux-emulator/
pub fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".local/share/nux-emulator")
}

/// Returns the Android Emulator system images directory.
pub fn sysimg_dir() -> PathBuf {
    PathBuf::from("/home/kk/Android/Sdk/system-images/android-35/google_apis/x86_64")
}

/// Returns the AOSP host tools output directory.
pub fn host_out() -> PathBuf {
    PathBuf::from("/build2/nux-emulator/nux-android-image/aosp/out/host/linux-x86")
}

/// Checks whether bootstrap has been performed.
pub fn is_bootstrapped() -> bool {
    data_dir().join("userdata.img").exists()
}

/// Creates persistent disk images (userdata + cache).
/// System/vendor/kernel/ramdisk come from the SDK system images (read-only).
pub fn bootstrap() -> Result<(), String> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;

    create_sparse_image(&dir.join("userdata.img"), "16G", "ext4")?;
    create_sparse_image(&dir.join("cache.img"), "2G", "ext4")?;

    log::info!("bootstrap: complete — images ready in {}", dir.display());
    Ok(())
}

fn create_sparse_image(path: &std::path::Path, size: &str, fs: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    log::info!("bootstrap: creating {} ({} {})...", path.display(), size, fs);

    let s = Command::new("truncate")
        .args(["-s", size, &path.to_string_lossy()])
        .status()
        .map_err(|e| format!("truncate: {e}"))?;
    if !s.success() {
        return Err(format!("truncate {} failed", path.display()));
    }

    match fs {
        "ext4" => {
            let s = Command::new("mkfs.ext4")
                .args(["-F", &path.to_string_lossy()])
                .status()
                .map_err(|e| format!("mkfs.ext4: {e}"))?;
            if !s.success() {
                return Err(format!("mkfs.ext4 {} failed", path.display()));
            }
        }
        _ => {}
    }
    Ok(())
}
