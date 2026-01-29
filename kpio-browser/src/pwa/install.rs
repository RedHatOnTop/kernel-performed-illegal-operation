//! PWA Installation
//!
//! Handles PWA installation flow and app management.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

use super::{DisplayMode, InstalledApp, AppIcon, PwaError};
use super::manifest::WebAppManifest;

/// Installation prompt
pub struct BeforeInstallPromptEvent {
    /// Platforms
    platforms: Vec<String>,
    /// User choice
    user_choice: Option<InstallChoice>,
    /// Default prevented
    default_prevented: bool,
}

impl BeforeInstallPromptEvent {
    /// Create new event
    pub fn new(platforms: Vec<String>) -> Self {
        Self {
            platforms,
            user_choice: None,
            default_prevented: false,
        }
    }

    /// Get platforms
    pub fn platforms(&self) -> &[String] {
        &self.platforms
    }

    /// Prevent default
    pub fn prevent_default(&mut self) {
        self.default_prevented = true;
    }

    /// Check if default prevented
    pub fn is_default_prevented(&self) -> bool {
        self.default_prevented
    }

    /// Prompt user for installation
    pub fn prompt(&mut self) -> InstallChoice {
        // Would show actual prompt
        // For now, return accepted
        let choice = InstallChoice {
            outcome: InstallOutcome::Accepted,
            platform: "web".to_string(),
        };
        self.user_choice = Some(choice.clone());
        choice
    }

    /// Get user choice
    pub fn user_choice(&self) -> Option<&InstallChoice> {
        self.user_choice.as_ref()
    }
}

/// Install choice
#[derive(Debug, Clone)]
pub struct InstallChoice {
    /// Outcome
    pub outcome: InstallOutcome,
    /// Platform
    pub platform: String,
}

/// Install outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    /// Accepted
    Accepted,
    /// Dismissed
    Dismissed,
}

/// App installed event
pub struct AppInstalledEvent {
    /// App ID
    app_id: String,
}

impl AppInstalledEvent {
    /// Create new event
    pub fn new(app_id: String) -> Self {
        Self { app_id }
    }

    /// Get app ID
    pub fn app_id(&self) -> &str {
        &self.app_id
    }
}

/// Install manager
pub struct InstallManager {
    /// Pending prompts by URL
    pending_prompts: BTreeMap<String, BeforeInstallPromptEvent>,
    /// Install criteria
    criteria: InstallCriteria,
}

impl InstallManager {
    /// Create new install manager
    pub const fn new() -> Self {
        Self {
            pending_prompts: BTreeMap::new(),
            criteria: InstallCriteria::default_const(),
        }
    }

    /// Check if URL can be installed
    pub fn can_install(&self, url: &str, manifest: &WebAppManifest) -> bool {
        // Check basic criteria
        if !manifest.is_valid_for_install() {
            return false;
        }

        // Check if already installed
        if self.is_installed(url) {
            return false;
        }

        // Check if has service worker (would need to check actual registration)
        // For now, assume all PWAs with manifest can be installed

        true
    }

    /// Check if URL is already installed
    pub fn is_installed(&self, url: &str) -> bool {
        // Would check against installed apps
        false
    }

    /// Create install prompt
    pub fn create_prompt(&mut self, url: &str) -> &BeforeInstallPromptEvent {
        let platforms = alloc::vec!["web".to_string()];
        let event = BeforeInstallPromptEvent::new(platforms);
        self.pending_prompts.insert(url.to_string(), event);
        self.pending_prompts.get(url).unwrap()
    }

    /// Get pending prompt
    pub fn get_prompt(&self, url: &str) -> Option<&BeforeInstallPromptEvent> {
        self.pending_prompts.get(url)
    }

    /// Get mutable pending prompt
    pub fn get_prompt_mut(&mut self, url: &str) -> Option<&mut BeforeInstallPromptEvent> {
        self.pending_prompts.get_mut(url)
    }

