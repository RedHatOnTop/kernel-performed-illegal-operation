//! Test harness for running E2E tests
//!
//! Provides the test runner and reporting infrastructure.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box;
use spin::Mutex;

use crate::{
    TestCase, TestContext, TestResult, TestStatus, TestSuite, SuiteReport,
    browser, performance,
};

/// Test runner for executing test suites
pub struct TestRunner {
    /// Suites to run
    suites: Vec<TestSuite>,
    /// Global configuration
    config: TestConfig,
    /// Event listeners
    listeners: Vec<Box<dyn TestListener + Send + Sync>>,
}

/// Test runner configuration
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Default timeout in milliseconds
    pub default_timeout_ms: u64,
    /// Retry failed tests
    pub retry_failed: u32,
    /// Run tests in parallel
    pub parallel: bool,
    /// Filter tests by tag
    pub tag_filter: Option<String>,
    /// Filter tests by name pattern
    pub name_filter: Option<String>,
    /// Stop on first failure
    pub fail_fast: bool,
    /// Capture screenshots on failure
    pub screenshot_on_failure: bool,
    /// Generate performance report
    pub performance_report: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            default_timeout_ms: 30000,
            retry_failed: 0,
            parallel: false,
            tag_filter: None,
            name_filter: None,
            fail_fast: false,
            screenshot_on_failure: true,
            performance_report: true,
        }
    }
}

/// Test event listener
pub trait TestListener {
    /// Called when a suite starts
    fn on_suite_start(&mut self, suite_name: &str);
    
    /// Called when a suite ends
    fn on_suite_end(&mut self, report: &SuiteReport);
    
    /// Called when a test starts
    fn on_test_start(&mut self, test_name: &str);
    
    /// Called when a test ends
    fn on_test_end(&mut self, test_name: &str, result: &TestResult);
}

/// Console reporter that prints to stdout
pub struct ConsoleReporter {
    verbose: bool,
}

impl ConsoleReporter {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

impl TestListener for ConsoleReporter {
    fn on_suite_start(&mut self, suite_name: &str) {
        // Would print to console
        let _ = suite_name;
    }

    fn on_suite_end(&mut self, report: &SuiteReport) {
        let _ = report;
    }

    fn on_test_start(&mut self, test_name: &str) {
        if self.verbose {
            let _ = test_name;
        }
    }

    fn on_test_end(&mut self, test_name: &str, result: &TestResult) {
        let _ = (test_name, result);
    }
}

/// JUnit XML reporter for CI integration
pub struct JUnitReporter {
    output_path: String,
    reports: Vec<SuiteReport>,
}

impl JUnitReporter {
    pub fn new(output_path: &str) -> Self {
        Self {
            output_path: String::from(output_path),
            reports: Vec::new(),
        }
    }

    /// Generate JUnit XML output
    pub fn generate_xml(&self) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<testsuites>\n");

        for report in &self.reports {
            xml.push_str(&alloc::format!(
                "  <testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" errors=\"{}\" time=\"{}\">\n",
                report.name,
                report.results.len(),
                report.failed,
                report.errors,
                report.total_duration_ms as f64 / 1000.0
            ));

            for (test_name, result) in &report.results {
                xml.push_str(&alloc::format!(
                    "    <testcase name=\"{}\" time=\"{}\"",
                    test_name,
                    result.duration_ms as f64 / 1000.0
                ));

                match result.status {
                    TestStatus::Passed => {
                        xml.push_str("/>\n");
                    }
                    TestStatus::Failed | TestStatus::Timeout => {
                        xml.push_str(">\n");
                        if let Some(ref error) = result.error {
                            xml.push_str(&alloc::format!(
                                "      <failure message=\"{}\">{}</failure>\n",
                                error, error
                            ));
                        }
                        xml.push_str("    </testcase>\n");
                    }
                    TestStatus::Error => {
                        xml.push_str(">\n");
                        if let Some(ref error) = result.error {
                            xml.push_str(&alloc::format!(
                                "      <error message=\"{}\">{}</error>\n",
                                error, error
                            ));
                        }
                        xml.push_str("    </testcase>\n");
                    }
                    TestStatus::Skipped => {
                        xml.push_str(">\n      <skipped/>\n    </testcase>\n");
                    }
                }
            }

            xml.push_str("  </testsuite>\n");
        }

