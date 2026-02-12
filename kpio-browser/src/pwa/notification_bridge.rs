//! Notification API Bridge
//!
//! Bridges the W3C Notification API surface to the kernel's
//! `NotificationCenter`.
//!
//! - `Notification.requestPermission()` → permission check / prompt
//! - `new Notification(title, options)` → enqueue toast
//! - `notification.close()` → dismiss

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

// ── Permission ──────────────────────────────────────────────

/// Permission state per app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationPermission {
    /// User hasn't been asked yet.
    Default,
    /// User granted permission.
    Granted,
    /// User denied permission.
    Denied,
}

// ── Notification Options ────────────────────────────────────

/// Options passed to `new Notification(title, options)`.
#[derive(Debug, Clone)]
pub struct NotificationOptions {
    /// Body text.
    pub body: String,
    /// Icon URL.
    pub icon: Option<String>,
    /// Badge URL.
    pub badge: Option<String>,
    /// Tag (de-duplication key).
    pub tag: Option<String>,
    /// Whether to vibrate.
    pub silent: bool,
    /// Action URL (KPIO extension).
    pub action_url: Option<String>,
}

impl Default for NotificationOptions {
    fn default() -> Self {
        Self {
            body: String::new(),
            icon: None,
            badge: None,
            tag: None,
            silent: false,
            action_url: None,
        }
    }
}

// ── Bridge ──────────────────────────────────────────────────

/// The notification bridge manages permission state and dispatches
/// notifications to the kernel.
pub struct NotificationBridge {
    /// app_id → permission
    permissions: BTreeMap<u64, NotificationPermission>,
    /// Pending permission requests (app_id list).
    pub pending_requests: Vec<u64>,
    /// Callback for sending to kernel notification center.
    /// `(app_id, app_name, title, body, action_url) → notification_id`
    kernel_show_fn: Option<fn(u64, &str, &str, &str, Option<&str>) -> u64>,
    /// Callback for dismissing in kernel.
    /// `(notification_id)`
    kernel_dismiss_fn: Option<fn(u64)>,
}

/// Global notification bridge.
pub static NOTIFICATION_BRIDGE: RwLock<NotificationBridge> =
    RwLock::new(NotificationBridge::new());

impl NotificationBridge {
    /// Create a new bridge (const for static init).
    pub const fn new() -> Self {
        Self {
            permissions: BTreeMap::new(),
            pending_requests: Vec::new(),
            kernel_show_fn: None,
            kernel_dismiss_fn: None,
        }
    }

    /// Register kernel callbacks.
    pub fn register_kernel_callbacks(
        &mut self,
        show_fn: fn(u64, &str, &str, &str, Option<&str>) -> u64,
        dismiss_fn: fn(u64),
    ) {
        self.kernel_show_fn = Some(show_fn);
        self.kernel_dismiss_fn = Some(dismiss_fn);
    }

    // ── Permission management ───────────────────────────────

    /// Check permission for an app.
    pub fn get_permission(&self, app_id: u64) -> NotificationPermission {
        self.permissions
            .get(&app_id)
            .copied()
            .unwrap_or(NotificationPermission::Default)
    }

    /// Request permission (initiates prompt flow).
    /// Returns the current state — if `Default`, caller should show a dialog.
    pub fn request_permission(&mut self, app_id: u64) -> NotificationPermission {
        match self.get_permission(app_id) {
            NotificationPermission::Granted => NotificationPermission::Granted,
            NotificationPermission::Denied => NotificationPermission::Denied,
            NotificationPermission::Default => {
                // Queue a permission request
                if !self.pending_requests.contains(&app_id) {
                    self.pending_requests.push(app_id);
                }
                NotificationPermission::Default
            }
        }
    }

    /// Grant permission (called from UI after user clicks "Allow").
    pub fn grant_permission(&mut self, app_id: u64) {
        self.permissions
            .insert(app_id, NotificationPermission::Granted);
        self.pending_requests.retain(|&id| id != app_id);
    }

    /// Deny permission (called from UI after user clicks "Block").
    pub fn deny_permission(&mut self, app_id: u64) {
        self.permissions
            .insert(app_id, NotificationPermission::Denied);
        self.pending_requests.retain(|&id| id != app_id);
    }

    /// Reset permission (for testing / settings).
    pub fn reset_permission(&mut self, app_id: u64) {
        self.permissions.remove(&app_id);
    }

    // ── Notification dispatch ───────────────────────────────

    /// Show a notification (equivalent to `new Notification(...)`).
    ///
    /// Returns `Ok(notification_id)` if permission is granted, `Err` otherwise.
    pub fn show_notification(
        &self,
        app_id: u64,
        app_name: &str,
        title: &str,
        options: &NotificationOptions,
    ) -> Result<u64, NotificationPermission> {
        match self.get_permission(app_id) {
            NotificationPermission::Granted => {
                if let Some(show) = self.kernel_show_fn {
                    let id = show(
                        app_id,
                        app_name,
                        title,
                        &options.body,
                        options.action_url.as_deref(),
                    );
                    Ok(id)
                } else {
                    // No kernel callback — silently succeed with dummy ID
                    Ok(0)
                }
            }
            other => Err(other),
        }
    }

    /// Close / dismiss a notification.
    pub fn close_notification(&self, notification_id: u64) {
        if let Some(dismiss) = self.kernel_dismiss_fn {
            dismiss(notification_id);
        }
    }

    /// Check if any apps have pending permission requests.
    pub fn has_pending_requests(&self) -> bool {
        !self.pending_requests.is_empty()
    }

    /// Pop the next pending permission request.
    pub fn pop_pending_request(&mut self) -> Option<u64> {
        if self.pending_requests.is_empty() {
            None
        } else {
            Some(self.pending_requests.remove(0))
        }
    }
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bridge() -> NotificationBridge {
        NotificationBridge::new()
    }

    #[test]
    fn default_permission() {
        let b = make_bridge();
        assert_eq!(
            b.get_permission(1),
            NotificationPermission::Default
        );
    }

    #[test]
    fn grant_and_check() {
        let mut b = make_bridge();
        b.grant_permission(1);
        assert_eq!(
            b.get_permission(1),
            NotificationPermission::Granted
        );
    }

    #[test]
    fn deny_and_check() {
        let mut b = make_bridge();
        b.deny_permission(1);
        assert_eq!(
            b.get_permission(1),
            NotificationPermission::Denied
        );
    }

    #[test]
    fn request_permission_queues() {
        let mut b = make_bridge();
        let result = b.request_permission(1);
        assert_eq!(result, NotificationPermission::Default);
        assert!(b.has_pending_requests());
    }

    #[test]
    fn show_without_permission_fails() {
        let b = make_bridge();
        let opts = NotificationOptions::default();
        let result = b.show_notification(1, "App", "Title", &opts);
        assert!(result.is_err());
    }

    #[test]
    fn show_with_granted_succeeds() {
        let mut b = make_bridge();
        b.grant_permission(1);
        let opts = NotificationOptions::default();
        let result = b.show_notification(1, "App", "Title", &opts);
        assert!(result.is_ok());
    }

    #[test]
    fn reset_permission() {
        let mut b = make_bridge();
        b.grant_permission(1);
        b.reset_permission(1);
        assert_eq!(
            b.get_permission(1),
            NotificationPermission::Default
        );
    }

    #[test]
    fn denied_blocks_notification() {
        let mut b = make_bridge();
        b.deny_permission(1);
        let opts = NotificationOptions::default();
        let result = b.show_notification(1, "App", "Title", &opts);
        assert_eq!(result, Err(NotificationPermission::Denied));
    }
}
