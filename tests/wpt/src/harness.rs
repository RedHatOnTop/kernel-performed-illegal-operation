//! WPT harness implementation
//!
//! Implements the testharness.js API for running WPT tests.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box;

use crate::{SubtestStatus, TestStatus};

/// WPT testharness.js compatible API
pub struct WptTestHarness {
    /// Tests registered
    tests: Vec<WptTest>,
    /// Setup function
    setup_fn: Option<Box<dyn FnOnce() + Send>>,
    /// Explicit done required
    explicit_done: bool,
    /// Is completed
    completed: bool,
    /// Timeout in ms
    timeout: u64,
}

/// A single WPT test
struct WptTest {
    name: String,
    func: Box<dyn FnOnce(&mut TestContext) + Send>,
    properties: TestProperties,
}

/// Test properties
#[derive(Debug, Clone, Default)]
pub struct TestProperties {
    /// Test timeout in ms
    pub timeout: Option<u64>,
    /// Expected to fail
    pub expected_fail: bool,
}

/// Test execution context
pub struct TestContext {
    /// Current test name
    name: String,
    /// Assertions
    assertions: Vec<Assertion>,
    /// Status
    status: SubtestStatus,
}

/// An assertion result
struct Assertion {
    passed: bool,
    message: String,
}

impl WptTestHarness {
    /// Create new harness
    pub fn new() -> Self {
        Self {
            tests: Vec::new(),
            setup_fn: None,
            explicit_done: false,
            completed: false,
            timeout: 10000,
        }
    }

    /// Setup function (called before tests)
    pub fn setup<F: FnOnce() + Send + 'static>(&mut self, f: F) {
        self.setup_fn = Some(Box::new(f));
    }

    /// Setup with properties
    pub fn setup_with_properties<F: FnOnce() + Send + 'static>(
        &mut self,
        _properties: SetupProperties,
        f: F,
    ) {
        self.setup_fn = Some(Box::new(f));
    }

    /// Register a synchronous test
    pub fn test<F>(&mut self, func: F, name: &str)
    where
        F: FnOnce(&mut TestContext) + Send + 'static,
    {
        self.test_with_properties(func, name, TestProperties::default());
    }

    /// Register a test with properties
    pub fn test_with_properties<F>(&mut self, func: F, name: &str, properties: TestProperties)
    where
        F: FnOnce(&mut TestContext) + Send + 'static,
    {
        self.tests.push(WptTest {
            name: String::from(name),
            func: Box::new(func),
            properties,
        });
    }

    /// Register an async test
    pub fn async_test<F>(&mut self, func: F, name: &str)
    where
        F: FnOnce(&mut AsyncTestContext) + Send + 'static,
    {
        // Wrap async test as sync for now
        let wrapper = move |ctx: &mut TestContext| {
            let mut async_ctx = AsyncTestContext::new(ctx);
            func(&mut async_ctx);
        };
        self.test(wrapper, name);
    }

    /// Register a promise test
    pub fn promise_test<F>(&mut self, func: F, name: &str)
    where
        F: FnOnce() + Send + 'static,
    {
        let wrapper = move |_ctx: &mut TestContext| {
            func();
        };
        self.test(wrapper, name);
    }

    /// Generate tests dynamically
    pub fn generate_tests<F, T>(&mut self, func: F, parameters: Vec<(String, T)>)
    where
        F: Fn(&mut TestContext, &T) + Clone + Send + 'static,
        T: Send + 'static,
    {
        for (name, param) in parameters {
            let func_clone = func.clone();
            self.test(
                move |ctx| func_clone(ctx, &param),
                &name,
            );
        }
    }

    /// Mark explicit done required
    pub fn set_explicit_done(&mut self) {
        self.explicit_done = true;
    }

    /// Signal done
    pub fn done(&mut self) {
        self.completed = true;
    }

    /// Run all registered tests
    pub fn run(&mut self) -> HarnessResult {
        // Run setup
        if let Some(setup) = self.setup_fn.take() {
            setup();
        }

        let mut results = Vec::new();
        let mut overall_status = TestStatus::Ok;

        // Take tests to avoid borrow issues
        let tests = core::mem::take(&mut self.tests);

        for test in tests {
            let mut ctx = TestContext::new(&test.name);
            (test.func)(&mut ctx);

            let status = ctx.status;
            if status != SubtestStatus::Pass && !test.properties.expected_fail {
                overall_status = TestStatus::Error;
            }

            results.push(SubtestResult {
                name: test.name,
                status,
                message: ctx.get_message(),
            });
        }

        HarnessResult {
            status: overall_status,
            subtests: results,
        }
    }
}

impl Default for WptTestHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// Setup properties
#[derive(Debug, Clone, Default)]
pub struct SetupProperties {
    /// Explicit done
    pub explicit_done: bool,
    /// Timeout
    pub timeout: Option<u64>,
    /// Allow uncaught exceptions
    pub allow_uncaught_exception: bool,
}

