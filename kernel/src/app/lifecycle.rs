//! App Lifecycle Manager
//!
//! Tracks running app instances and manages state transitions:
//!
//! ```text
//! Registered → Launching → Running → Suspended → Terminated
//!                  ↓                      ↑
//!                Failed ─── (auto-restart ≤ 3)
//! ```

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use super::error::AppError;
use super::registry::{KernelAppId, APP_REGISTRY};

// ── Types ───────────────────────────────────────────────────

/// Unique identifier for a running app instance.
///
/// The same app can have multiple instances (e.g., two PWA windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AppInstanceId(pub u64);

impl core::fmt::Display for AppInstanceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Runtime state of an app instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppRunState {
    /// `launch()` has been called; resources are being set up.
    Launching,
    /// The app is actively running.
    Running,
    /// The app is suspended (can be resumed).
    Suspended,
    /// The app has terminated normally.
    Terminated,
    /// The app crashed or failed to start.
    Failed,
}

impl AppRunState {
    /// Human-readable label for error messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            AppRunState::Launching => "launching",
            AppRunState::Running => "running",
            AppRunState::Suspended => "suspended",
            AppRunState::Terminated => "terminated",
            AppRunState::Failed => "failed",
        }
    }
}

/// Information about a running app instance.
#[derive(Debug, Clone)]
pub struct AppInstanceInfo {
    pub instance_id: AppInstanceId,
    pub app_id: KernelAppId,
    pub app_name: String,
    pub state: AppRunState,
    pub restart_count: u32,
}

/// Internal tracking entry for an app instance.
struct InstanceEntry {
    app_id: KernelAppId,
    app_name: String,
    state: AppRunState,
    restart_count: u32,
    max_restarts: u32,
}

// ── Lifecycle Manager ───────────────────────────────────────

/// Maximum concurrent app instances across all apps.
const MAX_INSTANCES: usize = 64;

/// Manages running app instances and their state transitions.
pub struct AppLifecycle {
    instances: BTreeMap<u64, InstanceEntry>,
    next_instance_id: u64,
}

impl AppLifecycle {
    /// Create a new lifecycle manager.
    pub const fn new() -> Self {
        Self {
            instances: BTreeMap::new(),
            next_instance_id: 1,
        }
    }

    /// Launch a new instance of an app.
    ///
    /// The app must be registered in `APP_REGISTRY`. Creates a new
    /// instance in `Launching` state, then immediately transitions
    /// to `Running`.
    pub fn launch(&mut self, app_id: KernelAppId) -> Result<AppInstanceId, AppError> {
        if self.instances.len() >= MAX_INSTANCES {
            return Err(AppError::TooManyInstances);
        }

        // Get app name from registry
        let app_name = {
            let registry = APP_REGISTRY.lock();
            let desc = registry.get(app_id).ok_or(AppError::NotFound)?;
            desc.name.clone()
        };

        let instance_id = AppInstanceId(self.next_instance_id);
        self.next_instance_id += 1;

        let entry = InstanceEntry {
            app_id,
            app_name: app_name.clone(),
            state: AppRunState::Launching,
            restart_count: 0,
            max_restarts: 3,
        };

        self.instances.insert(instance_id.0, entry);

        // Update last_launched in the registry
        {
            let mut registry = APP_REGISTRY.lock();
            if let Some(desc) = registry.get_mut(app_id) {
                desc.last_launched = Self::now();
            }
        }

        // Transition to Running — in a real implementation this would
        // happen after the window/process is fully initialised.
        self.set_state(instance_id, AppRunState::Running)?;

        crate::serial_println!(
            "[KPIO/App] Launched '{}' (app_id={}, instance_id={})",
            app_name,
            app_id,
            instance_id
        );

        Ok(instance_id)
    }

    /// Suspend a running instance.
    pub fn suspend(&mut self, instance_id: AppInstanceId) -> Result<(), AppError> {
        let entry = self
            .instances
            .get_mut(&instance_id.0)
            .ok_or(AppError::InstanceNotFound)?;

        if entry.state != AppRunState::Running {
            return Err(AppError::InvalidState {
                current: entry.state.as_str(),
                expected: "running",
            });
        }

        entry.state = AppRunState::Suspended;
        crate::serial_println!(
            "[KPIO/App] Suspended '{}' (instance={})",
            entry.app_name,
            instance_id
        );
        Ok(())
    }

