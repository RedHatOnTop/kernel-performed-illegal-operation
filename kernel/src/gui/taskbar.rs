//! Taskbar
//!
//! Modern flat bottom taskbar with system tray and clock.

use super::render::{Color, Renderer};
use super::theme::{Accent, Radius, Size, Spacing, Surface, Text, TrayColor};
use super::WindowId;
use alloc::string::String;
use alloc::vec::Vec;

/// Taskbar item
#[derive(Debug, Clone)]
pub struct TaskbarItem {
    pub window_id: WindowId,
    pub title: String,
    pub is_active: bool,
}

/// Start menu item
#[derive(Debug, Clone)]
pub struct StartMenuItem {
    pub name: String,
    pub app_type: AppType,
}

/// Application types
#[derive(Debug, Clone)]
pub enum AppType {
    Browser,
    Terminal,
    Files,
    Settings,
    /// Installed WebApp — launched by kernel app_id
    WebApp {
        app_id: u64,
        name: String,
        start_url: String,
        scope: String,
        theme_color: Option<u32>,
    },
}

/// Taskbar state
pub struct Taskbar {
    pub width: u32,
    pub height: u32,
    pub items: Vec<TaskbarItem>,
    pub active_window: Option<WindowId>,
    pub start_menu_open: bool,
    pub start_menu_items: Vec<StartMenuItem>,
    pub hovered_item: Option<usize>,
    pub hovered_start_item: Option<usize>,
    /// Frame counter used to derive the clock display
    pub frame_count: u64,
}

impl Taskbar {
    /// Create new taskbar
    pub fn new(width: u32, height: u32) -> Self {
        let start_menu_items = alloc::vec![
            StartMenuItem {
                name: String::from("Terminal"),
                app_type: AppType::Terminal
            },
            StartMenuItem {
                name: String::from("Files"),
                app_type: AppType::Files
            },
            StartMenuItem {
                name: String::from("Browser"),
                app_type: AppType::Browser
            },
            StartMenuItem {
                name: String::from("Settings"),
                app_type: AppType::Settings
            },
        ];

        Self {
            width,
            height,
            items: Vec::new(),
            active_window: None,
            start_menu_open: false,
            start_menu_items,
            hovered_item: None,
            hovered_start_item: None,
            frame_count: 0,
        }
    }

    /// Add window to taskbar
    pub fn add_window(&mut self, id: WindowId, title: &str) {
        self.items.push(TaskbarItem {
            window_id: id,
            title: String::from(title),
            is_active: false,
        });
    }

    /// Remove window from taskbar
    pub fn remove_window(&mut self, id: WindowId) {
        self.items.retain(|item| item.window_id != id);
    }

    /// Set the active window — updates indicator dots
    pub fn set_active(&mut self, id: Option<WindowId>) {
        self.active_window = id;
        for item in &mut self.items {
            item.is_active = Some(item.window_id) == id;
        }
    }

    /// Handle click - returns clicked item index if any
    pub fn on_click(&mut self, x: i32, _y: i32) -> Option<usize> {
        // Check start button (first 48 pixels)
        if x < 48 {
            self.start_menu_open = !self.start_menu_open;
            return None;
        }

        // Check taskbar items
        let item_width = 140i32;
        let start_x = 56i32;
        let idx = ((x - start_x) / (item_width + 4)) as usize;

        if idx < self.items.len() {
            self.hovered_item = Some(idx);
            return Some(idx);
        }

        None
    }

    /// Render taskbar
    pub fn render(&self, renderer: &mut Renderer, y_offset: u32) {
        let y = y_offset as i32;
        let h = Size::TASKBAR_HEIGHT;

        // Draw start menu if open
        if self.start_menu_open {
            self.render_start_menu(renderer, y);
        }

        // ── Flat solid background ──
        renderer.fill_rect(0, y, self.width, h, Surface::TASKBAR);

        // 1px top border
        renderer.draw_hline(0, y, self.width, Surface::TASKBAR_HIGHLIGHT);

        // ── Start button ──
        self.render_start_button(renderer, y);

        // ── Running-app items ──
        let item_width = 140i32;
        let mut x = 56i32;
        for (i, item) in self.items.iter().enumerate() {
            let hovered = self.hovered_item == Some(i);
            self.render_item(
                renderer,
                x,
                y,
                item_width,
                &item.title,
                hovered,
                item.is_active,
            );
            x += item_width + 4;
        }

        // ── System tray + Clock ──
        self.render_system_tray(renderer, y);
        self.render_clock(renderer, y);
    }

    // ── Start menu ──────────────────────────────────────────────

    fn render_start_menu(&self, renderer: &mut Renderer, taskbar_y: i32) {
        let mw = Size::MENU_WIDTH;
        let item_h = Size::MENU_ITEM_HEIGHT;
        let header_h = 52u32;
        let items_h = self.start_menu_items.len() as u32 * item_h + Spacing::SM;
        let mh = header_h + items_h + Spacing::SM;
        let mx = Spacing::SM as i32;
        let my = taskbar_y - mh as i32 - Spacing::SM as i32;

        // Flat shadow
        renderer.fill_rounded_rect_aa(
            mx + 1,
            my + 3,
            mw,
            mh,
            Radius::MENU + 2,
            Color::rgba(0, 0, 0, 30),
        );

        // Background
        renderer.fill_rounded_rect_aa(mx, my, mw, mh, Radius::MENU, Surface::MENU_BG);
        renderer.draw_rounded_rect_aa(mx, my, mw, mh, Radius::MENU, Color::rgba(255, 255, 255, 8));

        // Header
        renderer.fill_rounded_rect_aa(
            mx + 1,
            my + 1,
            mw - 2,
            header_h - 1,
            Radius::MENU,
            Surface::MENU_HEADER,
        );
        renderer.draw_text_scaled(mx + Spacing::LG as i32, my + 14, "KPIO", Text::ON_ACCENT, 2);

        // Menu items
        let mut iy = my + header_h as i32 + Spacing::XXS as i32;
        for (i, item) in self.start_menu_items.iter().enumerate() {
            let hovered = self.hovered_start_item == Some(i);
            if hovered {
                renderer.fill_rounded_rect_aa(
                    mx + Spacing::XXS as i32,
                    iy,
                    mw - Spacing::SM,
                    item_h,
                    Radius::SM,
                    Surface::MENU_HOVER,
                );
            }
            renderer.draw_text(
                mx + Spacing::LG as i32,
                iy + (item_h as i32 - 8) / 2,
                &item.name,
                Text::ON_DARK,
            );
            iy += item_h as i32;
        }
    }

