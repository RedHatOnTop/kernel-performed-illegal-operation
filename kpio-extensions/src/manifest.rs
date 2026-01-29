//! Extension Manifest Parser
//!
//! Parses Chrome extension Manifest V3 format.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

/// Manifest version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestVersion {
    V2 = 2,
    V3 = 3,
}

impl Default for ManifestVersion {
    fn default() -> Self {
        Self::V3
    }
}

/// Extension manifest (Manifest V3).
#[derive(Debug, Clone, Default)]
pub struct Manifest {
    /// Manifest version.
    pub manifest_version: ManifestVersion,
    /// Extension name.
    pub name: String,
    /// Extension version.
    pub version: String,
    /// Description.
    pub description: Option<String>,
    /// Author.
    pub author: Option<String>,
    /// Homepage URL.
    pub homepage_url: Option<String>,
    /// Update URL.
    pub update_url: Option<String>,
    /// Icons.
    pub icons: BTreeMap<String, String>,
    /// Permissions.
    pub permissions: Vec<String>,
    /// Optional permissions.
    pub optional_permissions: Vec<String>,
    /// Host permissions.
    pub host_permissions: Vec<String>,
    /// Background configuration.
    pub background: Option<BackgroundConfig>,
    /// Content scripts.
    pub content_scripts: Vec<ContentScript>,
    /// Action (toolbar button).
    pub action: Option<Action>,
    /// Options page.
    pub options_page: Option<String>,
    /// Options UI.
    pub options_ui: Option<OptionsUi>,
    /// Web accessible resources.
    pub web_accessible_resources: Vec<WebAccessibleResource>,
    /// Content security policy.
    pub content_security_policy: Option<ContentSecurityPolicy>,
    /// Commands (keyboard shortcuts).
    pub commands: BTreeMap<String, Command>,
    /// Declarative net request.
    pub declarative_net_request: Option<DeclarativeNetRequest>,
    /// Externally connectable.
    pub externally_connectable: Option<ExternallyConnectable>,
    /// Minimum Chrome version.
    pub minimum_chrome_version: Option<String>,
    /// Default locale.
    pub default_locale: Option<String>,
}

impl Manifest {
    /// Create a new manifest.
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            manifest_version: ManifestVersion::V3,
            name: name.to_string(),
            version: version.to_string(),
            ..Default::default()
        }
    }
    
    /// Add a permission.
    pub fn add_permission(&mut self, permission: &str) {
        if !self.permissions.contains(&permission.to_string()) {
            self.permissions.push(permission.to_string());
        }
    }
    
    /// Add a host permission.
    pub fn add_host_permission(&mut self, pattern: &str) {
        if !self.host_permissions.contains(&pattern.to_string()) {
            self.host_permissions.push(pattern.to_string());
        }
    }
    
    /// Add a content script.
    pub fn add_content_script(&mut self, script: ContentScript) {
        self.content_scripts.push(script);
    }
    
    /// Set background service worker.
    pub fn set_background_service_worker(&mut self, script: &str) {
        self.background = Some(BackgroundConfig {
            service_worker: Some(script.to_string()),
            scripts: None,
            persistent: None,
            module_type: None,
        });
    }
    
    /// Validate the manifest.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.name.is_empty() {
            return Err(ManifestError::MissingField("name"));
        }
        if self.version.is_empty() {
            return Err(ManifestError::MissingField("version"));
        }
        
        // Validate version format (major.minor.patch or major.minor.patch.build)
        let version_parts: Vec<&str> = self.version.split('.').collect();
        if version_parts.len() < 2 || version_parts.len() > 4 {
            return Err(ManifestError::InvalidVersion);
        }
        for part in version_parts {
            if part.parse::<u32>().is_err() {
                return Err(ManifestError::InvalidVersion);
            }
        }
        
        // Validate permissions
        for perm in &self.permissions {
            if !is_valid_permission(perm) {
                return Err(ManifestError::InvalidPermission(perm.clone()));
            }
        }
        
        // Validate host permissions
        for pattern in &self.host_permissions {
            if !is_valid_match_pattern(pattern) {
                return Err(ManifestError::InvalidMatchPattern(pattern.clone()));
            }
        }
        
        // Validate content scripts
        for script in &self.content_scripts {
            if script.matches.is_empty() {
                return Err(ManifestError::MissingField("content_scripts.matches"));
            }
            for pattern in &script.matches {
                if !is_valid_match_pattern(pattern) {
                    return Err(ManifestError::InvalidMatchPattern(pattern.clone()));
                }
            }
        }
        
        Ok(())
    }
}

