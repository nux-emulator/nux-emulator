//! KVM detection and capability checks.

#![allow(unsafe_code)]

use super::error::{VmError, VmResult};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Result of a single detection check.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

/// Aggregated KVM readiness report.
#[derive(Debug, Clone)]
pub struct KvmReadinessReport {
    pub ready: bool,
    pub checks: Vec<CheckResult>,
    pub warnings: Vec<String>,
}

/// Run all KVM readiness checks and return a structured report.
pub fn check_kvm_readiness() -> KvmReadinessReport {
    check_kvm_readiness_with_path("/dev/kvm")
}

/// Run all KVM readiness checks against a specific device path (for testing).
pub fn check_kvm_readiness_with_path(kvm_path: &str) -> KvmReadinessReport {
    let mut checks = Vec::new();
    let mut warnings = Vec::new();
    let mut all_passed = true;

    // Check 1: /dev/kvm existence and permissions
    match check_kvm_device(kvm_path) {
        Ok(()) => {
            checks.push(CheckResult {
                name: "kvm_device".to_owned(),
                passed: true,
                detail: format!("{kvm_path} is accessible"),
            });
        }
        Err(e) => {
            all_passed = false;
            checks.push(CheckResult {
                name: "kvm_device".to_owned(),
                passed: false,
                detail: e.to_string(),
            });
        }
    }

    // Check 2: KVM API version (only if device is accessible)
    if checks[0].passed {
        match check_kvm_api_version(kvm_path) {
            Ok(version) => {
                checks.push(CheckResult {
                    name: "kvm_api_version".to_owned(),
                    passed: true,
                    detail: format!("KVM API version {version}"),
                });
            }
            Err(e) => {
                all_passed = false;
                checks.push(CheckResult {
                    name: "kvm_api_version".to_owned(),
                    passed: false,
                    detail: e.to_string(),
                });
            }
        }
    }

    // Check 3: CPU virtualization features
    match check_cpu_virtualization() {
        Ok(info) => {
            checks.push(CheckResult {
                name: "cpu_virtualization".to_owned(),
                passed: true,
                detail: info,
            });
            // Check for EPT/NPT (warning only)
            if !has_extended_page_tables() {
                warnings.push("EPT/NPT not detected — performance may be degraded".to_owned());
            }
        }
        Err(e) => {
            all_passed = false;
            checks.push(CheckResult {
                name: "cpu_virtualization".to_owned(),
                passed: false,
                detail: e.to_string(),
            });
        }
    }

    KvmReadinessReport {
        ready: all_passed,
        checks,
        warnings,
    }
}

/// Check that `/dev/kvm` exists and is accessible.
///
/// # Errors
///
/// Returns `VmError::KvmNotAvailable` if the device is missing or not a
/// character device, or `VmError::KvmPermissionDenied` if inaccessible.
pub fn check_kvm_device(path: &str) -> VmResult<()> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(VmError::KvmNotAvailable(format!("{path} does not exist")));
    }

    let metadata = fs::metadata(p)
        .map_err(|e| VmError::KvmNotAvailable(format!("cannot stat {path}: {e}")))?;

    // Check it's a character device (mode & S_IFCHR)
    let mode = metadata.mode();
    if mode & 0o170_000 != 0o020_000 {
        return Err(VmError::KvmNotAvailable(format!(
            "{path} is not a character device"
        )));
    }

    // Check read/write access
    let file = fs::OpenOptions::new().read(true).write(true).open(p);
    if file.is_err() {
        return Err(VmError::KvmPermissionDenied);
    }

    Ok(())
}

/// Check KVM API version via ioctl.
///
/// Returns the API version number on success.
fn check_kvm_api_version(path: &str) -> VmResult<i32> {
    use std::os::unix::io::AsRawFd;

    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|_| VmError::KvmPermissionDenied)?;

    // KVM_GET_API_VERSION = 0xAE00
    let version = unsafe { libc::ioctl(file.as_raw_fd(), 0xAE00) };
    if version < 0 {
        return Err(VmError::KvmNotAvailable(
            "KVM_GET_API_VERSION ioctl failed".to_owned(),
        ));
    }

    if version != 12 {
        return Err(VmError::KvmUnsupportedVersion(version));
    }

    // Check required extensions
    check_kvm_extensions(file.as_raw_fd())?;

    Ok(version)
}

