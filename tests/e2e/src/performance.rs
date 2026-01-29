//! Performance measurement and benchmarking
//!
//! Provides performance testing utilities for regression detection.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// A performance measurement
#[derive(Debug, Clone)]
pub struct Measurement {
    /// Measurement name
    pub name: String,
    /// Duration in microseconds
    pub duration_us: u64,
    /// Start timestamp
    pub start_time: u64,
    /// End timestamp
    pub end_time: u64,
    /// Memory used at start (bytes)
    pub memory_start: u64,
    /// Memory used at end (bytes)
    pub memory_end: u64,
    /// Tags for categorization
    pub tags: Vec<String>,
}

impl Measurement {
    /// Create a new measurement
    pub fn new(name: &str, duration_us: u64) -> Self {
        Self {
            name: String::from(name),
            duration_us,
            start_time: 0,
            end_time: 0,
            memory_start: 0,
            memory_end: 0,
            tags: Vec::new(),
        }
    }

    /// Get duration in milliseconds
    pub fn duration_ms(&self) -> f64 {
        self.duration_us as f64 / 1000.0
    }

    /// Get duration in seconds
    pub fn duration_s(&self) -> f64 {
        self.duration_us as f64 / 1_000_000.0
    }

    /// Get memory delta in bytes
    pub fn memory_delta(&self) -> i64 {
        self.memory_end as i64 - self.memory_start as i64
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(String::from(tag));
        self
    }
}

/// Handle for an in-progress measurement
pub struct MeasurementHandle {
    name: String,
    start_time: u64,
    memory_start: u64,
}

impl MeasurementHandle {
    /// Create a new measurement handle
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            start_time: current_time_us(),
            memory_start: current_memory_usage(),
        }
    }

    /// Stop measurement and return result
    pub fn stop(self) -> Measurement {
        let end_time = current_time_us();
        let memory_end = current_memory_usage();
        
        Measurement {
            name: self.name,
            duration_us: end_time.saturating_sub(self.start_time),
            start_time: self.start_time,
            end_time,
            memory_start: self.memory_start,
            memory_end,
            tags: Vec::new(),
        }
    }
}

/// Get current time in microseconds
fn current_time_us() -> u64 {
    // In real implementation, would use kernel timer
    0
}

/// Get current memory usage
fn current_memory_usage() -> u64 {
    // In real implementation, would query memory allocator
    0
}

/// Performance report containing multiple measurements
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    /// Test name
    pub name: String,
    /// Individual measurements
    pub measurements: Vec<Measurement>,
    /// Summary statistics
    pub summary: ReportSummary,
}

impl PerformanceReport {
    /// Create a new performance report
    pub fn new(name: &str, measurements: Vec<Measurement>) -> Self {
        let summary = ReportSummary::from_measurements(&measurements);
        Self {
            name: String::from(name),
            measurements,
            summary,
        }
    }

    /// Add a measurement
    pub fn add_measurement(&mut self, measurement: Measurement) {
        self.measurements.push(measurement);
        self.summary = ReportSummary::from_measurements(&self.measurements);
    }

    /// Get measurements by tag
    pub fn by_tag(&self, tag: &str) -> Vec<&Measurement> {
        self.measurements
            .iter()
            .filter(|m| m.tags.iter().any(|t| t == tag))
            .collect()
    }
}

/// Summary statistics for a report
#[derive(Debug, Clone)]
pub struct ReportSummary {
    /// Total duration in microseconds
    pub total_duration_us: u64,
    /// Average duration in microseconds
    pub avg_duration_us: f64,
    /// Minimum duration in microseconds
    pub min_duration_us: u64,
    /// Maximum duration in microseconds
    pub max_duration_us: u64,
    /// Standard deviation
    pub std_dev: f64,
    /// P50 (median)
    pub p50: u64,
    /// P95
    pub p95: u64,
    /// P99
    pub p99: u64,
    /// Total memory delta
    pub total_memory_delta: i64,
    /// Measurement count
    pub count: usize,
}

impl ReportSummary {
    /// Create summary from measurements
    pub fn from_measurements(measurements: &[Measurement]) -> Self {
        if measurements.is_empty() {
            return Self {
                total_duration_us: 0,
                avg_duration_us: 0.0,
                min_duration_us: 0,
                max_duration_us: 0,
                std_dev: 0.0,
                p50: 0,
                p95: 0,
                p99: 0,
                total_memory_delta: 0,
                count: 0,
            };
        }

        let mut durations: Vec<u64> = measurements.iter().map(|m| m.duration_us).collect();
        durations.sort();

        let total: u64 = durations.iter().sum();
        let count = durations.len();
        let avg = total as f64 / count as f64;
        let min = durations[0];
        let max = durations[count - 1];

        // Calculate standard deviation
        let variance: f64 = durations.iter()
            .map(|d| {
                let diff = *d as f64 - avg;
                diff * diff
            })
            .sum::<f64>() / count as f64;
        let std_dev = libm::sqrt(variance);

        // Calculate percentiles
        let p50 = percentile(&durations, 50);
        let p95 = percentile(&durations, 95);
        let p99 = percentile(&durations, 99);

        let total_memory_delta: i64 = measurements.iter().map(|m| m.memory_delta()).sum();

        Self {
            total_duration_us: total,
            avg_duration_us: avg,
            min_duration_us: min,
            max_duration_us: max,
            std_dev,
            p50,
            p95,
            p99,
            total_memory_delta,
            count,
        }
    }
}