/// Background configuration.
#[derive(Debug, Clone, Default)]
pub struct BackgroundConfig {
    /// Service worker script (Manifest V3).
    pub service_worker: Option<String>,
    /// Background scripts (Manifest V2, deprecated in V3).
    pub scripts: Option<Vec<String>>,
    /// Persistent background page (Manifest V2 only).
    pub persistent: Option<bool>,
    /// Module type for service worker.
    pub module_type: Option<String>,
}

/// Content script configuration.
#[derive(Debug, Clone, Default)]
pub struct ContentScript {
    /// URL match patterns.
    pub matches: Vec<String>,
    /// Exclude matches.
    pub exclude_matches: Vec<String>,
    /// JavaScript files.
    pub js: Vec<String>,
    /// CSS files.
    pub css: Vec<String>,
    /// Run at timing.
    pub run_at: RunAt,
    /// All frames.
    pub all_frames: bool,
    /// Match about:blank.
    pub match_about_blank: bool,
    /// Match origin as fallback.
    pub match_origin_as_fallback: bool,
    /// World (isolated or main).
    pub world: ContentScriptWorld,
}

impl ContentScript {
    /// Create a new content script.
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Add a match pattern.
    pub fn add_match(&mut self, pattern: &str) {
        self.matches.push(pattern.to_string());
    }
    
    /// Add a JavaScript file.
    pub fn add_js(&mut self, file: &str) {
        self.js.push(file.to_string());
    }
    
    /// Add a CSS file.
    pub fn add_css(&mut self, file: &str) {
        self.css.push(file.to_string());
    }
}

/// Content script run timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunAt {
    /// Run at document start (before DOM).
    DocumentStart,
    /// Run at document end (after DOM, before load).
    #[default]
    DocumentEnd,
    /// Run at document idle (after load).
    DocumentIdle,
}

/// Content script world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentScriptWorld {
    /// Isolated world (default).
    #[default]
    Isolated,
    /// Main world (same as page scripts).
    Main,
}

/// Browser action / page action configuration.
#[derive(Debug, Clone, Default)]
pub struct Action {
    /// Default icon.
    pub default_icon: BTreeMap<String, String>,
    /// Default title.
    pub default_title: Option<String>,
    /// Default popup.
    pub default_popup: Option<String>,
}

/// Options UI configuration.
#[derive(Debug, Clone, Default)]
pub struct OptionsUi {
    /// Options page.
    pub page: String,
    /// Open in tab.
    pub open_in_tab: bool,
}

/// Web accessible resource.
#[derive(Debug, Clone, Default)]
pub struct WebAccessibleResource {
    /// Resources.
    pub resources: Vec<String>,
    /// Matches.
    pub matches: Vec<String>,
    /// Extension IDs.
    pub extension_ids: Vec<String>,
    /// Use dynamic URL.
    pub use_dynamic_url: bool,
}

/// Content security policy.
#[derive(Debug, Clone, Default)]
pub struct ContentSecurityPolicy {
    /// Extension pages CSP.
    pub extension_pages: Option<String>,
    /// Sandbox CSP.
    pub sandbox: Option<String>,
}

