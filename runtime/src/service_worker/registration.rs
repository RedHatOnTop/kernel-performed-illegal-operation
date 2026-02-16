//! Service Worker Registration
//!
//! Manages the registration state of service workers.

use alloc::string::String;
use alloc::vec::Vec;

use super::{Scope, ScriptUrl, ServiceWorkerId, UpdateState};

/// A service worker registration
///
/// Represents a registration between a service worker and its scope.
#[derive(Debug)]
pub struct ServiceWorkerRegistration {
    /// The worker ID
    worker_id: ServiceWorkerId,
    /// The scope
    scope: Scope,
    /// The script URL
    script_url: ScriptUrl,
    /// Update state
    update_state: UpdateState,
    /// Installing worker (if any)
    installing: Option<ServiceWorkerId>,
    /// Waiting worker (if any)
    waiting: Option<ServiceWorkerId>,
    /// Active worker (if any)
    active: Option<ServiceWorkerId>,
    /// Navigation preload state
    navigation_preload: NavigationPreloadState,
    /// Push subscription endpoint
    push_endpoint: Option<String>,
    /// Sync tags registered
    sync_tags: Vec<String>,
}

impl ServiceWorkerRegistration {
    /// Create a new registration
    pub fn new(worker_id: ServiceWorkerId, scope: Scope, script_url: ScriptUrl) -> Self {
        Self {
            worker_id,
            scope,
            script_url,
            update_state: UpdateState::None,
            installing: None,
            waiting: None,
            active: None,
            navigation_preload: NavigationPreloadState::default(),
            push_endpoint: None,
            sync_tags: Vec::new(),
        }
    }

    /// Get the worker ID
    pub fn worker_id(&self) -> ServiceWorkerId {
        self.worker_id
    }

    /// Get the scope
    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    /// Get the script URL
    pub fn script_url(&self) -> &ScriptUrl {
        &self.script_url
    }

    /// Get update state
    pub fn update_state(&self) -> UpdateState {
        self.update_state
    }

    /// Get installing worker
    pub fn installing(&self) -> Option<ServiceWorkerId> {
        self.installing
    }

    /// Set installing worker
    pub fn set_installing(&mut self, id: Option<ServiceWorkerId>) {
        self.installing = id;
    }

    /// Get waiting worker
    pub fn waiting(&self) -> Option<ServiceWorkerId> {
        self.waiting
    }

    /// Set waiting worker
    pub fn set_waiting(&mut self, id: Option<ServiceWorkerId>) {
        self.waiting = id;
    }

    /// Get active worker
    pub fn active(&self) -> Option<ServiceWorkerId> {
        self.active
    }

    /// Set active worker
    pub fn set_active(&mut self, id: Option<ServiceWorkerId>) {
        self.active = id;
    }

    /// Trigger an update check
    pub fn update(&mut self) {
        self.update_state = UpdateState::Checking;
    }

    /// Unregister this registration
    pub fn unregister(&mut self) {
        self.installing = None;
        self.waiting = None;
        self.active = None;
    }

    /// Get navigation preload state
    pub fn navigation_preload(&self) -> &NavigationPreloadState {
        &self.navigation_preload
    }

    /// Get navigation preload state mutably
    pub fn navigation_preload_mut(&mut self) -> &mut NavigationPreloadState {
        &mut self.navigation_preload
    }

    /// Get push endpoint
    pub fn push_endpoint(&self) -> Option<&str> {
        self.push_endpoint.as_deref()
    }

    /// Set push endpoint
    pub fn set_push_endpoint(&mut self, endpoint: Option<String>) {
        self.push_endpoint = endpoint;
    }

    /// Get sync tags
    pub fn sync_tags(&self) -> &[String] {
        &self.sync_tags
    }

    /// Register a sync tag
    pub fn register_sync(&mut self, tag: String) {
        if !self.sync_tags.contains(&tag) {
            self.sync_tags.push(tag);
        }
    }

    /// Unregister a sync tag
    pub fn unregister_sync(&mut self, tag: &str) {
        self.sync_tags.retain(|t| t != tag);
    }
}

/// Navigation preload state
#[derive(Debug, Clone)]
pub struct NavigationPreloadState {
    /// Whether navigation preload is enabled
    enabled: bool,
    /// Custom header value
    header_value: Option<String>,
}

impl Default for NavigationPreloadState {
    fn default() -> Self {
        Self {
            enabled: false,
            header_value: Some("true".into()),
        }
    }
}

impl NavigationPreloadState {
    /// Create new state
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if enabled
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Enable navigation preload
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable navigation preload
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Get header value
    pub fn header_value(&self) -> Option<&str> {
        self.header_value.as_deref()
    }

