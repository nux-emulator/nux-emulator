//! Prepares persistent disk images for direct kernel boot.

use std::path::PathBuf;
use std::process::Command;

/// Returns the persistent data directory: ~/.local/share/nux-emulator/
pub fn data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("nux-emulator")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".local/share/nux-emulator")
    }
}

/// Returns the AOSP product output directory.
pub fn product_out() -> PathBuf {
    PathBuf::from("/build2/nux-emulator/nux-android-image/aosp/out/target/product/vsoc_x86_64")
}

/// Returns the AOSP host tools output directory.
pub fn host_out() -> PathBuf {
    PathBuf::from("/build2/nux-emulator/nux-android-image/aosp/out/host/linux-x86")
}

/// Checks whether bootstrap has already been performed (userdata.img exists).
pub fn is_bootstrapped() -> bool {
    data_dir().join("userdata.img").exists()
}

/// Creates persistent disk images for the VM. Skips images that already exist.
pub fn bootstrap() -> Result<(), String> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create data dir: {e}"))?;

    create_f2fs_image(&dir.join("userdata.img"), "65G", "data")?;
    create_ext4_image(&dir.join("metadata.img"), "64M")?;
    create_zeroed_image(&dir.join("misc.img"), "1M")?;
    create_vfat_image(&dir.join("sdcard.img"), "2G")?;

    log::info!("Bootstrap complete: all disk images ready in {}", dir.display());
    Ok(())
}

fn create_f2fs_image(path: &PathBuf, size: &str, label: &str) -> Result<(), String> {
    if path.exists() {
        log::info!("Skipping {} (already exists)", path.display());
        return Ok(());
    }
    log::info!("Creating f2fs image: {}", path.display());
    run("truncate", &["-s", size, &path.to_string_lossy()])?;
    run("mkfs.f2fs", &["-f", "-l", label, &path.to_string_lossy()])?;
    Ok(())
}

fn create_ext4_image(path: &PathBuf, size: &str) -> Result<(), String> {
    if path.exists() {
        log::info!("Skipping {} (already exists)", path.display());
        return Ok(());
    }
    log::info!("Creating ext4 image: {}", path.display());
    run("truncate", &["-s", size, &path.to_string_lossy()])?;
    run("mkfs.ext4", &["-F", &path.to_string_lossy()])?;
    Ok(())
}

fn create_zeroed_image(path: &PathBuf, size: &str) -> Result<(), String> {
    if path.exists() {
        log::info!("Skipping {} (already exists)", path.display());
        return Ok(());
    }
    log::info!("Creating zeroed image: {}", path.display());
    run("truncate", &["-s", size, &path.to_string_lossy()])?;
    Ok(())
}

fn create_vfat_image(path: &PathBuf, size: &str) -> Result<(), String> {
    if path.exists() {
        log::info!("Skipping {} (already exists)", path.display());
        return Ok(());
    }
    log::info!("Creating vfat image: {}", path.display());
    run("truncate", &["-s", size, &path.to_string_lossy()])?;
    run("mkfs.vfat", &[&path.to_string_lossy()])?;
    Ok(())
}

fn run(cmd: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run {cmd}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{cmd} failed: {stderr}"));
    }
    Ok(())
}
