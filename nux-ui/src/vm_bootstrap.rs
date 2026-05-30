//! Prepares disk images for QEMU direct kernel boot.

use std::path::PathBuf;
use std::process::Command;

/// Returns the persistent data directory: ~/.local/share/nux-emulator/
pub fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".local/share/nux-emulator")
}

/// Returns the AOSP product output directory.
pub fn product_out() -> PathBuf {
    PathBuf::from("/build2/nux-emulator/nux-android-image/aosp/out/target/product/vsoc_x86_64")
}

/// Returns the AOSP host tools output directory.
pub fn host_out() -> PathBuf {
    PathBuf::from("/build2/nux-emulator/nux-android-image/aosp/out/host/linux-x86")
}

/// Checks whether bootstrap has been performed.
pub fn is_bootstrapped() -> bool {
    data_dir().join("userdata.img").exists()
}

/// Creates persistent disk images and combined ramdisk.
pub fn bootstrap() -> Result<(), String> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;

    create_combined_ramdisk(&dir.join("combined_ramdisk.img"))?;
    create_sparse_image(&dir.join("userdata.img"), "65G", "f2fs", "data")?;
    create_sparse_image(&dir.join("cache.img"), "2G", "ext4", "")?;

    log::info!("bootstrap: complete — images ready in {}", dir.display());
    Ok(())
}

fn create_combined_ramdisk(path: &std::path::Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    log::info!("bootstrap: creating combined ramdisk...");

    let product = product_out();
    let host = host_out();
    let tmp = std::env::temp_dir().join("nux_ramdisk_tmp");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| format!("mkdir tmp: {e}"))?;

    // Extract vendor ramdisk
    let output = Command::new(host.join("bin/unpack_bootimg"))
        .args(["--boot_img", &product.join("vendor_boot.img").to_string_lossy(),
               "--out", &tmp.to_string_lossy()])
        .output()
        .map_err(|e| format!("unpack_bootimg: {e}"))?;
    if !output.status.success() {
        return Err(format!("unpack_bootimg failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    // Concatenate vendor + generic ramdisk
    let vendor_rd = tmp.join("vendor_ramdisk00");
    let generic_rd = product.join("ramdisk.img");
    let output = Command::new("sh")
        .args(["-c", &format!("cat '{}' '{}' > '{}'",
            vendor_rd.display(), generic_rd.display(), path.display())])
        .output()
        .map_err(|e| format!("cat: {e}"))?;
    if !output.status.success() {
        return Err("cat ramdisks failed".into());
    }

    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}

fn create_sparse_image(path: &std::path::Path, size: &str, fs: &str, label: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    log::info!("bootstrap: creating {} ({} {})...", path.display(), size, fs);

    // Create sparse file
    let s = Command::new("truncate")
        .args(["-s", size, &path.to_string_lossy()])
        .status()
        .map_err(|e| format!("truncate: {e}"))?;
    if !s.success() {
        return Err(format!("truncate {} failed", path.display()));
    }

    // Format
    match fs {
        "f2fs" => {
            let mut args = vec!["-f".to_string()];
            if !label.is_empty() {
                args.push("-l".to_string());
                args.push(label.to_string());
            }
            args.push(path.to_string_lossy().to_string());
            let s = Command::new("mkfs.f2fs").args(&args).status()
                .map_err(|e| format!("mkfs.f2fs: {e}"))?;
            if !s.success() {
                return Err(format!("mkfs.f2fs {} failed", path.display()));
            }
        }
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
