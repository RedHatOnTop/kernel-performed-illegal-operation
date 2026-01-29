//! Background Sync API
//!
//! Implements background sync for offline-first applications.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::RwLock;

use super::ServiceWorkerId;

/// Sync event ID counter
static NEXT_SYNC_ID: AtomicU64 = AtomicU64::new(1);

/// Sync registration ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SyncId(u64);

impl SyncId {
    fn new() -> Self {
        Self(NEXT_SYNC_ID.fetch_add(1, Ordering::SeqCst))
    }
}

/// Sync registration state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    /// Pending (waiting for connectivity)
    Pending,
    /// Firing (sync event being dispatched)
    Firing,
    /// Reregistering (failed, will retry)
    Reregistering,
    /// Success (completed successfully)
    Success,
    /// Failed (max retries exceeded)
    Failed,
}

impl Default for SyncState {
    fn default() -> Self {
        Self::Pending
    }
}

/// Sync registration
#[derive(Debug, Clone)]
pub struct SyncRegistration {
    /// Registration ID
    id: SyncId,
    /// Tag (unique identifier within scope)
    tag: String,
    /// Current state
    state: SyncState,
    /// Retry count
    retry_count: u32,
    /// Max retries
    max_retries: u32,
    /// Minimum delay between retries (ms)
    min_retry_delay: u64,
    /// Whether last attempt failed
    last_chance: bool,
    /// Created timestamp
    created_at: u64,
    /// Last fired timestamp
    last_fired_at: Option<u64>,
}

impl SyncRegistration {
    /// Create new registration
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            id: SyncId::new(),
            tag: tag.into(),
            state: SyncState::Pending,
            retry_count: 0,
            max_retries: 3,
            min_retry_delay: 5 * 60 * 1000, // 5 minutes
            last_chance: false,
            created_at: 0, // Would use actual timestamp
            last_fired_at: None,
        }
    }

    /// Get ID
    pub fn id(&self) -> SyncId {
        self.id
    }

    /// Get tag
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Get state
    pub fn state(&self) -> SyncState {
        self.state
    }

    /// Check if this is the last chance
    pub fn last_chance(&self) -> bool {
        self.last_chance
    }

    /// Mark as firing
    pub fn mark_firing(&mut self) {
        self.state = SyncState::Firing;
        self.last_fired_at = Some(0); // Would use actual timestamp
    }

    /// Mark as success
    pub fn mark_success(&mut self) {
        self.state = SyncState::Success;
    }

    /// Mark as failed and potentially reregister
    pub fn mark_failed(&mut self) -> bool {
        self.retry_count += 1;
        if self.retry_count >= self.max_retries {
            self.last_chance = true;
            self.state = SyncState::Firing; // One last try
            false
        } else {
            self.state = SyncState::Reregistering;
            true
        }
    }

    /// Mark as permanently failed
    pub fn mark_permanently_failed(&mut self) {
        self.state = SyncState::Failed;
    }
}

/// Sync event
#[derive(Debug, Clone)]
pub struct SyncEvent {
    /// Registration tag
    pub tag: String,
    /// Whether this is the last chance
    pub last_chance: bool,
    /// Whether wait_until was called
    pub wait_until: bool,
}

impl SyncEvent {
    /// Create new sync event
    pub fn new(tag: impl Into<String>, last_chance: bool) -> Self {
        Self {
            tag: tag.into(),
            last_chance,
            wait_until: false,
        }
    }

    /// Mark as waiting
    pub fn wait_until(&mut self) {
        self.wait_until = true;
    }
}

/// Sync manager for a service worker
pub struct SyncManager {
    /// Worker ID
    worker_id: ServiceWorkerId,
    /// Registrations by tag
    registrations: BTreeMap<String, SyncRegistration>,
}

impl SyncManager {
    /// Create new sync manager
    pub fn new(worker_id: ServiceWorkerId) -> Self {
        Self {
            worker_id,
            registrations: BTreeMap::new(),
        }
    }

    /// Get worker ID
    pub fn worker_id(&self) -> ServiceWorkerId {
        self.worker_id
    }

    /// Register a sync
    pub fn register(&mut self, tag: impl Into<String>) -> SyncId {
        let tag = tag.into();
        if let Some(existing) = self.registrations.get(&tag) {
            return existing.id();
        }

        let registration = SyncRegistration::new(tag.clone());
        let id = registration.id();
        self.registrations.insert(tag, registration);
        id
    }

    /// Get a registration by tag
    pub fn get(&self, tag: &str) -> Option<&SyncRegistration> {
        self.registrations.get(tag)
    }

    /// Get mutable registration
    pub fn get_mut(&mut self, tag: &str) -> Option<&mut SyncRegistration> {
        self.registrations.get_mut(tag)
    }

    /// Get all tags
    pub fn get_tags(&self) -> Vec<String> {
        self.registrations.keys().cloned().collect()
    }

    /// Unregister a sync
    pub fn unregister(&mut self, tag: &str) -> bool {
        self.registrations.remove(tag).is_some()
    }

    /// Get pending syncs
    pub fn pending(&self) -> impl Iterator<Item = &SyncRegistration> {
        self.registrations.values().filter(|r| r.state == SyncState::Pending)
    }