/// Keyboard command.
#[derive(Debug, Clone, Default)]
pub struct Command {
    /// Suggested key.
    pub suggested_key: Option<SuggestedKey>,
    /// Description.
    pub description: Option<String>,
    /// Global command.
    pub global: bool,
}

/// Suggested keyboard shortcut.
#[derive(Debug, Clone, Default)]
pub struct SuggestedKey {
    /// Default key.
    pub default: Option<String>,
    /// Windows key.
    pub windows: Option<String>,
    /// Mac key.
    pub mac: Option<String>,
    /// Chrome OS key.
    pub chromeos: Option<String>,
    /// Linux key.
    pub linux: Option<String>,
}

/// Declarative net request configuration.
#[derive(Debug, Clone, Default)]
pub struct DeclarativeNetRequest {
    /// Rule resources.
    pub rule_resources: Vec<RuleResource>,
}

/// Rule resource.
#[derive(Debug, Clone, Default)]
pub struct RuleResource {
    /// ID.
    pub id: String,
    /// Enabled.
    pub enabled: bool,
    /// Path.
    pub path: String,
}

/// Externally connectable configuration.
#[derive(Debug, Clone, Default)]
pub struct ExternallyConnectable {
    /// IDs of extensions that can connect.
    pub ids: Vec<String>,
    /// URL matches for web pages.
    pub matches: Vec<String>,
    /// Accept TLS channel ID.
    pub accepts_tls_channel_id: bool,
}

/// Manifest parsing error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// Missing required field.
    MissingField(&'static str),
    /// Invalid version format.
    InvalidVersion,
    /// Invalid permission.
    InvalidPermission(String),
    /// Invalid match pattern.
    InvalidMatchPattern(String),
    /// Parse error.
    ParseError(String),
}

impl core::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingField(field) => write!(f, "Missing required field: {}", field),
            Self::InvalidVersion => write!(f, "Invalid version format"),
            Self::InvalidPermission(p) => write!(f, "Invalid permission: {}", p),
            Self::InvalidMatchPattern(p) => write!(f, "Invalid match pattern: {}", p),
            Self::ParseError(e) => write!(f, "Parse error: {}", e),
        }
    }
}

/// Check if a permission is valid.
fn is_valid_permission(permission: &str) -> bool {
    // Known permissions in Manifest V3
    const KNOWN_PERMISSIONS: &[&str] = &[
        "activeTab",
        "alarms",
        "background",
        "bookmarks",
        "browsingData",
        "certificateProvider",
        "clipboardRead",
        "clipboardWrite",
        "contentSettings",
        "contextMenus",
        "cookies",
        "debugger",
        "declarativeContent",
        "declarativeNetRequest",
        "declarativeNetRequestWithHostAccess",
        "declarativeNetRequestFeedback",
        "desktopCapture",
        "documentScan",
        "downloads",
        "downloads.open",
        "downloads.ui",
        "enterprise.deviceAttributes",
        "enterprise.hardwarePlatform",
        "enterprise.networkingAttributes",
        "enterprise.platformKeys",
        "favicon",
        "fileBrowserHandler",
        "fileSystemProvider",
        "fontSettings",
        "gcm",
        "geolocation",
        "history",
        "identity",
        "identity.email",
        "idle",
        "loginState",
        "management",
        "nativeMessaging",
        "notifications",
        "offscreen",
        "pageCapture",
        "platformKeys",
        "power",
        "printerProvider",
        "printing",
        "printingMetrics",
        "privacy",
        "processes",
        "proxy",
        "readingList",
        "runtime",
        "scripting",
        "search",
        "sessions",
        "sidePanel",
        "storage",
        "system.cpu",
        "system.display",
        "system.memory",
        "system.storage",
        "tabCapture",
        "tabGroups",
        "tabs",
        "topSites",
        "tts",
        "ttsEngine",
        "unlimitedStorage",
        "vpnProvider",
        "wallpaper",
        "webAuthenticationProxy",
        "webNavigation",
        "webRequest",
    ];
    
    KNOWN_PERMISSIONS.contains(&permission)
}

