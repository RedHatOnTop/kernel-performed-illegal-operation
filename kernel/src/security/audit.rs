//! Security Audit Logging
//!
//! This module provides audit logging for security-relevant events.

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::{Mutex, RwLock};

use super::policy::DomainId;
use super::sandbox::SandboxId;
use crate::browser::coordinator::TabId;
use crate::process::ProcessId;

/// Audit event severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AuditSeverity {
    /// Debug information.
    Debug = 0,
    /// Informational.
    Info = 1,
    /// Notice (normal but significant).
    Notice = 2,
    /// Warning (potential issue).
    Warning = 3,
    /// Error (failure).
    Error = 4,
    /// Critical (serious failure).
    Critical = 5,
    /// Alert (action required).
    Alert = 6,
    /// Emergency (system unusable).
    Emergency = 7,
}

/// Audit event category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditCategory {
    /// Authentication events.
    Auth,
    /// Authorization/access control.
    Access,
    /// Process events.
    Process,
    /// Network events.
    Network,
    /// File system events.
    FileSystem,
    /// IPC events.
    Ipc,
    /// Sandbox events.
    Sandbox,
    /// Resource events.
    Resource,
    /// Policy events.
    Policy,
    /// System events.
    System,
}

/// Audit event outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditOutcome {
    /// Operation succeeded.
    Success,
    /// Operation failed.
    Failure,
    /// Operation denied.
    Denied,
    /// Unknown outcome.
    Unknown,
}

/// Audit event.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// Event ID.
    pub id: u64,
    /// Timestamp (ticks since boot).
    pub timestamp: u64,
    /// Severity.
    pub severity: AuditSeverity,
    /// Category.
    pub category: AuditCategory,
    /// Outcome.
    pub outcome: AuditOutcome,
    /// Source process.
    pub pid: Option<ProcessId>,
    /// Source tab.
    pub tab: Option<TabId>,
    /// Domain.
    pub domain: Option<DomainId>,
    /// Sandbox.
    pub sandbox: Option<SandboxId>,
    /// Event message.
    pub message: String,
    /// Additional details.
    pub details: Vec<(String, String)>,
}

impl AuditEvent {
    /// Create a new audit event.
    pub fn new(severity: AuditSeverity, category: AuditCategory, message: &str) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);

        AuditEvent {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            timestamp: 0, // TODO: Get actual timestamp
            severity,
            category,
            outcome: AuditOutcome::Unknown,
            pid: None,
            tab: None,
            domain: None,
            sandbox: None,
            message: String::from(message),
            details: Vec::new(),
        }
    }

    /// Set outcome.
    pub fn with_outcome(mut self, outcome: AuditOutcome) -> Self {
        self.outcome = outcome;
        self
    }

    /// Set process.
    pub fn with_pid(mut self, pid: ProcessId) -> Self {
        self.pid = Some(pid);
        self
    }

    /// Set tab.
    pub fn with_tab(mut self, tab: TabId) -> Self {
        self.tab = Some(tab);
        self
    }

    /// Set domain.
    pub fn with_domain(mut self, domain: DomainId) -> Self {
        self.domain = Some(domain);
        self
    }

    /// Set sandbox.
    pub fn with_sandbox(mut self, sandbox: SandboxId) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    /// Add detail.
    pub fn with_detail(mut self, key: &str, value: &str) -> Self {
        self.details.push((String::from(key), String::from(value)));
        self
    }
}

/// Audit log storage.
pub struct AuditLog {
    /// Event buffer (ring buffer).
    events: VecDeque<AuditEvent>,
    /// Maximum events to store.
    max_events: usize,
    /// Total events logged.
    total_events: u64,
    /// Events dropped due to overflow.
    dropped_events: u64,
    /// Minimum severity to log.
    min_severity: AuditSeverity,
    /// Categories to log.
    enabled_categories: Vec<AuditCategory>,
}

impl AuditLog {
    /// Create new audit log.
    pub fn new(max_events: usize) -> Self {
        AuditLog {
            events: VecDeque::with_capacity(max_events),
            max_events,
            total_events: 0,
            dropped_events: 0,
            min_severity: AuditSeverity::Notice,
            enabled_categories: alloc::vec![
                AuditCategory::Auth,
                AuditCategory::Access,
                AuditCategory::Process,
                AuditCategory::Network,
                AuditCategory::Sandbox,
                AuditCategory::Policy,
            ],
        }
    }

    /// Log an event.
    pub fn log(&mut self, event: AuditEvent) {
        // Check severity
        if event.severity < self.min_severity {
            return;
        }

        // Check category
        if !self.enabled_categories.contains(&event.category) {
            return;
        }

        // Print to serial for now
        self.print_event(&event);

        // Store event
        if self.events.len() >= self.max_events {
            self.events.pop_front();
            self.dropped_events += 1;
        }

        self.events.push_back(event);
        self.total_events += 1;
    }

