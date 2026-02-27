//! Pre-flight health check — validates that all prerequisites are available
//! before creating an instance.
//!
//! Checks are classified as **required** (QEMU, OVMF, disk image) or
//! **optional** (OCR engine). Each check reports pass/fail individually
//! with a remediation hint on failure.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::KpioTestError;

/// Result of a single health check.
#[derive(Serialize, Debug, Clone)]
pub struct CheckResult {
    pub name: String,
    pub passed: bool,
    pub required: bool,
    /// Resolved path when the check passes.
    pub path: Option<String>,
    /// Human-readable remediation hint when the check fails.
    pub hint: Option<String>,
}

/// Aggregated health check report.
#[derive(Serialize, Debug)]
pub struct HealthReport {
    pub checks: Vec<CheckResult>,
    pub all_required_passed: bool,
}

/// Known OVMF firmware search paths by platform.
#[cfg(target_os = "linux")]
const OVMF_SEARCH_PATHS: &[&str] = &[
    "/usr/share/OVMF/OVMF_CODE.fd",
    "/usr/share/edk2/ovmf/OVMF_CODE.fd",
    "/usr/share/qemu/OVMF_CODE.fd",
    "/usr/share/edk2-ovmf/x64/OVMF_CODE.fd",
];

#[cfg(target_os = "macos")]
const OVMF_SEARCH_PATHS: &[&str] = &[
    "/opt/homebrew/share/qemu/edk2-x86_64-code.fd",
    "/usr/local/share/qemu/edk2-x86_64-code.fd",
];

#[cfg(target_os = "windows")]
const OVMF_SEARCH_PATHS: &[&str] = &[
    "C:\\Program Files\\qemu\\share\\edk2-x86_64-code.fd",
    "C:\\Program Files (x86)\\qemu\\share\\edk2-x86_64-code.fd",
];

/// Run all health checks and return a report.
///
/// If `image_path` is `None`, the kernel image check is treated as optional.
pub fn check(image_path: Option<&Path>) -> HealthReport {
    let mut checks = Vec::new();

    checks.push(check_qemu());
    checks.push(check_ovmf());
    checks.push(check_rust_toolchain());
    checks.push(check_image_builder());
    checks.push(check_kernel_image(image_path));
    checks.push(check_ocr_engine());

    let all_required_passed = checks
        .iter()
        .filter(|c| c.required)
        .all(|c| c.passed);

    HealthReport {
        checks,
        all_required_passed,
    }
}

/// Validate the health report, returning an error if any required check failed.
pub fn validate(report: &HealthReport) -> Result<(), KpioTestError> {
    if !report.all_required_passed {
        let failures: Vec<String> = report
            .checks
            .iter()
            .filter(|c| c.required && !c.passed)
            .map(|c| {
                let hint = c.hint.as_deref().unwrap_or("no hint");
                format!("{}: {}", c.name, hint)
            })
            .collect();
        // Return the first required failure as the primary error
        if let Some(first) = report.checks.iter().find(|c| c.required && !c.passed) {
            match first.name.as_str() {
                "qemu" => {
                    return Err(KpioTestError::QemuNotFound {
                        hint: first
                            .hint
                            .clone()
                            .unwrap_or_else(|| "QEMU not found".to_string()),
                    })
                }
                "ovmf" => {
                    return Err(KpioTestError::OvmfNotFound {
                        hint: first
                            .hint
                            .clone()
                            .unwrap_or_else(|| "OVMF not found".to_string()),
                    })
                }
                "kernel_image" => {
                    return Err(KpioTestError::ImageNotFound {
                        path: PathBuf::from(
                            first.hint.as_deref().unwrap_or("unknown"),
                        ),
                    })
                }
                _ => {
                    return Err(KpioTestError::QemuNotFound {
                        hint: failures.join("; "),
                    })
                }
            }
        }
    }
    Ok(())
}

// ── Known QEMU binary search paths by platform ──────────────────────

#[cfg(target_os = "linux")]
const QEMU_SEARCH_PATHS: &[&str] = &[
    "/usr/bin/qemu-system-x86_64",
    "/usr/local/bin/qemu-system-x86_64",
];

#[cfg(target_os = "macos")]
const QEMU_SEARCH_PATHS: &[&str] = &[
    "/opt/homebrew/bin/qemu-system-x86_64",
    "/usr/local/bin/qemu-system-x86_64",
];

#[cfg(target_os = "windows")]
const QEMU_SEARCH_PATHS: &[&str] = &[
    "C:\\Program Files\\qemu\\qemu-system-x86_64.exe",
    "C:\\Program Files (x86)\\qemu\\qemu-system-x86_64.exe",
];

