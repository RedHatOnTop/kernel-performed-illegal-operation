//! Toast Rendering
//!
//! Toast notifications appear at the top-right of the screen, stacked
//! vertically (max 3 simultaneous).  Each auto-dismisses after 5 seconds
//! or on user click.

use alloc::string::String;
use alloc::vec::Vec;

use super::render::{Color, Renderer};
use super::theme::Accent;

// ── Constants ───────────────────────────────────────────────

/// Maximum simultaneous toasts.
const MAX_VISIBLE: usize = 3;

/// Toast width in pixels.
const TOAST_WIDTH: u32 = 320;

/// Toast height in pixels.
const TOAST_HEIGHT: u32 = 80;

/// Margin between toasts (vertical).
const TOAST_GAP: u32 = 8;

/// Right margin from screen edge.
const TOAST_MARGIN_X: u32 = 16;

/// Top margin from screen edge.
const TOAST_MARGIN_Y: u32 = 16;

/// Auto-dismiss after this many render frames (≈ 5 seconds at 60 fps).
const AUTO_DISMISS_FRAMES: u64 = 300;

// ── Types ───────────────────────────────────────────────────

/// A visible toast on screen.
#[derive(Debug, Clone)]
pub struct Toast {
    /// Notification ID this toast represents.
    pub notification_id: u64,
    /// App name.
    pub app_name: String,
    /// Title text.
    pub title: String,
    /// Body text.
    pub body: String,
    /// Theme color (optional — overrides background accent).
    pub theme_color: Option<u32>,
    /// Frames remaining before auto-dismiss.
    pub frames_remaining: u64,
    /// Whether the user clicked the close button.
    pub dismissed: bool,
}

/// Toast manager — holds the visible toast stack.
pub struct ToastManager {
    /// Active toasts (newest at end).
    toasts: Vec<Toast>,
}

// ── Implementation ──────────────────────────────────────────

impl ToastManager {
    /// Create a new empty manager.
    pub const fn new() -> Self {
        Self { toasts: Vec::new() }
    }

    /// Enqueue a toast.  If already at `MAX_VISIBLE`, the oldest is evicted.
    pub fn push(
        &mut self,
        notification_id: u64,
        app_name: &str,
        title: &str,
        body: &str,
        theme_color: Option<u32>,
    ) {
        // Evict oldest if at capacity
        while self.toasts.len() >= MAX_VISIBLE {
            self.toasts.remove(0);
        }

        self.toasts.push(Toast {
            notification_id,
            app_name: String::from(app_name),
            title: String::from(title),
            body: String::from(body),
            theme_color,
            frames_remaining: AUTO_DISMISS_FRAMES,
            dismissed: false,
        });
    }

    /// Tick — decrement timers and remove expired / dismissed toasts.
    pub fn tick(&mut self) {
        for toast in &mut self.toasts {
            if toast.frames_remaining > 0 {
                toast.frames_remaining -= 1;
            }
        }
        self.toasts
            .retain(|t| t.frames_remaining > 0 && !t.dismissed);
    }

    /// Dismiss a specific toast by notification ID.
    pub fn dismiss(&mut self, notification_id: u64) {
        if let Some(t) = self
            .toasts
            .iter_mut()
            .find(|t| t.notification_id == notification_id)
        {
            t.dismissed = true;
        }
    }

    /// Handle a click at (mx, my) screen coordinates.
    /// Returns `Some(notification_id)` if the click hit a toast body (not close),
    /// or `None` if it didn't hit any toast.
    pub fn handle_click(&mut self, mx: i32, my: i32, screen_width: u32) -> Option<u64> {
        let base_x = screen_width as i32 - TOAST_MARGIN_X as i32 - TOAST_WIDTH as i32;

        for (i, toast) in self.toasts.iter_mut().enumerate() {
            let ty = TOAST_MARGIN_Y as i32 + i as i32 * (TOAST_HEIGHT as i32 + TOAST_GAP as i32);
            let tx = base_x;

            if mx >= tx && mx < tx + TOAST_WIDTH as i32 && my >= ty && my < ty + TOAST_HEIGHT as i32
            {
                // Check if close button area (top-right 24x24 of the toast)
                let close_x = tx + TOAST_WIDTH as i32 - 28;
                let close_y = ty + 4;
                if mx >= close_x && my >= close_y && my < close_y + 20 {
                    toast.dismissed = true;
                    return None;
                }

                // Body click — return notification ID
                toast.dismissed = true;
                return Some(toast.notification_id);
            }
        }
        None
    }