        xml.push_str("</testsuites>");
        xml
    }
}

impl TestListener for JUnitReporter {
    fn on_suite_start(&mut self, _suite_name: &str) {}

    fn on_suite_end(&mut self, report: &SuiteReport) {
        self.reports.push(report.clone());
    }

    fn on_test_start(&mut self, _test_name: &str) {}
    fn on_test_end(&mut self, _test_name: &str, _result: &TestResult) {}
}

impl TestRunner {
    /// Create a new test runner
    pub fn new() -> Self {
        Self {
            suites: Vec::new(),
            config: TestConfig::default(),
            listeners: Vec::new(),
        }
    }

    /// Create with configuration
    pub fn with_config(config: TestConfig) -> Self {
        Self {
            suites: Vec::new(),
            config,
            listeners: Vec::new(),
        }
    }

    /// Add a test suite
    pub fn add_suite(&mut self, suite: TestSuite) {
        self.suites.push(suite);
    }

    /// Add a listener
    pub fn add_listener<L: TestListener + Send + Sync + 'static>(&mut self, listener: L) {
        self.listeners.push(Box::new(listener));
    }

    /// Run all test suites
    pub fn run(&mut self) -> Vec<SuiteReport> {
        let mut reports = Vec::new();

        let suite_count = self.suites.len();
        for suite_idx in 0..suite_count {
            let report = self.run_suite_by_index(suite_idx);
            
            for listener in &mut self.listeners {
                listener.on_suite_end(&report);
            }
            
            let fail_fast = self.config.fail_fast && !report.all_passed();
            reports.push(report);

            if fail_fast {
                break;
            }
        }

        reports
    }

    /// Run a single suite by index
    fn run_suite_by_index(&mut self, suite_idx: usize) -> SuiteReport {
        let suite_name = self.suites[suite_idx].name.clone();
        
        for listener in &mut self.listeners {
            listener.on_suite_start(&suite_name);
        }

        let mut report = SuiteReport::new(suite_name);

        let test_count = self.suites[suite_idx].tests.len();
        
        for test_idx in 0..test_count {
            // Get test info without holding borrow
            let (test_name, test_tags, should_skip) = {
                let test = &self.suites[suite_idx].tests[test_idx];
                (
                    test.name.clone(),
                    test.tags.clone(),
                    !self.should_run_test_info(&test.name, &test.tags),
                )
            };
            
            if should_skip {
                report.add_result(
                    test_name,
                    TestResult::skipped(String::from("Filtered out")),
                );
                continue;
            }

            for listener in &mut self.listeners {
                listener.on_test_start(&test_name);
            }

            let result = self.run_test_by_index(suite_idx, test_idx);

            for listener in &mut self.listeners {
                listener.on_test_end(&test_name, &result);
            }

            report.add_result(test_name, result);

            if self.config.fail_fast && !report.all_passed() {
                break;
            }
        }

        report
    }

    /// Check if test should run based on filters (using extracted info)
    fn should_run_test_info(&self, name: &str, tags: &[String]) -> bool {
        // Check tag filter
        if let Some(ref tag_filter) = self.config.tag_filter {
            if !tags.iter().any(|t| t == tag_filter) {
                return false;
            }
        }

        // Check name filter
        if let Some(ref name_filter) = self.config.name_filter {
            if !name.contains(name_filter.as_str()) {
                return false;
            }
        }

        true
    }

    /// Run a single test by index
    fn run_test_by_index(&self, suite_idx: usize, test_idx: usize) -> TestResult {
        let mut attempts = 0;
        let max_attempts = 1 + self.config.retry_failed;

        while attempts < max_attempts {
            attempts += 1;
            
            let test_name = self.suites[suite_idx].tests[test_idx].name.clone();
            let mut context = TestContext::new(test_name);

            // Set up browser if required
            let requires_browser = self.suites[suite_idx].tests[test_idx].requires_browser;
            if requires_browser {
                match browser::launch() {
                    Ok(browser) => context.browser = Some(browser),
                    Err(e) => return TestResult::error(e, 0),
                }
            }

            // Run suite setup
            if let Some(ref setup) = self.suites[suite_idx].setup {
                if let Err(e) = setup(&mut context) {
                    return TestResult::error(alloc::format!("Setup failed: {}", e), 0);
                }
            }

            // Run test
            let result = (self.suites[suite_idx].tests[test_idx].test_fn)(&mut context);

            // Run suite teardown
            if let Some(ref teardown) = self.suites[suite_idx].teardown {
                let _ = teardown(&mut context);
            }

            // Capture screenshot on failure
            if self.config.screenshot_on_failure && result.status == TestStatus::Failed {
                let _ = context.take_screenshot("failure");
            }

            // Clean up browser
            if let Some(browser) = context.browser {
                let _ = browser.close();
            }

            if result.status == TestStatus::Passed {
                return result;
            }

            // Only retry if not the last attempt
            if attempts >= max_attempts {
                return result;
            }
        }

        TestResult::error(String::from("Exhausted all retry attempts"), 0)
    }
}

