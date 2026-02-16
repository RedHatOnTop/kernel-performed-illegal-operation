//! KPIO End-to-End Testing Framework
//!
//! This crate provides comprehensive end-to-end testing capabilities for the KPIO browser OS.
//! It includes browser automation, screenshot testing, performance benchmarking, and
//! cross-component integration tests.

#![no_std]
extern crate alloc;

pub mod assertions;
pub mod browser;
pub mod fixtures;
pub mod harness;
pub mod integration;
pub mod performance;
pub mod screenshot;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// Test result status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    /// Test passed successfully
    Passed,
    /// Test failed
    Failed,
    /// Test was skipped
    Skipped,
    /// Test timed out
    Timeout,
    /// Test encountered an error
    Error,
}

/// A single test case
pub struct TestCase {
    /// Test name
    pub name: String,
    /// Test description
    pub description: String,
    /// Test function
    pub test_fn: Box<dyn Fn(&mut TestContext) -> TestResult + Send + Sync>,
    /// Test timeout in milliseconds
    pub timeout_ms: u64,
    /// Tags for filtering
    pub tags: Vec<String>,
    /// Whether this test requires browser context
    pub requires_browser: bool,
}

/// Test execution context
pub struct TestContext {
    /// Current test name
    pub test_name: String,
    /// Browser instance (if available)
    pub browser: Option<browser::BrowserHandle>,
    /// Captured screenshots
    pub screenshots: Vec<screenshot::Screenshot>,
    /// Performance measurements
    pub measurements: Vec<performance::Measurement>,
    /// Log messages
    pub logs: Vec<LogEntry>,
}

/// Log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Log level
    pub level: LogLevel,
    /// Message
    pub message: String,
    /// Timestamp in milliseconds
    pub timestamp: u64,
}

/// Log levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Test result
#[derive(Debug, Clone)]
pub struct TestResult {
    /// Status
    pub status: TestStatus,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Performance data
    pub performance: Option<performance::PerformanceReport>,
}

impl TestResult {
    /// Create a passed result
    pub fn passed(duration_ms: u64) -> Self {
        Self {
            status: TestStatus::Passed,
            error: None,
            duration_ms,
            performance: None,
        }
    }

    /// Create a failed result
    pub fn failed(error: String, duration_ms: u64) -> Self {
        Self {
            status: TestStatus::Failed,
            error: Some(error),
            duration_ms,
            performance: None,
        }
    }

    /// Create a skipped result
    pub fn skipped(reason: String) -> Self {
        Self {
            status: TestStatus::Skipped,
            error: Some(reason),
            duration_ms: 0,
            performance: None,
        }
    }

    /// Create a timeout result
    pub fn timeout(timeout_ms: u64) -> Self {
        Self {
            status: TestStatus::Timeout,
            error: Some(alloc::format!("Test timed out after {}ms", timeout_ms)),
            duration_ms: timeout_ms,
            performance: None,
        }
    }

    /// Create an error result
    pub fn error(error: String, duration_ms: u64) -> Self {
        Self {
            status: TestStatus::Error,
            error: Some(error),
            duration_ms,
            performance: None,
        }
    }

    /// Add performance data
    pub fn with_performance(mut self, report: performance::PerformanceReport) -> Self {
        self.performance = Some(report);
        self
    }
}

impl TestContext {
    /// Create a new test context
    pub fn new(test_name: String) -> Self {
        Self {
            test_name,
            browser: None,
            screenshots: Vec::new(),
            measurements: Vec::new(),
            logs: Vec::new(),
        }
    }

    /// Log a debug message
    pub fn debug(&mut self, message: &str) {
        self.log(LogLevel::Debug, message);
    }

    /// Log an info message
    pub fn info(&mut self, message: &str) {
        self.log(LogLevel::Info, message);
    }

    /// Log a warning message
    pub fn warn(&mut self, message: &str) {
        self.log(LogLevel::Warn, message);
    }

    /// Log an error message
    pub fn error(&mut self, message: &str) {
        self.log(LogLevel::Error, message);
    }

    fn log(&mut self, level: LogLevel, message: &str) {
        self.logs.push(LogEntry {
            level,
            message: String::from(message),
            timestamp: self.current_time(),
        });
    }