/// Calculate percentile from sorted values
fn percentile(sorted: &[u64], p: u32) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p as f64 / 100.0) * (sorted.len() - 1) as f64) as usize;
    sorted[idx.min(sorted.len() - 1)]
}

impl fmt::Display for ReportSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Performance Summary ({} measurements)", self.count)?;
        writeln!(f, "  Total:  {:.2}ms", self.total_duration_us as f64 / 1000.0)?;
        writeln!(f, "  Avg:    {:.2}ms", self.avg_duration_us / 1000.0)?;
        writeln!(f, "  Min:    {:.2}ms", self.min_duration_us as f64 / 1000.0)?;
        writeln!(f, "  Max:    {:.2}ms", self.max_duration_us as f64 / 1000.0)?;
        writeln!(f, "  StdDev: {:.2}ms", self.std_dev / 1000.0)?;
        writeln!(f, "  P50:    {:.2}ms", self.p50 as f64 / 1000.0)?;
        writeln!(f, "  P95:    {:.2}ms", self.p95 as f64 / 1000.0)?;
        writeln!(f, "  P99:    {:.2}ms", self.p99 as f64 / 1000.0)?;
        write!(f, "  Memory: {} bytes", self.total_memory_delta)
    }
}

/// Benchmark runner
pub struct Benchmark {
    /// Benchmark name
    pub name: String,
    /// Warmup iterations
    pub warmup: u32,
    /// Benchmark iterations
    pub iterations: u32,
    /// Measurements
    measurements: Vec<Measurement>,
}

impl Benchmark {
    /// Create a new benchmark
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            warmup: 3,
            iterations: 10,
            measurements: Vec::new(),
        }
    }

    /// Set warmup iterations
    pub fn warmup(mut self, count: u32) -> Self {
        self.warmup = count;
        self
    }

    /// Set benchmark iterations
    pub fn iterations(mut self, count: u32) -> Self {
        self.iterations = count;
        self
    }

    /// Run the benchmark
    pub fn run<F>(&mut self, mut f: F) -> PerformanceReport
    where
        F: FnMut(),
    {
        // Warmup
        for _ in 0..self.warmup {
            f();
        }

        // Actual measurements
        for i in 0..self.iterations {
            let handle = MeasurementHandle::new(&alloc::format!("{}_{}", self.name, i));
            f();
            self.measurements.push(handle.stop());
        }

        PerformanceReport::new(&self.name, self.measurements.clone())
    }

    /// Run benchmark with setup and teardown
    pub fn run_with_setup<S, T, F>(
        &mut self,
        mut setup: S,
        mut teardown: T,
        mut f: F,
    ) -> PerformanceReport
    where
        S: FnMut(),
        T: FnMut(),
        F: FnMut(),
    {
        // Warmup
        for _ in 0..self.warmup {
            setup();
            f();
            teardown();
        }

        // Actual measurements
        for i in 0..self.iterations {
            setup();
            let handle = MeasurementHandle::new(&alloc::format!("{}_{}", self.name, i));
            f();
            self.measurements.push(handle.stop());
            teardown();
        }

        PerformanceReport::new(&self.name, self.measurements.clone())
    }
}

/// Performance regression checker
pub struct RegressionChecker {
    /// Baseline reports
    baselines: Vec<PerformanceReport>,
    /// Threshold for regression (percentage increase)
    threshold_percent: f64,
}

impl RegressionChecker {
    /// Create a new regression checker
    pub fn new(threshold_percent: f64) -> Self {
        Self {
            baselines: Vec::new(),
            threshold_percent,
        }
    }

    /// Add a baseline report
    pub fn add_baseline(&mut self, report: PerformanceReport) {
        self.baselines.push(report);
    }