    /// Fire pending syncs
    pub fn fire_pending(&mut self) -> Vec<SyncEvent> {
        let mut events = Vec::new();

        for registration in self.registrations.values_mut() {
            if registration.state == SyncState::Pending 
                || registration.state == SyncState::Reregistering 
            {
                registration.mark_firing();
                events.push(SyncEvent::new(
                    registration.tag.clone(),
                    registration.last_chance,
                ));
            }
        }

        events
    }

    /// Complete a sync
    pub fn complete(&mut self, tag: &str, success: bool) {
        if let Some(registration) = self.registrations.get_mut(tag) {
            if success {
                registration.mark_success();
                // Remove completed registrations
                self.registrations.remove(tag);
            } else {
                if registration.last_chance {
                    registration.mark_permanently_failed();
                    self.registrations.remove(tag);
                } else {
                    registration.mark_failed();
                }
            }
        }
    }
}

/// Periodic sync options
#[derive(Debug, Clone)]
pub struct PeriodicSyncOptions {
    /// Minimum interval in milliseconds
    pub min_interval: u64,
}

impl Default for PeriodicSyncOptions {
    fn default() -> Self {
        Self {
            min_interval: 24 * 60 * 60 * 1000, // 24 hours
        }
    }
}

/// Periodic sync registration
#[derive(Debug, Clone)]
pub struct PeriodicSyncRegistration {
    /// Tag
    tag: String,
    /// Minimum interval
    min_interval: u64,
    /// Next fire time
    next_fire_time: u64,
    /// Last successful sync
    last_success: Option<u64>,
}

impl PeriodicSyncRegistration {
    /// Create new periodic sync
    pub fn new(tag: impl Into<String>, options: PeriodicSyncOptions) -> Self {
        Self {
            tag: tag.into(),
            min_interval: options.min_interval,
            next_fire_time: 0, // Would calculate based on current time
            last_success: None,
        }
    }

    /// Get tag
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Get minimum interval
    pub fn min_interval(&self) -> u64 {
        self.min_interval
    }

    /// Check if ready to fire
    pub fn ready_to_fire(&self, current_time: u64) -> bool {
        current_time >= self.next_fire_time
    }

    /// Mark as successful
    pub fn mark_success(&mut self, current_time: u64) {
        self.last_success = Some(current_time);
        self.next_fire_time = current_time + self.min_interval;
    }
}

/// Periodic sync manager
pub struct PeriodicSyncManager {
    /// Worker ID
    worker_id: ServiceWorkerId,
    /// Registrations
    registrations: BTreeMap<String, PeriodicSyncRegistration>,
}

impl PeriodicSyncManager {
    /// Create new manager
    pub fn new(worker_id: ServiceWorkerId) -> Self {
        Self {
            worker_id,
            registrations: BTreeMap::new(),
        }
    }

    /// Register a periodic sync
    pub fn register(&mut self, tag: impl Into<String>, options: PeriodicSyncOptions) {
        let tag = tag.into();
        let registration = PeriodicSyncRegistration::new(tag.clone(), options);
        self.registrations.insert(tag, registration);
    }

    /// Unregister
    pub fn unregister(&mut self, tag: &str) -> bool {
        self.registrations.remove(tag).is_some()
    }

    /// Get tags
    pub fn get_tags(&self) -> Vec<String> {
        self.registrations.keys().cloned().collect()
    }

    /// Get ready syncs
    pub fn get_ready(&self, current_time: u64) -> Vec<&PeriodicSyncRegistration> {
        self.registrations
            .values()
            .filter(|r| r.ready_to_fire(current_time))
            .collect()
    }
}

/// Global sync manager registry
pub struct SyncManagerRegistry {
    /// Managers by worker ID
    managers: BTreeMap<ServiceWorkerId, SyncManager>,
    /// Periodic managers by worker ID
    periodic_managers: BTreeMap<ServiceWorkerId, PeriodicSyncManager>,
}

impl SyncManagerRegistry {
    /// Create new registry
    pub const fn new() -> Self {
        Self {
            managers: BTreeMap::new(),
            periodic_managers: BTreeMap::new(),
        }
    }

    /// Get or create sync manager
    pub fn get_or_create(&mut self, worker_id: ServiceWorkerId) -> &mut SyncManager {
        if !self.managers.contains_key(&worker_id) {
            self.managers.insert(worker_id, SyncManager::new(worker_id));
        }
        self.managers.get_mut(&worker_id).unwrap()
    }

    /// Get or create periodic sync manager
    pub fn get_or_create_periodic(&mut self, worker_id: ServiceWorkerId) -> &mut PeriodicSyncManager {
        if !self.periodic_managers.contains_key(&worker_id) {
            self.periodic_managers.insert(worker_id, PeriodicSyncManager::new(worker_id));
        }
        self.periodic_managers.get_mut(&worker_id).unwrap()
    }

    /// Remove managers for a worker
    pub fn remove(&mut self, worker_id: ServiceWorkerId) {
        self.managers.remove(&worker_id);
        self.periodic_managers.remove(&worker_id);
    }
}

impl Default for SyncManagerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global registry
pub static SYNC_REGISTRY: RwLock<SyncManagerRegistry> = RwLock::new(SyncManagerRegistry::new());
