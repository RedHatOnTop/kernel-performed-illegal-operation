//! Test manifest parsing and verification engine.
//!
//! Loads TOML test manifests, evaluates serial log content against
//! declared patterns, and reports per-check pass/fail results.

use std::collections::HashMap;
use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::KpioTestError;

// ── Data models ──────────────────────────────────────────────────────

/// Top-level test manifest loaded from a TOML file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestManifest {
    pub metadata: ManifestMetadata,
    pub suites: HashMap<String, TestSuite>,
}

/// Manifest-level metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestMetadata {
    pub name: String,
    pub version: String,
}

/// A named test suite containing an ordered list of checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestSuite {
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub memory: Option<String>,
    #[serde(default)]
    pub virtio_net: Option<bool>,
    #[serde(default)]
    pub virtio_blk: Option<String>,
    #[serde(default)]
    pub extra_args: Option<Vec<String>>,
    pub checks: Vec<TestCheck>,
}

/// A single verification check within a test suite.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestCheck {
    pub pattern: String,
    pub label: String,
    #[serde(default)]
    pub regex: bool,
    #[serde(default = "default_expect")]
    pub expect: CheckExpectation,
}

/// Whether a pattern is expected to be present or absent in the serial log.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CheckExpectation {
    #[serde(rename = "present")]
    Present,
    #[serde(rename = "absent")]
    Absent,
}

fn default_expect() -> CheckExpectation {
    CheckExpectation::Present
}

// ── Verification output ──────────────────────────────────────────────

/// Result of evaluating a single check against the serial log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckResult {
    pub name: String,
    pub pattern: String,
    pub status: String,
    pub expected: String,
}

/// Full verification output for a test suite.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerifyOutput {
    pub test_id: String,
    pub timestamp: String,
    pub mode: String,
    pub instance_name: String,
    pub manifest_path: String,
    pub overall_pass: bool,
    pub pass_count: usize,
    pub fail_count: usize,
    pub checks: Vec<CheckResult>,
    pub duration_ms: u64,
    pub serial_log_lines: usize,
}

// ── Parsing ──────────────────────────────────────────────────────────

/// Parse a test manifest from a TOML file path.
pub fn parse_manifest(path: &Path) -> Result<TestManifest, KpioTestError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            KpioTestError::ManifestParseError(format!("manifest file not found: {}", path.display()))
        } else {
            KpioTestError::Io(e)
        }
    })?;
    parse_manifest_str(&content)
}

/// Parse a test manifest from a TOML string.
pub fn parse_manifest_str(content: &str) -> Result<TestManifest, KpioTestError> {
    toml::from_str(content).map_err(|e| {
        KpioTestError::ManifestParseError(format!("invalid TOML: {e}"))
    })
}

// ── Pattern matching engine ──────────────────────────────────────────

/// Check whether a pattern matches anywhere in the serial log content.
///
/// - When `is_regex` is false, performs a literal substring search.
/// - When `is_regex` is true, compiles the pattern as a regex.
pub fn pattern_matches(content: &str, pattern: &str, is_regex: bool) -> Result<bool, KpioTestError> {
    if is_regex {
        let re = Regex::new(pattern).map_err(|e| {
            KpioTestError::ManifestParseError(format!("invalid regex pattern '{}': {}", pattern, e))
        })?;
        Ok(content.lines().any(|line| re.is_match(line)))
    } else {
        Ok(content.lines().any(|line| line.contains(pattern)))
    }
}

// ── Evaluation ───────────────────────────────────────────────────────

