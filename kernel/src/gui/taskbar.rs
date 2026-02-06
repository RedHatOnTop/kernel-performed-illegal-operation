//! Taskbar
//!
//! Modern bottom taskbar with frosted glass appearance, rounded items,
//! start menu and system tray.

use super::render::{Color, Renderer};
use super::theme::{Surface, Text, Accent, Radius, Spacing, Size};
use super::WindowId;
use alloc::string::String;
use alloc::vec::Vec;

/// Taskbar item
#[derive(Debug, Clone)]
pub struct TaskbarItem {
    pub window_id: WindowId,
    pub title: String,
}

/// Start menu item
#[derive(Debug, Clone)]
pub struct StartMenuItem {
    pub name: String,
    pub app_type: AppType,
}

/// Application types
#[derive(Debug, Clone, Copy)]
pub enum AppType {
    Browser,
    Terminal,
    Files,
    Settings,
}

/// Taskbar state
pub struct Taskbar {
    pub width: u32,
    pub height: u32,
    pub items: Vec<TaskbarItem>,
    pub start_menu_open: bool,
    pub start_menu_items: Vec<StartMenuItem>,
    pub hovered_item: Option<usize>,
    pub hovered_start_item: Option<usize>,
}

impl Taskbar {
    /// Create new taskbar
    pub fn new(width: u32, height: u32) -> Self {
        let start_menu_items = alloc::vec![
            StartMenuItem { name: String::from("Browser"), app_type: AppType::Browser },
            StartMenuItem { name: String::from("Terminal"), app_type: AppType::Terminal },
            StartMenuItem { name: String::from("Files"), app_type: AppType::Files },
            StartMenuItem { name: String::from("Settings"), app_type: AppType::Settings },
        ];

        Self {
            width,
            height,
            items: Vec::new(),
            start_menu_open: false,
            start_menu_items,
            hovered_item: None,
            hovered_start_item: None,
        }
    }

    /// Add window to taskbar
    pub fn add_window(&mut self, id: WindowId, title: &str) {
        self.items.push(TaskbarItem {
            window_id: id,
            title: String::from(title),
        });
    }

    /// Remove window from taskbar
    pub fn remove_window(&mut self, id: WindowId) {
        self.items.retain(|item| item.window_id != id);
    }

    /// Handle click - returns clicked item index if any
    pub fn on_click(&mut self, x: i32, _y: i32) -> Option<usize> {
        // Check start button (first 48 pixels)
        if x < 48 {
            self.start_menu_open = !self.start_menu_open;
            return None;
        }

        // Check taskbar items
        let item_width = 130i32;
        let start_x = 54i32;
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

        // Draw start menu if open (before taskbar so it sits above)
        if self.start_menu_open {
            self.render_start_menu(renderer, y);
        }

        // ── Background (frosted glass effect) ──
        renderer.fill_rect(0, y, self.width, h, Surface::TASKBAR);

        // Subtle top highlight line
        renderer.draw_hline(0, y, self.width, Surface::TASKBAR_BORDER);

        // ── Start button ──
        self.render_start_button(renderer, y);

        // ── Running-app items ──
        let item_width = 130i32;
        let mut x = 54i32;
        for (i, item) in self.items.iter().enumerate() {
            let hovered = self.hovered_item == Some(i);
            self.render_item(renderer, x, y, item_width, &item.title, hovered);
            x += item_width + 4;
        }

        // ── Clock / system tray ──
        self.render_clock(renderer, y);
    }

    // ── Start menu ──────────────────────────────────────────────

