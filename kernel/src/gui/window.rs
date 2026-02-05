//! Window System
//!
//! Windowing primitives for GUI applications.

use super::render::{Color, Renderer};
use alloc::string::String;
use alloc::vec::Vec;

/// Window identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

/// Window state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    Normal,
    Minimized,
    Maximized,
}

/// Window structure
pub struct Window {
    pub id: WindowId,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub state: WindowState,
    pub content: WindowContent,
    /// Input text buffer for text input
    pub input_buffer: String,
    /// Saved position for restore from maximized/minimized
    saved_x: i32,
    saved_y: i32,
    saved_width: u32,
    saved_height: u32,
    /// Hovered button (-1=none, 0=close, 1=maximize, 2=minimize)
    pub hovered_button: i8,
    /// Scroll position for content
    pub scroll_y: i32,
}

/// Window content types
pub enum WindowContent {
    /// Simple text content
    Text(String),
    /// Browser window
    Browser { url: String, content: String },
    /// Terminal window
    Terminal { lines: Vec<String>, cursor_pos: usize },
    /// File manager
    FileManager { path: String, items: Vec<String> },
    /// Settings
    Settings,
}

impl Window {
    /// Create a new window
    pub fn new(id: WindowId, title: &str, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            title: String::from(title),
            x,
            y,
            width,
            height,
            state: WindowState::Normal,
            content: WindowContent::Text(String::from("Window content here")),
            input_buffer: String::new(),
            saved_x: x,
            saved_y: y,
            saved_width: width,
            saved_height: height,
            hovered_button: -1,
            scroll_y: 0,
        }
    }

    /// Create browser window
    pub fn new_browser(id: WindowId, x: i32, y: i32) -> Self {
        Self {
            id,
            title: String::from("KPIO Browser"),
            x,
            y,
            width: 800,
            height: 600,
            state: WindowState::Normal,
            content: WindowContent::Browser {
                url: String::from("kpio://home"),
                content: String::from("Welcome to KPIO Browser"),
            },
            input_buffer: String::new(),
            saved_x: x,
            saved_y: y,
            saved_width: 800,
            saved_height: 600,
            hovered_button: -1,
            scroll_y: 0,
        }
    }

    /// Create terminal window
    pub fn new_terminal(id: WindowId, x: i32, y: i32) -> Self {
        Self {
            id,
            title: String::from("Terminal"),
            x,
            y,
            width: 640,
            height: 400,
            state: WindowState::Normal,
            content: WindowContent::Terminal {
                lines: alloc::vec![
                    String::from("KPIO Terminal v1.0"),
                    String::from("Type 'help' for commands"),
                    String::from(""),
                    String::from("$ _"),
                ],
                cursor_pos: 0,
            },
            input_buffer: String::new(),
            saved_x: x,
            saved_y: y,
            saved_width: 640,
            saved_height: 400,
            hovered_button: -1,
            scroll_y: 0,
        }
    }

    /// Create file manager window
    pub fn new_files(id: WindowId, x: i32, y: i32) -> Self {
        Self {
            id,
            title: String::from("Files"),
            x,
            y,
            width: 700,
            height: 500,
            state: WindowState::Normal,
            content: WindowContent::FileManager {
                path: String::from("/home"),
                items: alloc::vec![
                    String::from("Documents"),
                    String::from("Downloads"),
                    String::from("Pictures"),
                    String::from("Music"),
                    String::from("Videos"),
                ],
            },
            input_buffer: String::new(),
            saved_x: x,
            saved_y: y,
            saved_width: 700,
            saved_height: 500,
            hovered_button: -1,
            scroll_y: 0,
        }
    }

    /// Create settings window
    pub fn new_settings(id: WindowId, x: i32, y: i32) -> Self {
        Self {
            id,
            title: String::from("Settings"),
            x,
            y,
            width: 600,
            height: 450,
            state: WindowState::Normal,
            content: WindowContent::Settings,
            input_buffer: String::new(),
            saved_x: x,
            saved_y: y,
            saved_width: 600,
            saved_height: 450,
            hovered_button: -1,
            scroll_y: 0,
        }
    }

    /// Minimize window
    pub fn minimize(&mut self) {
        if self.state != WindowState::Minimized {
            if self.state == WindowState::Normal {
                self.saved_x = self.x;
                self.saved_y = self.y;
                self.saved_width = self.width;
                self.saved_height = self.height;
            }
            self.state = WindowState::Minimized;
        }
    }

    /// Maximize window (needs screen dimensions)
    pub fn maximize(&mut self, screen_w: u32, screen_h: u32, taskbar_h: u32) {
        if self.state == WindowState::Maximized {
            // Restore to normal
            self.x = self.saved_x;
            self.y = self.saved_y;
            self.width = self.saved_width;
            self.height = self.saved_height;
            self.state = WindowState::Normal;
        } else {
            // Save current state if normal
            if self.state == WindowState::Normal {
                self.saved_x = self.x;
                self.saved_y = self.y;
                self.saved_width = self.width;
                self.saved_height = self.height;
            }
            // Maximize
            self.x = 0;
            self.y = 0;
            self.width = screen_w;
            self.height = screen_h - taskbar_h;
            self.state = WindowState::Maximized;
        }
    }

    /// Restore window from minimized
    pub fn restore(&mut self) {
        if self.state == WindowState::Minimized {
            self.state = WindowState::Normal;
        }
    }

    /// Check if visible (not minimized)
    pub fn is_visible(&self) -> bool {
        self.state != WindowState::Minimized
    }

    /// Update button hover state
    pub fn update_hover(&mut self, local_x: i32, local_y: i32) {
        self.hovered_button = -1;
        if local_y < 24 && local_y >= 0 {
            let close_x = self.width as i32 - 24;
            let max_x = close_x - 24;
            let min_x = max_x - 24;
            
            if local_x >= close_x && local_x < close_x + 24 {
                self.hovered_button = 0; // Close
            } else if local_x >= max_x && local_x < max_x + 24 {
                self.hovered_button = 1; // Maximize
            } else if local_x >= min_x && local_x < min_x + 24 {
                self.hovered_button = 2; // Minimize
            }
        }
    }

    /// Check if point is inside window
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.width as i32 &&
        py >= self.y && py < self.y + self.height as i32
    }

    /// Handle click
    pub fn on_click(&mut self, local_x: i32, local_y: i32, _pressed: bool) {
        // Check close button (top right)
        if local_y < 24 && local_x >= (self.width as i32 - 24) {
            // Close button clicked - handled by GUI system
        }
    }

    /// Handle key input
    pub fn on_key(&mut self, key: char, pressed: bool) {
        if !pressed {
            return;
        }

        match &mut self.content {
            WindowContent::Terminal { lines, .. } => {
                if key == '\n' {
                    // Execute command
                    let cmd = self.input_buffer.clone();
                    lines.push(alloc::format!("$ {}", cmd));
                    
                    // Parse and execute command
                    let parts: Vec<&str> = cmd.split_whitespace().collect();
                    if !parts.is_empty() {
                        match parts[0] {
                            "help" => {
                                lines.push(String::from("KPIO Terminal - Available commands:"));
                                lines.push(String::from("  help     - Show this help"));
                                lines.push(String::from("  clear    - Clear screen"));
                                lines.push(String::from("  echo     - Print text"));
                                lines.push(String::from("  uname    - System information"));
                                lines.push(String::from("  whoami   - Current user"));
                                lines.push(String::from("  date     - Show date"));
                                lines.push(String::from("  ls       - List files"));
                                lines.push(String::from("  pwd      - Print working directory"));
                                lines.push(String::from("  cat      - Display file contents"));
                                lines.push(String::from("  neofetch - System info"));
                            }
                            "clear" => {
                                lines.clear();
                            }
                            "echo" => {
                                let text = parts[1..].join(" ");
                                lines.push(text);
                            }
                            "uname" => {
                                if parts.len() > 1 && parts[1] == "-a" {
                                    lines.push(String::from("KPIO 1.0.0 x86_64 KPIO-Kernel"));
                                } else {
                                    lines.push(String::from("KPIO"));
                                }
                            }
                            "whoami" => {
                                lines.push(String::from("root"));
                            }
                            "date" => {
                                lines.push(String::from("Wed Feb  4 12:00:00 KST 2026"));
                            }
                            "ls" => {
                                lines.push(String::from("Documents  Downloads  Pictures  Music  Videos"));
                            }
                            "pwd" => {
                                lines.push(String::from("/home/root"));
                            }
                            "cat" => {
                                if parts.len() > 1 {
                                    lines.push(alloc::format!("cat: {}: File contents here", parts[1]));
                                } else {
                                    lines.push(String::from("cat: missing file operand"));
                                }
                            }
                            "neofetch" => {
                                lines.push(String::from(""));
                                lines.push(String::from("  _  ______  ___ ___  "));
                                lines.push(String::from(" | |/ /  _ \\|_ _/ _ \\ "));
                                lines.push(String::from(" | ' /| |_) || | | | |"));
                                lines.push(String::from(" | . \\|  __/ | | |_| |"));
                                lines.push(String::from(" |_|\\_\\_|   |___\\___/ "));
                                lines.push(String::from(""));
                                lines.push(String::from(" OS: KPIO 1.0.0"));
                                lines.push(String::from(" Kernel: x86_64"));
                                lines.push(String::from(" Shell: kpio-term"));
                                lines.push(String::from(" Resolution: 1280x720"));
                                lines.push(String::from(" CPU: QEMU Virtual CPU"));
                                lines.push(String::from(" Memory: 512 MB"));
                            }
                            "" => {}
                            _ => {
                                lines.push(alloc::format!("kpio: command not found: {}", parts[0]));
                            }
                        }
                    }
                    
                    lines.push(String::from("$ "));
                    self.input_buffer.clear();
                    
                    // Keep only last 50 lines
                    while lines.len() > 50 {
                        lines.remove(0);
                    }
                } else if key == '\x08' {
                    // Backspace
                    self.input_buffer.pop();
                } else {
                    self.input_buffer.push(key);
                }
            }
            WindowContent::Browser { url, content } => {
                if key == '\n' {
                    // Navigate to URL
                    let new_url = if self.input_buffer.is_empty() {
                        url.clone()
                    } else {
                        self.input_buffer.clone()
                    };
                    
                    *url = new_url.clone();
                    
                    // Render page based on URL
                    *content = match new_url.as_str() {
                        "kpio://home" | "" => String::from("Welcome to KPIO Browser!\n\nTry navigating to:\n- kpio://settings\n- kpio://about\n- kpio://help"),
                        "kpio://settings" => String::from("KPIO Browser Settings\n\n- Theme: Dark\n- Home Page: kpio://home\n- Search Engine: KPIO Search"),
                        "kpio://about" => String::from("KPIO Browser v1.0\n\nA simple web browser for KPIO OS.\nBuilt with Rust and WASM."),
                        "kpio://help" => String::from("KPIO Browser Help\n\n- Type a URL in the address bar\n- Press Enter to navigate\n- Use kpio:// for internal pages"),
                        _ => alloc::format!("Loading {}...\n\n(Page content would appear here)", new_url),
                    };
                    
                    self.input_buffer.clear();
                } else if key == '\x08' {
                    self.input_buffer.pop();
                } else {
                    self.input_buffer.push(key);
                }
            }
            _ => {}
        }
    }

    /// Render window
    pub fn render(&self, renderer: &mut Renderer, is_active: bool) {
        let title_bar_height = 24;
        
        // Draw shadow
        renderer.fill_rect(self.x + 4, self.y + 4, self.width, self.height, Color::rgba(0, 0, 0, 100));

        // Draw window border
        renderer.fill_rect(self.x - 1, self.y - 1, self.width + 2, self.height + 2, Color::DARK_GRAY);

        // Draw title bar
        let title_color = if is_active {
            Color::WINDOW_TITLE_ACTIVE
        } else {
            Color::WINDOW_TITLE_INACTIVE
        };
        renderer.fill_rect(self.x, self.y, self.width, title_bar_height, title_color);

        // Draw title text
        renderer.draw_text(self.x + 8, self.y + 4, &self.title, Color::WHITE);

        // Draw close button
        let close_x = self.x + self.width as i32 - 24;
        renderer.fill_rect(close_x, self.y, 24, title_bar_height, Color::CLOSE_BUTTON_HOVER);
        renderer.draw_text(close_x + 8, self.y + 4, "X", Color::WHITE);

        // Draw minimize button
        let min_x = close_x - 24;
        renderer.fill_rect(min_x, self.y, 24, title_bar_height, Color::BUTTON_HOVER);
        renderer.draw_text(min_x + 8, self.y + 4, "_", Color::BLACK);

        // Draw maximize button
        let max_x = min_x - 24;
        renderer.fill_rect(max_x, self.y, 24, title_bar_height, Color::BUTTON_HOVER);
        renderer.draw_text(max_x + 8, self.y + 4, "O", Color::BLACK);

        // Draw window content area
        let content_y = self.y + title_bar_height as i32;
        let content_height = self.height - title_bar_height;
        renderer.fill_rect(self.x, content_y, self.width, content_height, Color::WINDOW_BG);

        // Render content
        self.render_content(renderer, self.x, content_y, self.width, content_height);
    }

    /// Render window content
    fn render_content(&self, renderer: &mut Renderer, x: i32, y: i32, _w: u32, _h: u32) {
        match &self.content {
            WindowContent::Text(text) => {
                renderer.draw_text(x + 10, y + 10, text, Color::BLACK);
            }
            WindowContent::Browser { url, content } => {
                // Draw address bar background
                renderer.fill_rect(x + 5, y + 5, self.width - 10, 24, Color::WHITE);
                renderer.draw_rect(x + 5, y + 5, self.width - 10, 24, Color::GRAY);
                
                // Show input buffer if typing, otherwise show URL
                let display_url = if !self.input_buffer.is_empty() {
                    alloc::format!("{}|", self.input_buffer)
                } else {
                    url.clone()
                };
                renderer.draw_text(x + 10, y + 9, &display_url, Color::BLACK);

                // Draw content area with multiline support
                let mut line_y = y + 40;
                for line in content.lines() {
                    renderer.draw_text(x + 10, line_y, line, Color::BLACK);
                    line_y += 14;
                }
            }
            WindowContent::Terminal { lines, .. } => {
                // Draw terminal background
                let content_height = self.height.saturating_sub(24);
                renderer.fill_rect(x, y, self.width, content_height, Color::BLACK);

                // Calculate visible lines
                let max_lines = (content_height / 12) as usize;
                let visible_lines: Vec<&String> = lines.iter().rev().take(max_lines.saturating_sub(1)).collect();
                
                // Draw lines (oldest first)
                let mut line_y = y + 5;
                for line in visible_lines.iter().rev() {
                    renderer.draw_text(x + 5, line_y, line, Color::rgb(0, 255, 0));
                    line_y += 12;
                }
                
                // Draw current input line with cursor
                let input_line = alloc::format!("$ {}|", self.input_buffer);
                renderer.draw_text(x + 5, line_y, &input_line, Color::rgb(0, 255, 0));
            }
            WindowContent::FileManager { path, items } => {
                // Draw path bar
                renderer.fill_rect(x + 5, y + 5, self.width - 10, 24, Color::WHITE);
                renderer.draw_rect(x + 5, y + 5, self.width - 10, 24, Color::GRAY);
                renderer.draw_text(x + 10, y + 9, path, Color::BLACK);

                // Draw folder icon and items
                let mut item_y = y + 45;
                for item in items {
                    // Draw folder icon (simple square)
                    renderer.fill_rect(x + 10, item_y, 16, 14, Color::rgb(255, 200, 50));
                    renderer.draw_text(x + 32, item_y, item, Color::BLACK);
                    item_y += 24;
                }
            }
            WindowContent::Settings => {
                // Draw header
                renderer.draw_text_scaled(x + 10, y + 10, "Settings", Color::BLACK, 2);
                
                // Draw setting categories
                let categories = [
                    ("Display", "Resolution, brightness, theme"),
                    ("Sound", "Volume, output device"),
                    ("Network", "WiFi, Ethernet, VPN"),
                    ("System", "Updates, backup, security"),
                    ("About", "KPIO OS version 1.0"),
                ];
                
                let mut item_y = y + 50;
                for (name, desc) in categories {
                    // Draw category box
                    renderer.fill_rect(x + 10, item_y, self.width - 20, 40, Color::rgb(250, 250, 250));
                    renderer.draw_rect(x + 10, item_y, self.width - 20, 40, Color::LIGHT_GRAY);
                    renderer.draw_text(x + 20, item_y + 5, name, Color::BLACK);
                    renderer.draw_text(x + 20, item_y + 20, desc, Color::GRAY);
                    item_y += 50;
                }
            }
        }
    }
}
