//! Service Worker Module
//!
//! Implements Service Worker lifecycle management and event handling
//! for Progressive Web App support.

mod cache;
mod events;
mod fetch;
mod lifecycle;
mod registration;
mod sync;

pub use cache::*;
pub use events::*;
pub use fetch::*;
pub use lifecycle::*;
pub use registration::*;
pub use sync::*;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::RwLock;

/// Service Worker global ID counter
static NEXT_SW_ID: AtomicU64 = AtomicU64::new(1);

/// Service Worker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceWorkerState {
    /// Initial state, being parsed
    Parsed,
    /// Installing (install event fired)
    Installing,
    /// Installed, waiting to activate
    Installed,
    /// Activating (activate event fired)
    Activating,
    /// Active and controlling pages
    Activated,
    /// Marked for removal
    Redundant,
}

impl Default for ServiceWorkerState {
    fn default() -> Self {
        Self::Parsed
    }
}

/// Service Worker update state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateState {
    /// No update available
    None,
    /// Update checking in progress
    Checking,
    /// Update available
    Available,
    /// Update downloading
    Downloading,
    /// Update ready to install
    Ready,
}

/// Service Worker error types
#[derive(Debug, Clone)]
pub enum ServiceWorkerError {
    /// Registration failed
    RegistrationFailed(String),
    /// Script fetch failed
    ScriptFetchFailed(String),
    /// Script evaluation failed
    ScriptEvalFailed(String),
    /// State transition invalid
    InvalidStateTransition,
    /// Already registered
    AlreadyRegistered,
    /// Not found
    NotFound,
    /// Timeout during operation
    Timeout,
    /// Security error
    SecurityError(String),
    /// Quota exceeded
    QuotaExceeded,
    /// Network error
    NetworkError,
}

/// Service Worker ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ServiceWorkerId(u64);

impl ServiceWorkerId {
    /// Create a new unique ID
    pub fn new() -> Self {
        Self(NEXT_SW_ID.fetch_add(1, Ordering::SeqCst))
    }

    /// Get raw value
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl Default for ServiceWorkerId {
    fn default() -> Self {
        Self::new()
    }
}

/// Service Worker scope
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Scope(String);

impl Scope {
    /// Create a new scope
    pub fn new(path: impl Into<String>) -> Self {
        let mut path = path.into();
        if !path.ends_with('/') {
            path.push('/');
        }
        Self(path)
    }

    /// Get the path
    pub fn path(&self) -> &str {
        &self.0
    }

    /// Check if a URL is within this scope
    pub fn contains(&self, url: &str) -> bool {
        url.starts_with(&self.0)
    }
}

/// Service Worker script URL
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptUrl(String);

impl ScriptUrl {
    /// Create a new script URL
    pub fn new(url: impl Into<String>) -> Self {
        Self(url.into())
    }

    /// Get the URL
    pub fn url(&self) -> &str {
        &self.0
    }
}

/// Service Worker configuration
#[derive(Debug, Clone)]
pub struct ServiceWorkerConfig {
    /// The scope this worker controls
    pub scope: Scope,
    /// The script URL
    pub script_url: ScriptUrl,
    /// Update via cache mode
    pub update_via_cache: UpdateViaCache,
    /// Navigation preload enabled
    pub navigation_preload: bool,
}

/// Update via cache mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateViaCache {
    /// Import scripts cached, main script not
    Imports,
    /// All scripts cached
    All,
    /// No caching
    None,
}

impl Default for UpdateViaCache {
    fn default() -> Self {
        Self::Imports
    }
}

/// A Service Worker instance
pub struct ServiceWorker {
    /// Unique identifier
    id: ServiceWorkerId,
    /// Configuration
    config: ServiceWorkerConfig,
    /// Current state
    state: ServiceWorkerState,
    /// Update state
    update_state: UpdateState,
    /// Script version hash (for update detection)
    script_hash: Option<[u8; 32]>,
    /// Associated caches
    caches: Vec<String>,
    /// Install timestamp
    installed_at: Option<u64>,
    /// Activation timestamp
    activated_at: Option<u64>,
}

impl ServiceWorker {
    /// Create a new service worker
    pub fn new(config: ServiceWorkerConfig) -> Self {
        Self {
            id: ServiceWorkerId::new(),
            config,
            state: ServiceWorkerState::Parsed,
            update_state: UpdateState::None,
            script_hash: None,
            caches: Vec::new(),
            installed_at: None,
            activated_at: None,
        }
    }

    /// Get the worker ID
    pub fn id(&self) -> ServiceWorkerId {
        self.id
    }

    /// Get the scope
    pub fn scope(&self) -> &Scope {
        &self.config.scope
    }

    /// Get the script URL
    pub fn script_url(&self) -> &ScriptUrl {
        &self.config.script_url
    }

    /// Get current state
    pub fn state(&self) -> ServiceWorkerState {
        self.state
    }