/// Test builder for fluent test creation
pub struct TestBuilder {
    name: String,
    description: String,
    timeout_ms: u64,
    tags: Vec<String>,
    requires_browser: bool,
}

impl TestBuilder {
    /// Create a new test builder
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            description: String::new(),
            timeout_ms: 30000,
            tags: Vec::new(),
            requires_browser: false,
        }
    }

    /// Set description
    pub fn description(mut self, desc: &str) -> Self {
        self.description = String::from(desc);
        self
    }

    /// Set timeout
    pub fn timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    /// Add tag
    pub fn tag(mut self, tag: &str) -> Self {
        self.tags.push(String::from(tag));
        self
    }

    /// Require browser
    pub fn with_browser(mut self) -> Self {
        self.requires_browser = true;
        self
    }

    /// Build with test function
    pub fn build<F>(self, f: F) -> TestCase
    where
        F: Fn(&mut TestContext) -> TestResult + Send + Sync + 'static,
    {
        TestCase {
            name: self.name,
            description: self.description,
            test_fn: Box::new(f),
            timeout_ms: self.timeout_ms,
            tags: self.tags,
            requires_browser: self.requires_browser,
        }
    }
}

/// Create a test case
pub fn test<F>(name: &str, f: F) -> TestCase
where
    F: Fn(&mut TestContext) -> TestResult + Send + Sync + 'static,
{
    TestBuilder::new(name).build(f)
}

/// Create a browser test case
pub fn browser_test<F>(name: &str, f: F) -> TestCase
where
    F: Fn(&mut TestContext) -> TestResult + Send + Sync + 'static,
{
    TestBuilder::new(name).with_browser().build(f)
}

/// Global test registry for automatic test discovery
static TEST_REGISTRY: Mutex<Option<Vec<TestSuite>>> = Mutex::new(None);

/// Register a test suite globally
pub fn register_suite(suite: TestSuite) {
    let mut registry = TEST_REGISTRY.lock();
    if registry.is_none() {
        *registry = Some(Vec::new());
    }
    registry.as_mut().unwrap().push(suite);
}

/// Get all registered suites (takes ownership, clears registry)
pub fn registered_suites() -> Vec<TestSuite> {
    let mut registry = TEST_REGISTRY.lock();
    registry.take().unwrap_or_default()
}

/// Run all registered tests
pub fn run_all() -> Vec<SuiteReport> {
    let mut runner = TestRunner::new();
    runner.add_listener(ConsoleReporter::new(true));
    
    for suite in registered_suites() {
        runner.add_suite(suite);
    }
    
    runner.run()
}

/// Run tests with configuration
pub fn run_with_config(config: TestConfig) -> Vec<SuiteReport> {
    let mut runner = TestRunner::with_config(config);
    runner.add_listener(ConsoleReporter::new(true));
    
    for suite in registered_suites() {
        runner.add_suite(suite);
    }
    
    runner.run()
}