    /// Resume a suspended instance.
    pub fn resume(&mut self, instance_id: AppInstanceId) -> Result<(), AppError> {
        let entry = self
            .instances
            .get_mut(&instance_id.0)
            .ok_or(AppError::InstanceNotFound)?;

        if entry.state != AppRunState::Suspended {
            return Err(AppError::InvalidState {
                current: entry.state.as_str(),
                expected: "suspended",
            });
        }

        entry.state = AppRunState::Running;
        crate::serial_println!(
            "[KPIO/App] Resumed '{}' (instance={})",
            entry.app_name,
            instance_id
        );
        Ok(())
    }

    /// Terminate a running or suspended instance.
    pub fn terminate(&mut self, instance_id: AppInstanceId) -> Result<(), AppError> {
        let entry = self
            .instances
            .get_mut(&instance_id.0)
            .ok_or(AppError::InstanceNotFound)?;

        match entry.state {
            AppRunState::Running | AppRunState::Suspended | AppRunState::Launching => {
                entry.state = AppRunState::Terminated;
                crate::serial_println!(
                    "[KPIO/App] Terminated '{}' (instance={})",
                    entry.app_name,
                    instance_id
                );
                Ok(())
            }
            AppRunState::Terminated => Ok(()), // idempotent
            AppRunState::Failed => {
                entry.state = AppRunState::Terminated;
                Ok(())
            }
        }
    }

    /// Report a crash for an instance.
    ///
    /// If the restart count is below the maximum, the instance
    /// transitions back to `Launching` → `Running`. Otherwise it
    /// stays in `Failed`.
    pub fn report_crash(&mut self, instance_id: AppInstanceId) -> Result<bool, AppError> {
        let entry = self
            .instances
            .get_mut(&instance_id.0)
            .ok_or(AppError::InstanceNotFound)?;

        entry.restart_count += 1;

        if entry.restart_count <= entry.max_restarts {
            entry.state = AppRunState::Running; // auto-restart
            crate::serial_println!(
                "[KPIO/App] Auto-restarted '{}' (instance={}, attempt={}/{})",
                entry.app_name,
                instance_id,
                entry.restart_count,
                entry.max_restarts
            );
            Ok(true) // restarted
        } else {
            entry.state = AppRunState::Failed;
            crate::serial_println!(
                "[KPIO/App] '{}' failed permanently after {} crashes (instance={})",
                entry.app_name,
                entry.restart_count,
                instance_id
            );
            Ok(false) // gave up
        }
    }

    /// Get the current state of an instance.
    pub fn get_state(&self, instance_id: AppInstanceId) -> Result<AppRunState, AppError> {
        self.instances
            .get(&instance_id.0)
            .map(|e| e.state)
            .ok_or(AppError::InstanceNotFound)
    }

    /// List all active (non-terminated) instances.
    pub fn list_running(&self) -> Vec<AppInstanceInfo> {
        self.instances
            .iter()
            .filter(|(_, e)| e.state != AppRunState::Terminated)
            .map(|(&iid, e)| AppInstanceInfo {
                instance_id: AppInstanceId(iid),
                app_id: e.app_id,
                app_name: e.app_name.clone(),
                state: e.state,
                restart_count: e.restart_count,
            })
            .collect()
    }

    /// List all instances (including terminated) for a given app.
    pub fn instances_of(&self, app_id: KernelAppId) -> Vec<AppInstanceInfo> {
        self.instances
            .iter()
            .filter(|(_, e)| e.app_id == app_id)
            .map(|(&iid, e)| AppInstanceInfo {
                instance_id: AppInstanceId(iid),
                app_id: e.app_id,
                app_name: e.app_name.clone(),
                state: e.state,
                restart_count: e.restart_count,
            })
            .collect()
    }

    /// Clean up terminated instances to free memory.
    pub fn gc(&mut self) -> usize {
        let before = self.instances.len();
        self.instances.retain(|_, e| e.state != AppRunState::Terminated);
        let removed = before - self.instances.len();
        if removed > 0 {
            crate::serial_println!("[KPIO/App] GC: removed {} terminated instances", removed);
        }
        removed
    }

    /// Total number of tracked instances (including terminated).
    pub fn total_instances(&self) -> usize {
        self.instances.len()
    }

    // ── Internal ────────────────────────────────────────────

    fn set_state(
        &mut self,
        instance_id: AppInstanceId,
        new_state: AppRunState,
    ) -> Result<(), AppError> {
        let entry = self
            .instances
            .get_mut(&instance_id.0)
            .ok_or(AppError::InstanceNotFound)?;
        entry.state = new_state;
        Ok(())
    }

