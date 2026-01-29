//! Test assertions
//!
//! Provides assertion utilities for E2E tests.

use alloc::string::String;
use alloc::format;
use crate::browser::{ElementHandle, BrowserHandle, JsValue};
use crate::screenshot::{Screenshot, ComparisonResult};

/// Assertion result
pub type AssertResult = Result<(), String>;

/// Assert that a condition is true
pub fn assert_true(condition: bool, message: &str) -> AssertResult {
    if condition {
        Ok(())
    } else {
        Err(String::from(message))
    }
}

/// Assert that a condition is false
pub fn assert_false(condition: bool, message: &str) -> AssertResult {
    if !condition {
        Ok(())
    } else {
        Err(String::from(message))
    }
}

/// Assert equality
pub fn assert_eq<T: PartialEq + core::fmt::Debug>(left: T, right: T) -> AssertResult {
    if left == right {
        Ok(())
    } else {
        Err(format!("Expected {:?} to equal {:?}", left, right))
    }
}

/// Assert inequality
pub fn assert_ne<T: PartialEq + core::fmt::Debug>(left: T, right: T) -> AssertResult {
    if left != right {
        Ok(())
    } else {
        Err(format!("Expected {:?} to not equal {:?}", left, right))
    }
}

/// Assert that value is Some
pub fn assert_some<T: core::fmt::Debug>(value: &Option<T>) -> AssertResult {
    match value {
        Some(_) => Ok(()),
        None => Err(String::from("Expected Some, got None")),
    }
}

/// Assert that value is None
pub fn assert_none<T: core::fmt::Debug>(value: &Option<T>) -> AssertResult {
    match value {
        None => Ok(()),
        Some(v) => Err(format!("Expected None, got Some({:?})", v)),
    }
}

/// Assert that result is Ok
pub fn assert_ok<T, E: core::fmt::Debug>(result: &Result<T, E>) -> AssertResult {
    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Expected Ok, got Err({:?})", e)),
    }
}

/// Assert that result is Err
pub fn assert_err<T: core::fmt::Debug, E>(result: &Result<T, E>) -> AssertResult {
    match result {
        Err(_) => Ok(()),
        Ok(v) => Err(format!("Expected Err, got Ok({:?})", v)),
    }
}

/// Assert string contains substring
pub fn assert_contains(haystack: &str, needle: &str) -> AssertResult {
    if haystack.contains(needle) {
        Ok(())
    } else {
        Err(format!("Expected '{}' to contain '{}'", haystack, needle))
    }
}

/// Assert string starts with prefix
pub fn assert_starts_with(string: &str, prefix: &str) -> AssertResult {
    if string.starts_with(prefix) {
        Ok(())
    } else {
        Err(format!("Expected '{}' to start with '{}'", string, prefix))
    }
}

/// Assert string ends with suffix
pub fn assert_ends_with(string: &str, suffix: &str) -> AssertResult {
    if string.ends_with(suffix) {
        Ok(())
    } else {
        Err(format!("Expected '{}' to end with '{}'", string, suffix))
    }
}

/// Assert string matches pattern (simple glob)
pub fn assert_matches(string: &str, pattern: &str) -> AssertResult {
    // Simple pattern matching - just check contains for now
    if pattern.contains('*') {
        let parts: alloc::vec::Vec<&str> = pattern.split('*').collect();
        let mut remaining = string;
        for part in parts {
            if part.is_empty() {
                continue;
            }
            if let Some(pos) = remaining.find(part) {
                remaining = &remaining[pos + part.len()..];
            } else {
                return Err(format!("Expected '{}' to match pattern '{}'", string, pattern));
            }
        }
        Ok(())
    } else {
        assert_eq(string, pattern)
    }
}

/// Assert value is greater than
pub fn assert_gt<T: PartialOrd + core::fmt::Debug>(left: T, right: T) -> AssertResult {
    if left > right {
        Ok(())
    } else {
        Err(format!("Expected {:?} > {:?}", left, right))
    }
}

/// Assert value is greater than or equal
pub fn assert_gte<T: PartialOrd + core::fmt::Debug>(left: T, right: T) -> AssertResult {
    if left >= right {
        Ok(())
    } else {
        Err(format!("Expected {:?} >= {:?}", left, right))
    }
}

/// Assert value is less than
pub fn assert_lt<T: PartialOrd + core::fmt::Debug>(left: T, right: T) -> AssertResult {
    if left < right {
        Ok(())
    } else {
        Err(format!("Expected {:?} < {:?}", left, right))
    }
}

/// Assert value is less than or equal
pub fn assert_lte<T: PartialOrd + core::fmt::Debug>(left: T, right: T) -> AssertResult {
    if left <= right {
        Ok(())
    } else {
        Err(format!("Expected {:?} <= {:?}", left, right))
    }
}

/// Assert value is in range
pub fn assert_in_range<T: PartialOrd + core::fmt::Debug>(value: T, min: T, max: T) -> AssertResult {
    if value >= min && value <= max {
        Ok(())
    } else {
        Err(format!("Expected {:?} to be in range [{:?}, {:?}]", value, min, max))
    }
}

