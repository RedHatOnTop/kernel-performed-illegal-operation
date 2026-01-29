//! WPT manifest parsing
//!
//! Parses WPT MANIFEST.json files to discover tests.

use alloc::string::String;
use alloc::vec::Vec;
use hashbrown::HashMap;

use crate::{TestMetadata, TestType, ExpectedResult};

/// WPT manifest
pub struct Manifest {
    /// Version
    pub version: u32,
    /// URL base
    pub url_base: String,
    /// Tests by type
    pub tests: HashMap<String, Vec<ManifestTest>>,
}

/// A test entry in the manifest
#[derive(Debug, Clone)]
pub struct ManifestTest {
    /// Test path
    pub path: String,
    /// Test URL
    pub url: String,
    /// Test type (testharness, reftest, etc.)
    pub test_type: String,
    /// References (for reftests)
    pub references: Vec<Reference>,
    /// Script metadata
    pub script_metadata: Vec<ScriptMetadata>,
}

/// Reference for reftests
#[derive(Debug, Clone)]
pub struct Reference {
    /// Reference URL
    pub url: String,
    /// Relation (== or !=)
    pub relation: String,
}

/// Script metadata from test file
#[derive(Debug, Clone)]
pub struct ScriptMetadata {
    /// Metadata key
    pub key: String,
    /// Metadata value
    pub value: String,
}

impl Manifest {
    /// Parse manifest from JSON string
    pub fn parse(json: &str) -> Result<Self, String> {
        // Simplified JSON parsing
        // In real implementation, use a proper JSON parser
        
        // Look for version
        let version = 8; // Default WPT manifest version
        
        // Look for url_base
        let url_base = String::from("/");
        
        let tests = HashMap::new();
        
        // Would parse actual JSON here
        let _ = json;
        
        Ok(Self {
            version,
            url_base,
            tests,
        })
    }

    /// Get all tests of a specific type
    pub fn get_tests(&self, test_type: &str) -> Vec<&ManifestTest> {
        self.tests
            .get(test_type)
            .map(|tests| tests.iter().collect())
            .unwrap_or_default()
    }

    /// Get all tests
    pub fn all_tests(&self) -> Vec<&ManifestTest> {
        self.tests.values().flatten().collect()
    }

    /// Convert manifest test to metadata
    pub fn to_metadata(&self, test: &ManifestTest) -> TestMetadata {
        let test_type = match test.test_type.as_str() {
            "testharness" => TestType::TestHarness,
            "reftest" => TestType::RefTest,
            "visual" => TestType::Visual,
            "manual" => TestType::Manual,
            "crashtest" => TestType::CrashTest,
            "print-reftest" => TestType::PrintRefTest,
            _ => TestType::TestHarness,
        };

        // Extract timeout from metadata
        let timeout = test.script_metadata.iter()
            .find(|m| m.key == "timeout")
            .and_then(|m| m.value.parse().ok())
            .unwrap_or(10);

        TestMetadata {
            path: test.path.clone(),
            test_type,
            title: test.path.clone(),
            expected: ExpectedResult::Pass,
            timeout,
            disabled: None,
            preconditions: Vec::new(),
        }
    }

    /// Filter tests by path pattern
    pub fn filter_by_path(&self, pattern: &str) -> Vec<&ManifestTest> {
        self.all_tests()
            .into_iter()
            .filter(|t| t.path.contains(pattern))
            .collect()
    }

    /// Get test count
    pub fn test_count(&self) -> usize {
        self.tests.values().map(|v| v.len()).sum()
    }

    /// Get test count by type
    pub fn test_count_by_type(&self, test_type: &str) -> usize {
        self.tests.get(test_type).map(|v| v.len()).unwrap_or(0)
    }
}

/// Expectations file parser
pub struct ExpectationsFile {
    /// Expectations by test path
    expectations: HashMap<String, TestExpectation>,
}

/// Expectation for a single test
#[derive(Debug, Clone)]
pub struct TestExpectation {
    /// Expected result
    pub expected: ExpectedResult,
    /// Disabled reason
    pub disabled: Option<String>,
    /// Bug reference
    pub bug: Option<String>,
    /// Subtest expectations
    pub subtests: HashMap<String, ExpectedResult>,
}