    /// Install app
    pub fn install(&mut self, url: &str, manifest: &WebAppManifest) -> Result<InstalledApp, PwaError> {
        let app_id = generate_app_id(url);

        let icons: Vec<AppIcon> = manifest.icons.iter()
            .map(|i| i.to_app_icon())
            .collect();

        let app = InstalledApp {
            id: app_id.clone(),
            name: manifest.display_name().to_string(),
            start_url: manifest.start_url.clone(),
            scope: manifest.scope.clone().unwrap_or_else(|| "/".to_string()),
            display: manifest.display,
            theme_color: manifest.theme_color_u32(),
            background_color: manifest.background_color_u32(),
            icons,
            installed_at: 0, // Would get current time
            last_launched: None,
        };

        // Remove pending prompt
        self.pending_prompts.remove(url);

        Ok(app)
    }

    /// Uninstall app
    pub fn uninstall(&mut self, app_id: &str) -> Result<(), PwaError> {
        // Would remove app data
        Ok(())
    }

    /// Get install criteria
    pub fn criteria(&self) -> &InstallCriteria {
        &self.criteria
    }

    /// Set install criteria
    pub fn set_criteria(&mut self, criteria: InstallCriteria) {
        self.criteria = criteria;
    }
}

impl Default for InstallManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Install criteria
#[derive(Debug, Clone)]
pub struct InstallCriteria {
    /// Require HTTPS
    pub require_https: bool,
    /// Require service worker
    pub require_service_worker: bool,
    /// Require manifest
    pub require_manifest: bool,
    /// Require name
    pub require_name: bool,
    /// Require icons
    pub require_icons: bool,
    /// Minimum icon size
    pub min_icon_size: u32,
    /// Require start URL
    pub require_start_url: bool,
    /// Require display mode
    pub require_display_mode: bool,
    /// Allowed display modes
    pub allowed_display_modes: Vec<DisplayMode>,
}

impl InstallCriteria {
    /// Create default criteria
    pub fn new() -> Self {
        Self {
            require_https: true,
            require_service_worker: true,
            require_manifest: true,
            require_name: true,
            require_icons: true,
            min_icon_size: 144,
            require_start_url: true,
            require_display_mode: true,
            allowed_display_modes: alloc::vec![
                DisplayMode::Standalone,
                DisplayMode::Fullscreen,
                DisplayMode::MinimalUi,
            ],
        }
    }

    /// Create default criteria (const)
    pub const fn default_const() -> Self {
        Self {
            require_https: true,
            require_service_worker: true,
            require_manifest: true,
            require_name: true,
            require_icons: true,
            min_icon_size: 144,
            require_start_url: true,
            require_display_mode: true,
            allowed_display_modes: Vec::new(), // Can't create vec in const
        }
    }

    /// Check manifest against criteria
    pub fn check(&self, manifest: &WebAppManifest, has_service_worker: bool, is_https: bool) -> InstallCheckResult {
        let mut errors = Vec::new();

        if self.require_https && !is_https {
            errors.push(InstallCheckError::NotHttps);
        }

        if self.require_service_worker && !has_service_worker {
            errors.push(InstallCheckError::NoServiceWorker);
        }

        if self.require_name && manifest.name.is_none() && manifest.short_name.is_none() {
            errors.push(InstallCheckError::NoName);
        }

        if self.require_icons && manifest.icons.is_empty() {
            errors.push(InstallCheckError::NoIcons);
        }

        if self.require_icons && !manifest.icons.is_empty() {
            let has_large_icon = manifest.icons.iter().any(|icon| {
                icon.sizes.split_whitespace().any(|s| {
                    if let Some((w, _)) = s.split_once('x') {
                        w.parse::<u32>().ok().map(|w| w >= self.min_icon_size).unwrap_or(false)
                    } else {
                        false
                    }
                })
            });
            if !has_large_icon {
                errors.push(InstallCheckError::IconTooSmall(self.min_icon_size));
            }
        }

        if self.require_display_mode && !self.allowed_display_modes.is_empty() {
            if !self.allowed_display_modes.contains(&manifest.display) {
                errors.push(InstallCheckError::InvalidDisplayMode);
            }
        }

        InstallCheckResult { errors }
    }
}