/// `verify <name> --manifest <path> [--mode <suite>]` handler.
pub fn verify(args: crate::cli::VerifyArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = crate::store::read_state(&args.name)?;
    crate::watchdog::enforce(&mut state)?;

    let manifest = parse_manifest(&args.manifest)?;

    let log_path = crate::store::serial_log_path(&args.name);
    let content = std::fs::read_to_string(&log_path).unwrap_or_default();
    let serial_log_lines = content.lines().count();

    let start = std::time::Instant::now();

    // Determine which suites to evaluate
    let suites_to_run: Vec<(&str, &TestSuite)> = match &args.mode {
        Some(mode) => {
            let suite = manifest.suites.get(mode.as_str()).ok_or_else(|| {
                KpioTestError::ManifestParseError(format!("suite '{}' not found in manifest", mode))
            })?;
            vec![(mode.as_str(), suite)]
        }
        None => manifest.suites.iter().map(|(k, v)| (k.as_str(), v)).collect(),
    };

    let mut all_results = Vec::new();
    let mut total_pass = 0usize;
    let mut total_fail = 0usize;

    for (_suite_name, suite) in &suites_to_run {
        let (results, pass, fail) = evaluate_suite(&suite.checks, &content)?;
        total_pass += pass;
        total_fail += fail;
        all_results.extend(results);
    }

    let duration_ms = start.elapsed().as_millis() as u64;
    let overall_pass = total_fail == 0;

    let mode_str = args.mode.clone().unwrap_or_else(|| "all".to_string());
    let timestamp = chrono::Utc::now().to_rfc3339();

    let output = VerifyOutput {
        test_id: args.name.clone(),
        timestamp,
        mode: mode_str,
        instance_name: args.name,
        manifest_path: args.manifest.display().to_string(),
        overall_pass,
        pass_count: total_pass,
        fail_count: total_fail,
        checks: all_results,
        duration_ms,
        serial_log_lines,
    };

    if overall_pass {
        Ok(serde_json::to_value(output)?)
    } else {
        // Still emit the output but return an error for exit code 1
        let value = serde_json::to_value(&output)?;
        // We need to emit the output before returning the error.
        // The caller in main.rs handles Ok → emit, Err → emit_error.
        // For verify, we want to emit the full result even on failure.
        // Return Ok with the value; the caller checks overall_pass via exit code.
        // Actually, per the design, verify returns exit code 1 on failure.
        // We'll return the error but the output is lost. Let's print it here.
        println!("{}", serde_json::to_string(&value)?);
        Err(KpioTestError::VerificationFailed {
            fail_count: total_fail,
        })
    }
}

/// Evaluate a single check against serial log content.
pub fn evaluate_check(check: &TestCheck, content: &str) -> Result<CheckResult, KpioTestError> {
    let found = pattern_matches(content, &check.pattern, check.regex)?;

    let pass = match check.expect {
        CheckExpectation::Present => found,
        CheckExpectation::Absent => !found,
    };

    Ok(CheckResult {
        name: check.label.clone(),
        pattern: check.pattern.clone(),
        status: if pass { "pass".to_string() } else { "fail".to_string() },
        expected: match check.expect {
            CheckExpectation::Present => "present".to_string(),
            CheckExpectation::Absent => "absent".to_string(),
        },
    })
}