/// Check required KVM extensions.
fn check_kvm_extensions(fd: i32) -> VmResult<()> {
    // KVM_CHECK_EXTENSION = 0xAE03
    let required = [
        (0, "KVM_CAP_IRQCHIP"),
        (3, "KVM_CAP_USER_MEMORY"),
        (37, "KVM_CAP_SET_TSS_ADDR"),
    ];

    let mut missing = Vec::new();
    for (cap, name) in &required {
        let ret = unsafe { libc::ioctl(fd, 0xAE03, *cap) };
        if ret <= 0 {
            missing.push(*name);
        }
    }

    if !missing.is_empty() {
        return Err(VmError::MissingExtension(missing.join(", ")));
    }

    Ok(())
}

/// Check CPU virtualization support via CPUID.
fn check_cpu_virtualization() -> VmResult<String> {
    #[cfg(target_arch = "x86_64")]
    {
        let vendor = get_cpu_vendor();
        let is_intel = vendor == "GenuineIntel";
        let is_amd = vendor == "AuthenticAMD";

        if is_intel {
            // Check VT-x: CPUID.1:ECX bit 5
            let result = unsafe { core::arch::x86_64::__cpuid(1) };
            if result.ecx & (1 << 5) == 0 {
                return Err(VmError::CpuFeatureMissing(
                    "Intel VT-x not supported by this CPU".to_owned(),
                ));
            }
            Ok("Intel VT-x supported".to_owned())
        } else if is_amd {
            // Check AMD-V: CPUID.8000_0001:ECX bit 2
            let result = unsafe { core::arch::x86_64::__cpuid(0x8000_0001) };
            if result.ecx & (1 << 2) == 0 {
                return Err(VmError::CpuFeatureMissing(
                    "AMD-V (SVM) not supported by this CPU".to_owned(),
                ));
            }
            Ok("AMD-V (SVM) supported".to_owned())
        } else {
            Err(VmError::CpuFeatureMissing(format!(
                "unknown CPU vendor: {vendor}"
            )))
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        Err(VmError::CpuFeatureMissing(
            "KVM detection only supported on x86_64".to_owned(),
        ))
    }
}

/// Check for EPT (Intel) or NPT (AMD).
fn has_extended_page_tables() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        let vendor = get_cpu_vendor();
        if vendor == "GenuineIntel" {
            // Heuristic: modern Intel CPUs (Nehalem+) all support EPT.
            let result = unsafe { core::arch::x86_64::__cpuid(1) };
            let family = ((result.eax >> 8) & 0xF) + ((result.eax >> 20) & 0xFF);
            let model = ((result.eax >> 4) & 0xF) | (((result.eax >> 16) & 0xF) << 4);
            family >= 6 && model >= 26
        } else if vendor == "AuthenticAMD" {
            // NPT: CPUID.8000_000A:EDX bit 0
            let result = unsafe { core::arch::x86_64::__cpuid(0x8000_000A) };
            result.edx & 1 != 0
        } else {
            false
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Get CPU vendor string from CPUID leaf 0.
#[cfg(target_arch = "x86_64")]
fn get_cpu_vendor() -> String {
    let result = unsafe { core::arch::x86_64::__cpuid(0) };
    let vendor_bytes: [u8; 12] = [
        result.ebx.to_le_bytes(),
        result.edx.to_le_bytes(),
        result.ecx.to_le_bytes(),
    ]
    .concat()
    .try_into()
    .unwrap_or([0; 12]);
    String::from_utf8_lossy(&vendor_bytes).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kvm_device_missing() {
        let result = check_kvm_device("/dev/nonexistent_kvm_device");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not available") || err.contains("does not exist"));
    }

    #[test]
    fn kvm_device_not_char_device() {
        let dir = tempfile::tempdir().unwrap();
        let fake_kvm = dir.path().join("kvm");
        std::fs::write(&fake_kvm, "").unwrap();
        let result = check_kvm_device(fake_kvm.to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn readiness_report_with_missing_device() {
        let report = check_kvm_readiness_with_path("/dev/nonexistent_kvm_device");
        assert!(!report.ready);
        assert!(!report.checks.is_empty());
        assert!(!report.checks[0].passed);
    }

    #[test]
    fn readiness_report_on_real_host() {
        let report = check_kvm_readiness();
        assert!(!report.checks.is_empty());
        for check in &report.checks {
            assert!(!check.name.is_empty());
            assert!(!check.detail.is_empty());
        }
    }
}
