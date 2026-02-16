//! Service Worker Lifecycle Management
//!
//! Handles service worker state transitions and lifecycle events.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use super::{ServiceWorker, ServiceWorkerError, ServiceWorkerId, ServiceWorkerState};

/// Lifecycle event types
#[derive(Debug, Clone)]
pub enum LifecycleEvent {
    /// Install event
    Install(InstallEvent),
    /// Activate event  
    Activate(ActivateEvent),
    /// Update found
    UpdateFound(UpdateFoundEvent),
    /// State change
    StateChange(StateChangeEvent),
    /// Controller change
    ControllerChange,
}

/// Install event data
#[derive(Debug, Clone)]
pub struct InstallEvent {
    /// Worker ID
    pub worker_id: ServiceWorkerId,
    /// Whether to wait until complete
    pub wait_until: bool,
}

impl InstallEvent {
    /// Create new install event
    pub fn new(worker_id: ServiceWorkerId) -> Self {
        Self {
            worker_id,
            wait_until: false,
        }
    }

    /// Mark as waiting until promises complete
    pub fn wait_until(&mut self) {
        self.wait_until = true;
    }
}

/// Activate event data
#[derive(Debug, Clone)]
pub struct ActivateEvent {
    /// Worker ID
    pub worker_id: ServiceWorkerId,
    /// Whether to wait until complete
    pub wait_until: bool,
}

impl ActivateEvent {
    /// Create new activate event
    pub fn new(worker_id: ServiceWorkerId) -> Self {
        Self {
            worker_id,
            wait_until: false,
        }
    }

    /// Mark as waiting until promises complete
    pub fn wait_until(&mut self) {
        self.wait_until = true;
    }
}

/// Update found event data
#[derive(Debug, Clone)]
pub struct UpdateFoundEvent {
    /// Scope path
    pub scope: String,
    /// New worker ID
    pub new_worker_id: ServiceWorkerId,
}

/// State change event data
#[derive(Debug, Clone)]
pub struct StateChangeEvent {
    /// Worker ID
    pub worker_id: ServiceWorkerId,
    /// Old state
    pub old_state: ServiceWorkerState,
    /// New state
    pub new_state: ServiceWorkerState,
}

/// Lifecycle manager
pub struct LifecycleManager {
    /// Pending events
    pending_events: Vec<LifecycleEvent>,
    /// Event listeners
    listeners: Vec<Box<dyn Fn(&LifecycleEvent) + Send + Sync>>,
}

impl LifecycleManager {
    /// Create new lifecycle manager
    pub fn new() -> Self {
        Self {
            pending_events: Vec::new(),
            listeners: Vec::new(),
        }
    }

    /// Add event listener
    pub fn add_listener(&mut self, listener: Box<dyn Fn(&LifecycleEvent) + Send + Sync>) {
        self.listeners.push(listener);
    }

    /// Dispatch an event
    pub fn dispatch(&mut self, event: LifecycleEvent) {
        for listener in &self.listeners {
            listener(&event);
        }
        self.pending_events.push(event);
    }

    /// Get pending events
    pub fn pending_events(&self) -> &[LifecycleEvent] {
        &self.pending_events
    }

    /// Clear pending events
    pub fn clear_pending(&mut self) {
        self.pending_events.clear();
    }

    /// Transition worker state
    pub fn transition_state(
        &mut self,
        worker: &mut ServiceWorker,
        new_state: ServiceWorkerState,
    ) -> Result<(), ServiceWorkerError> {
        let old_state = worker.state;

        // Validate transition
        if !is_valid_transition(old_state, new_state) {
            return Err(ServiceWorkerError::InvalidStateTransition);
        }

        // Apply transition
        worker.state = new_state;

        // Dispatch state change event
        self.dispatch(LifecycleEvent::StateChange(StateChangeEvent {
            worker_id: worker.id(),
            old_state,
            new_state,
        }));

        // Dispatch specific events
        match new_state {
            ServiceWorkerState::Installing => {
                self.dispatch(LifecycleEvent::Install(InstallEvent::new(worker.id())));
            }
            ServiceWorkerState::Activating => {
                self.dispatch(LifecycleEvent::Activate(ActivateEvent::new(worker.id())));
            }
            _ => {}
        }

        Ok(())
    }

    /// Skip waiting (immediately activate a waiting worker)
    pub fn skip_waiting(&mut self, worker: &mut ServiceWorker) -> Result<(), ServiceWorkerError> {
        if worker.state != ServiceWorkerState::Installed {
            return Err(ServiceWorkerError::InvalidStateTransition);
        }

        self.transition_state(worker, ServiceWorkerState::Activating)?;
        self.transition_state(worker, ServiceWorkerState::Activated)?;

        Ok(())
    }

