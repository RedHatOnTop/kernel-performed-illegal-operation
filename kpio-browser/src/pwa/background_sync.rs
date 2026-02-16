//! Background Sync
//!
//! Implements the Background Sync API:
//! - `SyncManager.register(tag)` — schedule a sync task
//! - On network reconnection, dispatch `sync` events to SWs
//! - Retry with backoff: 30s → 60s → 300s, max 3 attempts
//! - Task persistence at `/apps/data/{app_id}/sync_tasks.json`

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

// ── Constants ───────────────────────────────────────────────

/// Maximum retry attempts before discarding a sync task.
const MAX_RETRIES: u32 = 3;

/// Retry delays in seconds: [30, 60, 300].
const RETRY_DELAYS: [u64; 3] = [30, 60, 300];

// ── Types ───────────────────────────────────────────────────

/// Network connectivity state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkState {
    /// Connected to network.
    Online,
    /// No network connectivity.
    Offline,
}

/// A registered sync task.
#[derive(Debug, Clone)]
pub struct SyncTask {
    /// The sync tag.
    pub tag: String,
    /// App ID.
    pub app_id: u64,
    /// Number of times this task has been attempted.
    pub attempts: u32,
    /// Whether the task is currently pending (waiting for next attempt).
    pub pending: bool,
    /// Frame count until next retry (0 = ready to fire).
    pub retry_delay_remaining: u64,
}

/// Result of a sync event dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncResult {
    /// Sync event handled successfully.
    Success,
    /// Sync event handler failed — will retry.
    Failed,
}

/// Background sync manager for a single app.
pub struct SyncManager {
    /// App ID.
    app_id: u64,
    /// Registered sync tasks: tag → SyncTask.
    tasks: BTreeMap<String, SyncTask>,
    /// Current network state.
    network_state: NetworkState,
    /// Whether the network state just changed from offline to online.
    just_came_online: bool,
}

impl SyncManager {
    /// Create a new sync manager for an app.
    pub fn new(app_id: u64) -> Self {
        Self {
            app_id,
            tasks: BTreeMap::new(),
            network_state: NetworkState::Online,
            just_came_online: false,
        }
    }

    /// Register a sync task with the given tag.
    pub fn register(&mut self, tag: &str) -> Result<(), &'static str> {
        if self.tasks.contains_key(tag) {
            // Already registered — reset attempts
            if let Some(task) = self.tasks.get_mut(tag) {
                task.attempts = 0;
                task.pending = true;
                task.retry_delay_remaining = 0;
            }
            return Ok(());
        }

        self.tasks.insert(
            String::from(tag),
            SyncTask {
                tag: String::from(tag),
                app_id: self.app_id,
                attempts: 0,
                pending: true,
                retry_delay_remaining: 0,
            },
        );
        Ok(())
    }

    /// Unregister a sync task.
    pub fn unregister(&mut self, tag: &str) -> bool {
        self.tasks.remove(tag).is_some()
    }

    /// Get all registered tags.
    pub fn get_tags(&self) -> Vec<&str> {
        self.tasks.keys().map(|s| s.as_str()).collect()
    }

    /// Number of registered tasks.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Update network state. If transitioning Offline → Online, sets flag.
    pub fn update_network_state(&mut self, state: NetworkState) {
        if self.network_state == NetworkState::Offline && state == NetworkState::Online {
            self.just_came_online = true;
            // Reset retry delays for pending tasks
            for task in self.tasks.values_mut() {
                if task.pending {
                    task.retry_delay_remaining = 0;
                }
            }
        }
        self.network_state = state;
    }

    /// Current network state.
    pub fn network_state(&self) -> NetworkState {
        self.network_state
    }

    /// Check if we just came online (and consume the flag).
    pub fn consume_online_transition(&mut self) -> bool {
        if self.just_came_online {
            self.just_came_online = false;
            true
        } else {
            false
        }
    }

    /// Get tasks that are ready to fire (pending, delay expired, online).
    pub fn ready_tasks(&self) -> Vec<&SyncTask> {
        if self.network_state != NetworkState::Online {
            return Vec::new();
        }
        self.tasks
            .values()
            .filter(|t| t.pending && t.retry_delay_remaining == 0)
            .collect()
    }

    /// Report the result of a sync event dispatch.
    pub fn report_result(&mut self, tag: &str, result: SyncResult) {
        if let Some(task) = self.tasks.get_mut(tag) {
            match result {
                SyncResult::Success => {
                    // Task completed — remove it
                    self.tasks.remove(tag);
                }
                SyncResult::Failed => {
                    task.attempts += 1;
                    if task.attempts >= MAX_RETRIES {
                        // Discard after max retries
                        self.tasks.remove(tag);
                    } else {
                        // Schedule retry with backoff
                        let delay_idx = (task.attempts as usize - 1).min(RETRY_DELAYS.len() - 1);
                        // Convert seconds to approximate frames (60fps)
                        task.retry_delay_remaining = RETRY_DELAYS[delay_idx] * 60;
                    }
                }
            }
        }
    }

    /// Tick — decrement retry delays.
    pub fn tick(&mut self) {
        for task in self.tasks.values_mut() {
            if task.retry_delay_remaining > 0 {
                task.retry_delay_remaining -= 1;
            }
        }
    }

    /// Serialize tasks to JSON for VFS persistence.
    pub fn to_json(&self) -> String {
        let mut json = String::from("[");
        let mut first = true;
        for task in self.tasks.values() {
            if !first {
                json.push(',');
            }
            json.push_str(&alloc::format!(
                "{{\"tag\":\"{}\",\"attempts\":{},\"pending\":{}}}",
                task.tag,
                task.attempts,
                task.pending
            ));
            first = false;
        }
        json.push(']');
        json
    }

    /// VFS path for this app's sync tasks.
    pub fn vfs_path(&self) -> String {
        alloc::format!("/apps/data/{}/sync_tasks.json", self.app_id)
    }

    /// App ID.
    pub fn app_id(&self) -> u64 {
        self.app_id
    }
}

