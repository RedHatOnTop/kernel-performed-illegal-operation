//! Notification Panel
//!
//! Slide-down panel triggered by the taskbar bell icon.
//! Shows recent notifications grouped by app, with a "mark all read" button.

use alloc::string::String;
use alloc::vec::Vec;

use super::notification::{Notification, NotificationId, NOTIFICATION_CENTER};
use super::render::{Color, Renderer};
use super::theme::Accent;

// ── Constants ───────────────────────────────────────────────

/// Panel width.
const PANEL_WIDTH: u32 = 360;

/// Maximum panel height.
const PANEL_MAX_HEIGHT: u32 = 480;

/// Row height per notification entry.
const ROW_HEIGHT: u32 = 56;

/// Header height (title + "Mark all read" button).
const HEADER_HEIGHT: u32 = 36;

/// Right margin from screen edge.
const PANEL_MARGIN_RIGHT: u32 = 8;

// ── Bell icon constants ─────────────────────────────────────

/// Bell icon width in the taskbar.
pub const BELL_ICON_WIDTH: u32 = 28;

/// Bell icon height.
pub const BELL_ICON_HEIGHT: u32 = 28;

// ── Panel ───────────────────────────────────────────────────

/// The notification panel state.
pub struct NotificationPanel {
    /// Whether the panel is visible.
    pub visible: bool,
    /// Scroll offset (for when list > max height).
    pub scroll_offset: u32,
}

impl NotificationPanel {
    /// Create a new (hidden) panel.
    pub const fn new() -> Self {
        Self {
            visible: false,
            scroll_offset: 0,
        }
    }