    /// Claim clients (take control of all pages in scope)
    pub fn claim(&mut self, worker: &ServiceWorker) -> Result<(), ServiceWorkerError> {
        if worker.state != ServiceWorkerState::Activated {
            return Err(ServiceWorkerError::InvalidStateTransition);
        }

        // Dispatch controller change event
        self.dispatch(LifecycleEvent::ControllerChange);

        Ok(())
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a state transition is valid
fn is_valid_transition(from: ServiceWorkerState, to: ServiceWorkerState) -> bool {
    use ServiceWorkerState::*;

    matches!(
        (from, to),
        // Normal lifecycle
        (Parsed, Installing) |
        (Installing, Installed) |
        (Installing, Redundant) |  // Install failed
        (Installed, Activating) |
        (Activating, Activated) |
        (Activating, Redundant) |  // Activate failed
        (Activated, Redundant) |   // Replaced by new worker
        // Skip waiting
        (Installed, Activating)
    )
}

/// Update check result
#[derive(Debug, Clone)]
pub enum UpdateCheckResult {
    /// No update available
    NoUpdate,
    /// Update available, returning new script hash
    UpdateAvailable([u8; 32]),
    /// Update check failed
    Failed(String),
}

/// Perform update check for a service worker
pub fn check_for_update(_script_url: &str, current_hash: Option<[u8; 32]>) -> UpdateCheckResult {
    // In a real implementation, this would:
    // 1. Fetch the script from the network
    // 2. Compare the hash with current_hash
    // 3. Return UpdateAvailable if different

    // For now, return no update
    UpdateCheckResult::NoUpdate
}

/// Force update of a service worker
pub fn force_update(worker_id: ServiceWorkerId) -> Result<(), ServiceWorkerError> {
    // Would trigger an immediate update bypass cache
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service_worker::{Scope, ScriptUrl, ServiceWorkerConfig, UpdateViaCache};

    fn make_worker() -> ServiceWorker {
        let config = ServiceWorkerConfig {
            scope: Scope::new("/"),
            script_url: ScriptUrl::new("/sw.js"),
            update_via_cache: UpdateViaCache::default(),
            navigation_preload: false,
        };
        ServiceWorker::new(config)
    }

    #[test]
    fn test_valid_transition_parsed_to_installing() {
        let mut manager = LifecycleManager::new();
        let mut worker = make_worker();
        assert!(manager
            .transition_state(&mut worker, ServiceWorkerState::Installing)
            .is_ok());
        assert_eq!(worker.state(), ServiceWorkerState::Installing);
    }

    #[test]
    fn test_valid_transition_full_lifecycle() {
        let mut manager = LifecycleManager::new();
        let mut worker = make_worker();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Installing)
            .unwrap();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Installed)
            .unwrap();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Activating)
            .unwrap();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Activated)
            .unwrap();
        assert!(worker.is_active());
    }

    #[test]
    fn test_invalid_transition_parsed_to_activated() {
        let mut manager = LifecycleManager::new();
        let mut worker = make_worker();
        let result = manager.transition_state(&mut worker, ServiceWorkerState::Activated);
        assert!(matches!(
            result,
            Err(ServiceWorkerError::InvalidStateTransition)
        ));
    }

    #[test]
    fn test_invalid_transition_installing_to_activating() {
        let mut manager = LifecycleManager::new();
        let mut worker = make_worker();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Installing)
            .unwrap();
        // Cannot skip Installed
        let result = manager.transition_state(&mut worker, ServiceWorkerState::Activating);
        assert!(matches!(
            result,
            Err(ServiceWorkerError::InvalidStateTransition)
        ));
    }

    #[test]
    fn test_transition_to_redundant_from_installing() {
        let mut manager = LifecycleManager::new();
        let mut worker = make_worker();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Installing)
            .unwrap();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Redundant)
            .unwrap();
        assert_eq!(worker.state(), ServiceWorkerState::Redundant);
    }

    #[test]
    fn test_skip_waiting() {
        let mut manager = LifecycleManager::new();
        let mut worker = make_worker();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Installing)
            .unwrap();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Installed)
            .unwrap();
        manager.skip_waiting(&mut worker).unwrap();
        assert_eq!(worker.state(), ServiceWorkerState::Activated);
    }

    #[test]
    fn test_skip_waiting_wrong_state() {
        let mut manager = LifecycleManager::new();
        let mut worker = make_worker();
        // Worker is in Parsed state, skip_waiting should fail
        let result = manager.skip_waiting(&mut worker);
        assert!(result.is_err());
    }

    #[test]
    fn test_claim_activated_worker() {
        let mut manager = LifecycleManager::new();
        let mut worker = make_worker();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Installing)
            .unwrap();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Installed)
            .unwrap();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Activating)
            .unwrap();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Activated)
            .unwrap();
        assert!(manager.claim(&worker).is_ok());
    }

    #[test]
    fn test_claim_non_activated_worker() {
        let mut manager = LifecycleManager::new();
        let worker = make_worker();
        let result = manager.claim(&worker);
        assert!(matches!(
            result,
            Err(ServiceWorkerError::InvalidStateTransition)
        ));
    }

    #[test]
    fn test_pending_events_tracked() {
        let mut manager = LifecycleManager::new();
        let mut worker = make_worker();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Installing)
            .unwrap();
        // Should have StateChange + Install events
        assert!(manager.pending_events().len() >= 2);
    }

    #[test]
    fn test_clear_pending_events() {
        let mut manager = LifecycleManager::new();
        let mut worker = make_worker();
        manager
            .transition_state(&mut worker, ServiceWorkerState::Installing)
            .unwrap();
        manager.clear_pending();
        assert!(manager.pending_events().is_empty());
    }

    #[test]
    fn test_install_event_wait_until() {
        let id = ServiceWorkerId::new();
        let mut event = InstallEvent::new(id);
        assert!(!event.wait_until);
        event.wait_until();
        assert!(event.wait_until);
    }

    #[test]
    fn test_check_for_update_returns_no_update() {
        let result = check_for_update("/sw.js", None);
        assert!(matches!(result, UpdateCheckResult::NoUpdate));
    }
}
