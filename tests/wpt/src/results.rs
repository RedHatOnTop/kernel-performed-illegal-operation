//! WPT results handling
//!
//! Provides result collection, reporting, and export functionality.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

use crate::{RunSummary, TestResult, TestStatus, SubtestResult, SubtestStatus};

/// Result reporter trait
pub trait ResultReporter {
    /// Generate report from summary
    fn generate(&self, summary: &RunSummary) -> String;
}

/// JSON result reporter
pub struct JsonReporter;

impl JsonReporter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl ResultReporter for JsonReporter {
    fn generate(&self, summary: &RunSummary) -> String {
        let mut json = String::from("{\n");
        
        json.push_str(&format!("  \"total\": {},\n", summary.total));
        json.push_str(&format!("  \"passed\": {},\n", summary.passed));
        json.push_str(&format!("  \"failed\": {},\n", summary.failed));
        json.push_str(&format!("  \"timeout\": {},\n", summary.timeout));
        json.push_str(&format!("  \"skipped\": {},\n", summary.skipped));
        json.push_str(&format!("  \"pass_rate\": {:.2},\n", summary.pass_rate()));
        json.push_str(&format!("  \"duration_s\": {:.2},\n", summary.duration_s));
        
        json.push_str("  \"results\": [\n");
        for (i, result) in summary.results.iter().enumerate() {
            json.push_str(&format!(
                "    {{\"path\": \"{}\", \"status\": \"{:?}\", \"duration_ms\": {}}}",
                result.path,
                result.status,
                result.duration_ms
            ));
            if i < summary.results.len() - 1 {
                json.push_str(",\n");
            } else {
                json.push('\n');
            }
        }
        json.push_str("  ]\n");
        json.push_str("}");
        
        json
    }
}

/// Markdown result reporter
pub struct MarkdownReporter {
    include_failures: bool,
}

impl MarkdownReporter {
    pub fn new(include_failures: bool) -> Self {
        Self { include_failures }
    }
}

impl ResultReporter for MarkdownReporter {
    fn generate(&self, summary: &RunSummary) -> String {
        let mut md = String::from("# WPT Test Results\n\n");
        
        md.push_str("## Summary\n\n");
        md.push_str("| Metric | Value |\n");
        md.push_str("|--------|-------|\n");
        md.push_str(&format!("| Total | {} |\n", summary.total));
        md.push_str(&format!("| Passed | {} |\n", summary.passed));
        md.push_str(&format!("| Failed | {} |\n", summary.failed));
        md.push_str(&format!("| Timeout | {} |\n", summary.timeout));
        md.push_str(&format!("| Skipped | {} |\n", summary.skipped));
        md.push_str(&format!("| Pass Rate | {:.2}% |\n", summary.pass_rate()));
        md.push_str(&format!("| Duration | {:.2}s |\n\n", summary.duration_s));
        
        if self.include_failures {
            let failures: Vec<_> = summary.results.iter()
                .filter(|r| matches!(r.status, TestStatus::Error | TestStatus::Timeout))
                .collect();
            
            if !failures.is_empty() {
                md.push_str("## Failures\n\n");
                for result in failures {
                    md.push_str(&format!("### {}\n", result.path));
                    md.push_str(&format!("- Status: {:?}\n", result.status));
                    if let Some(ref msg) = result.message {
                        md.push_str(&format!("- Message: {}\n", msg));
                    }
                    md.push('\n');
                }
            }
        }
        
        md
    }
}

/// HTML result reporter
pub struct HtmlReporter;

