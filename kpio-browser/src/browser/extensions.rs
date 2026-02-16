//! Extension System
//!
//! Experimental browser extension support.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Extension manager
#[derive(Debug, Clone)]
pub struct ExtensionManager {
    /// Installed extensions
    pub extensions: Vec<Extension>,
    /// Extension settings
    pub settings: ExtensionSettings,
    /// Enabled extension IDs
    pub enabled: Vec<String>,
    /// Permissions granted
    pub permissions: BTreeMap<String, Vec<Permission>>,
}

impl ExtensionManager {
    /// Create new manager
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            settings: ExtensionSettings::default(),
            enabled: Vec::new(),
            permissions: BTreeMap::new(),
        }
    }

    /// Install extension
    pub fn install(&mut self, extension: Extension) -> Result<(), ExtensionError> {
        // Check if already installed
        if self.extensions.iter().any(|e| e.id == extension.id) {
            return Err(ExtensionError::AlreadyInstalled);
        }

        // Validate manifest
        if extension.manifest.manifest_version < 2 {
            return Err(ExtensionError::UnsupportedManifestVersion);
        }

        // Check permissions
        if !self.settings.allow_all_permissions {
            for perm in &extension.manifest.permissions {
                if Self::is_dangerous_permission(perm) && !self.settings.allow_dangerous {
                    return Err(ExtensionError::DangerousPermission(perm.clone()));
                }
            }
        }

        let id = extension.id.clone();
        self.extensions.push(extension);

        if self.settings.auto_enable {
            self.enabled.push(id);
        }

        Ok(())
    }

    /// Uninstall extension
    pub fn uninstall(&mut self, id: &str) -> bool {
        if let Some(idx) = self.extensions.iter().position(|e| e.id == id) {
            self.extensions.remove(idx);
            self.enabled.retain(|e| e != id);
            self.permissions.remove(id);
            true
        } else {
            false
        }
    }

    /// Enable extension
    pub fn enable(&mut self, id: &str) -> bool {
        if self.extensions.iter().any(|e| e.id == id) && !self.enabled.contains(&id.to_string()) {
            self.enabled.push(id.to_string());
            true
        } else {
            false
        }
    }

    /// Disable extension
    pub fn disable(&mut self, id: &str) {
        self.enabled.retain(|e| e != id);
    }

    /// Toggle extension
    pub fn toggle(&mut self, id: &str) {
        if self.is_enabled(id) {
            self.disable(id);
        } else {
            self.enable(id);
        }
    }

    /// Is extension enabled
    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.contains(&id.to_string())
    }

    /// Get extension
    pub fn get(&self, id: &str) -> Option<&Extension> {
        self.extensions.iter().find(|e| e.id == id)
    }

    /// Get enabled extensions
    pub fn enabled_extensions(&self) -> Vec<&Extension> {
        self.extensions
            .iter()
            .filter(|e| self.enabled.contains(&e.id))
            .collect()
    }

    /// Grant permission
    pub fn grant_permission(&mut self, ext_id: &str, permission: Permission) {
        self.permissions
            .entry(ext_id.to_string())
            .or_insert_with(Vec::new)
            .push(permission);
    }

    /// Revoke permission
    pub fn revoke_permission(&mut self, ext_id: &str, permission: &Permission) {
        if let Some(perms) = self.permissions.get_mut(ext_id) {
            perms.retain(|p| p != permission);
        }
    }

    /// Check permission
    pub fn has_permission(&self, ext_id: &str, permission: &Permission) -> bool {
        self.permissions
            .get(ext_id)
            .map(|perms| perms.contains(permission))
            .unwrap_or(false)
    }

    /// Get extensions with content scripts for URL
    pub fn content_scripts_for_url(&self, url: &str) -> Vec<(&Extension, &ContentScript)> {
        let mut result = Vec::new();

        for ext in self.enabled_extensions() {
            for script in &ext.manifest.content_scripts {
                if script.matches_url(url) {
                    result.push((ext, script));
                }
            }
        }

        result
    }

    /// Check if permission is dangerous
    fn is_dangerous_permission(perm: &str) -> bool {
        matches!(
            perm,
            "tabs"
                | "history"
                | "cookies"
                | "webRequest"
                | "webRequestBlocking"
                | "<all_urls>"
                | "*://*/*"
                | "nativeMessaging"
                | "debugger"
        )
    }
}