    /// Render all visible toasts.
    pub fn render(&self, renderer: &mut Renderer, screen_width: u32) {
        let base_x = screen_width as i32 - TOAST_MARGIN_X as i32 - TOAST_WIDTH as i32;

        for (i, toast) in self.toasts.iter().enumerate() {
            let y = TOAST_MARGIN_Y as i32 + i as i32 * (TOAST_HEIGHT as i32 + TOAST_GAP as i32);

            // Background
            let bg = if let Some(tc) = toast.theme_color {
                Color::rgb(
                    ((tc >> 16) & 0xFF) as u8,
                    ((tc >> 8) & 0xFF) as u8,
                    (tc & 0xFF) as u8,
                )
            } else {
                Color::rgb(40, 40, 55)
            };
            renderer.fill_rect(base_x, y, TOAST_WIDTH, TOAST_HEIGHT, bg);

            // Border
            let border = Accent::PRIMARY;
            renderer.fill_rect(base_x, y, TOAST_WIDTH, 2, border);

            // App name (small, top-left)
            renderer.draw_text(
                base_x + 10,
                y + 6,
                &toast.app_name,
                Color::rgb(160, 160, 180),
            );

            // Close button "✕" (top-right)
            renderer.draw_text(
                base_x + TOAST_WIDTH as i32 - 22,
                y + 6,
                "x",
                Color::rgb(180, 180, 180),
            );

            // Title (bold — we just render normally, no bold font)
            renderer.draw_text(base_x + 10, y + 24, &toast.title, Color::WHITE);

            // Body (up to ~40 chars per line, 2 lines max)
            let body_line1 = if toast.body.len() > 40 {
                &toast.body[..40]
            } else {
                &toast.body
            };
            renderer.draw_text(base_x + 10, y + 42, body_line1, Color::rgb(200, 200, 210));

            if toast.body.len() > 40 {
                let end = core::cmp::min(toast.body.len(), 80);
                let body_line2 = &toast.body[40..end];
                renderer.draw_text(base_x + 10, y + 58, body_line2, Color::rgb(200, 200, 210));
            }
        }
    }

    /// Number of active toasts.
    pub fn count(&self) -> usize {
        self.toasts.len()
    }

    /// Check if any toasts are visible.
    pub fn is_empty(&self) -> bool {
        self.toasts.is_empty()
    }
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_count() {
        let mut tm = ToastManager::new();
        tm.push(1, "App", "Title", "Body", None);
        assert_eq!(tm.count(), 1);
    }

    #[test]
    fn max_visible_eviction() {
        let mut tm = ToastManager::new();
        for i in 0..5 {
            tm.push(i, "App", "T", "B", None);
        }
        assert_eq!(tm.count(), MAX_VISIBLE);
    }

    #[test]
    fn auto_dismiss() {
        let mut tm = ToastManager::new();
        tm.push(1, "App", "T", "B", None);
        // Tick down all frames
        for _ in 0..AUTO_DISMISS_FRAMES {
            tm.tick();
        }
        assert!(tm.is_empty());
    }

    #[test]
    fn manual_dismiss() {
        let mut tm = ToastManager::new();
        tm.push(42, "App", "T", "B", None);
        tm.dismiss(42);
        tm.tick();
        assert!(tm.is_empty());
    }

    #[test]
    fn tick_preserves_active() {
        let mut tm = ToastManager::new();
        tm.push(1, "App", "T", "B", None);
        tm.tick();
        assert_eq!(tm.count(), 1); // Still alive (299 frames left)
    }
}