    /// Check if the worker is active
    pub fn is_active(&self) -> bool {
        self.state == ServiceWorkerState::Activated
    }

    /// Check if the worker is installing
    pub fn is_installing(&self) -> bool {
        self.state == ServiceWorkerState::Installing
    }

    /// Check if the worker is waiting
    pub fn is_waiting(&self) -> bool {
        self.state == ServiceWorkerState::Installed
    }
}

/// Service Worker Container
///
/// Manages all service workers for an origin.
pub struct ServiceWorkerContainer {
    /// Origin this container belongs to
    origin: String,
    /// Registered service workers by scope
    registrations: BTreeMap<Scope, ServiceWorkerRegistration>,
    /// Active workers by ID
    workers: BTreeMap<ServiceWorkerId, ServiceWorker>,
    /// Controller for the current page (if any)
    controller: Option<ServiceWorkerId>,
}

impl ServiceWorkerContainer {
    /// Create a new container
    pub fn new(origin: impl Into<String>) -> Self {
        Self {
            origin: origin.into(),
            registrations: BTreeMap::new(),
            workers: BTreeMap::new(),
            controller: None,
        }
    }

    /// Get the origin
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Register a service worker
    pub fn register(
        &mut self,
        script_url: &str,
        scope: Option<&str>,
    ) -> Result<ServiceWorkerId, ServiceWorkerError> {
        // Determine scope
        let scope = match scope {
            Some(s) => Scope::new(s),
            None => {
                // Default scope is the directory of the script
                let mut default_scope = script_url.to_string();
                if let Some(pos) = default_scope.rfind('/') {
                    default_scope.truncate(pos + 1);
                }
                Scope::new(default_scope)
            }
        };

        // Check if already registered
        if self.registrations.contains_key(&scope) {
            return Err(ServiceWorkerError::AlreadyRegistered);
        }

        // Create configuration
        let config = ServiceWorkerConfig {
            scope: scope.clone(),
            script_url: ScriptUrl::new(script_url),
            update_via_cache: UpdateViaCache::default(),
            navigation_preload: false,
        };

        // Create worker
        let worker = ServiceWorker::new(config);
        let id = worker.id();

        // Create registration
        let registration =
            ServiceWorkerRegistration::new(id, scope.clone(), ScriptUrl::new(script_url));

        // Store
        self.registrations.insert(scope, registration);
        self.workers.insert(id, worker);

        Ok(id)
    }

    /// Get registration for a scope
    pub fn get_registration(&self, scope: &Scope) -> Option<&ServiceWorkerRegistration> {
        self.registrations.get(scope)
    }

    /// Get all registrations
    pub fn get_registrations(&self) -> impl Iterator<Item = &ServiceWorkerRegistration> {
        self.registrations.values()
    }

    /// Get worker by ID
    pub fn get_worker(&self, id: ServiceWorkerId) -> Option<&ServiceWorker> {
        self.workers.get(&id)
    }

    /// Get mutable worker by ID
    pub fn get_worker_mut(&mut self, id: ServiceWorkerId) -> Option<&mut ServiceWorker> {
        self.workers.get_mut(&id)
    }

    /// Get the controller for the current page
    pub fn controller(&self) -> Option<&ServiceWorker> {
        self.controller.and_then(|id| self.workers.get(&id))
    }

    /// Set the controller
    pub fn set_controller(&mut self, id: ServiceWorkerId) {
        if self.workers.contains_key(&id) {
            self.controller = Some(id);
        }
    }

    /// Unregister a service worker
    pub fn unregister(&mut self, scope: &Scope) -> Result<(), ServiceWorkerError> {
        if let Some(registration) = self.registrations.remove(scope) {
            // Mark worker as redundant
            if let Some(worker) = self.workers.get_mut(&registration.worker_id()) {
                worker.state = ServiceWorkerState::Redundant;
            }

            // Clear controller if it was this worker
            if self.controller == Some(registration.worker_id()) {
                self.controller = None;
            }

            Ok(())
        } else {
            Err(ServiceWorkerError::NotFound)
        }
    }

    /// Find matching registration for a URL
    pub fn match_registration(&self, url: &str) -> Option<&ServiceWorkerRegistration> {
        // Find the longest matching scope
        self.registrations
            .iter()
            .filter(|(scope, _)| scope.contains(url))
            .max_by_key(|(scope, _)| scope.path().len())
            .map(|(_, reg)| reg)
    }
}

/// Global service worker manager
pub struct ServiceWorkerManager {
    /// Containers by origin
    containers: BTreeMap<String, ServiceWorkerContainer>,
}

impl ServiceWorkerManager {
    /// Create a new manager
    pub const fn new() -> Self {
        Self {
            containers: BTreeMap::new(),
        }
    }

