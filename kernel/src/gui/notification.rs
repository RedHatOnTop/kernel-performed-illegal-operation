//! Kernel Notification Center
//!
//! Manages notifications from PWA and system sources.
//! - Per-notification state: read/unread
//! - History: up to 50 items (FIFO eviction)
//! - Global singleton: `NOTIFICATION_CENTER`

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

// ── Constants ───────────────────────────────────────────────

/// Maximum notifications kept in history.
const MAX_HISTORY: usize = 50;

// ── Types ───────────────────────────────────────────────────

/// Unique notification identifier.
pub type NotificationId = u64;

/// A single notification.
#[derive(Debug, Clone)]
pub struct Notification {
    /// Unique ID.
    pub id: NotificationId,
    /// Originating app (kernel app ID).
    pub app_id: u64,
    /// Human-readable app name.
    pub app_name: String,
    /// Notification title (bold).
    pub title: String,
    /// Body text (up to 2 lines).
    pub body: String,
    /// Optional icon data (raw RGBA).
    pub icon_data: Option<Vec<u8>>,
    /// Timestamp (ticks or monotonic counter).
    pub timestamp: u64,
    /// Whether the user has seen this.
    pub read: bool,
    /// URL to open when notification is clicked.
    pub action_url: Option<String>,
}

/// The notification center — collects, stores, and serves notifications.
pub struct NotificationCenter {
    /// History (newest at back).
    history: VecDeque<Notification>,
    /// Counter for IDs.
    next_id: u64,
    /// IDs currently displayed as toasts (awaiting render).
    pub(crate) toast_queue: VecDeque<NotificationId>,
}

// ── Global Instance ─────────────────────────────────────────

/// Global notification center.
pub static NOTIFICATION_CENTER: Mutex<NotificationCenter> = Mutex::new(NotificationCenter::new());

// ── Implementation ──────────────────────────────────────────

impl NotificationCenter {
    /// Create an empty center (const for static init).
    pub const fn new() -> Self {
        Self {
            history: VecDeque::new(),
            next_id: 1,
            toast_queue: VecDeque::new(),
        }
    }

    /// Show a notification: adds to history and enqueues a toast.
    pub fn show(
        &mut self,
        app_id: u64,
        app_name: &str,
        title: &str,
        body: &str,
        icon_data: Option<Vec<u8>>,
        action_url: Option<String>,
    ) -> NotificationId {
        let id = self.next_id;
        self.next_id += 1;

        let notif = Notification {
            id,
            app_id,
            app_name: String::from(app_name),
            title: String::from(title),
            body: String::from(body),
            icon_data,
            timestamp: Self::now(),
            read: false,
            action_url,
        };

        // Evict oldest if at capacity
        while self.history.len() >= MAX_HISTORY {
            self.history.pop_front();
        }

        self.history.push_back(notif);
        self.toast_queue.push_back(id);

        id
    }

    /// Dismiss a notification toast (remove from toast queue only).
    pub fn dismiss(&mut self, id: NotificationId) {
        self.toast_queue.retain(|&tid| tid != id);
    }

    /// List all unread notifications.
    pub fn list_unread(&self) -> Vec<&Notification> {
        self.history.iter().filter(|n| !n.read).collect()
    }

    /// List all notifications.
    pub fn list_all(&self) -> Vec<&Notification> {
        self.history.iter().collect()
    }

    /// Mark a notification as read.
    pub fn mark_read(&mut self, id: NotificationId) {
        if let Some(n) = self.history.iter_mut().find(|n| n.id == id) {
            n.read = true;
        }
    }

    /// Mark all as read.
    pub fn mark_all_read(&mut self) {
        for n in self.history.iter_mut() {
            n.read = true;
        }
    }

    /// Clear all notifications.
    pub fn clear_all(&mut self) {
        self.history.clear();
        self.toast_queue.clear();
    }

    /// Get a specific notification.
    pub fn get(&self, id: NotificationId) -> Option<&Notification> {
        self.history.iter().find(|n| n.id == id)
    }

    /// Number of unread notifications.
    pub fn unread_count(&self) -> usize {
        self.history.iter().filter(|n| !n.read).count()
    }

    /// Total notification count.
    pub fn total_count(&self) -> usize {
        self.history.len()
    }

    /// Pop the next toast to display (oldest first).
    pub fn pop_toast(&mut self) -> Option<NotificationId> {
        self.toast_queue.pop_front()
    }

    /// Check if there are pending toasts.
    pub fn has_pending_toasts(&self) -> bool {
        !self.toast_queue.is_empty()
    }

    /// Monotonic "now" — simple counter since we don't have real time.
    fn now() -> u64 {
        // In a real implementation, this would read a monotonic clock.
        // For now, use a static counter.
        static COUNTER: spin::Mutex<u64> = spin::Mutex::new(0);
        let mut c = COUNTER.lock();
        *c += 1;
        *c
    }
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_center() -> NotificationCenter {
        NotificationCenter::new()
    }

    #[test]
    fn show_and_list() {
        let mut nc = make_center();
        nc.show(1, "TestApp", "Hello", "World", None, None);
        assert_eq!(nc.total_count(), 1);
        assert_eq!(nc.unread_count(), 1);
    }

    #[test]
    fn mark_read() {
        let mut nc = make_center();
        let id = nc.show(1, "App", "Title", "Body", None, None);
        nc.mark_read(id);
        assert_eq!(nc.unread_count(), 0);
    }

    #[test]
    fn dismiss_toast() {
        let mut nc = make_center();
        let id = nc.show(1, "App", "Title", "Body", None, None);
        assert!(nc.has_pending_toasts());
        nc.dismiss(id);
        assert!(!nc.has_pending_toasts());
    }

    #[test]
    fn fifo_eviction() {
        let mut nc = make_center();
        for i in 0..60 {
            nc.show(1, "App", &alloc::format!("N{}", i), "body", None, None);
        }
        assert_eq!(nc.total_count(), MAX_HISTORY);
        // Oldest should be evicted; first remaining should be N10
        let first = nc.history.front().unwrap();
        assert_eq!(first.title, "N10");
    }

    #[test]
    fn clear_all() {
        let mut nc = make_center();
        nc.show(1, "App", "A", "B", None, None);
        nc.show(1, "App", "C", "D", None, None);
        nc.clear_all();
        assert_eq!(nc.total_count(), 0);
        assert!(!nc.has_pending_toasts());
    }

    #[test]
    fn pop_toast_order() {
        let mut nc = make_center();
        let id1 = nc.show(1, "App", "First", "", None, None);
        let id2 = nc.show(1, "App", "Second", "", None, None);
        assert_eq!(nc.pop_toast(), Some(id1));
        assert_eq!(nc.pop_toast(), Some(id2));
        assert_eq!(nc.pop_toast(), None);
    }

    #[test]
    fn list_unread_filters() {
        let mut nc = make_center();
        let id1 = nc.show(1, "App", "A", "", None, None);
        let _id2 = nc.show(1, "App", "B", "", None, None);
        nc.mark_read(id1);
        let unread = nc.list_unread();
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].title, "B");
    }

    #[test]
    fn action_url() {
        let mut nc = make_center();
        let id = nc.show(
            1,
            "App",
            "Click me",
            "body",
            None,
            Some(String::from("https://example.com")),
        );
        let notif = nc.get(id).unwrap();
        assert_eq!(notif.action_url.as_deref(), Some("https://example.com"));
    }
}