    /// Toggle visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        self.scroll_offset = 0;
    }

    /// Show the panel.
    pub fn show(&mut self) {
        self.visible = true;
        self.scroll_offset = 0;
    }

    /// Hide the panel.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Handle a click within the panel.
    /// Returns `Some(notification_id)` if a notification row was clicked,
    /// or `None` for header/outside clicks.
    pub fn handle_click(
        &mut self,
        mx: i32,
        my: i32,
        screen_width: u32,
        taskbar_top: i32,
    ) -> Option<NotificationId> {
        if !self.visible {
            return None;
        }

        let panel_x = screen_width as i32 - PANEL_MARGIN_RIGHT as i32 - PANEL_WIDTH as i32;
        let panel_y = taskbar_top - PANEL_MAX_HEIGHT as i32;

        // Check bounds
        if mx < panel_x || mx >= panel_x + PANEL_WIDTH as i32 || my < panel_y || my >= taskbar_top {
            // Outside panel — close it
            self.hide();
            return None;
        }

        let rel_y = (my - panel_y) as u32;

        // Header area — "Mark all read" button
        if rel_y < HEADER_HEIGHT {
            // Click on header → mark all read
            let mut center = NOTIFICATION_CENTER.lock();
            center.mark_all_read();
            return None;
        }

        // Notification rows
        let row_y = rel_y - HEADER_HEIGHT + self.scroll_offset;
        let row_index = row_y / ROW_HEIGHT;

        let center = NOTIFICATION_CENTER.lock();
        let all = center.list_all();

        // Reverse so newest is on top
        if (row_index as usize) < all.len() {
            let idx = all.len() - 1 - row_index as usize;
            let id = all[idx].id;
            drop(center);
            // Mark as read
            NOTIFICATION_CENTER.lock().mark_read(id);
            return Some(id);
        }

        None
    }

    /// Render the panel.
    pub fn render(&self, renderer: &mut Renderer, screen_width: u32, taskbar_top: i32) {
        if !self.visible {
            return;
        }

        let center = NOTIFICATION_CENTER.lock();
        let all_notifs = center.list_all();
        let unread_count = center.unread_count();

        let content_height = HEADER_HEIGHT + (all_notifs.len() as u32) * ROW_HEIGHT;
        let panel_height = core::cmp::min(content_height, PANEL_MAX_HEIGHT);

        let panel_x = screen_width as i32 - PANEL_MARGIN_RIGHT as i32 - PANEL_WIDTH as i32;
        let panel_y = taskbar_top - panel_height as i32;

        // Background
        renderer.fill_rect(
            panel_x,
            panel_y,
            PANEL_WIDTH,
            panel_height,
            Color::rgb(30, 30, 42),
        );

        // Border
        renderer.fill_rect(panel_x, panel_y, PANEL_WIDTH, 1, Accent::PRIMARY);

        // Header
        renderer.draw_text(panel_x + 12, panel_y + 10, "Notifications", Color::WHITE);

        // "Mark all read" link
        if unread_count > 0 {
            renderer.draw_text(
                panel_x + PANEL_WIDTH as i32 - 120,
                panel_y + 10,
                "Mark all read",
                Accent::PRIMARY_LIGHT,
            );
        }

        // Separator
        renderer.fill_rect(
            panel_x,
            panel_y + HEADER_HEIGHT as i32 - 1,
            PANEL_WIDTH,
            1,
            Color::rgb(60, 60, 75),
        );

        // Notification rows (newest first)
        let max_visible_rows = ((panel_height - HEADER_HEIGHT) / ROW_HEIGHT) as usize;
        let start_idx = if all_notifs.len() > max_visible_rows {
            all_notifs.len() - max_visible_rows
        } else {
            0
        };

        for (i, notif) in all_notifs[start_idx..].iter().rev().enumerate() {
            let row_y = panel_y + HEADER_HEIGHT as i32 + (i as i32) * ROW_HEIGHT as i32;

            if row_y + ROW_HEIGHT as i32 > taskbar_top {
                break;
            }

            // Unread indicator
            if !notif.read {
                renderer.fill_rect(
                    panel_x + 4,
                    row_y + ROW_HEIGHT as i32 / 2 - 3,
                    6,
                    6,
                    Accent::PRIMARY,
                );
            }

            // App name (small)
            renderer.draw_text(
                panel_x + 16,
                row_y + 6,
                &notif.app_name,
                Color::rgb(140, 140, 160),
            );

            // Title
            let title_color = if notif.read {
                Color::rgb(180, 180, 190)
            } else {
                Color::WHITE
            };
            renderer.draw_text(panel_x + 16, row_y + 22, &notif.title, title_color);

            // Body snippet (truncated)
            let body_snippet = if notif.body.len() > 45 {
                &notif.body[..45]
            } else {
                &notif.body
            };
            renderer.draw_text(
                panel_x + 16,
                row_y + 38,
                body_snippet,
                Color::rgb(140, 140, 155),
            );

            // Row separator
            renderer.fill_rect(
                panel_x + 12,
                row_y + ROW_HEIGHT as i32 - 1,
                PANEL_WIDTH - 24,
                1,
                Color::rgb(50, 50, 65),
            );
        }
    }

    /// Render the bell icon in the taskbar.
    /// Returns `true` if there are unread notifications (badge).
    pub fn render_bell_icon(renderer: &mut Renderer, x: i32, y: i32) -> bool {
        let center = NOTIFICATION_CENTER.lock();
        let unread = center.unread_count();

        // Bell body (simplified: a trapezoid-ish shape)
        // Top of bell
        renderer.fill_rect(x + 10, y + 4, 8, 2, Color::rgb(200, 200, 210));
        // Bell body
        renderer.fill_rect(x + 8, y + 6, 12, 10, Color::rgb(200, 200, 210));
        // Bell bottom (wider)
        renderer.fill_rect(x + 6, y + 16, 16, 3, Color::rgb(200, 200, 210));
        // Clapper
        renderer.fill_rect(x + 12, y + 20, 4, 3, Color::rgb(200, 200, 210));

        // Badge (red dot with count)
        if unread > 0 {
            let badge_color = Accent::DANGER;
            renderer.fill_rect(x + 18, y + 2, 10, 10, badge_color);

            // Number (single digit)
            let badge_text = if unread > 9 {
                "9+"
            } else {
                // We can't easily format here, just show dot
                ""
            };
            if !badge_text.is_empty() {
                renderer.draw_text(x + 19, y + 3, badge_text, Color::WHITE);
            }

            return true;
        }

        false
    }
}