// Browser-specific assertions

/// Assert element exists
pub fn assert_element_exists(element: &Option<ElementHandle>) -> AssertResult {
    match element {
        Some(_) => Ok(()),
        None => Err(String::from("Expected element to exist")),
    }
}

/// Assert element is visible
pub fn assert_element_visible(element: &ElementHandle) -> AssertResult {
    if element.is_visible() {
        Ok(())
    } else {
        Err(String::from("Expected element to be visible"))
    }
}

/// Assert element is not visible
pub fn assert_element_hidden(element: &ElementHandle) -> AssertResult {
    if !element.is_visible() {
        Ok(())
    } else {
        Err(String::from("Expected element to be hidden"))
    }
}

/// Assert element is enabled
pub fn assert_element_enabled(element: &ElementHandle) -> AssertResult {
    if element.is_enabled() {
        Ok(())
    } else {
        Err(String::from("Expected element to be enabled"))
    }
}

/// Assert element is disabled
pub fn assert_element_disabled(element: &ElementHandle) -> AssertResult {
    if !element.is_enabled() {
        Ok(())
    } else {
        Err(String::from("Expected element to be disabled"))
    }
}

/// Assert element text content
pub fn assert_element_text(element: &ElementHandle, expected: &str) -> AssertResult {
    let actual = element.text_content();
    if actual == expected {
        Ok(())
    } else {
        Err(format!("Expected element text '{}', got '{}'", expected, actual))
    }
}

/// Assert element text contains
pub fn assert_element_text_contains(element: &ElementHandle, substring: &str) -> AssertResult {
    let actual = element.text_content();
    if actual.contains(substring) {
        Ok(())
    } else {
        Err(format!("Expected element text to contain '{}', got '{}'", substring, actual))
    }
}

/// Assert element has attribute
pub fn assert_element_has_attribute(element: &ElementHandle, name: &str) -> AssertResult {
    if element.attribute(name).is_some() {
        Ok(())
    } else {
        Err(format!("Expected element to have attribute '{}'", name))
    }
}

/// Assert element attribute value
pub fn assert_element_attribute(element: &ElementHandle, name: &str, expected: &str) -> AssertResult {
    match element.attribute(name) {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(format!(
            "Expected attribute '{}' to be '{}', got '{}'",
            name, expected, actual
        )),
        None => Err(format!("Element does not have attribute '{}'", name)),
    }
}

/// Assert element is checked (checkbox/radio)
pub fn assert_element_checked(element: &ElementHandle) -> AssertResult {
    if element.is_checked() {
        Ok(())
    } else {
        Err(String::from("Expected element to be checked"))
    }
}

/// Assert element is not checked
pub fn assert_element_not_checked(element: &ElementHandle) -> AssertResult {
    if !element.is_checked() {
        Ok(())
    } else {
        Err(String::from("Expected element to not be checked"))
    }
}

// Browser/page assertions

/// Assert page URL
pub fn assert_url(browser: &BrowserHandle, expected: &str) -> AssertResult {
    let actual = browser.url();
    if actual == expected {
        Ok(())
    } else {
        Err(format!("Expected URL '{}', got '{}'", expected, actual))
    }
}

/// Assert page URL contains
pub fn assert_url_contains(browser: &BrowserHandle, substring: &str) -> AssertResult {
    let actual = browser.url();
    if actual.contains(substring) {
        Ok(())
    } else {
        Err(format!("Expected URL to contain '{}', got '{}'", substring, actual))
    }
}

/// Assert page title
pub fn assert_title(browser: &BrowserHandle, expected: &str) -> AssertResult {
    let actual = browser.title();
    if actual == expected {
        Ok(())
    } else {
        Err(format!("Expected title '{}', got '{}'", expected, actual))
    }
}

/// Assert page title contains
pub fn assert_title_contains(browser: &BrowserHandle, substring: &str) -> AssertResult {
    let actual = browser.title();
    if actual.contains(substring) {
        Ok(())
    } else {
        Err(format!("Expected title to contain '{}', got '{}'", substring, actual))
    }
}

// Screenshot assertions

/// Assert screenshot matches baseline
pub fn assert_screenshot_matches(result: &ComparisonResult) -> AssertResult {
    if result.matches {
        Ok(())
    } else {
        Err(format!(
            "Screenshot mismatch: {:.2}% different ({} pixels)",
            result.diff_percentage, result.diff_pixels
        ))
    }
}

/// Assert screenshot matches with custom threshold
pub fn assert_screenshot_matches_threshold(result: &ComparisonResult, threshold: f32) -> AssertResult {
    if result.diff_percentage <= threshold {
        Ok(())
    } else {
        Err(format!(
            "Screenshot difference {:.2}% exceeds threshold {:.2}%",
            result.diff_percentage, threshold
        ))
    }
}

// JavaScript value assertions

/// Assert JavaScript value is truthy
pub fn assert_js_truthy(value: &JsValue) -> AssertResult {
    match value {
        JsValue::Undefined | JsValue::Null => {
            Err(String::from("Expected truthy value, got undefined/null"))
        }
        JsValue::Boolean(false) => {
            Err(String::from("Expected truthy value, got false"))
        }
        JsValue::Number(n) if *n == 0.0 => {
            Err(String::from("Expected truthy value, got 0"))
        }
        JsValue::String(s) if s.is_empty() => {
            Err(String::from("Expected truthy value, got empty string"))
        }
        _ => Ok(()),
    }
}