    fn render_start_menu(&self, renderer: &mut Renderer, taskbar_y: i32) {
        let mw = Size::MENU_WIDTH;
        let item_h = Size::MENU_ITEM_HEIGHT;
        let header_h = 48u32;
        let items_h = self.start_menu_items.len() as u32 * item_h + Spacing::SM;
        let mh = header_h + items_h + Spacing::SM;
        let mx = Spacing::SM as i32;
        let my = taskbar_y - mh as i32 - Spacing::SM as i32;

        // Shadow
        renderer.draw_shadow_box(mx, my, mw, mh, Radius::MENU, 0, 6, 20,
                                  Color::rgba(0, 0, 0, 55));

        // Background
        renderer.fill_rounded_rect_aa(mx, my, mw, mh, Radius::MENU, Surface::MENU_BG);
        renderer.draw_rounded_rect_aa(mx, my, mw, mh, Radius::MENU,
                                       Color::rgba(255, 255, 255, 10));

        // Header gradient
        renderer.fill_rounded_rect_aa(mx + 1, my + 1, mw - 2, header_h - 1,
                                        Radius::MENU, Surface::MENU_HEADER);
        renderer.draw_text_scaled(mx + Spacing::MD as i32, my + 12, "KPIO",
                                   Text::ON_ACCENT, 2);

        // Menu items
        let mut iy = my + header_h as i32 + Spacing::XXS as i32;
        for (i, item) in self.start_menu_items.iter().enumerate() {
            let hovered = self.hovered_start_item == Some(i);
            if hovered {
                renderer.fill_rounded_rect_aa(
                    mx + Spacing::XXS as i32, iy,
                    mw - Spacing::SM, item_h,
                    Radius::SM, Surface::MENU_HOVER,
                );
            }
            renderer.draw_text(mx + Spacing::LG as i32, iy + (item_h as i32 - 8) / 2,
                               &item.name, Text::ON_DARK);
            iy += item_h as i32;
        }
    }

    /// Check start menu click
    pub fn check_start_menu_click(&mut self, x: i32, y: i32, screen_height: u32) -> Option<AppType> {
        if !self.start_menu_open {
            return None;
        }

        let taskbar_y = (screen_height - self.height) as i32;
        let item_h = Size::MENU_ITEM_HEIGHT;
        let header_h = 48u32;
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
                    let app_type = self.start_menu_items[idx].app_type;
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
        let bh = Size::TASKBAR_HEIGHT - 8;
        let bx = Spacing::SM as i32;
        let by = y + 4;

        let bg = if self.start_menu_open {
            Surface::TASKBAR_ACTIVE
        } else {
            Surface::TASKBAR_ITEM
        };
        renderer.fill_rounded_rect_aa(bx, by, bw, bh, Radius::TASKBAR_ITEM, bg);

        // "K" logo
        renderer.draw_text_scaled(bx + 10, by + 4, "K", Accent::PRIMARY, 3);
    }

    // ── Taskbar item ────────────────────────────────────────────

    fn render_item(&self, renderer: &mut Renderer, x: i32, y: i32, width: i32, title: &str, hovered: bool) {
        let ih = Size::TASKBAR_HEIGHT - 8;
        let iy = y + 4;

        let bg = if hovered { Surface::TASKBAR_HOVER } else { Surface::TASKBAR_ITEM };
        renderer.fill_rounded_rect_aa(x, iy, width as u32, ih, Radius::TASKBAR_ITEM, bg);

        // Truncate title
        let max_chars = ((width - 12) / 8) as usize;
        let display: String = if title.len() > max_chars {
            title.chars().take(max_chars.saturating_sub(2)).collect::<String>() + ".."
        } else {
            String::from(title)
        };
        renderer.draw_text(x + 8, iy as i32 + (ih as i32 - 8) / 2, &display, Text::ON_DARK);

        // Active indicator dot
        renderer.fill_circle_aa(x + width / 2, y + Size::TASKBAR_HEIGHT as i32 - 4, 2, Accent::PRIMARY);
    }

    // ── Clock ───────────────────────────────────────────────────

    fn render_clock(&self, renderer: &mut Renderer, y: i32) {
        let clock_x = (self.width - 56) as i32;
        let cy = y + (Size::TASKBAR_HEIGHT as i32 - 8) / 2;
        renderer.draw_text(clock_x, cy, "12:00", Text::ON_DARK);
    }
}