    fn current_time(&self) -> u64 {
        // In real implementation, this would use kernel time
        0
    }

    /// Take a screenshot
    pub fn take_screenshot(&mut self, name: &str) -> Result<(), String> {
        if let Some(ref mut browser) = self.browser {
            let screenshot = browser.capture_screenshot(name)?;
            self.screenshots.push(screenshot);
            Ok(())
        } else {
            Err(String::from("No browser context available"))
        }
    }

    /// Start a performance measurement
    pub fn start_measurement(&mut self, name: &str) -> performance::MeasurementHandle {
        performance::MeasurementHandle::new(name)
    }

    /// Record a measurement
    pub fn record_measurement(&mut self, measurement: performance::Measurement) {
        self.measurements.push(measurement);
    }
}

/// Test suite containing multiple test cases
pub struct TestSuite {
    /// Suite name
    pub name: String,
    /// Test cases
    pub tests: Vec<TestCase>,
    /// Setup function
    pub setup: Option<Box<dyn Fn(&mut TestContext) -> Result<(), String> + Send + Sync>>,
    /// Teardown function
    pub teardown: Option<Box<dyn Fn(&mut TestContext) -> Result<(), String> + Send + Sync>>,
}

impl TestSuite {
    /// Create a new test suite
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            tests: Vec::new(),
            setup: None,
            teardown: None,
        }
    }

    /// Add a test case
    pub fn add_test(&mut self, test: TestCase) {
        self.tests.push(test);
    }

    /// Set suite setup function
    pub fn set_setup<F>(&mut self, f: F)
    where
        F: Fn(&mut TestContext) -> Result<(), String> + Send + Sync + 'static,
    {
        self.setup = Some(Box::new(f));
    }

    /// Set suite teardown function
    pub fn set_teardown<F>(&mut self, f: F)
    where
        F: Fn(&mut TestContext) -> Result<(), String> + Send + Sync + 'static,
    {
        self.teardown = Some(Box::new(f));
    }

    /// Get test count
    pub fn test_count(&self) -> usize {
        self.tests.len()
    }
}

/// Test suite report
#[derive(Debug, Clone)]
pub struct SuiteReport {
    /// Suite name
    pub name: String,
    /// Individual test results
    pub results: Vec<(String, TestResult)>,
    /// Total duration
    pub total_duration_ms: u64,
    /// Passed count
    pub passed: usize,
    /// Failed count
    pub failed: usize,
    /// Skipped count
    pub skipped: usize,
    /// Error count
    pub errors: usize,
}

impl SuiteReport {
    /// Create a new suite report
    pub fn new(name: String) -> Self {
        Self {
            name,
            results: Vec::new(),
            total_duration_ms: 0,
            passed: 0,
            failed: 0,
            skipped: 0,
            errors: 0,
        }
    }

    /// Add a test result
    pub fn add_result(&mut self, test_name: String, result: TestResult) {
        self.total_duration_ms += result.duration_ms;
        match result.status {
            TestStatus::Passed => self.passed += 1,
            TestStatus::Failed => self.failed += 1,
            TestStatus::Skipped => self.skipped += 1,
            TestStatus::Timeout => self.failed += 1,
            TestStatus::Error => self.errors += 1,
        }
        self.results.push((test_name, result));
    }

    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.failed == 0 && self.errors == 0
    }

    /// Get pass rate as percentage
    pub fn pass_rate(&self) -> f32 {
        let total = self.passed + self.failed + self.errors;
        if total == 0 {
            100.0
        } else {
            (self.passed as f32 / total as f32) * 100.0
        }
    }
}

impl fmt::Display for SuiteReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Test Suite: {}", self.name)?;
        writeln!(f, "==========================")?;
        writeln!(f, "Passed:  {} ✓", self.passed)?;
        writeln!(f, "Failed:  {} ✗", self.failed)?;
        writeln!(f, "Skipped: {} ○", self.skipped)?;
        writeln!(f, "Errors:  {} !", self.errors)?;
        writeln!(f, "Duration: {}ms", self.total_duration_ms)?;
        writeln!(f, "Pass Rate: {:.1}%", self.pass_rate())
    }
}