    /// Print event to serial.
    fn print_event(&self, event: &AuditEvent) {
        let severity_str = match event.severity {
            AuditSeverity::Debug => "DEBUG",
            AuditSeverity::Info => "INFO",
            AuditSeverity::Notice => "NOTICE",
            AuditSeverity::Warning => "WARN",
            AuditSeverity::Error => "ERROR",
            AuditSeverity::Critical => "CRIT",
            AuditSeverity::Alert => "ALERT",
            AuditSeverity::Emergency => "EMERG",
        };

        let outcome_str = match event.outcome {
            AuditOutcome::Success => "OK",
            AuditOutcome::Failure => "FAIL",
            AuditOutcome::Denied => "DENY",
            AuditOutcome::Unknown => "?",
        };

        crate::serial_println!(
            "[AUDIT][{}][{:?}][{}] {}",
            severity_str,
            event.category,
            outcome_str,
            event.message
        );
    }

    /// Get recent events.
    pub fn recent(&self, count: usize) -> impl Iterator<Item = &AuditEvent> {
        self.events.iter().rev().take(count)
    }

    /// Search events.
    pub fn search<F>(&self, predicate: F) -> Vec<&AuditEvent>
    where
        F: Fn(&AuditEvent) -> bool,
    {
        self.events.iter().filter(|e| predicate(e)).collect()
    }

    /// Get events by severity.
    pub fn by_severity(&self, severity: AuditSeverity) -> Vec<&AuditEvent> {
        self.search(|e| e.severity >= severity)
    }

    /// Get events by category.
    pub fn by_category(&self, category: AuditCategory) -> Vec<&AuditEvent> {
        self.search(|e| e.category == category)
    }

    /// Get events for process.
    pub fn for_process(&self, pid: ProcessId) -> Vec<&AuditEvent> {
        self.search(|e| e.pid == Some(pid))
    }

    /// Get events for tab.
    pub fn for_tab(&self, tab: TabId) -> Vec<&AuditEvent> {
        self.search(|e| e.tab == Some(tab))
    }

    /// Get denied events.
    pub fn denied(&self) -> Vec<&AuditEvent> {
        self.search(|e| e.outcome == AuditOutcome::Denied)
    }

    /// Set minimum severity.
    pub fn set_min_severity(&mut self, severity: AuditSeverity) {
        self.min_severity = severity;
    }

    /// Enable category.
    pub fn enable_category(&mut self, category: AuditCategory) {
        if !self.enabled_categories.contains(&category) {
            self.enabled_categories.push(category);
        }
    }

    /// Disable category.
    pub fn disable_category(&mut self, category: AuditCategory) {
        self.enabled_categories.retain(|c| *c != category);
    }

    /// Get statistics.
    pub fn stats(&self) -> AuditStats {
        AuditStats {
            total_events: self.total_events,
            stored_events: self.events.len() as u64,
            dropped_events: self.dropped_events,
            denied_events: self
                .events
                .iter()
                .filter(|e| e.outcome == AuditOutcome::Denied)
                .count() as u64,
        }
    }

    /// Clear all events.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

/// Audit statistics.
#[derive(Debug, Clone, Copy)]
pub struct AuditStats {
    /// Total events logged.
    pub total_events: u64,
    /// Currently stored events.
    pub stored_events: u64,
    /// Events dropped.
    pub dropped_events: u64,
    /// Denied events count.
    pub denied_events: u64,
}

/// Global audit log.
static AUDIT_LOG: RwLock<Option<AuditLog>> = RwLock::new(None);

/// Initialize audit log.
pub fn init() {
    let mut log = AUDIT_LOG.write();
    *log = Some(AuditLog::new(1024));
    crate::serial_println!("[Audit] Log initialized");
}

/// Log an event.
pub fn log(event: AuditEvent) {
    if let Some(log) = AUDIT_LOG.write().as_mut() {
        log.log(event);
    }
}

/// Log access denied.
pub fn log_denied(
    category: AuditCategory,
    message: &str,
    pid: Option<ProcessId>,
    tab: Option<TabId>,
) {
    let mut event = AuditEvent::new(AuditSeverity::Warning, category, message)
        .with_outcome(AuditOutcome::Denied);

    if let Some(p) = pid {
        event = event.with_pid(p);
    }
    if let Some(t) = tab {
        event = event.with_tab(t);
    }

    log(event);
}

/// Log success.
pub fn log_success(category: AuditCategory, message: &str) {
    let event =
        AuditEvent::new(AuditSeverity::Info, category, message).with_outcome(AuditOutcome::Success);
    log(event);
}

/// Get audit stats.
pub fn stats() -> Option<AuditStats> {
    Some(AUDIT_LOG.read().as_ref()?.stats())
}