impl ExpectationsFile {
    /// Parse expectations from INI-like format
    pub fn parse(content: &str) -> Result<Self, String> {
        let mut expectations = HashMap::new();
        let mut current_test: Option<String> = None;
        let mut current_expectation: Option<TestExpectation> = None;

        for line in content.lines() {
            let line = line.trim();
            
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                // Save previous test
                if let (Some(test), Some(exp)) = (current_test.take(), current_expectation.take()) {
                    expectations.insert(test, exp);
                }
                
                // Start new test
                let path = &line[1..line.len()-1];
                current_test = Some(String::from(path));
                current_expectation = Some(TestExpectation {
                    expected: ExpectedResult::Pass,
                    disabled: None,
                    bug: None,
                    subtests: HashMap::new(),
                });
            } else if let Some(ref mut exp) = current_expectation {
                // Parse key-value
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim();
                    let value = value.trim();
                    
                    match key {
                        "expected" => {
                            exp.expected = parse_expected_result(value);
                        }
                        "disabled" => {
                            exp.disabled = Some(String::from(value));
                        }
                        "bug" => {
                            exp.bug = Some(String::from(value));
                        }
                        _ => {
                            // Subtest expectation
                            if key.starts_with('[') && key.ends_with(']') {
                                let subtest_name = &key[1..key.len()-1];
                                exp.subtests.insert(
                                    String::from(subtest_name),
                                    parse_expected_result(value),
                                );
                            }
                        }
                    }
                }
            }
        }

        // Save last test
        if let (Some(test), Some(exp)) = (current_test, current_expectation) {
            expectations.insert(test, exp);
        }

        Ok(Self { expectations })
    }

    /// Get expectation for a test
    pub fn get(&self, path: &str) -> Option<&TestExpectation> {
        self.expectations.get(path)
    }

    /// Check if test is disabled
    pub fn is_disabled(&self, path: &str) -> bool {
        self.expectations.get(path)
            .map(|e| e.disabled.is_some())
            .unwrap_or(false)
    }

    /// Get expected result
    pub fn expected_result(&self, path: &str) -> ExpectedResult {
        self.expectations.get(path)
            .map(|e| e.expected.clone())
            .unwrap_or(ExpectedResult::Pass)
    }

    /// Apply expectations to metadata
    pub fn apply_to_metadata(&self, metadata: &mut TestMetadata) {
        if let Some(exp) = self.expectations.get(&metadata.path) {
            metadata.expected = exp.expected.clone();
            metadata.disabled = exp.disabled.clone();
        }
    }
}

/// Parse expected result from string
fn parse_expected_result(s: &str) -> ExpectedResult {
    match s.to_uppercase().as_str() {
        "PASS" | "OK" => ExpectedResult::Pass,
        "FAIL" | "ERROR" => ExpectedResult::Fail,
        "TIMEOUT" => ExpectedResult::Timeout,
        "CRASH" => ExpectedResult::Crash,
        "FLAKY" => ExpectedResult::Flaky,
        _ => ExpectedResult::Pass,
    }
}

/// Test path utilities
pub struct TestPath;

impl TestPath {
    /// Get test directory
    pub fn directory(path: &str) -> &str {
        path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("")
    }

    /// Get test filename
    pub fn filename(path: &str) -> &str {
        path.rsplit_once('/').map(|(_, file)| file).unwrap_or(path)
    }

    /// Get test extension
    pub fn extension(path: &str) -> &str {
        Self::filename(path)
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .unwrap_or("")
    }

    /// Check if path matches pattern
    pub fn matches(path: &str, pattern: &str) -> bool {
        if pattern.contains('*') {
            // Simple glob matching
            let parts: Vec<&str> = pattern.split('*').collect();
            let mut remaining = path;
            for part in parts {
                if part.is_empty() {
                    continue;
                }
                if let Some(pos) = remaining.find(part) {
                    remaining = &remaining[pos + part.len()..];
                } else {
                    return false;
                }
            }
            true
        } else {
            path == pattern
        }
    }

    /// Normalize path
    pub fn normalize(path: &str) -> String {
        let mut result = String::new();
        let mut last_was_slash = false;
        
        for c in path.chars() {
            if c == '/' {
                if !last_was_slash {
                    result.push(c);
                }
                last_was_slash = true;
            } else {
                result.push(c);
                last_was_slash = false;
            }
        }
        
        // Ensure leading slash
        if !result.starts_with('/') {
            result.insert(0, '/');
        }
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_utilities() {
        assert_eq!(TestPath::directory("/dom/test.html"), "/dom");
        assert_eq!(TestPath::filename("/dom/test.html"), "test.html");
        assert_eq!(TestPath::extension("/dom/test.html"), "html");
        assert!(TestPath::matches("/dom/test.html", "/dom/*"));
        assert!(TestPath::matches("/dom/sub/test.html", "/dom/**/*.html"));
    }
}