// ── Global Registry ─────────────────────────────────────────

/// Global registry mapping app_id → SyncManager.
pub struct SyncRegistry {
    managers: BTreeMap<u64, SyncManager>,
}

impl SyncRegistry {
    pub const fn new() -> Self {
        Self {
            managers: BTreeMap::new(),
        }
    }

    /// Get or create a sync manager for an app.
    pub fn get_or_create(&mut self, app_id: u64) -> &mut SyncManager {
        self.managers
            .entry(app_id)
            .or_insert_with(|| SyncManager::new(app_id))
    }

    /// Get a sync manager for an app (immutable).
    pub fn get(&self, app_id: u64) -> Option<&SyncManager> {
        self.managers.get(&app_id)
    }

    /// Remove an app's sync manager.
    pub fn remove(&mut self, app_id: u64) {
        self.managers.remove(&app_id);
    }

    /// Update network state for all managers.
    pub fn update_all_network_state(&mut self, state: NetworkState) {
        for manager in self.managers.values_mut() {
            manager.update_network_state(state);
        }
    }

    /// Tick all managers.
    pub fn tick_all(&mut self) {
        for manager in self.managers.values_mut() {
            manager.tick();
        }
    }

    /// Collect all ready tasks across all apps.
    pub fn all_ready_tasks(&self) -> Vec<(u64, &str)> {
        let mut result = Vec::new();
        for (app_id, manager) in &self.managers {
            for task in manager.ready_tasks() {
                result.push((*app_id, task.tag.as_str()));
            }
        }
        result
    }
}

/// Global sync registry.
pub static SYNC_REGISTRY: RwLock<SyncRegistry> = RwLock::new(SyncRegistry::new());

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_get_tags() {
        let mut sm = SyncManager::new(1);
        sm.register("outbox-sync").unwrap();
        assert_eq!(sm.get_tags(), vec!["outbox-sync"]);
    }

    #[test]
    fn unregister() {
        let mut sm = SyncManager::new(1);
        sm.register("sync1").unwrap();
        assert!(sm.unregister("sync1"));
        assert!(sm.get_tags().is_empty());
    }

    #[test]
    fn ready_tasks_when_online() {
        let mut sm = SyncManager::new(1);
        sm.register("sync1").unwrap();
        let ready = sm.ready_tasks();
        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn no_ready_tasks_when_offline() {
        let mut sm = SyncManager::new(1);
        sm.register("sync1").unwrap();
        sm.update_network_state(NetworkState::Offline);
        assert!(sm.ready_tasks().is_empty());
    }

    #[test]
    fn success_removes_task() {
        let mut sm = SyncManager::new(1);
        sm.register("sync1").unwrap();
        sm.report_result("sync1", SyncResult::Success);
        assert_eq!(sm.task_count(), 0);
    }

    #[test]
    fn failure_retries_with_backoff() {
        let mut sm = SyncManager::new(1);
        sm.register("sync1").unwrap();

        // First failure
        sm.report_result("sync1", SyncResult::Failed);
        assert_eq!(sm.task_count(), 1);
        // Should have retry delay
        let task = sm.tasks.get("sync1").unwrap();
        assert_eq!(task.attempts, 1);
        assert!(task.retry_delay_remaining > 0);
    }

    #[test]
    fn max_retries_discards() {
        let mut sm = SyncManager::new(1);
        sm.register("sync1").unwrap();

        for _ in 0..MAX_RETRIES {
            // Clear delay for test
            if let Some(t) = sm.tasks.get_mut("sync1") {
                t.retry_delay_remaining = 0;
            }
            sm.report_result("sync1", SyncResult::Failed);
        }

        assert_eq!(sm.task_count(), 0);
    }

    #[test]
    fn offline_to_online_triggers_ready() {
        let mut sm = SyncManager::new(1);
        sm.register("sync1").unwrap();
        sm.update_network_state(NetworkState::Offline);
        assert!(sm.ready_tasks().is_empty());

        sm.update_network_state(NetworkState::Online);
        assert!(sm.consume_online_transition());
        assert_eq!(sm.ready_tasks().len(), 1);
    }

    #[test]
    fn tick_decrements_delay() {
        let mut sm = SyncManager::new(1);
        sm.register("sync1").unwrap();
        sm.report_result("sync1", SyncResult::Failed);

        let initial_delay = sm.tasks.get("sync1").unwrap().retry_delay_remaining;
        sm.tick();
        let after_tick = sm.tasks.get("sync1").unwrap().retry_delay_remaining;
        assert_eq!(after_tick, initial_delay - 1);
    }

    #[test]
    fn json_serialization() {
        let mut sm = SyncManager::new(1);
        sm.register("outbox").unwrap();
        let json = sm.to_json();
        assert!(json.contains("outbox"));
        assert!(json.contains("\"pending\":true"));
    }
}