impl Default for ExtensionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension
#[derive(Debug, Clone)]
pub struct Extension {
    /// Unique ID
    pub id: String,
    /// Manifest
    pub manifest: ExtensionManifest,
    /// Installation source
    pub source: ExtensionSource,
    /// Installed timestamp
    pub installed_at: u64,
    /// Last updated timestamp
    pub updated_at: u64,
    /// Extension state
    pub state: ExtensionState,
    /// Icon data URL
    pub icon: Option<String>,
}

/// Extension manifest (Chrome-compatible)
#[derive(Debug, Clone)]
pub struct ExtensionManifest {
    /// Manifest version
    pub manifest_version: u32,
    /// Name
    pub name: String,
    /// Version
    pub version: String,
    /// Description
    pub description: String,
    /// Author
    pub author: Option<String>,
    /// Homepage URL
    pub homepage_url: Option<String>,
    /// Icons
    pub icons: BTreeMap<u32, String>,
    /// Permissions
    pub permissions: Vec<String>,
    /// Optional permissions
    pub optional_permissions: Vec<String>,
    /// Host permissions
    pub host_permissions: Vec<String>,
    /// Background script/service worker
    pub background: Option<BackgroundScript>,
    /// Content scripts
    pub content_scripts: Vec<ContentScript>,
    /// Browser action (toolbar button)
    pub browser_action: Option<BrowserAction>,
    /// Page action
    pub page_action: Option<PageAction>,
    /// Options page
    pub options_page: Option<String>,
    /// Options UI
    pub options_ui: Option<OptionsUI>,
    /// Web accessible resources
    pub web_accessible_resources: Vec<String>,
    /// Commands (keyboard shortcuts)
    pub commands: BTreeMap<String, Command>,
}

impl Default for ExtensionManifest {
    fn default() -> Self {
        Self {
            manifest_version: 3,
            name: String::new(),
            version: String::from("1.0.0"),
            description: String::new(),
            author: None,
            homepage_url: None,
            icons: BTreeMap::new(),
            permissions: Vec::new(),
            optional_permissions: Vec::new(),
            host_permissions: Vec::new(),
            background: None,
            content_scripts: Vec::new(),
            browser_action: None,
            page_action: None,
            options_page: None,
            options_ui: None,
            web_accessible_resources: Vec::new(),
            commands: BTreeMap::new(),
        }
    }
}

/// Background script configuration
#[derive(Debug, Clone)]
pub struct BackgroundScript {
    /// Service worker (MV3)
    pub service_worker: Option<String>,
    /// Scripts (MV2)
    pub scripts: Vec<String>,
    /// Persistent (MV2)
    pub persistent: bool,
}

/// Content script
#[derive(Debug, Clone)]
pub struct ContentScript {
    /// URL match patterns
    pub matches: Vec<String>,
    /// Exclude matches
    pub exclude_matches: Vec<String>,
    /// CSS files
    pub css: Vec<String>,
    /// JS files
    pub js: Vec<String>,
    /// Run at
    pub run_at: RunAt,
    /// All frames
    pub all_frames: bool,
    /// Match about:blank
    pub match_about_blank: bool,
}

impl ContentScript {
    /// Check if script should run on URL
    pub fn matches_url(&self, url: &str) -> bool {
        // Check exclude first
        for pattern in &self.exclude_matches {
            if Self::match_pattern(pattern, url) {
                return false;
            }
        }

        // Check matches
        for pattern in &self.matches {
            if Self::match_pattern(pattern, url) {
                return true;
            }
        }

        false
    }

    /// Simple pattern matching
    fn match_pattern(pattern: &str, url: &str) -> bool {
        if pattern == "<all_urls>" {
            return true;
        }

        // Very simplified pattern matching
        // Real implementation would be more complex
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            let mut pos = 0;

            for (i, part) in parts.iter().enumerate() {
                if part.is_empty() {
                    continue;
                }

                if let Some(found) = url[pos..].find(part) {
                    if i == 0 && found != 0 {
                        return false;
                    }
                    pos += found + part.len();
                } else {
                    return false;
                }
            }
            true
        } else {
            url == pattern
        }
    }
}

/// When to run content script
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunAt {
    DocumentStart,
    #[default]
    DocumentEnd,
    DocumentIdle,
}

/// Browser action (toolbar button)
#[derive(Debug, Clone)]
pub struct BrowserAction {
    /// Default icon
    pub default_icon: Option<String>,
    /// Default title (tooltip)
    pub default_title: Option<String>,
    /// Default popup HTML
    pub default_popup: Option<String>,
}

