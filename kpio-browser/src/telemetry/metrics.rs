//! Metrics Collection
//!
//! Aggregated metrics without personal data.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// Counter metric
#[derive(Debug, Clone, Default)]
pub struct Counter {
    /// Value
    value: u64,
    /// Labels
    labels: BTreeMap<String, String>,
}

impl Counter {
    /// Create new counter
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment
    pub fn increment(&mut self) {
        self.value = self.value.saturating_add(1);
    }

    /// Add value
    pub fn add(&mut self, n: u64) {
        self.value = self.value.saturating_add(n);
    }

    /// Get value
    pub fn value(&self) -> u64 {
        self.value
    }

    /// Reset
    pub fn reset(&mut self) {
        self.value = 0;
    }

    /// Add label
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }
}

/// Gauge metric
#[derive(Debug, Clone, Default)]
pub struct Gauge {
    /// Value
    value: f64,
    /// Labels
    labels: BTreeMap<String, String>,
}

impl Gauge {
    /// Create new gauge
    pub fn new() -> Self {
        Self::default()
    }

    /// Set value
    pub fn set(&mut self, value: f64) {
        self.value = value;
    }

    /// Get value
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Increment
    pub fn increment(&mut self, delta: f64) {
        self.value += delta;
    }

    /// Decrement
    pub fn decrement(&mut self, delta: f64) {
        self.value -= delta;
    }

    /// Add label
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }
}

/// Histogram metric
#[derive(Debug, Clone)]
pub struct Histogram {
    /// Buckets
    buckets: Vec<(f64, u64)>,
    /// Sum of all values
    sum: f64,
    /// Count of observations
    count: u64,
    /// Labels
    labels: BTreeMap<String, String>,
}

impl Histogram {
    /// Create new histogram with default buckets
    pub fn new() -> Self {
        Self::with_buckets(&[
            0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ])
    }

    /// Create with custom buckets
    pub fn with_buckets(boundaries: &[f64]) -> Self {
        let buckets = boundaries.iter()
            .map(|&b| (b, 0u64))
            .collect();

        Self {
            buckets,
            sum: 0.0,
            count: 0,
            labels: BTreeMap::new(),
        }
    }

    /// Observe a value
    pub fn observe(&mut self, value: f64) {
        self.sum += value;
        self.count += 1;

        for (boundary, count) in &mut self.buckets {
            if value <= *boundary {
                *count += 1;
            }
        }
    }

    /// Get sum
    pub fn sum(&self) -> f64 {
        self.sum
    }

    /// Get count
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Get mean
    pub fn mean(&self) -> f64 {
        if self.count > 0 {
            self.sum / self.count as f64
        } else {
            0.0
        }
    }

    /// Get buckets
    pub fn buckets(&self) -> &[(f64, u64)] {
        &self.buckets
    }

    /// Reset
    pub fn reset(&mut self) {
        self.sum = 0.0;
        self.count = 0;
        for (_, count) in &mut self.buckets {
            *count = 0;
        }
    }

    /// Add label
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics collector
pub struct MetricsCollector {
    /// Counters
    pub counters: BTreeMap<String, Counter>,
    /// Gauges
    pub gauges: BTreeMap<String, Gauge>,
    /// Histograms
    pub histograms: BTreeMap<String, Histogram>,
}

impl MetricsCollector {
    /// Create new collector
    pub const fn new() -> Self {
        Self {
            counters: BTreeMap::new(),
            gauges: BTreeMap::new(),
            histograms: BTreeMap::new(),
        }
    }

    /// Get or create counter
    pub fn counter(&mut self, name: &str) -> &mut Counter {
        if !self.counters.contains_key(name) {
            self.counters.insert(name.into(), Counter::new());
        }
        self.counters.get_mut(name).unwrap()
    }

    /// Get or create gauge
    pub fn gauge(&mut self, name: &str) -> &mut Gauge {
        if !self.gauges.contains_key(name) {
            self.gauges.insert(name.into(), Gauge::new());
        }
        self.gauges.get_mut(name).unwrap()
    }

    /// Get or create histogram
    pub fn histogram(&mut self, name: &str) -> &mut Histogram {
        if !self.histograms.contains_key(name) {
            self.histograms.insert(name.into(), Histogram::new());
        }
        self.histograms.get_mut(name).unwrap()
    }

    /// Increment counter
    pub fn increment(&mut self, name: &str) {
        self.counter(name).increment();
    }

    /// Add to counter
    pub fn add(&mut self, name: &str, n: u64) {
        self.counter(name).add(n);
    }

    /// Set gauge
    pub fn set_gauge(&mut self, name: &str, value: f64) {
        self.gauge(name).set(value);
    }

    /// Observe histogram
    pub fn observe(&mut self, name: &str, value: f64) {
        self.histogram(name).observe(value);
    }

    /// Reset all metrics
    pub fn reset(&mut self) {
        self.counters.clear();
        self.gauges.clear();
        self.histograms.clear();
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard browser metrics
pub struct BrowserMetrics;

impl BrowserMetrics {
    // Counter names
    pub const PAGE_LOADS: &'static str = "browser.page_loads";
    pub const JS_ERRORS: &'static str = "browser.js_errors";
    pub const CACHE_HITS: &'static str = "browser.cache_hits";
    pub const CACHE_MISSES: &'static str = "browser.cache_misses";
    pub const TABS_OPENED: &'static str = "browser.tabs_opened";
    pub const TABS_CLOSED: &'static str = "browser.tabs_closed";

    // Gauge names
    pub const MEMORY_USAGE: &'static str = "browser.memory_usage_mb";
    pub const ACTIVE_TABS: &'static str = "browser.active_tabs";
    pub const CPU_USAGE: &'static str = "browser.cpu_usage_percent";

    // Histogram names
    pub const PAGE_LOAD_TIME: &'static str = "browser.page_load_time_seconds";
    pub const FIRST_PAINT: &'static str = "browser.first_paint_seconds";
    pub const DNS_LOOKUP: &'static str = "browser.dns_lookup_seconds";
    pub const TLS_HANDSHAKE: &'static str = "browser.tls_handshake_seconds";
}
