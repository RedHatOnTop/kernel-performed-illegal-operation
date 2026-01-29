//! Telemetry Module
//!
//! Privacy-respecting metrics collection for KPIO Browser.

pub mod metrics;
pub mod reporter;
pub mod consent;

pub use metrics::*;
pub use reporter::*;
pub use consent::*;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

/// Telemetry error
#[derive(Debug, Clone)]
pub enum TelemetryError {
    /// Telemetry disabled
    Disabled,
    /// No consent
    NoConsent,
    /// Rate limited
    RateLimited,
    /// Send failed
    SendFailed(String),
}

/// Telemetry level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TelemetryLevel {
    /// No telemetry
    Off,
    /// Critical errors only
    Critical,
    /// Basic usage metrics
    Basic,
    /// Full telemetry
    Full,
}

impl Default for TelemetryLevel {
    fn default() -> Self {
        Self::Basic
    }
}

/// Event category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventCategory {
    /// Page navigation
    Navigation,
    /// Performance
    Performance,
    /// Crashes
    Crash,
    /// Feature usage
    Feature,
    /// Settings changes
    Settings,
    /// Extension events
    Extension,
    /// Network events
    Network,
    /// Security events
    Security,
}

/// Telemetry event
#[derive(Debug, Clone)]
pub struct TelemetryEvent {
    /// Event ID
    pub id: u64,
    /// Category
    pub category: EventCategory,
    /// Event name
    pub name: String,
    /// Timestamp
    pub timestamp: u64,
    /// Properties
    pub properties: BTreeMap<String, PropertyValue>,
    /// Session ID
    pub session_id: String,
    /// Required level
    pub required_level: TelemetryLevel,
}

/// Property value
#[derive(Debug, Clone)]
pub enum PropertyValue {
    /// String
    String(String),
    /// Integer
    Int(i64),
    /// Float
    Float(f64),
    /// Boolean
    Bool(bool),
    /// List
    List(Vec<PropertyValue>),
}

impl TelemetryEvent {
    /// Create new event
    pub fn new(category: EventCategory, name: impl Into<String>) -> Self {
        Self {
            id: 0,
            category,
            name: name.into(),
            timestamp: 0,
            properties: BTreeMap::new(),
            session_id: String::new(),
            required_level: TelemetryLevel::Basic,
        }
    }

    /// Add string property
    pub fn with_string(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), PropertyValue::String(value.into()));
        self
    }

    /// Add integer property
    pub fn with_int(mut self, key: impl Into<String>, value: i64) -> Self {
        self.properties.insert(key.into(), PropertyValue::Int(value));
        self
    }

    /// Add float property
    pub fn with_float(mut self, key: impl Into<String>, value: f64) -> Self {
        self.properties.insert(key.into(), PropertyValue::Float(value));
        self
    }

    /// Add bool property
    pub fn with_bool(mut self, key: impl Into<String>, value: bool) -> Self {
        self.properties.insert(key.into(), PropertyValue::Bool(value));
        self
    }

    /// Set required level
    pub fn with_level(mut self, level: TelemetryLevel) -> Self {
        self.required_level = level;
        self
    }
}

/// Telemetry manager
pub struct TelemetryManager {
    /// Enabled
    enabled: bool,
    /// Level
    level: TelemetryLevel,
    /// Session ID
    session_id: String,
    /// Event queue
    event_queue: Vec<TelemetryEvent>,
    /// Next event ID
    next_id: u64,
    /// Consent given
    consent: ConsentStatus,
    /// Metrics collector
    metrics: MetricsCollector,
    /// Reporter
    reporter: TelemetryReporter,
}

impl TelemetryManager {
    /// Create new telemetry manager
    pub fn new() -> Self {
        Self {
            enabled: true,
            level: TelemetryLevel::Basic,
            session_id: Self::generate_session_id(),
            event_queue: Vec::new(),
            next_id: 0,
            consent: ConsentStatus::default(),
            metrics: MetricsCollector::new(),
            reporter: TelemetryReporter::new(),
        }
    }

