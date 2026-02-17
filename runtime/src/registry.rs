//! App Registry — persistent (in-memory MVP) registry of installed applications.
//!
//! The `AppRegistry` provides CRUD operations for managing installed KPIO apps.
//! In the MVP this is backed by a `BTreeMap`; future versions will persist to
//! the kernel's VFS.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::package::AppManifest;

/// Unique application identifier.
pub type AppId = String;

/// App registry — stores installed application manifests.
pub struct AppRegistry {
    /// Installed apps keyed by app ID.
    apps: BTreeMap<AppId, AppManifest>,
}

impl AppRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            apps: BTreeMap::new(),
        }
    }

    /// Register an app. Returns the app ID on success.
    ///
    /// Fails if the manifest is missing required fields or if an app
    /// with the same ID is already installed (use `update` instead).
    pub fn register(&mut self, manifest: AppManifest) -> Result<AppId, RegistryError> {
        // Validate required fields
        if manifest.id.is_empty() {
            return Err(RegistryError::InvalidManifest(String::from(
                "app id is empty",
            )));
        }
        if manifest.name.is_empty() {
            return Err(RegistryError::InvalidManifest(String::from(
                "app name is empty",
            )));
        }
        if manifest.version.is_empty() {
            return Err(RegistryError::InvalidManifest(String::from(
                "app version is empty",
            )));
        }

        if self.apps.contains_key(&manifest.id) {
            return Err(RegistryError::AlreadyInstalled(manifest.id.clone()));
        }

        let id = manifest.id.clone();
        self.apps.insert(id.clone(), manifest);
        Ok(id)
    }

    /// Unregister (remove) an app by ID.
    pub fn unregister(&mut self, app_id: &str) -> Result<(), RegistryError> {
        if self.apps.remove(app_id).is_none() {
            return Err(RegistryError::NotFound(String::from(app_id)));
        }
        Ok(())
    }

    /// Get an app manifest by ID.
    pub fn get(&self, app_id: &str) -> Option<&AppManifest> {
        self.apps.get(app_id)
    }

    /// List all installed app manifests.
    pub fn list(&self) -> Vec<&AppManifest> {
        self.apps.values().collect()
    }

    /// Check if an app is installed.
    pub fn is_installed(&self, app_id: &str) -> bool {
        self.apps.contains_key(app_id)
    }

    /// Number of installed apps.
    pub fn count(&self) -> usize {
        self.apps.len()
    }

    /// Update an existing app's manifest.
    /// Returns the previous manifest on success.
    pub fn update(&mut self, manifest: AppManifest) -> Result<AppManifest, RegistryError> {
        if !self.apps.contains_key(&manifest.id) {
            return Err(RegistryError::NotFound(manifest.id.clone()));
        }
        let old = self.apps.insert(manifest.id.clone(), manifest).unwrap();
        Ok(old)
    }
}

impl Default for AppRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// App with the same ID already installed.
    AlreadyInstalled(String),
    /// App not found.
    NotFound(String),
    /// Manifest validation failed.
    InvalidManifest(String),
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{FsPermission, ManifestPermissions};

    fn make_manifest(id: &str, name: &str, version: &str) -> AppManifest {
        AppManifest {
            id: String::from(id),
            name: String::from(name),
            version: String::from(version),
            description: None,
            author: None,
            icon: None,
            entry: String::from("app.wasm"),
            permissions: ManifestPermissions::default(),
            min_kpio_version: None,
        }
    }

    #[test]
    fn test_register_and_get() {
        let mut reg = AppRegistry::new();
        let manifest = make_manifest("com.test.app", "Test App", "1.0.0");
        let id = reg.register(manifest).unwrap();
        assert_eq!(id, "com.test.app");

        let m = reg.get("com.test.app").unwrap();
        assert_eq!(m.name, "Test App");
        assert_eq!(m.version, "1.0.0");
    }

    #[test]
    fn test_register_duplicate_error() {
        let mut reg = AppRegistry::new();
        reg.register(make_manifest("com.test.app", "App", "1.0.0"))
            .unwrap();
        let result = reg.register(make_manifest("com.test.app", "App2", "2.0.0"));
        assert!(matches!(result, Err(RegistryError::AlreadyInstalled(_))));
    }

    #[test]
    fn test_register_empty_id_error() {
        let mut reg = AppRegistry::new();
        let result = reg.register(make_manifest("", "App", "1.0.0"));
        assert!(matches!(result, Err(RegistryError::InvalidManifest(_))));
    }

    #[test]
    fn test_register_empty_name_error() {
        let mut reg = AppRegistry::new();
        let result = reg.register(make_manifest("com.test", "", "1.0.0"));
        assert!(matches!(result, Err(RegistryError::InvalidManifest(_))));
    }

    #[test]
    fn test_register_empty_version_error() {
        let mut reg = AppRegistry::new();
        let result = reg.register(make_manifest("com.test", "App", ""));
        assert!(matches!(result, Err(RegistryError::InvalidManifest(_))));
    }

    #[test]
    fn test_unregister() {
        let mut reg = AppRegistry::new();
        reg.register(make_manifest("com.test.app", "App", "1.0.0"))
            .unwrap();
        assert!(reg.is_installed("com.test.app"));
        reg.unregister("com.test.app").unwrap();
        assert!(!reg.is_installed("com.test.app"));
    }

    #[test]
    fn test_unregister_not_found() {
        let mut reg = AppRegistry::new();
        let result = reg.unregister("com.nonexistent");
        assert!(matches!(result, Err(RegistryError::NotFound(_))));
    }

    #[test]
    fn test_list() {
        let mut reg = AppRegistry::new();
        reg.register(make_manifest("com.a", "A", "1.0.0")).unwrap();
        reg.register(make_manifest("com.b", "B", "2.0.0")).unwrap();
        reg.register(make_manifest("com.c", "C", "3.0.0")).unwrap();

        let all = reg.list();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_is_installed() {
        let mut reg = AppRegistry::new();
        assert!(!reg.is_installed("com.test"));
        reg.register(make_manifest("com.test", "T", "1.0.0"))
            .unwrap();
        assert!(reg.is_installed("com.test"));
    }

    #[test]
    fn test_count() {
        let mut reg = AppRegistry::new();
        assert_eq!(reg.count(), 0);
        reg.register(make_manifest("com.a", "A", "1.0")).unwrap();
        assert_eq!(reg.count(), 1);
        reg.register(make_manifest("com.b", "B", "1.0")).unwrap();
        assert_eq!(reg.count(), 2);
        reg.unregister("com.a").unwrap();
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn test_update() {
        let mut reg = AppRegistry::new();
        reg.register(make_manifest("com.test", "App", "1.0.0"))
            .unwrap();
        let old = reg
            .update(make_manifest("com.test", "App Updated", "2.0.0"))
            .unwrap();
        assert_eq!(old.version, "1.0.0");
        let new = reg.get("com.test").unwrap();
        assert_eq!(new.version, "2.0.0");
        assert_eq!(new.name, "App Updated");
    }

    #[test]
    fn test_update_not_found() {
        let mut reg = AppRegistry::new();
        let result = reg.update(make_manifest("com.nonexistent", "X", "1.0.0"));
        assert!(matches!(result, Err(RegistryError::NotFound(_))));
    }
}