/// Page action
#[derive(Debug, Clone)]
pub struct PageAction {
    /// Default icon
    pub default_icon: Option<String>,
    /// Default title
    pub default_title: Option<String>,
    /// Default popup
    pub default_popup: Option<String>,
}

/// Options UI
#[derive(Debug, Clone)]
pub struct OptionsUI {
    /// Page
    pub page: String,
    /// Open in tab
    pub open_in_tab: bool,
}

/// Keyboard command
#[derive(Debug, Clone)]
pub struct Command {
    /// Suggested key
    pub suggested_key: Option<String>,
    /// Description
    pub description: String,
}

/// Extension source
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionSource {
    /// Official store
    Store,
    /// Developer mode (unpacked)
    Developer,
    /// Sideloaded
    Sideloaded,
    /// Policy installed
    Policy,
}

/// Extension state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtensionState {
    #[default]
    Installed,
    Enabled,
    Disabled,
    NeedsUpdate,
    Error,
}

/// Permission
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Permission {
    /// Access to tabs
    Tabs,
    /// Access to history
    History,
    /// Access to bookmarks
    Bookmarks,
    /// Access to cookies
    Cookies,
    /// Access to storage
    Storage,
    /// Access to notifications
    Notifications,
    /// Web request
    WebRequest,
    /// Web request blocking
    WebRequestBlocking,
    /// Host permission
    Host(String),
    /// Other
    Other(String),
}

/// Extension settings
#[derive(Debug, Clone)]
pub struct ExtensionSettings {
    /// Allow extensions from outside store
    pub allow_sideloading: bool,
    /// Allow developer mode
    pub developer_mode: bool,
    /// Auto-enable new extensions
    pub auto_enable: bool,
    /// Allow dangerous permissions
    pub allow_dangerous: bool,
    /// Allow all permissions without prompt
    pub allow_all_permissions: bool,
    /// Disabled extensions run in private mode
    pub run_in_private: bool,
}

impl Default for ExtensionSettings {
    fn default() -> Self {
        Self {
            allow_sideloading: true, // Experimental
            developer_mode: true,    // Experimental
            auto_enable: false,
            allow_dangerous: false,
            allow_all_permissions: false,
            run_in_private: false,
        }
    }
}

/// Extension error
#[derive(Debug, Clone)]
pub enum ExtensionError {
    /// Already installed
    AlreadyInstalled,
    /// Not found
    NotFound,
    /// Invalid manifest
    InvalidManifest(String),
    /// Unsupported manifest version
    UnsupportedManifestVersion,
    /// Dangerous permission required
    DangerousPermission(String),
    /// Install not allowed
    InstallNotAllowed,
    /// Parse error
    ParseError(String),
}

// =============================================================================
// Extension API (Browser APIs exposed to extensions)
// =============================================================================

/// Extension runtime message
#[derive(Debug, Clone)]
pub struct ExtensionMessage {
    /// Sender extension ID
    pub sender_id: String,
    /// Sender tab ID
    pub sender_tab: Option<u64>,
    /// Message data (JSON)
    pub data: String,
}

/// Extension storage
#[derive(Debug, Clone, Default)]
pub struct ExtensionStorage {
    /// Local storage (per extension)
    pub local: BTreeMap<String, String>,
    /// Sync storage (per extension, would sync if connected)
    pub sync: BTreeMap<String, String>,
}

impl ExtensionStorage {
    /// Get local value
    pub fn get_local(&self, key: &str) -> Option<&String> {
        self.local.get(key)
    }

    /// Set local value
    pub fn set_local(&mut self, key: &str, value: &str) {
        self.local.insert(key.to_string(), value.to_string());
    }

    /// Remove local value
    pub fn remove_local(&mut self, key: &str) {
        self.local.remove(key);
    }

    /// Clear local storage
    pub fn clear_local(&mut self) {
        self.local.clear();
    }

    /// Get sync value
    pub fn get_sync(&self, key: &str) -> Option<&String> {
        self.sync.get(key)
    }

    /// Set sync value
    pub fn set_sync(&mut self, key: &str, value: &str) {
        self.sync.insert(key.to_string(), value.to_string());
    }

    /// Remove sync value
    pub fn remove_sync(&mut self, key: &str) {
        self.sync.remove(key);
    }

    /// Clear sync storage
    pub fn clear_sync(&mut self) {
        self.sync.clear();
    }
}

/// Browser action badge
#[derive(Debug, Clone, Default)]
pub struct ActionBadge {
    /// Badge text
    pub text: String,
    /// Badge background color
    pub background_color: Option<String>,
    /// Badge text color
    pub text_color: Option<String>,
}