    /// Generate session ID
    fn generate_session_id() -> String {
        // Would use proper random ID
        "session_placeholder".to_string()
    }

    /// Is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled && self.consent.telemetry_allowed()
    }

    /// Set level
    pub fn set_level(&mut self, level: TelemetryLevel) {
        self.level = level;
    }

    /// Get level
    pub fn level(&self) -> TelemetryLevel {
        self.level
    }

    /// Record event
    pub fn record(&mut self, mut event: TelemetryEvent) -> Result<u64, TelemetryError> {
        if !self.is_enabled() {
            return Err(TelemetryError::Disabled);
        }

        if event.required_level > self.level {
            return Err(TelemetryError::Disabled);
        }

        event.id = self.next_id;
        self.next_id += 1;
        event.session_id = self.session_id.clone();
        
        let id = event.id;
        self.event_queue.push(event);

        // Auto-flush if queue is large
        if self.event_queue.len() > 100 {
            let _ = self.flush();
        }

        Ok(id)
    }

    /// Record navigation
    pub fn record_navigation(&mut self, url_hash: &str, load_time_ms: u64) -> Result<u64, TelemetryError> {
        let event = TelemetryEvent::new(EventCategory::Navigation, "page_load")
            .with_string("url_hash", url_hash)
            .with_int("load_time_ms", load_time_ms as i64);
        
        self.record(event)
    }

    /// Record performance metric
    pub fn record_performance(&mut self, metric: &str, value: f64) -> Result<u64, TelemetryError> {
        let event = TelemetryEvent::new(EventCategory::Performance, metric)
            .with_float("value", value);
        
        self.record(event)
    }

    /// Record crash
    pub fn record_crash(&mut self, crash_type: &str, message: &str) -> Result<u64, TelemetryError> {
        let event = TelemetryEvent::new(EventCategory::Crash, "crash")
            .with_string("type", crash_type)
            .with_string("message", message)
            .with_level(TelemetryLevel::Critical);
        
        self.record(event)
    }

    /// Record feature usage
    pub fn record_feature(&mut self, feature: &str) -> Result<u64, TelemetryError> {
        let event = TelemetryEvent::new(EventCategory::Feature, "feature_used")
            .with_string("feature", feature)
            .with_level(TelemetryLevel::Full);
        
        self.record(event)
    }

    /// Flush events
    pub fn flush(&mut self) -> Result<usize, TelemetryError> {
        if self.event_queue.is_empty() {
            return Ok(0);
        }

        let events = core::mem::take(&mut self.event_queue);
        let count = events.len();

        // Filter by consent
        let filtered: Vec<_> = events.into_iter()
            .filter(|e| self.consent.allows_category(e.category))
            .collect();

        if filtered.is_empty() {
            return Ok(0);
        }

        self.reporter.send_batch(&filtered)
            .map_err(|e| TelemetryError::SendFailed(e.to_string()))?;

        Ok(count)
    }

    /// Get metrics collector
    pub fn metrics(&mut self) -> &mut MetricsCollector {
        &mut self.metrics
    }

    /// Update consent
    pub fn update_consent(&mut self, consent: ConsentStatus) {
        self.consent = consent;
    }

    /// Clear all data
    pub fn clear_data(&mut self) {
        self.event_queue.clear();
        self.session_id = Self::generate_session_id();
        self.metrics.reset();
    }
}

impl Default for TelemetryManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global telemetry manager
pub static TELEMETRY: RwLock<TelemetryManager> = RwLock::new(TelemetryManager {
    enabled: true,
    level: TelemetryLevel::Basic,
    session_id: String::new(),
    event_queue: Vec::new(),
    next_id: 0,
    consent: ConsentStatus {
        telemetry: false,
        crash_reports: true,
        usage_statistics: false,
        personalization: false,
    },
    metrics: MetricsCollector {
        counters: BTreeMap::new(),
        gauges: BTreeMap::new(),
        histograms: BTreeMap::new(),
    },
    reporter: TelemetryReporter {
        endpoint: String::new(),
        batch_size: 50,
        retry_count: 3,
        compression: true,
    },
});