    fn now() -> u64 {
        static COUNTER: core::sync::atomic::AtomicU64 =
            core::sync::atomic::AtomicU64::new(1);
        COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
    }
}

// ── Global instance ─────────────────────────────────────────

/// Global app lifecycle manager.
pub static APP_LIFECYCLE: Mutex<AppLifecycle> = Mutex::new(AppLifecycle::new());

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::registry::{KernelAppType, APP_REGISTRY};
    use alloc::string::String;

    /// Helper: register a test app and return its ID.
    fn register_test_app(name: &str) -> KernelAppId {
        let mut reg = APP_REGISTRY.lock();
        reg.register(
            KernelAppType::WebApp {
                scope: String::from(name),
                offline_capable: false,
            },
            String::from(name),
            String::from(name),
            None,
        )
        .unwrap()
    }

    #[test]
    fn test_launch_and_get_state() {
        let app_id = register_test_app("test_launch_app");
        let mut lc = AppLifecycle::new();
        let iid = lc.launch(app_id).unwrap();
        assert_eq!(lc.get_state(iid).unwrap(), AppRunState::Running);
    }

    #[test]
    fn test_launch_nonexistent_app() {
        let mut lc = AppLifecycle::new();
        let result = lc.launch(KernelAppId(99999));
        assert!(matches!(result, Err(AppError::NotFound)));
    }

    #[test]
    fn test_suspend_resume_cycle() {
        let app_id = register_test_app("test_suspend_app");
        let mut lc = AppLifecycle::new();
        let iid = lc.launch(app_id).unwrap();

        lc.suspend(iid).unwrap();
        assert_eq!(lc.get_state(iid).unwrap(), AppRunState::Suspended);

        lc.resume(iid).unwrap();
        assert_eq!(lc.get_state(iid).unwrap(), AppRunState::Running);
    }

    #[test]
    fn test_suspend_non_running_fails() {
        let app_id = register_test_app("test_suspend_fail_app");
        let mut lc = AppLifecycle::new();
        let iid = lc.launch(app_id).unwrap();
        lc.terminate(iid).unwrap();

        let result = lc.suspend(iid);
        assert!(matches!(result, Err(AppError::InvalidState { .. })));
    }

    #[test]
    fn test_terminate() {
        let app_id = register_test_app("test_terminate_app");
        let mut lc = AppLifecycle::new();
        let iid = lc.launch(app_id).unwrap();
        lc.terminate(iid).unwrap();
        assert_eq!(lc.get_state(iid).unwrap(), AppRunState::Terminated);
    }

    #[test]
    fn test_crash_auto_restart() {
        let app_id = register_test_app("test_crash_app");
        let mut lc = AppLifecycle::new();
        let iid = lc.launch(app_id).unwrap();

        // Crashes 1-3 should auto-restart
        for _ in 0..3 {
            let restarted = lc.report_crash(iid).unwrap();
            assert!(restarted);
            assert_eq!(lc.get_state(iid).unwrap(), AppRunState::Running);
        }

        // Crash 4 should fail permanently
        let restarted = lc.report_crash(iid).unwrap();
        assert!(!restarted);
        assert_eq!(lc.get_state(iid).unwrap(), AppRunState::Failed);
    }

    #[test]
    fn test_list_running() {
        let app_id = register_test_app("test_list_app");
        let mut lc = AppLifecycle::new();
        let iid1 = lc.launch(app_id).unwrap();
        let iid2 = lc.launch(app_id).unwrap();

        assert_eq!(lc.list_running().len(), 2);

        lc.terminate(iid1).unwrap();
        let running = lc.list_running();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].instance_id, iid2);
    }

    #[test]
    fn test_gc() {
        let app_id = register_test_app("test_gc_app");
        let mut lc = AppLifecycle::new();
        let iid = lc.launch(app_id).unwrap();
        lc.terminate(iid).unwrap();

        assert_eq!(lc.total_instances(), 1);
        let removed = lc.gc();
        assert_eq!(removed, 1);
        assert_eq!(lc.total_instances(), 0);
    }

    #[test]
    fn test_instance_not_found() {
        let lc = AppLifecycle::new();
        assert!(matches!(
            lc.get_state(AppInstanceId(12345)),
            Err(AppError::InstanceNotFound)
        ));
    }
}
