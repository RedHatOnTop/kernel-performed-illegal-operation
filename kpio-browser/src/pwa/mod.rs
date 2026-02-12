//! Progressive Web App (PWA) Support
//!
//! Implements PWA features including Web App Manifest, installation flow,
//! and standalone window mode.

pub mod manifest;
pub mod window;
pub mod push;
pub mod install;
pub mod kernel_bridge;
pub mod sw_bridge;
pub mod cache_storage;
pub mod fetch_interceptor;
pub mod idb_engine;
pub mod indexed_db;
pub mod web_storage;

pub use manifest::*;
pub use window::*;
pub use push::*;
pub use install::*;
pub use kernel_bridge::KernelAppId;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

/// PWA installation state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallState {
    /// Not installable
    NotInstallable,
    /// Can be installed
    Installable,
    /// Installation in progress
    Installing,
    /// Installed
    Installed,
}

impl Default for InstallState {
    fn default() -> Self {
        Self::NotInstallable
    }
}

/// PWA error types
#[derive(Debug, Clone)]
pub enum PwaError {
    /// Manifest fetch failed
    ManifestFetchFailed(String),
    /// Invalid manifest
    InvalidManifest(String),
    /// Installation failed
    InstallationFailed(String),
    /// Not installable
    NotInstallable,
    /// Already installed
    AlreadyInstalled,
    /// Service worker required
    ServiceWorkerRequired,
    /// Permission denied
    PermissionDenied,
}

/// Installed PWA app info
#[derive(Debug, Clone)]
pub struct InstalledApp {
    /// App ID
    pub id: String,
    /// App name
    pub name: String,
    /// Start URL
    pub start_url: String,
    /// Scope
    pub scope: String,
    /// Display mode
    pub display: DisplayMode,
    /// Theme color
    pub theme_color: Option<u32>,
    /// Background color
    pub background_color: Option<u32>,
    /// Icons
    pub icons: Vec<AppIcon>,
    /// Installation timestamp
    pub installed_at: u64,
    /// Last launched timestamp
    pub last_launched: Option<u64>,
    /// Kernel-assigned app ID (if registered with kernel app manager)
    pub kernel_app_id: Option<kernel_bridge::KernelAppId>,
}

/// App icon
#[derive(Debug, Clone)]
pub struct AppIcon {
    /// Icon URL
    pub src: String,
    /// Icon sizes
    pub sizes: String,
    /// Icon type
    pub icon_type: String,
    /// Purpose
    pub purpose: IconPurpose,
}

/// Icon purpose
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconPurpose {
    /// Any purpose
    Any,
    /// Maskable icon
    Maskable,
    /// Monochrome icon
    Monochrome,
}

impl Default for IconPurpose {
    fn default() -> Self {
        Self::Any
    }
}

/// Display mode for PWA
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    /// Fullscreen mode
    Fullscreen,
    /// Standalone mode (app-like)
    Standalone,
    /// Minimal UI
    MinimalUi,
    /// Browser mode
    Browser,
}

impl Default for DisplayMode {
    fn default() -> Self {
        Self::Browser
    }
}

impl DisplayMode {
    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "fullscreen" => Self::Fullscreen,
            "standalone" => Self::Standalone,
            "minimal-ui" => Self::MinimalUi,
            "browser" => Self::Browser,
            _ => Self::Browser,
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fullscreen => "fullscreen",
            Self::Standalone => "standalone",
            Self::MinimalUi => "minimal-ui",
            Self::Browser => "browser",
        }
    }
}

/// PWA manager
pub struct PwaManager {
    /// Installed apps by ID
    installed_apps: BTreeMap<String, InstalledApp>,
    /// Pending installs
    pending_installs: Vec<String>,
}

impl PwaManager {
    /// Create new PWA manager
    pub const fn new() -> Self {
        Self {
            installed_apps: BTreeMap::new(),
            pending_installs: Vec::new(),
        }
    }

    /// Get installed apps
    pub fn installed_apps(&self) -> impl Iterator<Item = &InstalledApp> {
        self.installed_apps.values()
    }

    /// Get app by ID
    pub fn get_app(&self, id: &str) -> Option<&InstalledApp> {
        self.installed_apps.get(id)
    }

    /// Check if app is installed
    pub fn is_installed(&self, scope: &str) -> bool {
        self.installed_apps.values().any(|app| app.scope == scope)
    }

    /// Install an app
    pub fn install(&mut self, mut app: InstalledApp) -> Result<(), PwaError> {
        if self.installed_apps.contains_key(&app.id) {
            return Err(PwaError::AlreadyInstalled);
        }

        // If kernel bridge is connected and app doesn't have a kernel ID yet,
        // register with kernel via the bridge
        if app.kernel_app_id.is_none() && kernel_bridge::is_connected() {
            // Build a minimal manifest to pass through the bridge
            let mut manifest = WebAppManifest::new();
            manifest.name = Some(app.name.clone());
            manifest.start_url = app.start_url.clone();
            manifest.scope = Some(app.scope.clone());
            manifest.display = app.display;

            if let Ok(kid) = kernel_bridge::pwa_install_to_kernel(&manifest) {
                app.kernel_app_id = Some(kid);
            }
        }

        self.installed_apps.insert(app.id.clone(), app);
        Ok(())
    }

    /// Uninstall an app
    pub fn uninstall(&mut self, id: &str) -> Result<(), PwaError> {
        if let Some(app) = self.installed_apps.remove(id) {
            // If the app was registered with the kernel, unregister it
            if let Some(kid) = app.kernel_app_id {
                let _ = kernel_bridge::pwa_uninstall_from_kernel(kid);
            }
            Ok(())
        } else {
            Err(PwaError::InvalidManifest("App not found".into()))
        }
    }

    /// Launch an app
    pub fn launch(&mut self, id: &str) -> Result<String, PwaError> {
        if let Some(app) = self.installed_apps.get_mut(id) {
            app.last_launched = Some(0); // Would use actual timestamp
            Ok(app.start_url.clone())
        } else {
            Err(PwaError::InvalidManifest("App not found".into()))
        }
    }

    /// Get app count
    pub fn app_count(&self) -> usize {
        self.installed_apps.len()
    }
}

impl Default for PwaManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global PWA manager
pub static PWA_MANAGER: RwLock<PwaManager> = RwLock::new(PwaManager::new());

/// Initialize PWA subsystem
pub fn init() {
    // Load installed apps from storage
}

/// Check if a URL is installable as PWA
pub fn check_installable(url: &str) -> InstallState {
    // Would check for:
    // 1. Valid manifest
    // 2. Service worker
    // 3. HTTPS
    // 4. Not already installed
    InstallState::NotInstallable
}