/// Assert JavaScript value is falsy
pub fn assert_js_falsy(value: &JsValue) -> AssertResult {
    match value {
        JsValue::Undefined | JsValue::Null | JsValue::Boolean(false) => Ok(()),
        JsValue::Number(n) if *n == 0.0 => Ok(()),
        JsValue::String(s) if s.is_empty() => Ok(()),
        _ => Err(String::from("Expected falsy value")),
    }
}

/// Assert JavaScript value equals expected
pub fn assert_js_eq(value: &JsValue, expected: &JsValue) -> AssertResult {
    // Simple equality check
    match (value, expected) {
        (JsValue::Undefined, JsValue::Undefined) => Ok(()),
        (JsValue::Null, JsValue::Null) => Ok(()),
        (JsValue::Boolean(a), JsValue::Boolean(b)) if a == b => Ok(()),
        (JsValue::Number(a), JsValue::Number(b)) if (a - b).abs() < f64::EPSILON => Ok(()),
        (JsValue::String(a), JsValue::String(b)) if a == b => Ok(()),
        _ => Err(format!("JavaScript values do not match")),
    }
}

// Performance assertions

/// Assert duration is under threshold
pub fn assert_performance_under(duration_ms: f64, threshold_ms: f64) -> AssertResult {
    if duration_ms <= threshold_ms {
        Ok(())
    } else {
        Err(format!(
            "Performance: {:.2}ms exceeds threshold {:.2}ms",
            duration_ms, threshold_ms
        ))
    }
}

/// Assert memory usage is under threshold
pub fn assert_memory_under(bytes: u64, threshold_bytes: u64) -> AssertResult {
    if bytes <= threshold_bytes {
        Ok(())
    } else {
        Err(format!(
            "Memory usage: {} bytes exceeds threshold {} bytes",
            bytes, threshold_bytes
        ))
    }
}

/// Assertion builder for fluent assertions
pub struct Expect<T> {
    value: T,
    description: Option<String>,
}

impl<T> Expect<T> {
    /// Create new expectation
    pub fn new(value: T) -> Self {
        Self {
            value,
            description: None,
        }
    }

    /// Add description for better error messages
    pub fn described_as(mut self, desc: &str) -> Self {
        self.description = Some(String::from(desc));
        self
    }

    /// Get the underlying value
    pub fn value(&self) -> &T {
        &self.value
    }
}

impl<T: PartialEq + core::fmt::Debug> Expect<T> {
    /// Assert equals
    pub fn to_equal(self, expected: T) -> AssertResult {
        if self.value == expected {
            Ok(())
        } else {
            let desc = self.description.unwrap_or_else(|| String::from("Value"));
            Err(format!("{}: expected {:?}, got {:?}", desc, expected, self.value))
        }
    }

    /// Assert not equals
    pub fn to_not_equal(self, expected: T) -> AssertResult {
        if self.value != expected {
            Ok(())
        } else {
            let desc = self.description.unwrap_or_else(|| String::from("Value"));
            Err(format!("{}: expected not {:?}", desc, expected))
        }
    }
}

impl Expect<bool> {
    /// Assert is true
    pub fn to_be_true(self) -> AssertResult {
        if self.value {
            Ok(())
        } else {
            let desc = self.description.unwrap_or_else(|| String::from("Condition"));
            Err(format!("{}: expected true, got false", desc))
        }
    }

    /// Assert is false
    pub fn to_be_false(self) -> AssertResult {
        if !self.value {
            Ok(())
        } else {
            let desc = self.description.unwrap_or_else(|| String::from("Condition"));
            Err(format!("{}: expected false, got true", desc))
        }
    }
}

impl Expect<&str> {
    /// Assert contains
    pub fn to_contain(self, substring: &str) -> AssertResult {
        if self.value.contains(substring) {
            Ok(())
        } else {
            let desc = self.description.unwrap_or_else(|| String::from("String"));
            Err(format!("{}: '{}' does not contain '{}'", desc, self.value, substring))
        }
    }
}

impl<T: PartialOrd + core::fmt::Debug> Expect<T> {
    /// Assert greater than
    pub fn to_be_greater_than(self, other: T) -> AssertResult {
        if self.value > other {
            Ok(())
        } else {
            let desc = self.description.unwrap_or_else(|| String::from("Value"));
            Err(format!("{}: {:?} is not greater than {:?}", desc, self.value, other))
        }
    }

    /// Assert less than
    pub fn to_be_less_than(self, other: T) -> AssertResult {
        if self.value < other {
            Ok(())
        } else {
            let desc = self.description.unwrap_or_else(|| String::from("Value"));
            Err(format!("{}: {:?} is not less than {:?}", desc, self.value, other))
        }
    }
}

/// Create an expectation
pub fn expect<T>(value: T) -> Expect<T> {
    Expect::new(value)
}