    /// Get or create container for an origin
    pub fn container(&mut self, origin: &str) -> &mut ServiceWorkerContainer {
        if !self.containers.contains_key(origin) {
            self.containers
                .insert(origin.to_string(), ServiceWorkerContainer::new(origin));
        }
        self.containers.get_mut(origin).unwrap()
    }

    /// Get container for an origin (immutable)
    pub fn get_container(&self, origin: &str) -> Option<&ServiceWorkerContainer> {
        self.containers.get(origin)
    }

    /// Check if any service worker is controlling a URL
    pub fn get_controller_for_url(&self, origin: &str, url: &str) -> Option<&ServiceWorker> {
        self.containers
            .get(origin)
            .and_then(|c| c.match_registration(url))
            .and_then(|reg| {
                self.containers
                    .get(origin)
                    .and_then(|c| c.get_worker(reg.worker_id()))
            })
            .filter(|w| w.is_active())
    }
}

impl Default for ServiceWorkerManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global manager instance
pub static SERVICE_WORKER_MANAGER: RwLock<ServiceWorkerManager> =
    RwLock::new(ServiceWorkerManager::new());

/// Initialize service worker subsystem
pub fn init() {
    // Any initialization needed
}

use alloc::string::ToString;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_contains() {
        let scope = Scope::new("/app");
        assert!(scope.contains("/app/index.html"));
        assert!(scope.contains("/app/sub/page.html"));
        assert!(!scope.contains("/other/page.html"));
    }

    #[test]
    fn test_scope_auto_trailing_slash() {
        let scope = Scope::new("/app");
        assert_eq!(scope.path(), "/app/");
        let scope2 = Scope::new("/app/");
        assert_eq!(scope2.path(), "/app/");
    }

    #[test]
    fn test_service_worker_initial_state() {
        let config = ServiceWorkerConfig {
            scope: Scope::new("/"),
            script_url: ScriptUrl::new("/sw.js"),
            update_via_cache: UpdateViaCache::default(),
            navigation_preload: false,
        };
        let worker = ServiceWorker::new(config);
        assert_eq!(worker.state(), ServiceWorkerState::Parsed);
        assert!(!worker.is_active());
        assert!(!worker.is_installing());
        assert!(!worker.is_waiting());
    }

    #[test]
    fn test_container_register_and_get() {
        let mut container = ServiceWorkerContainer::new("https://example.com");
        let id = container.register("/sw.js", Some("/app")).unwrap();
        assert!(container.get_worker(id).is_some());
        let scope = Scope::new("/app");
        assert!(container.get_registration(&scope).is_some());
    }

    #[test]
    fn test_container_register_duplicate_fails() {
        let mut container = ServiceWorkerContainer::new("https://example.com");
        container.register("/sw.js", Some("/app")).unwrap();
        let result = container.register("/sw2.js", Some("/app"));
        assert!(matches!(result, Err(ServiceWorkerError::AlreadyRegistered)));
    }

    #[test]
    fn test_container_unregister() {
        let mut container = ServiceWorkerContainer::new("https://example.com");
        let id = container.register("/sw.js", Some("/app")).unwrap();
        let scope = Scope::new("/app");
        container.unregister(&scope).unwrap();
        // Worker should still exist but be Redundant
        let worker = container.get_worker(id).unwrap();
        assert_eq!(worker.state(), ServiceWorkerState::Redundant);
    }

    #[test]
    fn test_container_unregister_not_found() {
        let mut container = ServiceWorkerContainer::new("https://example.com");
        let scope = Scope::new("/nonexistent");
        assert!(matches!(
            container.unregister(&scope),
            Err(ServiceWorkerError::NotFound)
        ));
    }

    #[test]
    fn test_container_match_registration() {
        let mut container = ServiceWorkerContainer::new("https://example.com");
        container.register("/sw.js", Some("/app")).unwrap();
        // A URL within the scope should match
        let reg = container.match_registration("/app/page.html");
        assert!(reg.is_some());
        // A URL outside the scope should not match
        let reg = container.match_registration("/other/page.html");
        assert!(reg.is_none());
    }

    #[test]
    fn test_container_set_controller() {
        let mut container = ServiceWorkerContainer::new("https://example.com");
        let id = container.register("/sw.js", Some("/")).unwrap();
        assert!(container.controller().is_none());
        container.set_controller(id);
        assert!(container.controller().is_some());
    }

    #[test]
    fn test_manager_container_creation() {
        let mut manager = ServiceWorkerManager::new();
        let container = manager.container("https://example.com");
        assert_eq!(container.origin(), "https://example.com");
        // Accessing same origin should return same container
        let container2 = manager.container("https://example.com");
        assert_eq!(container2.origin(), "https://example.com");
    }

    #[test]
    fn test_service_worker_id_unique() {
        let id1 = ServiceWorkerId::new();
        let id2 = ServiceWorkerId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_script_url() {
        let url = ScriptUrl::new("/service-worker.js");
        assert_eq!(url.url(), "/service-worker.js");
    }
}
