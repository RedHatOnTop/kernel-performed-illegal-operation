//! Web Platform Tests (WPT) Integration
//!
//! Provides harness for running W3C Web Platform Tests against KPIO browser.

#![no_std]
extern crate alloc;

pub mod harness;
pub mod manifest;
pub mod results;
pub mod runner;

use alloc::string::String;
use alloc::vec::Vec;

/// WPT test types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestType {
    /// JavaScript test using testharness.js
    TestHarness,
    /// Reference test comparing rendering
    RefTest,
    /// Visual test with manual inspection
    Visual,
    /// Manual test requiring user interaction
    Manual,
    /// Crash test (should not crash)
    CrashTest,
    /// Print reference test
    PrintRefTest,
}

/// WPT test metadata
#[derive(Debug, Clone)]
pub struct TestMetadata {
    /// Test file path relative to WPT root
    pub path: String,
    /// Test type
    pub test_type: TestType,
    /// Test title
    pub title: String,
    /// Expected result
    pub expected: ExpectedResult,
    /// Test timeout in seconds
    pub timeout: u32,
    /// Disable reason (if any)
    pub disabled: Option<String>,
    /// Preconditions
    pub preconditions: Vec<String>,
}

/// Expected test result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedResult {
    /// Test should pass
    Pass,
    /// Test should fail
    Fail,
    /// Test times out
    Timeout,
    /// Test causes crash
    Crash,
    /// Result varies
    Flaky,
}

/// WPT test result
#[derive(Debug, Clone)]
pub struct TestResult {
    /// Test path
    pub path: String,
    /// Actual status
    pub status: TestStatus,
    /// Subtest results (for testharness.js tests)
    pub subtests: Vec<SubtestResult>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Error message (if any)
    pub message: Option<String>,
    /// Stack trace (if any)
    pub stack: Option<String>,
}

/// Test status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    /// All subtests passed
    Ok,
    /// Some subtests failed or errored
    Error,
    /// Test timed out
    Timeout,
    /// Test crashed
    Crash,
    /// Test was skipped
    Skip,
}

/// Subtest result (for testharness.js)
#[derive(Debug, Clone)]
pub struct SubtestResult {
    /// Subtest name
    pub name: String,
    /// Subtest status
    pub status: SubtestStatus,
    /// Error message
    pub message: Option<String>,
    /// Expected status
    pub expected: Option<SubtestStatus>,
}

/// Subtest status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtestStatus {
    /// Subtest passed
    Pass,
    /// Subtest failed
    Fail,
    /// Subtest timed out
    Timeout,
    /// Subtest not run
    NotRun,
    /// Precondition failed
    PreconditionFailed,
}

/// WPT run configuration
#[derive(Debug, Clone)]
pub struct WptConfig {
    /// WPT root directory
    pub wpt_root: String,
    /// Browser binary path
    pub browser_binary: String,
    /// Test filter (glob pattern)
    pub filter: Option<String>,
    /// Run in headless mode
    pub headless: bool,
    /// Number of parallel processes
    pub processes: u32,
    /// Default timeout in seconds
    pub timeout: u32,
    /// Log level
    pub log_level: LogLevel,
    /// Output directory for results
    pub output_dir: String,
    /// Run only failing tests
    pub only_failing: bool,
    /// Run unstable tests
    pub run_unstable: bool,
}

impl Default for WptConfig {
    fn default() -> Self {
        Self {
            wpt_root: String::from("/wpt"),
            browser_binary: String::from("kpio-browser"),
            filter: None,
            headless: true,
            processes: 4,
            timeout: 60,
            log_level: LogLevel::Info,
            output_dir: String::from("./wpt-results"),
            only_failing: false,
            run_unstable: false,
        }
    }
}

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// WPT run summary
#[derive(Debug, Clone)]
pub struct RunSummary {
    /// Total tests
    pub total: usize,
    /// Passed tests
    pub passed: usize,
    /// Failed tests
    pub failed: usize,
    /// Timed out tests
    pub timeout: usize,
    /// Crashed tests
    pub crashed: usize,
    /// Skipped tests
    pub skipped: usize,
    /// Unexpected results
    pub unexpected: usize,
    /// Total duration in seconds
    pub duration_s: f64,
    /// Individual results
    pub results: Vec<TestResult>,
}

impl RunSummary {
    /// Create empty summary
    pub fn new() -> Self {
        Self {
            total: 0,
            passed: 0,
            failed: 0,
            timeout: 0,
            crashed: 0,
            skipped: 0,
            unexpected: 0,
            duration_s: 0.0,
            results: Vec::new(),
        }
    }

    /// Add a result
    pub fn add_result(&mut self, result: TestResult) {
        self.total += 1;
        match result.status {
            TestStatus::Ok => self.passed += 1,
            TestStatus::Error => self.failed += 1,
            TestStatus::Timeout => self.timeout += 1,
            TestStatus::Crash => self.crashed += 1,
            TestStatus::Skip => self.skipped += 1,
        }
        self.results.push(result);
    }

    /// Get pass rate as percentage
    pub fn pass_rate(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.passed as f32 / self.total as f32) * 100.0
    }

    /// Get effective total (excluding skipped)
    pub fn effective_total(&self) -> usize {
        self.total - self.skipped
    }
}

impl Default for RunSummary {
    fn default() -> Self {
        Self::new()
    }
}

/// WPT test categories
pub struct WptCategories;

impl WptCategories {
    /// Core DOM tests
    pub const DOM: &'static str = "dom";
    /// HTML parsing and semantics
    pub const HTML: &'static str = "html";
    /// CSS tests
    pub const CSS: &'static str = "css";
    /// JavaScript/ECMAScript
    pub const ECMASCRIPT: &'static str = "ecmascript";
    /// Fetch API
    pub const FETCH: &'static str = "fetch";
    /// XMLHttpRequest
    pub const XHR: &'static str = "xhr";
    /// URL handling
    pub const URL: &'static str = "url";
    /// Encoding
    pub const ENCODING: &'static str = "encoding";
    /// Web Storage
    pub const STORAGE: &'static str = "storage";
    /// Web Workers
    pub const WORKERS: &'static str = "workers";
    /// Service Workers
    pub const SERVICE_WORKERS: &'static str = "service-workers";
    /// WebSockets
    pub const WEBSOCKETS: &'static str = "websockets";
    /// Content Security Policy
    pub const CSP: &'static str = "content-security-policy";
    /// CORS
    pub const CORS: &'static str = "cors";

    /// Get high priority categories for KPIO
    pub fn high_priority() -> Vec<&'static str> {
        alloc::vec![
            Self::DOM,
            Self::HTML,
            Self::CSS,
            Self::FETCH,
            Self::URL,
            Self::ENCODING,
        ]
    }

    /// Get all supported categories
    pub fn all() -> Vec<&'static str> {
        alloc::vec![
            Self::DOM,
            Self::HTML,
            Self::CSS,
            Self::ECMASCRIPT,
            Self::FETCH,
            Self::XHR,
            Self::URL,
            Self::ENCODING,
            Self::STORAGE,
            Self::WORKERS,
            Self::SERVICE_WORKERS,
            Self::WEBSOCKETS,
            Self::CSP,
            Self::CORS,
        ]
    }
}