/// Evaluate all checks in a test suite against serial log content.
///
/// Returns per-check results along with aggregate pass/fail counts.
pub fn evaluate_suite(
    checks: &[TestCheck],
    content: &str,
) -> Result<(Vec<CheckResult>, usize, usize), KpioTestError> {
    let mut results = Vec::with_capacity(checks.len());
    let mut pass_count = 0usize;
    let mut fail_count = 0usize;

    for check in checks {
        let result = evaluate_check(check, content)?;
        if result.status == "pass" {
            pass_count += 1;
        } else {
            fail_count += 1;
        }
        results.push(result);
    }

    Ok((results, pass_count, fail_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_toml() -> &'static str {
        r#"
[metadata]
name = "Test Suite"
version = "1.0"

[suites.boot]
timeout = 30
memory = "512M"
checks = [
    { pattern = "Hello, Kernel", label = "Kernel entry" },
    { pattern = "Heap initialized", label = "Heap init" },
]

[suites.io]
timeout = 60
virtio_net = true
checks = [
    { pattern = "NIC initialized", label = "NIC init" },
    { pattern = "PANIC", label = "No panic", expect = "absent" },
    { pattern = "E2E.*PASSED", label = "E2E test", regex = true },
]
"#
    }

    #[test]
    fn parse_valid_manifest() {
        let manifest = parse_manifest_str(sample_toml()).unwrap();
        assert_eq!(manifest.metadata.name, "Test Suite");
        assert_eq!(manifest.suites.len(), 2);
        assert_eq!(manifest.suites["boot"].checks.len(), 2);
        assert_eq!(manifest.suites["io"].checks.len(), 3);
    }

    #[test]
    fn parse_invalid_toml_returns_error() {
        let result = parse_manifest_str("this is not valid toml {{{}}}");
        assert!(result.is_err());
        match result.unwrap_err() {
            KpioTestError::ManifestParseError(msg) => assert!(msg.contains("invalid TOML")),
            other => panic!("expected ManifestParseError, got: {other:?}"),
        }
    }

    #[test]
    fn literal_pattern_match() {
        let content = "Hello, Kernel\nGDT initialized\nHeap initialized";
        assert!(pattern_matches(content, "Hello, Kernel", false).unwrap());
        assert!(pattern_matches(content, "GDT", false).unwrap());
        assert!(!pattern_matches(content, "MISSING", false).unwrap());
    }

    #[test]
    fn regex_pattern_match() {
        let content = "E2E test PASSED\nAnother line";
        assert!(pattern_matches(content, "E2E.*PASSED", true).unwrap());
        assert!(!pattern_matches(content, "E2E.*FAILED", true).unwrap());
    }

    #[test]
    fn invalid_regex_returns_error() {
        let result = pattern_matches("content", "[invalid", true);
        assert!(result.is_err());
    }

    #[test]
    fn evaluate_present_check_pass() {
        let check = TestCheck {
            pattern: "Hello".to_string(),
            label: "greeting".to_string(),
            regex: false,
            expect: CheckExpectation::Present,
        };
        let result = evaluate_check(&check, "Hello, World").unwrap();
        assert_eq!(result.status, "pass");
    }

    #[test]
    fn evaluate_present_check_fail() {
        let check = TestCheck {
            pattern: "Missing".to_string(),
            label: "missing".to_string(),
            regex: false,
            expect: CheckExpectation::Present,
        };
        let result = evaluate_check(&check, "Hello, World").unwrap();
        assert_eq!(result.status, "fail");
    }

    #[test]
    fn evaluate_absent_check_pass() {
        let check = TestCheck {
            pattern: "PANIC".to_string(),
            label: "no panic".to_string(),
            regex: false,
            expect: CheckExpectation::Absent,
        };
        let result = evaluate_check(&check, "Hello, World").unwrap();
        assert_eq!(result.status, "pass");
    }

    #[test]
    fn evaluate_absent_check_fail() {
        let check = TestCheck {
            pattern: "PANIC".to_string(),
            label: "no panic".to_string(),
            regex: false,
            expect: CheckExpectation::Absent,
        };
        let result = evaluate_check(&check, "PANIC at line 42").unwrap();
        assert_eq!(result.status, "fail");
    }

    #[test]
    fn evaluate_suite_all_pass() {
        let checks = vec![
            TestCheck {
                pattern: "Hello".to_string(),
                label: "greeting".to_string(),
                regex: false,
                expect: CheckExpectation::Present,
            },
            TestCheck {
                pattern: "PANIC".to_string(),
                label: "no panic".to_string(),
                regex: false,
                expect: CheckExpectation::Absent,
            },
        ];
        let (results, pass, fail) = evaluate_suite(&checks, "Hello, World").unwrap();
        assert_eq!(pass, 2);
        assert_eq!(fail, 0);
        assert!(results.iter().all(|r| r.status == "pass"));
    }

    #[test]
    fn evaluate_suite_mixed_results() {
        let checks = vec![
            TestCheck {
                pattern: "Hello".to_string(),
                label: "greeting".to_string(),
                regex: false,
                expect: CheckExpectation::Present,
            },
            TestCheck {
                pattern: "Missing".to_string(),
                label: "missing".to_string(),
                regex: false,
                expect: CheckExpectation::Present,
            },
        ];
        let (_, pass, fail) = evaluate_suite(&checks, "Hello, World").unwrap();
        assert_eq!(pass, 1);
        assert_eq!(fail, 1);
    }

    #[test]
    fn check_expectation_default_is_present() {
        assert_eq!(default_expect(), CheckExpectation::Present);
    }

    #[test]
    fn check_expectation_serde_round_trip() {
        for exp in [CheckExpectation::Present, CheckExpectation::Absent] {
            let json = serde_json::to_string(&exp).unwrap();
            let back: CheckExpectation = serde_json::from_str(&json).unwrap();
            assert_eq!(exp, back);
        }
    }

    #[test]
    fn manifest_toml_round_trip() {
        let manifest = parse_manifest_str(sample_toml()).unwrap();
        let toml_str = toml::to_string(&manifest).unwrap();
        let back: TestManifest = toml::from_str(&toml_str).unwrap();
        assert_eq!(manifest, back);
    }
}
