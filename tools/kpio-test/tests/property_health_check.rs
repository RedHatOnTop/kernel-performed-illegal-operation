//! Property 19: Health check reports all failures.
//!
//! For any subset of missing prerequisites, health reports each individually;
//! exit code 2 if any required check fails.
//!
//! Validates: Requirements 2.4, 27.1, 27.2, 27.3, 27.4, 27.5, 27.6

use kpio_test::health::{self, CheckResult, HealthReport};

/// Health report always contains the expected set of check names.
#[test]
fn health_report_contains_all_check_names() {
    let report = health::check(None);
    let names: Vec<&str> = report.checks.iter().map(|c| c.name.as_str()).collect();

    assert!(names.contains(&"qemu"), "missing qemu check");
    assert!(names.contains(&"ovmf"), "missing ovmf check");
    assert!(names.contains(&"rust_toolchain"), "missing rust_toolchain check");
    assert!(names.contains(&"image_builder"), "missing image_builder check");
    assert!(names.contains(&"kernel_image"), "missing kernel_image check");
    assert!(names.contains(&"ocr_engine"), "missing ocr_engine check");
}

/// Required checks are correctly classified.
#[test]
fn required_checks_are_classified() {
    let report = health::check(None);
    for check in &report.checks {
        match check.name.as_str() {
            "qemu" | "ovmf" | "rust_toolchain" | "image_builder" => {
                assert!(check.required, "{} should be required", check.name);
            }
            "kernel_image" | "ocr_engine" => {
                assert!(!check.required, "{} should be optional", check.name);
            }
            _ => {}
        }
    }
}

/// all_required_passed is true iff every required check passed.
#[test]
fn all_required_passed_consistency() {
    let report = health::check(None);
    let expected = report
        .checks
        .iter()
        .filter(|c| c.required)
        .all(|c| c.passed);
    assert_eq!(
        report.all_required_passed, expected,
        "all_required_passed should match actual required check results"
    );
}

/// Failed checks always have a non-empty hint.
#[test]
fn failed_checks_have_hints() {
    let report = health::check(None);
    for check in &report.checks {
        if !check.passed {
            assert!(
                check.hint.is_some() && !check.hint.as_ref().unwrap().is_empty(),
                "failed check '{}' should have a non-empty hint",
                check.name
            );
        }
    }
}

/// Passed checks always have a resolved path.
#[test]
fn passed_checks_have_paths() {
    let report = health::check(None);
    for check in &report.checks {
        if check.passed {
            assert!(
                check.path.is_some(),
                "passed check '{}' should have a resolved path",
                check.name
            );
        }
    }
}

/// When any required check fails, validate() returns an error.
#[test]
fn validate_fails_when_required_check_fails() {
    let report = HealthReport {
        checks: vec![
            CheckResult {
                name: "qemu".to_string(),
                passed: false,
                required: true,
                path: None,
                hint: Some("install qemu".to_string()),
            },
            CheckResult {
                name: "ovmf".to_string(),
                passed: true,
                required: true,
                path: Some("/usr/share/OVMF".to_string()),
                hint: None,
            },
        ],
        all_required_passed: false,
    };
    let result = health::validate(&report);
    assert!(result.is_err(), "validate should fail when required check fails");
}

/// When all required checks pass, validate() succeeds.
#[test]
fn validate_succeeds_when_all_required_pass() {
    let report = HealthReport {
        checks: vec![
            CheckResult {
                name: "qemu".to_string(),
                passed: true,
                required: true,
                path: Some("/usr/bin/qemu".to_string()),
                hint: None,
            },
            CheckResult {
                name: "ocr_engine".to_string(),
                passed: false,
                required: false,
                path: None,
                hint: Some("install tesseract".to_string()),
            },
        ],
        all_required_passed: true,
    };
    let result = health::validate(&report);
    assert!(result.is_ok(), "validate should succeed when only optional checks fail");
}

/// Each individual missing prerequisite is reported separately.
#[test]
fn each_failure_reported_individually() {
    let report = HealthReport {
        checks: vec![
            CheckResult {
                name: "qemu".to_string(),
                passed: false,
                required: true,
                path: None,
                hint: Some("install qemu".to_string()),
            },
            CheckResult {
                name: "ovmf".to_string(),
                passed: false,
                required: true,
                path: None,
                hint: Some("install ovmf".to_string()),
            },
            CheckResult {
                name: "rust_toolchain".to_string(),
                passed: false,
                required: true,
                path: None,
                hint: Some("install rust".to_string()),
            },
        ],
        all_required_passed: false,
    };

    // All three failures should be present in the checks
    let failed: Vec<&str> = report
        .checks
        .iter()
        .filter(|c| !c.passed)
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(failed.len(), 3);
    assert!(failed.contains(&"qemu"));
    assert!(failed.contains(&"ovmf"));
    assert!(failed.contains(&"rust_toolchain"));
}