    /// Check start menu click
    pub fn check_start_menu_click(
        &mut self,
        x: i32,
        y: i32,
        screen_height: u32,
    ) -> Option<AppType> {
        if !self.start_menu_open {
            return None;
        }

        let taskbar_y = (screen_height - self.height) as i32;
        let item_h = Size::MENU_ITEM_HEIGHT;
        let header_h = 52u32;
        let items_h = self.start_menu_items.len() as u32 * item_h + Spacing::SM;
        let mh = header_h + items_h + Spacing::SM;
        let mw = Size::MENU_WIDTH;
        let mx = Spacing::SM as i32;
        let my = taskbar_y - mh as i32 - Spacing::SM as i32;

        if x >= mx && x < mx + mw as i32 && y >= my && y < my + mh as i32 {
            let item_start = my + header_h as i32 + Spacing::XXS as i32;
            if y >= item_start {
                let idx = ((y - item_start) / item_h as i32) as usize;
                if idx < self.start_menu_items.len() {
                    let app_type = self.start_menu_items[idx].app_type.clone();
                    self.start_menu_open = false;
                    return Some(app_type);
                }
            }
        }

        None
    }

    // ── Start button ────────────────────────────────────────────

    fn render_start_button(&self, renderer: &mut Renderer, y: i32) {
        let bw = 44u32;
        let bh = Size::TASKBAR_HEIGHT - 10;
        let bx = Spacing::XS as i32;
        let by = y + 5;

        let bg = if self.start_menu_open {
            Surface::TASKBAR_ACTIVE
        } else {
            Surface::TASKBAR_ITEM
        };
        renderer.fill_rounded_rect_aa(bx, by, bw, bh, Radius::TASKBAR_ITEM, bg);

        // "K" logo flat
        renderer.draw_text_scaled(bx + 10, by + 6, "K", Accent::PRIMARY, 3);
    }

    // ── Taskbar item ────────────────────────────────────────────

    fn render_item(
        &self,
        renderer: &mut Renderer,
        x: i32,
        y: i32,
        width: i32,
        title: &str,
        hovered: bool,
        active: bool,
    ) {
        let ih = Size::TASKBAR_HEIGHT - 10;
        let iy = y + 5;

        let bg = if hovered {
            Surface::TASKBAR_HOVER
        } else if active {
            Surface::TASKBAR_ACTIVE
        } else {
            Surface::TASKBAR_ITEM
        };
        renderer.fill_rounded_rect_aa(x, iy, width as u32, ih, Radius::TASKBAR_ITEM, bg);

        // Truncate title
        let max_chars = ((width - 16) / 8) as usize;
        let display: String = if title.len() > max_chars {
            title
                .chars()
                .take(max_chars.saturating_sub(2))
                .collect::<String>()
                + ".."
        } else {
            String::from(title)
        };
        renderer.draw_text(
            x + 8,
            iy as i32 + (ih as i32 - 8) / 2,
            &display,
            Text::ON_DARK,
        );

        // Active indicator — small accent pill below the item
        if active {
            let pill_w = 16u32;
            let pill_x = x + (width - pill_w as i32) / 2;
            let pill_y = y + Size::TASKBAR_HEIGHT as i32 - 4;
            renderer.fill_rounded_rect_aa(pill_x, pill_y, pill_w, 3, 1, Surface::ACTIVE_INDICATOR);
        }
    }

    // ── System tray ──────────────────────────────────────────────

    fn render_system_tray(&self, renderer: &mut Renderer, y: i32) {
        let tray_x = (self.width - 120) as i32;
        let cy = y + (Size::TASKBAR_HEIGHT as i32 - 8) / 2;

        // Volume icon: ♪ (simple glyph)
        renderer.draw_text(tray_x, cy, "Vol", TrayColor::ICON);

        // Network icon
        renderer.draw_text(tray_x + 32, cy, "Net", TrayColor::ICON);

        // Separator before clock
        renderer.draw_vline(
            tray_x + 62,
            y + 12,
            Size::TASKBAR_HEIGHT - 24,
            Color::rgba(255, 255, 255, 10),
        );
    }

    // ── Clock ───────────────────────────────────────────────────

    fn render_clock(&self, renderer: &mut Renderer, y: i32) {
        // Derive HH:MM from frame_count (approx 60fps → seconds)
        let total_seconds = self.frame_count / 60;
        let hours = ((total_seconds / 3600) % 24) as u32;
        let minutes = ((total_seconds / 60) % 60) as u32;

        let time_str = alloc::format!("{:02}:{:02}", hours, minutes);
        let clock_x = (self.width - 50) as i32;
        let cy = y + (Size::TASKBAR_HEIGHT as i32 - 8) / 2;
        renderer.draw_text(clock_x, cy, &time_str, Text::ON_DARK);
    }
}