    /// Check for regression against baselines
    pub fn check(&self, current: &PerformanceReport) -> RegressionResult {
        let baseline = self.baselines.iter().find(|b| b.name == current.name);
        
        match baseline {
            Some(baseline) => {
                let baseline_avg = baseline.summary.avg_duration_us;
                let current_avg = current.summary.avg_duration_us;
                
                if baseline_avg == 0.0 {
                    return RegressionResult::NoBaseline;
                }
                
                let change_percent = ((current_avg - baseline_avg) / baseline_avg) * 100.0;
                
                if change_percent > self.threshold_percent {
                    RegressionResult::Regression {
                        baseline_avg_us: baseline_avg,
                        current_avg_us: current_avg,
                        change_percent,
                    }
                } else if change_percent < -self.threshold_percent {
                    RegressionResult::Improvement {
                        baseline_avg_us: baseline_avg,
                        current_avg_us: current_avg,
                        change_percent: -change_percent,
                    }
                } else {
                    RegressionResult::NoChange {
                        baseline_avg_us: baseline_avg,
                        current_avg_us: current_avg,
                    }
                }
            }
            None => RegressionResult::NoBaseline,
        }
    }
}

/// Regression check result
#[derive(Debug)]
pub enum RegressionResult {
    /// No baseline available
    NoBaseline,
    /// No significant change
    NoChange {
        baseline_avg_us: f64,
        current_avg_us: f64,
    },
    /// Performance regression detected
    Regression {
        baseline_avg_us: f64,
        current_avg_us: f64,
        change_percent: f64,
    },
    /// Performance improvement detected
    Improvement {
        baseline_avg_us: f64,
        current_avg_us: f64,
        change_percent: f64,
    },
}

impl RegressionResult {
    /// Check if this is a regression
    pub fn is_regression(&self) -> bool {
        matches!(self, RegressionResult::Regression { .. })
    }

    /// Check if this is an improvement
    pub fn is_improvement(&self) -> bool {
        matches!(self, RegressionResult::Improvement { .. })
    }
}

/// Page load performance metrics
#[derive(Debug, Clone)]
pub struct PageLoadMetrics {
    /// DNS lookup time
    pub dns_lookup_ms: f64,
    /// TCP connection time
    pub tcp_connect_ms: f64,
    /// TLS handshake time
    pub tls_handshake_ms: f64,
    /// Time to first byte
    pub time_to_first_byte_ms: f64,
    /// DOM content loaded
    pub dom_content_loaded_ms: f64,
    /// Load event fired
    pub load_event_ms: f64,
    /// First paint
    pub first_paint_ms: f64,
    /// First contentful paint
    pub first_contentful_paint_ms: f64,
    /// Largest contentful paint
    pub largest_contentful_paint_ms: f64,
    /// Time to interactive
    pub time_to_interactive_ms: f64,
    /// Total blocking time
    pub total_blocking_time_ms: f64,
    /// Cumulative layout shift
    pub cumulative_layout_shift: f64,
}

impl Default for PageLoadMetrics {
    fn default() -> Self {
        Self {
            dns_lookup_ms: 0.0,
            tcp_connect_ms: 0.0,
            tls_handshake_ms: 0.0,
            time_to_first_byte_ms: 0.0,
            dom_content_loaded_ms: 0.0,
            load_event_ms: 0.0,
            first_paint_ms: 0.0,
            first_contentful_paint_ms: 0.0,
            largest_contentful_paint_ms: 0.0,
            time_to_interactive_ms: 0.0,
            total_blocking_time_ms: 0.0,
            cumulative_layout_shift: 0.0,
        }
    }
}

impl PageLoadMetrics {
    /// Check if metrics meet performance budget
    pub fn meets_budget(&self, budget: &PerformanceBudget) -> bool {
        self.time_to_first_byte_ms <= budget.time_to_first_byte_ms
            && self.first_contentful_paint_ms <= budget.first_contentful_paint_ms
            && self.largest_contentful_paint_ms <= budget.largest_contentful_paint_ms
            && self.time_to_interactive_ms <= budget.time_to_interactive_ms
            && self.total_blocking_time_ms <= budget.total_blocking_time_ms
            && self.cumulative_layout_shift <= budget.cumulative_layout_shift
    }
}

/// Performance budget thresholds
#[derive(Debug, Clone)]
pub struct PerformanceBudget {
    pub time_to_first_byte_ms: f64,
    pub first_contentful_paint_ms: f64,
    pub largest_contentful_paint_ms: f64,
    pub time_to_interactive_ms: f64,
    pub total_blocking_time_ms: f64,
    pub cumulative_layout_shift: f64,
}

impl Default for PerformanceBudget {
    fn default() -> Self {
        // Default to "good" thresholds based on Web Vitals
        Self {
            time_to_first_byte_ms: 800.0,
            first_contentful_paint_ms: 1800.0,
            largest_contentful_paint_ms: 2500.0,
            time_to_interactive_ms: 3800.0,
            total_blocking_time_ms: 200.0,
            cumulative_layout_shift: 0.1,
        }
    }
}
