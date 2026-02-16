//! KPIO Browser Extension System
//!
//! This crate provides Chrome extension compatibility for KPIO browser.
//! It implements Manifest V3 extension format and common browser APIs.
//!
//! # Modules
//!
//! - `manifest`: Extension manifest parsing (Manifest V3)
//! - `sandbox`: Extension isolation and execution
//! - `api`: Browser extension APIs (chrome.*)
//! - `store`: Extension installation and updates
//! - `content`: Content script injection

#![no_std]

extern crate alloc;

pub mod api;
pub mod content;
pub mod manifest;
pub mod sandbox;
pub mod store;

use alloc::string::String;
use alloc::vec::Vec;
use hashbrown::HashMap;
use spin::RwLock;

/// Extension ID (unique identifier).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtensionId(pub String);

impl ExtensionId {
    /// Create a new extension ID.
    pub fn new(id: &str) -> Self {
        Self(id.into())
    }

    /// Get as string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Generate an ID from extension name (for development).
    pub fn from_name(name: &str) -> Self {
        // Create a simple hash-like ID from name
        let mut hash = 0u64;
        for b in name.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(b as u64);
        }
        Self(alloc::format!("{:032x}", hash))
    }
}

/// Extension state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionState {
    /// Extension is disabled.
    Disabled,
    /// Extension is enabled and running.
    Enabled,
    /// Extension is being installed.
    Installing,
    /// Extension encountered an error.
    Error,
    /// Extension is being updated.
    Updating,
}

/// Loaded extension information.
#[derive(Debug, Clone)]
pub struct Extension {
    /// Extension ID.
    pub id: ExtensionId,
    /// Manifest data.
    pub manifest: manifest::Manifest,
    /// Current state.
    pub state: ExtensionState,
    /// Installation path.
    pub path: String,
    /// Whether it's a development extension (unpacked).
    pub is_dev: bool,
}

impl Extension {
    /// Create a new extension.
    pub fn new(id: ExtensionId, manifest: manifest::Manifest, path: &str) -> Self {
        Self {
            id,
            manifest,
            state: ExtensionState::Disabled,
            path: path.into(),
            is_dev: false,
        }
    }

    /// Check if extension has a permission.
    pub fn has_permission(&self, permission: &str) -> bool {
        self.manifest.permissions.iter().any(|p| p == permission)
    }

    /// Check if extension has host permission for URL.
    pub fn has_host_permission(&self, url: &str) -> bool {
        // Check against host_permissions patterns
        for pattern in &self.manifest.host_permissions {
            if content::match_pattern(pattern, url) {
                return true;
            }
        }
        false
    }
}

/// Extension manager.
pub struct ExtensionManager {
    /// Loaded extensions.
    extensions: RwLock<HashMap<String, Extension>>,
    /// Event listeners.
    listeners: RwLock<Vec<ExtensionEventListener>>,
}

/// Extension event listener.
type ExtensionEventListener = alloc::boxed::Box<dyn Fn(&ExtensionEvent) + Send + Sync>;

/// Extension event.
#[derive(Debug, Clone)]
pub enum ExtensionEvent {
    /// Extension installed.
    Installed(ExtensionId),
    /// Extension uninstalled.
    Uninstalled(ExtensionId),
    /// Extension enabled.
    Enabled(ExtensionId),
    /// Extension disabled.
    Disabled(ExtensionId),
    /// Extension updated.
    Updated(ExtensionId),
}

impl ExtensionManager {
    /// Create a new extension manager.
    pub fn new() -> Self {
        Self {
            extensions: RwLock::new(HashMap::new()),
            listeners: RwLock::new(Vec::new()),
        }
    }

    /// Load an extension from manifest.
    pub fn load_extension(
        &self,
        manifest: manifest::Manifest,
        path: &str,
    ) -> Result<ExtensionId, ExtensionError> {
        let id = ExtensionId::from_name(&manifest.name);
        let extension = Extension::new(id.clone(), manifest, path);

        let mut extensions = self.extensions.write();
        if extensions.contains_key(&id.0) {
            return Err(ExtensionError::AlreadyInstalled);
        }

        extensions.insert(id.0.clone(), extension);
        self.emit_event(ExtensionEvent::Installed(id.clone()));

        Ok(id)
    }

    /// Unload an extension.
    pub fn unload_extension(&self, id: &ExtensionId) -> Result<(), ExtensionError> {
        let mut extensions = self.extensions.write();
        if extensions.remove(&id.0).is_some() {
            self.emit_event(ExtensionEvent::Uninstalled(id.clone()));
            Ok(())
        } else {
            Err(ExtensionError::NotFound)
        }
    }

    /// Enable an extension.
    pub fn enable_extension(&self, id: &ExtensionId) -> Result<(), ExtensionError> {
        let mut extensions = self.extensions.write();
        if let Some(ext) = extensions.get_mut(&id.0) {
            ext.state = ExtensionState::Enabled;
            self.emit_event(ExtensionEvent::Enabled(id.clone()));
            Ok(())
        } else {
            Err(ExtensionError::NotFound)
        }
    }

    /// Disable an extension.
    pub fn disable_extension(&self, id: &ExtensionId) -> Result<(), ExtensionError> {
        let mut extensions = self.extensions.write();
        if let Some(ext) = extensions.get_mut(&id.0) {
            ext.state = ExtensionState::Disabled;
            self.emit_event(ExtensionEvent::Disabled(id.clone()));
            Ok(())
        } else {
            Err(ExtensionError::NotFound)
        }
    }

    /// Get an extension by ID.
    pub fn get_extension(&self, id: &ExtensionId) -> Option<Extension> {
        self.extensions.read().get(&id.0).cloned()
    }

    /// Get all extensions.
    pub fn get_all_extensions(&self) -> Vec<Extension> {
        self.extensions.read().values().cloned().collect()
    }

    /// Get enabled extensions.
    pub fn get_enabled_extensions(&self) -> Vec<Extension> {
        self.extensions
            .read()
            .values()
            .filter(|e| e.state == ExtensionState::Enabled)
            .cloned()
            .collect()
    }

    /// Add event listener.
    pub fn add_listener(&self, listener: ExtensionEventListener) {
        self.listeners.write().push(listener);
    }

    /// Emit event to listeners.
    fn emit_event(&self, event: ExtensionEvent) {
        for listener in self.listeners.read().iter() {
            listener(&event);
        }
    }
}

impl Default for ExtensionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionError {
    /// Extension not found.
    NotFound,
    /// Extension already installed.
    AlreadyInstalled,
    /// Invalid manifest.
    InvalidManifest,
    /// Permission denied.
    PermissionDenied,
    /// Installation failed.
    InstallationFailed,
    /// Update failed.
    UpdateFailed,
    /// Extension disabled.
    ExtensionDisabled,
}

/// Global extension manager instance (lazy initialized).
static EXTENSION_MANAGER: spin::Lazy<ExtensionManager> = spin::Lazy::new(ExtensionManager::new);

/// Get the global extension manager.
pub fn extension_manager() -> &'static ExtensionManager {
    &EXTENSION_MANAGER
}