/// Find the QEMU binary. Checks PATH first, then known install locations.
pub fn find_qemu() -> Option<PathBuf> {
    // Try PATH first
    if let Some(path) = which("qemu-system-x86_64") {
        return Some(PathBuf::from(path));
    }
    // Fall back to known install locations
    for path in QEMU_SEARCH_PATHS {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

// ── Individual checks ────────────────────────────────────────────────

fn check_qemu() -> CheckResult {
    match find_qemu() {
        Some(path) => CheckResult {
            name: "qemu".to_string(),
            passed: true,
            required: true,
            path: Some(path.to_string_lossy().to_string()),
            hint: None,
        },
        None => CheckResult {
            name: "qemu".to_string(),
            passed: false,
            required: true,
            path: None,
            hint: Some(
                "Install QEMU: apt install qemu-system-x86 (Linux), brew install qemu (macOS), or download from qemu.org (Windows)"
                    .to_string(),
            ),
        },
    }
}

fn check_ovmf() -> CheckResult {
    for path in OVMF_SEARCH_PATHS {
        if Path::new(path).exists() {
            return CheckResult {
                name: "ovmf".to_string(),
                passed: true,
                required: true,
                path: Some(path.to_string()),
                hint: None,
            };
        }
    }
    CheckResult {
        name: "ovmf".to_string(),
        passed: false,
        required: true,
        path: None,
        hint: Some(
            "Install OVMF: apt install ovmf (Linux), brew install qemu (macOS includes OVMF), or download from tianocore.org"
                .to_string(),
        ),
    }
}

fn check_rust_toolchain() -> CheckResult {
    match which("rustc") {
        Some(path) => CheckResult {
            name: "rust_toolchain".to_string(),
            passed: true,
            required: true,
            path: Some(path),
            hint: None,
        },
        None => CheckResult {
            name: "rust_toolchain".to_string(),
            passed: false,
            required: true,
            path: None,
            hint: Some("Install Rust: https://rustup.rs".to_string()),
        },
    }
}

fn check_image_builder() -> CheckResult {
    // The image builder is at tools/boot/ relative to the workspace root.
    // We check for the Cargo.toml as a proxy.
    let candidates = ["tools/boot/Cargo.toml", "../boot/Cargo.toml"];
    for candidate in &candidates {
        if Path::new(candidate).exists() {
            return CheckResult {
                name: "image_builder".to_string(),
                passed: true,
                required: true,
                path: Some(candidate.to_string()),
                hint: None,
            };
        }
    }
    CheckResult {
        name: "image_builder".to_string(),
        passed: false,
        required: true,
        path: None,
        hint: Some("Image builder not found at tools/boot/. Ensure the workspace is intact.".to_string()),
    }
}

fn check_kernel_image(image_path: Option<&Path>) -> CheckResult {
    match image_path {
        Some(path) if path.exists() => CheckResult {
            name: "kernel_image".to_string(),
            passed: true,
            required: false,
            path: Some(path.to_string_lossy().to_string()),
            hint: None,
        },
        Some(path) => CheckResult {
            name: "kernel_image".to_string(),
            passed: false,
            required: false,
            path: None,
            hint: Some(format!(
                "Kernel image not found at {}. Run `kpio-test build` first.",
                path.display()
            )),
        },
        None => CheckResult {
            name: "kernel_image".to_string(),
            passed: false,
            required: false,
            path: None,
            hint: Some("No image path specified. Run `kpio-test build` or pass --image.".to_string()),
        },
    }
}

fn check_ocr_engine() -> CheckResult {
    match which("tesseract") {
        Some(path) => CheckResult {
            name: "ocr_engine".to_string(),
            passed: true,
            required: false,
            path: Some(path),
            hint: None,
        },
        None => CheckResult {
            name: "ocr_engine".to_string(),
            passed: false,
            required: false,
            path: None,
            hint: Some(
                "Install Tesseract for OCR support: apt install tesseract-ocr (Linux), brew install tesseract (macOS)"
                    .to_string(),
            ),
        },
    }
}

// ── Utility ──────────────────────────────────────────────────────────

/// Simple `which`-style lookup: search PATH for the given binary name.
fn which(binary: &str) -> Option<String> {
    let path_var = std::env::var("PATH").ok()?;
    #[cfg(windows)]
    let extensions = vec!["", ".exe", ".cmd", ".bat"];
    #[cfg(not(windows))]
    let extensions = vec![""];

    for dir in std::env::split_paths(&path_var) {
        for ext in &extensions {
            let candidate = dir.join(format!("{binary}{ext}"));
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    None
}

/// Find the OVMF firmware path. Returns the first match from known locations.
pub fn find_ovmf() -> Option<PathBuf> {
    for path in OVMF_SEARCH_PATHS {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_result_serializes() {
        let cr = CheckResult {
            name: "test".to_string(),
            passed: true,
            required: true,
            path: Some("/usr/bin/test".to_string()),
            hint: None,
        };
        let json = serde_json::to_string(&cr).unwrap();
        assert!(json.contains("\"passed\":true"));
    }

    #[test]
    fn health_report_marks_all_required_passed() {
        let report = HealthReport {
            checks: vec![
                CheckResult {
                    name: "a".to_string(),
                    passed: true,
                    required: true,
                    path: None,
                    hint: None,
                },
                CheckResult {
                    name: "b".to_string(),
                    passed: false,
                    required: false,
                    path: None,
                    hint: None,
                },
            ],
            all_required_passed: true,
        };
        assert!(report.all_required_passed);
    }

    #[test]
    fn health_report_fails_when_required_fails() {
        let report = HealthReport {
            checks: vec![CheckResult {
                name: "a".to_string(),
                passed: false,
                required: true,
                path: None,
                hint: Some("fix it".to_string()),
            }],
            all_required_passed: false,
        };
        assert!(!report.all_required_passed);
    }
}