impl HtmlReporter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HtmlReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl ResultReporter for HtmlReporter {
    fn generate(&self, summary: &RunSummary) -> String {
        let mut html = String::from(r#"<!DOCTYPE html>
<html>
<head>
    <title>WPT Test Results</title>
    <style>
        body { font-family: sans-serif; margin: 20px; }
        .summary { display: grid; grid-template-columns: repeat(3, 1fr); gap: 20px; margin-bottom: 20px; }
        .card { padding: 20px; border-radius: 8px; text-align: center; }
        .passed { background: #e8f5e9; color: #2e7d32; }
        .failed { background: #ffebee; color: #c62828; }
        .skipped { background: #fff3e0; color: #ef6c00; }
        .progress { height: 20px; background: #eee; border-radius: 10px; overflow: hidden; }
        .progress-bar { height: 100%; background: #4caf50; }
        table { width: 100%; border-collapse: collapse; margin-top: 20px; }
        th, td { padding: 10px; text-align: left; border-bottom: 1px solid #ddd; }
        th { background: #f5f5f5; }
        .status-ok { color: #2e7d32; }
        .status-error { color: #c62828; }
        .status-timeout { color: #ef6c00; }
        .status-skip { color: #757575; }
    </style>
</head>
<body>
    <h1>WPT Test Results</h1>
"#);

        // Summary cards
        html.push_str("<div class=\"summary\">\n");
        html.push_str(&format!(
            "<div class=\"card passed\"><h2>{}</h2><p>Passed</p></div>\n",
            summary.passed
        ));
        html.push_str(&format!(
            "<div class=\"card failed\"><h2>{}</h2><p>Failed</p></div>\n",
            summary.failed
        ));
        html.push_str(&format!(
            "<div class=\"card skipped\"><h2>{}</h2><p>Skipped</p></div>\n",
            summary.skipped
        ));
        html.push_str("</div>\n");

        // Progress bar
        html.push_str("<div class=\"progress\">\n");
        html.push_str(&format!(
            "<div class=\"progress-bar\" style=\"width: {}%\"></div>\n",
            summary.pass_rate()
        ));
        html.push_str("</div>\n");
        html.push_str(&format!("<p>Pass Rate: {:.2}%</p>\n", summary.pass_rate()));

        // Results table
        html.push_str("<table>\n");
        html.push_str("<thead><tr><th>Test</th><th>Status</th><th>Duration</th></tr></thead>\n");
        html.push_str("<tbody>\n");
        for result in &summary.results {
            let status_class = match result.status {
                TestStatus::Ok => "status-ok",
                TestStatus::Error => "status-error",
                TestStatus::Timeout => "status-timeout",
                TestStatus::Skip => "status-skip",
                TestStatus::Crash => "status-error",
            };
            html.push_str(&format!(
                "<tr><td>{}</td><td class=\"{}\">{:?}</td><td>{}ms</td></tr>\n",
                result.path,
                status_class,
                result.status,
                result.duration_ms
            ));
        }
        html.push_str("</tbody>\n");
        html.push_str("</table>\n");

        html.push_str("</body>\n</html>");
        html
    }
}

/// wptreport.json compatible format
#[derive(Debug, Clone)]
pub struct WptReportFormat {
    /// Run info
    pub run_info: RunInfo,
    /// Results
    pub results: Vec<WptResult>,
}

/// Run information
#[derive(Debug, Clone)]
pub struct RunInfo {
    /// Product name
    pub product: String,
    /// Browser version
    pub browser_version: String,
    /// OS name
    pub os: String,
    /// OS version
    pub os_version: String,
    /// Revision/commit
    pub revision: String,
}

impl Default for RunInfo {
    fn default() -> Self {
        Self {
            product: String::from("kpio"),
            browser_version: String::from("0.1.0"),
            os: String::from("kpio-os"),
            os_version: String::from("0.1.0"),
            revision: String::new(),
        }
    }
}

/// WPT result in wptreport format
#[derive(Debug, Clone)]
pub struct WptResult {
    /// Test path
    pub test: String,
    /// Test status
    pub status: String,
    /// Test message
    pub message: Option<String>,
    /// Duration in ms
    pub duration: u64,
    /// Subtests
    pub subtests: Vec<WptSubtest>,
}

/// WPT subtest in wptreport format
#[derive(Debug, Clone)]
pub struct WptSubtest {
    /// Subtest name
    pub name: String,
    /// Subtest status
    pub status: String,
    /// Subtest message
    pub message: Option<String>,
}

impl WptReportFormat {
    /// Create from run summary
    pub fn from_summary(summary: &RunSummary, run_info: RunInfo) -> Self {
        let results = summary.results.iter().map(|r| {
            WptResult {
                test: r.path.clone(),
                status: match r.status {
                    TestStatus::Ok => String::from("OK"),
                    TestStatus::Error => String::from("ERROR"),
                    TestStatus::Timeout => String::from("TIMEOUT"),
                    TestStatus::Crash => String::from("CRASH"),
                    TestStatus::Skip => String::from("SKIP"),
                },
                message: r.message.clone(),
                duration: r.duration_ms,
                subtests: r.subtests.iter().map(|s| {
                    WptSubtest {
                        name: s.name.clone(),
                        status: match s.status {
                            SubtestStatus::Pass => String::from("PASS"),
                            SubtestStatus::Fail => String::from("FAIL"),
                            SubtestStatus::Timeout => String::from("TIMEOUT"),
                            SubtestStatus::NotRun => String::from("NOTRUN"),
                            SubtestStatus::PreconditionFailed => String::from("PRECONDITION_FAILED"),
                        },
                        message: s.message.clone(),
                    }
                }).collect(),
            }
        }).collect();

        Self { run_info, results }
    }

    /// Generate JSON output
    pub fn to_json(&self) -> String {
        let mut json = String::from("{\n");
        
        // Run info
        json.push_str("  \"run_info\": {\n");
        json.push_str(&format!("    \"product\": \"{}\",\n", self.run_info.product));
        json.push_str(&format!("    \"browser_version\": \"{}\",\n", self.run_info.browser_version));
        json.push_str(&format!("    \"os\": \"{}\",\n", self.run_info.os));
        json.push_str(&format!("    \"os_version\": \"{}\",\n", self.run_info.os_version));
        json.push_str(&format!("    \"revision\": \"{}\"\n", self.run_info.revision));
        json.push_str("  },\n");
        
        // Results
        json.push_str("  \"results\": [\n");
        for (i, result) in self.results.iter().enumerate() {
            json.push_str("    {\n");
            json.push_str(&format!("      \"test\": \"{}\",\n", result.test));
            json.push_str(&format!("      \"status\": \"{}\",\n", result.status));
            json.push_str(&format!("      \"duration\": {},\n", result.duration));
            
            // Subtests
            json.push_str("      \"subtests\": [\n");
            for (j, subtest) in result.subtests.iter().enumerate() {
                json.push_str(&format!(
                    "        {{\"name\": \"{}\", \"status\": \"{}\"}}",
                    subtest.name, subtest.status
                ));
                if j < result.subtests.len() - 1 {
                    json.push_str(",\n");
                } else {
                    json.push('\n');
                }
            }
            json.push_str("      ]\n");
            
            json.push_str("    }");
            if i < self.results.len() - 1 {
                json.push_str(",\n");
            } else {
                json.push('\n');
            }
        }
        json.push_str("  ]\n");
        json.push_str("}");
        
        json
    }
}

/// Result comparison for regression detection
pub struct ResultComparator {
    baseline: RunSummary,
}

impl ResultComparator {
    /// Create with baseline
    pub fn new(baseline: RunSummary) -> Self {
        Self { baseline }
    }

    /// Compare with current results
    pub fn compare(&self, current: &RunSummary) -> ComparisonResult {
        let mut new_passes = Vec::new();
        let mut new_failures = Vec::new();
        let mut fixed = Vec::new();
        let mut regressions = Vec::new();

        // Build baseline lookup
        let mut baseline_map: hashbrown::HashMap<&str, &TestResult> = hashbrown::HashMap::new();
        for result in &self.baseline.results {
            baseline_map.insert(&result.path, result);
        }

        for current_result in &current.results {
            match baseline_map.get(current_result.path.as_str()) {
                Some(baseline_result) => {
                    match (&baseline_result.status, &current_result.status) {
                        (TestStatus::Error, TestStatus::Ok) => {
                            fixed.push(current_result.path.clone());
                        }
                        (TestStatus::Ok, TestStatus::Error) => {
                            regressions.push(current_result.path.clone());
                        }
                        _ => {}
                    }
                }
                None => {
                    // New test
                    if current_result.status == TestStatus::Ok {
                        new_passes.push(current_result.path.clone());
                    } else {
                        new_failures.push(current_result.path.clone());
                    }
                }
            }
        }

        ComparisonResult {
            baseline_pass_rate: self.baseline.pass_rate(),
            current_pass_rate: current.pass_rate(),
            new_passes,
            new_failures,
            fixed,
            regressions,
        }
    }
}

/// Comparison result
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    /// Baseline pass rate
    pub baseline_pass_rate: f32,
    /// Current pass rate
    pub current_pass_rate: f32,
    /// New passing tests
    pub new_passes: Vec<String>,
    /// New failing tests
    pub new_failures: Vec<String>,
    /// Tests that were fixed
    pub fixed: Vec<String>,
    /// Tests that regressed
    pub regressions: Vec<String>,
}

impl ComparisonResult {
    /// Check if there are regressions
    pub fn has_regressions(&self) -> bool {
        !self.regressions.is_empty()
    }

    /// Get pass rate delta
    pub fn pass_rate_delta(&self) -> f32 {
        self.current_pass_rate - self.baseline_pass_rate
    }

    /// Generate summary
    pub fn summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str(&format!("Pass rate: {:.2}% -> {:.2}% ({:+.2}%)\n",
            self.baseline_pass_rate, self.current_pass_rate, self.pass_rate_delta()));
        summary.push_str(&format!("Fixed: {}\n", self.fixed.len()));
        summary.push_str(&format!("Regressions: {}\n", self.regressions.len()));
        summary.push_str(&format!("New passes: {}\n", self.new_passes.len()));
        summary.push_str(&format!("New failures: {}\n", self.new_failures.len()));
        summary
    }
}