/// Check if a match pattern is valid.
fn is_valid_match_pattern(pattern: &str) -> bool {
    // Match pattern format: <scheme>://<host>/<path>
    // Scheme: http, https, file, ftp, *, or chrome-extension
    // Host: * or *.<host> or <host>
    // Path: /* or specific path
    
    if pattern == "<all_urls>" {
        return true;
    }
    
    let parts: Vec<&str> = pattern.splitn(2, "://").collect();
    if parts.len() != 2 {
        return false;
    }
    
    let scheme = parts[0];
    let rest = parts[1];
    
    // Validate scheme
    if !matches!(scheme, "http" | "https" | "file" | "ftp" | "*" | "chrome-extension") {
        return false;
    }
    
    // Split host and path
    if let Some(slash_pos) = rest.find('/') {
        let host = &rest[..slash_pos];
        let _path = &rest[slash_pos..];
        
        // Validate host
        if host.is_empty() && scheme != "file" {
            return false;
        }
        
        // Host can be * or *.domain or exact domain
        if host != "*" && !host.is_empty() {
            if host.starts_with("*.") {
                // Wildcard subdomain
                if host.len() <= 2 {
                    return false;
                }
            }
            // Otherwise exact host - basic validation
        }
        
        true
    } else {
        false
    }
}

/// Parse a manifest from JSON-like structure.
/// In a real implementation, this would parse actual JSON.
pub fn parse_manifest(json: &str) -> Result<Manifest, ManifestError> {
    // Simplified parsing - in real impl would use serde_json
    let mut manifest = Manifest::default();
    
    // Very basic parsing for demo
    if json.contains("\"manifest_version\": 2") || json.contains("\"manifest_version\":2") {
        manifest.manifest_version = ManifestVersion::V2;
    } else {
        manifest.manifest_version = ManifestVersion::V3;
    }
    
    // Extract name
    if let Some(start) = json.find("\"name\":") {
        let rest = &json[start + 7..];
        if let Some(quote_start) = rest.find('"') {
            let rest = &rest[quote_start + 1..];
            if let Some(quote_end) = rest.find('"') {
                manifest.name = rest[..quote_end].to_string();
            }
        }
    }
    
    // Extract version
    if let Some(start) = json.find("\"version\":") {
        let rest = &json[start + 10..];
        if let Some(quote_start) = rest.find('"') {
            let rest = &rest[quote_start + 1..];
            if let Some(quote_end) = rest.find('"') {
                manifest.version = rest[..quote_end].to_string();
            }
        }
    }
    
    if manifest.name.is_empty() {
        return Err(ManifestError::MissingField("name"));
    }
    if manifest.version.is_empty() {
        return Err(ManifestError::MissingField("version"));
    }
    
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_manifest_creation() {
        let mut manifest = Manifest::new("Test Extension", "1.0.0");
        manifest.add_permission("storage");
        manifest.add_host_permission("https://*.example.com/*");
        
        assert!(manifest.validate().is_ok());
        assert!(manifest.permissions.contains(&"storage".to_string()));
    }
    
    #[test]
    fn test_match_pattern_validation() {
        assert!(is_valid_match_pattern("<all_urls>"));
        assert!(is_valid_match_pattern("https://*.example.com/*"));
        assert!(is_valid_match_pattern("*://*/*"));
        assert!(is_valid_match_pattern("http://example.com/path/*"));
        
        assert!(!is_valid_match_pattern("invalid"));
        assert!(!is_valid_match_pattern("http://"));
    }
    
    #[test]
    fn test_permission_validation() {
        assert!(is_valid_permission("storage"));
        assert!(is_valid_permission("tabs"));
        assert!(is_valid_permission("activeTab"));
        
        assert!(!is_valid_permission("invalid_permission"));
    }
}
