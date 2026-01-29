//! User Consent Management
//!
//! GDPR/CCPA compliant consent management.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use super::EventCategory;

/// Consent status
#[derive(Debug, Clone, Copy, Default)]
pub struct ConsentStatus {
    /// Telemetry consent
    pub telemetry: bool,
    /// Crash report consent
    pub crash_reports: bool,
    /// Usage statistics consent
    pub usage_statistics: bool,
    /// Personalization consent
    pub personalization: bool,
}

impl ConsentStatus {
    /// Create with all consent
    pub fn all_allowed() -> Self {
        Self {
            telemetry: true,
            crash_reports: true,
            usage_statistics: true,
            personalization: true,
        }
    }

    /// Create with no consent
    pub fn none_allowed() -> Self {
        Self::default()
    }

    /// Check if telemetry is allowed
    pub fn telemetry_allowed(&self) -> bool {
        self.telemetry || self.usage_statistics
    }

    /// Check if category is allowed
    pub fn allows_category(&self, category: EventCategory) -> bool {
        match category {
            EventCategory::Crash => self.crash_reports,
            EventCategory::Performance => self.usage_statistics,
            EventCategory::Navigation => self.usage_statistics,
            EventCategory::Feature => self.usage_statistics,
            EventCategory::Settings => self.telemetry,
            EventCategory::Extension => self.telemetry,
            EventCategory::Network => self.telemetry,
            EventCategory::Security => self.crash_reports,
        }
    }
}

/// Consent request
#[derive(Debug, Clone)]
pub struct ConsentRequest {
    /// Request ID
    pub id: String,
    /// Description
    pub description: String,
    /// Category
    pub category: ConsentCategory,
    /// Required
    pub required: bool,
    /// Default value
    pub default: bool,
}

/// Consent category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentCategory {
    /// Essential (always enabled)
    Essential,
    /// Analytics
    Analytics,
    /// Marketing
    Marketing,
    /// Personalization
    Personalization,
}

/// Consent record
#[derive(Debug, Clone)]
pub struct ConsentRecord {
    /// Timestamp
    pub timestamp: u64,
    /// Version of consent form
    pub version: String,
    /// Granted consents
    pub granted: Vec<String>,
    /// Denied consents
    pub denied: Vec<String>,
    /// Source
    pub source: ConsentSource,
}

/// Consent source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentSource {
    /// User prompt
    UserPrompt,
    /// Settings page
    Settings,
    /// API call
    Api,
    /// Default
    Default,
}

/// Consent manager
pub struct ConsentManager {
    /// Current status
    status: ConsentStatus,
    /// Consent history
    history: Vec<ConsentRecord>,
    /// Consent version
    version: String,
    /// Last updated
    last_updated: u64,
    /// Is GDPR applicable
    gdpr_applicable: bool,
    /// Is CCPA applicable
    ccpa_applicable: bool,
}

impl ConsentManager {
    /// Create new consent manager
    pub fn new() -> Self {
        Self {
            status: ConsentStatus::default(),
            history: Vec::new(),
            version: "1.0".to_string(),
            last_updated: 0,
            gdpr_applicable: false,
            ccpa_applicable: false,
        }
    }

    /// Get current status
    pub fn status(&self) -> &ConsentStatus {
        &self.status
    }

    /// Update consent
    pub fn update(&mut self, new_status: ConsentStatus, source: ConsentSource) {
        let record = ConsentRecord {
            timestamp: 0, // Would use actual time
            version: self.version.clone(),
            granted: self.collect_granted(&new_status),
            denied: self.collect_denied(&new_status),
            source,
        };

        self.history.push(record);
        self.status = new_status;
        self.last_updated = 0; // Would use actual time
    }

    /// Collect granted consents
    fn collect_granted(&self, status: &ConsentStatus) -> Vec<String> {
        let mut granted = Vec::new();
        if status.telemetry { granted.push("telemetry".to_string()); }
        if status.crash_reports { granted.push("crash_reports".to_string()); }
        if status.usage_statistics { granted.push("usage_statistics".to_string()); }
        if status.personalization { granted.push("personalization".to_string()); }
        granted
    }

    /// Collect denied consents
    fn collect_denied(&self, status: &ConsentStatus) -> Vec<String> {
        let mut denied = Vec::new();
        if !status.telemetry { denied.push("telemetry".to_string()); }
        if !status.crash_reports { denied.push("crash_reports".to_string()); }
        if !status.usage_statistics { denied.push("usage_statistics".to_string()); }
        if !status.personalization { denied.push("personalization".to_string()); }
        denied
    }

    /// Revoke all consent
    pub fn revoke_all(&mut self) {
        self.update(ConsentStatus::none_allowed(), ConsentSource::UserPrompt);
    }

    /// Grant all consent
    pub fn grant_all(&mut self) {
        self.update(ConsentStatus::all_allowed(), ConsentSource::UserPrompt);
    }

    /// Check if needs consent prompt
    pub fn needs_prompt(&self) -> bool {
        self.history.is_empty()
    }

    /// Check if consent needs renewal
    pub fn needs_renewal(&self) -> bool {
        // Would check if version changed or time expired
        false
    }

    /// Get consent history
    pub fn history(&self) -> &[ConsentRecord] {
        &self.history
    }

    /// Set jurisdiction
    pub fn set_jurisdiction(&mut self, gdpr: bool, ccpa: bool) {
        self.gdpr_applicable = gdpr;
        self.ccpa_applicable = ccpa;
    }

    /// Export consent data (for data portability)
    pub fn export(&self) -> ConsentExport {
        ConsentExport {
            status: self.status,
            history: self.history.clone(),
            version: self.version.clone(),
            exported_at: 0,
        }
    }
}

impl Default for ConsentManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Consent export data
#[derive(Debug, Clone)]
pub struct ConsentExport {
    /// Current status
    pub status: ConsentStatus,
    /// History
    pub history: Vec<ConsentRecord>,
    /// Version
    pub version: String,
    /// Export timestamp
    pub exported_at: u64,
}

/// Privacy policy version
#[derive(Debug, Clone)]
pub struct PrivacyPolicyVersion {
    /// Version
    pub version: String,
    /// Effective date
    pub effective_date: String,
    /// Changes summary
    pub changes: Vec<String>,
    /// Full text URL
    pub url: String,
}

impl PrivacyPolicyVersion {
    /// Check if version is current
    pub fn is_current(&self, current_version: &str) -> bool {
        self.version == current_version
    }
}