impl TestContext {
    /// Create new context
    fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            assertions: Vec::new(),
            status: SubtestStatus::Pass,
        }
    }

    /// Get assertion message
    fn get_message(&self) -> Option<String> {
        for assertion in &self.assertions {
            if !assertion.passed {
                return Some(assertion.message.clone());
            }
        }
        None
    }

    fn add_assertion(&mut self, passed: bool, message: String) {
        if !passed {
            self.status = SubtestStatus::Fail;
        }
        self.assertions.push(Assertion { passed, message });
    }

    /// Assert true
    pub fn assert_true(&mut self, actual: bool, description: &str) {
        self.add_assertion(actual, String::from(description));
    }

    /// Assert false
    pub fn assert_false(&mut self, actual: bool, description: &str) {
        self.add_assertion(!actual, String::from(description));
    }

    /// Assert equals
    pub fn assert_equals<T: PartialEq + core::fmt::Debug>(
        &mut self,
        actual: T,
        expected: T,
        description: &str,
    ) {
        let passed = actual == expected;
        let message = if passed {
            String::from(description)
        } else {
            alloc::format!("{}: expected {:?}, got {:?}", description, expected, actual)
        };
        self.add_assertion(passed, message);
    }

    /// Assert not equals
    pub fn assert_not_equals<T: PartialEq + core::fmt::Debug>(
        &mut self,
        actual: T,
        expected: T,
        description: &str,
    ) {
        let passed = actual != expected;
        self.add_assertion(passed, String::from(description));
    }

    /// Assert array equals
    pub fn assert_array_equals<T: PartialEq + core::fmt::Debug>(
        &mut self,
        actual: &[T],
        expected: &[T],
        description: &str,
    ) {
        let passed = actual == expected;
        self.add_assertion(passed, String::from(description));
    }

    /// Assert approx equals
    pub fn assert_approx_equals(
        &mut self,
        actual: f64,
        expected: f64,
        epsilon: f64,
        description: &str,
    ) {
        let passed = (actual - expected).abs() <= epsilon;
        self.add_assertion(passed, String::from(description));
    }

    /// Assert less than
    pub fn assert_less_than<T: PartialOrd + core::fmt::Debug>(
        &mut self,
        actual: T,
        expected: T,
        description: &str,
    ) {
        let passed = actual < expected;
        self.add_assertion(passed, String::from(description));
    }

    /// Assert greater than
    pub fn assert_greater_than<T: PartialOrd + core::fmt::Debug>(
        &mut self,
        actual: T,
        expected: T,
        description: &str,
    ) {
        let passed = actual > expected;
        self.add_assertion(passed, String::from(description));
    }

    /// Assert in array
    pub fn assert_in_array<T: PartialEq + core::fmt::Debug>(
        &mut self,
        actual: &T,
        expected: &[T],
        description: &str,
    ) {
        let passed = expected.contains(actual);
        self.add_assertion(passed, String::from(description));
    }

    /// Assert regexp match
    pub fn assert_regexp_match(
        &mut self,
        actual: &str,
        pattern: &str,
        description: &str,
    ) {
        // Simplified: just check contains for now
        let passed = actual.contains(pattern);
        self.add_assertion(passed, String::from(description));
    }

    /// Assert class string
    pub fn assert_class_string(
        &mut self,
        object_class: &str,
        expected: &str,
        description: &str,
    ) {
        let passed = object_class == expected;
        self.add_assertion(passed, String::from(description));
    }

    /// Assert own property
    pub fn assert_own_property(
        &mut self,
        has_property: bool,
        property_name: &str,
        description: &str,
    ) {
        let message = if has_property {
            String::from(description)
        } else {
            alloc::format!("{}: missing property {}", description, property_name)
        };
        self.add_assertion(has_property, message);
    }

    /// Assert throws
    pub fn assert_throws<F: FnOnce() -> Result<(), String>>(
        &mut self,
        expected_type: &str,
        func: F,
        description: &str,
    ) {
        let result = func();
        let passed = match result {
            Err(e) => e.contains(expected_type),
            Ok(_) => false,
        };
        self.add_assertion(passed, String::from(description));
    }

    /// Assert throws dom exception
    pub fn assert_throws_dom(
        &mut self,
        expected_name: &str,
        func: impl FnOnce() -> Result<(), String>,
        description: &str,
    ) {
        self.assert_throws(expected_name, func, description);
    }

    /// Assert unreached
    pub fn assert_unreached(&mut self, description: &str) {
        self.add_assertion(false, String::from(description));
    }

    /// Step function for async tests
    pub fn step<F: FnOnce(&mut Self)>(&mut self, func: F) {
        func(self);
    }

    /// Done for async tests
    pub fn done(&mut self) {
        // Mark as complete
    }
}

/// Async test context
pub struct AsyncTestContext<'a> {
    ctx: &'a mut TestContext,
}

impl<'a> AsyncTestContext<'a> {
    fn new(ctx: &'a mut TestContext) -> Self {
        Self { ctx }
    }

    /// Step function
    pub fn step<F: FnOnce(&mut TestContext)>(&mut self, func: F) {
        func(self.ctx);
    }

    /// Step function that returns value
    pub fn step_func<F: FnOnce(&mut TestContext)>(&mut self, func: F) {
        func(self.ctx);
    }

    /// Mark as done
    pub fn done(&mut self) {
        self.ctx.done();
    }

    /// Add cleanup
    pub fn add_cleanup<F: FnOnce()>(&mut self, _func: F) {
        // Would add cleanup function
    }
}

/// Subtest result
#[derive(Debug, Clone)]
pub struct SubtestResult {
    pub name: String,
    pub status: SubtestStatus,
    pub message: Option<String>,
}

/// Harness result
#[derive(Debug, Clone)]
pub struct HarnessResult {
    pub status: TestStatus,
    pub subtests: Vec<SubtestResult>,
}

impl HarnessResult {
    /// Get pass count
    pub fn pass_count(&self) -> usize {
        self.subtests.iter().filter(|s| s.status == SubtestStatus::Pass).count()
    }

    /// Get fail count
    pub fn fail_count(&self) -> usize {
        self.subtests.iter().filter(|s| s.status == SubtestStatus::Fail).count()
    }

    /// Get pass rate
    pub fn pass_rate(&self) -> f32 {
        if self.subtests.is_empty() {
            return 100.0;
        }
        (self.pass_count() as f32 / self.subtests.len() as f32) * 100.0
    }
}

/// Format assertion message
pub fn format_value<T: core::fmt::Debug>(value: &T) -> String {
    alloc::format!("{:?}", value)
}