impl Default for InstallCriteria {
    fn default() -> Self {
        Self::new()
    }
}

/// Install check result
#[derive(Debug)]
pub struct InstallCheckResult {
    /// Errors
    pub errors: Vec<InstallCheckError>,
}

impl InstallCheckResult {
    /// Check if installable
    pub fn is_installable(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Install check error
#[derive(Debug, Clone)]
pub enum InstallCheckError {
    /// Not HTTPS
    NotHttps,
    /// No service worker
    NoServiceWorker,
    /// No manifest
    NoManifest,
    /// No name
    NoName,
    /// No icons
    NoIcons,
    /// Icon too small
    IconTooSmall(u32),
    /// Invalid display mode
    InvalidDisplayMode,
    /// No start URL
    NoStartUrl,
}

/// Launch handler
#[derive(Debug, Clone)]
pub struct LaunchHandler {
    /// Client mode
    client_mode: LaunchClientMode,
}

impl Default for LaunchHandler {
    fn default() -> Self {
        Self {
            client_mode: LaunchClientMode::Auto,
        }
    }
}

impl LaunchHandler {
    /// Create new launch handler
    pub fn new(client_mode: LaunchClientMode) -> Self {
        Self { client_mode }
    }

    /// Get client mode
    pub fn client_mode(&self) -> LaunchClientMode {
        self.client_mode
    }
}

/// Launch client mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchClientMode {
    /// Auto (browser decides)
    Auto,
    /// Navigate existing client
    NavigateExisting,
    /// Focus existing client
    FocusExisting,
    /// Navigate new client
    NavigateNew,
}

/// Launch params
#[derive(Debug, Clone)]
pub struct LaunchParams {
    /// Target URL
    target_url: Option<String>,
    /// Files
    files: Vec<FileSystemHandle>,
}

impl LaunchParams {
    /// Create new launch params
    pub fn new() -> Self {
        Self {
            target_url: None,
            files: Vec::new(),
        }
    }

    /// Get target URL
    pub fn target_url(&self) -> Option<&str> {
        self.target_url.as_deref()
    }

    /// Set target URL
    pub fn set_target_url(&mut self, url: String) {
        self.target_url = Some(url);
    }

    /// Get files
    pub fn files(&self) -> &[FileSystemHandle] {
        &self.files
    }

    /// Add file
    pub fn add_file(&mut self, handle: FileSystemHandle) {
        self.files.push(handle);
    }
}

impl Default for LaunchParams {
    fn default() -> Self {
        Self::new()
    }
}

/// File system handle (simplified)
#[derive(Debug, Clone)]
pub struct FileSystemHandle {
    /// Name
    pub name: String,
    /// Kind
    pub kind: FileSystemHandleKind,
}

/// File system handle kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSystemHandleKind {
    /// File
    File,
    /// Directory
    Directory,
}

/// Launch queue
pub struct LaunchQueue {
    /// Consumer callback set
    consumer_set: bool,
    /// Queued launch params
    queue: Vec<LaunchParams>,
}

impl LaunchQueue {
    /// Create new launch queue
    pub const fn new() -> Self {
        Self {
            consumer_set: false,
            queue: Vec::new(),
        }
    }

    /// Set consumer
    pub fn set_consumer(&mut self) {
        self.consumer_set = true;
        // Would call consumer with queued params
    }

    /// Enqueue launch params
    pub fn enqueue(&mut self, params: LaunchParams) {
        if self.consumer_set {
            // Would call consumer immediately
        } else {
            self.queue.push(params);
        }
    }

    /// Get queued params
    pub fn drain_queue(&mut self) -> Vec<LaunchParams> {
        core::mem::take(&mut self.queue)
    }
}

impl Default for LaunchQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Global install manager
pub static INSTALL_MANAGER: RwLock<InstallManager> = RwLock::new(InstallManager::new());

// Helper functions

fn generate_app_id(url: &str) -> String {
    // Simple hash of URL
    let mut hash: u64 = 5381;
    for byte in url.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    alloc::format!("pwa_{:016x}", hash)
}
