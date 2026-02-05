//! Taskbar
//!
//! Bottom taskbar with start menu, running apps, and system tray.

use super::render::{Color, Renderer};
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
        let item_width = 120;
        let start_x = 52;
        let idx = ((x - start_x) / item_width) as usize;
        
        if idx < self.items.len() {
            self.hovered_item = Some(idx);
            return Some(idx);
        }
        
        None
    }

    /// Render taskbar
    pub fn render(&self, renderer: &mut Renderer, y_offset: u32) {
        let y = y_offset as i32;

        // Draw start menu if open
        if self.start_menu_open {
            self.render_start_menu(renderer, y);
        }

        // Draw background
        renderer.fill_rect(0, y, self.width, self.height, Color::TASKBAR_BG);
        
        // Draw top border
        renderer.draw_hline(0, y, self.width, Color::rgb(60, 60, 60));

        // Draw start button
        self.render_start_button(renderer, y);

        // Draw taskbar items
        let item_width = 120;
        let mut x = 52i32;
        
        for (i, item) in self.items.iter().enumerate() {
            let is_hovered = self.hovered_item == Some(i);
            self.render_item(renderer, x, y, item_width, &item.title, is_hovered);
            x += item_width + 2;
        }

        // Draw clock on the right
        self.render_clock(renderer, y);
    }
    
    /// Render start menu
    fn render_start_menu(&self, renderer: &mut Renderer, taskbar_y: i32) {
        let menu_width = 200u32;
        let menu_height = 200u32;
        let menu_x = 0i32;
        let menu_y = taskbar_y - menu_height as i32;
        
        // Draw menu background
        renderer.fill_rect(menu_x, menu_y, menu_width, menu_height, Color::rgb(40, 40, 40));
        renderer.draw_rect(menu_x, menu_y, menu_width, menu_height, Color::rgb(80, 80, 80));
        
        // Draw header
        renderer.fill_rect(menu_x, menu_y, menu_width, 40, Color::rgb(0, 100, 180));
        renderer.draw_text_scaled(menu_x + 10, menu_y + 8, "KPIO", Color::WHITE, 2);
        
        // Draw menu items
        let mut item_y = menu_y + 50;
        for (i, item) in self.start_menu_items.iter().enumerate() {
            let bg = if self.hovered_start_item == Some(i) {
                Color::rgb(60, 60, 60)
            } else {
                Color::rgb(40, 40, 40)
            };
            
            renderer.fill_rect(menu_x + 5, item_y, menu_width - 10, 30, bg);
            renderer.draw_text(menu_x + 15, item_y + 8, &item.name, Color::WHITE);
            item_y += 35;
        }
    }

    /// Check start menu click
    pub fn check_start_menu_click(&mut self, x: i32, y: i32, screen_height: u32) -> Option<AppType> {
        if !self.start_menu_open {
            return None;
        }
        
        let taskbar_y = (screen_height - self.height) as i32;
        let menu_y = taskbar_y - 200;
        
        // Check if click is in menu area
        if x >= 0 && x < 200 && y >= menu_y && y < taskbar_y {
            let item_y_start = menu_y + 50;
            let idx = ((y - item_y_start) / 35) as usize;
            
            if idx < self.start_menu_items.len() {
                let app_type = self.start_menu_items[idx].app_type;
                self.start_menu_open = false;
                return Some(app_type);
            }
        }
        
        None
    }

    /// Render start button
    fn render_start_button(&self, renderer: &mut Renderer, y: i32) {
        let bg = if self.start_menu_open {
            Color::rgb(50, 50, 50)
        } else {
            Color::TASKBAR_BG
        };
        
        renderer.fill_rect(2, y + 2, 44, self.height - 4, bg);
        
        // Draw KPIO logo
        renderer.draw_text_scaled(8, y + 8, "K", Color::rgb(0, 150, 255), 3);
    }

    /// Render taskbar item
    fn render_item(&self, renderer: &mut Renderer, x: i32, y: i32, width: i32, title: &str, hovered: bool) {
        let bg = if hovered {
            Color::rgb(60, 60, 60)
        } else {
            Color::rgb(45, 45, 45)
        };

        renderer.fill_rect(x, y + 4, width as u32, self.height - 8, bg);
        
        // Truncate title if too long
        let max_chars = (width / 8 - 2) as usize;
        let display_title: String = if title.len() > max_chars {
            title.chars().take(max_chars - 2).collect::<String>() + ".."
        } else {
            String::from(title)
        };
        
        renderer.draw_text(x + 4, y + 12, &display_title, Color::WHITE);
    }

    /// Render clock
    fn render_clock(&self, renderer: &mut Renderer, y: i32) {
        // Simple static clock for now
        let clock_x = (self.width - 60) as i32;
        renderer.draw_text(clock_x, y + 12, "12:00", Color::WHITE);
    }
}