    /// Set header value
    pub fn set_header_value(&mut self, value: impl Into<String>) {
        self.header_value = Some(value.into());
    }

    /// Get state
    pub fn get_state(&self) -> (bool, Option<&str>) {
        (self.enabled, self.header_value.as_deref())
    }
}

/// Registration options
#[derive(Debug, Clone)]
pub struct RegistrationOptions {
    /// The scope
    pub scope: Option<String>,
    /// Script type
    pub script_type: WorkerType,
    /// Update via cache
    pub update_via_cache: super::UpdateViaCache,
}

impl Default for RegistrationOptions {
    fn default() -> Self {
        Self {
            scope: None,
            script_type: WorkerType::Classic,
            update_via_cache: super::UpdateViaCache::Imports,
        }
    }
}

/// Worker script type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerType {
    /// Classic script
    Classic,
    /// ES module
    Module,
}

impl Default for WorkerType {
    fn default() -> Self {
        Self::Classic
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service_worker::{Scope, ScriptUrl, ServiceWorkerId, UpdateState};

    fn make_registration() -> ServiceWorkerRegistration {
        let id = ServiceWorkerId::new();
        let scope = Scope::new("/app");
        let url = ScriptUrl::new("/sw.js");
        ServiceWorkerRegistration::new(id, scope, url)
    }

    #[test]
    fn test_registration_creation() {
        let reg = make_registration();
        assert_eq!(reg.scope().path(), "/app/");
        assert_eq!(reg.script_url().url(), "/sw.js");
        assert_eq!(reg.update_state(), UpdateState::None);
        assert!(reg.installing().is_none());
        assert!(reg.waiting().is_none());
        assert!(reg.active().is_none());
    }

    #[test]
    fn test_set_installing_waiting_active() {
        let mut reg = make_registration();
        let id = ServiceWorkerId::new();
        reg.set_installing(Some(id));
        assert_eq!(reg.installing(), Some(id));
        reg.set_waiting(Some(id));
        assert_eq!(reg.waiting(), Some(id));
        reg.set_active(Some(id));
        assert_eq!(reg.active(), Some(id));
    }

    #[test]
    fn test_unregister_clears_workers() {
        let mut reg = make_registration();
        let id = ServiceWorkerId::new();
        reg.set_installing(Some(id));
        reg.set_waiting(Some(id));
        reg.set_active(Some(id));
        reg.unregister();
        assert!(reg.installing().is_none());
        assert!(reg.waiting().is_none());
        assert!(reg.active().is_none());
    }

    #[test]
    fn test_update_sets_checking() {
        let mut reg = make_registration();
        reg.update();
        assert_eq!(reg.update_state(), UpdateState::Checking);
    }

    #[test]
    fn test_navigation_preload_default() {
        let state = NavigationPreloadState::new();
        assert!(!state.enabled());
        assert_eq!(state.header_value(), Some("true"));
    }

    #[test]
    fn test_navigation_preload_enable_disable() {
        let mut state = NavigationPreloadState::new();
        state.enable();
        assert!(state.enabled());
        state.disable();
        assert!(!state.enabled());
    }

    #[test]
    fn test_navigation_preload_header_value() {
        let mut state = NavigationPreloadState::new();
        state.set_header_value("custom-value");
        assert_eq!(state.header_value(), Some("custom-value"));
    }

    #[test]
    fn test_sync_tags_register_dedup() {
        let mut reg = make_registration();
        reg.register_sync("tag1".into());
        reg.register_sync("tag1".into()); // duplicate
        reg.register_sync("tag2".into());
        assert_eq!(reg.sync_tags().len(), 2);
    }

    #[test]
    fn test_sync_tags_unregister() {
        let mut reg = make_registration();
        reg.register_sync("tag1".into());
        reg.register_sync("tag2".into());
        reg.unregister_sync("tag1");
        assert_eq!(reg.sync_tags().len(), 1);
        assert_eq!(reg.sync_tags()[0], "tag2");
    }

    #[test]
    fn test_push_endpoint() {
        let mut reg = make_registration();
        assert!(reg.push_endpoint().is_none());
        reg.set_push_endpoint(Some("https://push.example.com/endpoint".into()));
        assert_eq!(
            reg.push_endpoint(),
            Some("https://push.example.com/endpoint")
        );
    }

    #[test]
    fn test_registration_options_default() {
        let opts = RegistrationOptions::default();
        assert!(opts.scope.is_none());
        assert_eq!(opts.script_type, WorkerType::Classic);
    }
}
