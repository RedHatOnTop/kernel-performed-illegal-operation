//! Window System
//!
//! Modern window manager with rounded corners, soft shadows,
//! themed title bars and polished application content areas.

use super::render::{Color, Renderer};
use super::theme::{Surface, Text, Accent, Shadow, Spacing, Radius, Size, TermTheme, IconColor, Shadows};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Get folder contents for virtual filesystem
fn get_folder_contents(path: &str) -> Vec<String> {
    match path {
        "/" => alloc::vec![
            String::from("home"),
            String::from("etc"),
            String::from("usr"),
            String::from("var"),
            String::from("tmp"),
        ],
        "/home" => alloc::vec![
            String::from(".."),
            String::from("Documents"),
            String::from("Downloads"),
            String::from("Pictures"),
            String::from("Music"),
            String::from("Videos"),
        ],
        "/home/Documents" => alloc::vec![
            String::from(".."),
            String::from("readme.txt"),
            String::from("notes.txt"),
            String::from("report.pdf"),
        ],
        "/home/Downloads" => alloc::vec![
            String::from(".."),
            String::from("installer.exe"),
            String::from("archive.zip"),
        ],
        "/home/Pictures" => alloc::vec![
            String::from(".."),
            String::from("vacation.jpg"),
            String::from("screenshot.png"),
        ],
        "/home/Music" => alloc::vec![
            String::from(".."),
            String::from("track01.mp3"),
            String::from("album/"),
        ],
        "/home/Videos" => alloc::vec![
            String::from(".."),
            String::from("tutorial.mp4"),
        ],
        "/etc" => alloc::vec![
            String::from(".."),
            String::from("passwd"),
            String::from("hosts"),
            String::from("config/"),
        ],
        "/usr" => alloc::vec![
            String::from(".."),
            String::from("bin/"),
            String::from("lib/"),
            String::from("share/"),
        ],
        "/var" => alloc::vec![
            String::from(".."),
            String::from("log/"),
            String::from("cache/"),
        ],
        "/tmp" => alloc::vec![
            String::from(".."),
        ],
        _ => {
            // Default: show parent link
            alloc::vec![String::from("..")]
        }
    }
}

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
        let tb = Size::TITLE_BAR_HEIGHT as i32;
        let bw = Size::WIN_BTN_W as i32;
        if local_y < tb && local_y >= 0 {
            let close_x = self.width as i32 - bw;
            let max_x = close_x - bw;
            let min_x = max_x - bw;

            if local_x >= close_x && local_x < close_x + bw {
                self.hovered_button = 0; // Close
            } else if local_x >= max_x && local_x < max_x + bw {
                self.hovered_button = 1; // Maximize
            } else if local_x >= min_x && local_x < min_x + bw {
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
    pub fn on_click(&mut self, local_x: i32, local_y: i32, pressed: bool) {
        if !pressed {
            return;
        }
        
        let title_bar_height = Size::TITLE_BAR_HEIGHT as i32;
        let content_y = local_y - title_bar_height;
        
        if content_y < 0 {
            // Clicked on title bar - handled elsewhere
            return;
        }
        
        match &mut self.content {
            WindowContent::FileManager { path, items } => {
                // Address bar click
                if content_y >= 5 && content_y < 29 {
                    // Focus address bar (future: allow editing path)
                    return;
                }
                
                // Item click (items start at y=45)
                let item_start_y = 45;
                let item_height = 24;
                
                if content_y >= item_start_y {
                    let idx = ((content_y - item_start_y) / item_height) as usize;
                    if idx < items.len() {
                        // Navigate into folder
                        let item = items[idx].clone();
                        if item == ".." {
                            // Go up one level
                            if let Some(last_slash) = path.rfind('/') {
                                if last_slash > 0 {
                                    *path = path[..last_slash].to_string();
                                } else {
                                    *path = String::from("/");
                                }
                            }
                        } else {
                            // Navigate into folder
                            if *path == "/" {
                                *path = alloc::format!("/{}", item);
                            } else {
                                *path = alloc::format!("{}/{}", path, item);
                            }
                        }
                        
                        // Update items based on new path
                        *items = get_folder_contents(path);
                    }
                }
            }
            WindowContent::Settings => {
                // Settings category click
                let item_start_y = 50;
                let item_height = 50;
                
                if content_y >= item_start_y {
                    let _idx = ((content_y - item_start_y) / item_height) as usize;
                    // Future: Open settings sub-panel
                }
            }
            WindowContent::Browser { .. } => {
                // Click in address bar area
                if content_y >= 5 && content_y < 29 {
                    // Focus address bar
                }
            }
            _ => {}
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
        let tb = Size::TITLE_BAR_HEIGHT;
        let r = Radius::WINDOW;
        let bw = Size::WIN_BTN_W;
        let bh = Size::WIN_BTN_H;

        // ── Shadow ──
        let shadow = Shadows::WINDOW;
        renderer.draw_shadow_box(
            self.x, self.y, self.width, self.height,
            r, shadow.offset_x, shadow.offset_y, shadow.blur, shadow.color,
        );

        // ── Window body ──
        renderer.fill_rounded_rect_aa(self.x, self.y, self.width, self.height, r,
                                       Surface::WINDOW_BG);

        // ── Title bar ──
        let title_color = if is_active {
            Surface::WINDOW_TITLE_ACTIVE
        } else {
            Surface::WINDOW_TITLE_INACTIVE
        };
        // Fill top portion with title-bar colour (only top corners rounded)
        renderer.fill_rounded_rect_aa(self.x, self.y, self.width, tb, r, title_color);
        // Flatten the bottom half of the title-bar rounded rect
        renderer.fill_rect(self.x, self.y + r as i32, self.width, tb - r, title_color);

        // Title bar separator
        renderer.draw_hline(self.x, self.y + tb as i32 - 1, self.width,
                            Color::rgba(0, 0, 0, 12));

        // Title text (vertically centered)
        renderer.draw_text(self.x + Spacing::MD as i32, self.y + (tb as i32 - 8) / 2,
                           &self.title, Text::PRIMARY);

        // ── Window control buttons ──
        let close_x = self.x + self.width as i32 - bw as i32;
        let max_x = close_x - bw as i32;
        let min_x = max_x - bw as i32;

        // Close button
        let close_bg = if self.hovered_button == 0 {
            Accent::DANGER
        } else {
            Color::TRANSPARENT
        };
        renderer.fill_rect(close_x, self.y, bw, bh, close_bg);
        let close_text_c = if self.hovered_button == 0 { Text::ON_ACCENT } else { Text::SECONDARY };
        renderer.draw_text(close_x + (bw as i32 - 8) / 2, self.y + (bh as i32 - 8) / 2,
                           "X", close_text_c);

        // Maximize button
        let max_bg = if self.hovered_button == 1 {
            Color::rgba(0, 0, 0, 15)
        } else {
            Color::TRANSPARENT
        };
        renderer.fill_rect(max_x, self.y, bw, bh, max_bg);
        renderer.draw_text(max_x + (bw as i32 - 8) / 2, self.y + (bh as i32 - 8) / 2,
                           "O", Text::SECONDARY);

        // Minimize button
        let min_bg = if self.hovered_button == 2 {
            Color::rgba(0, 0, 0, 15)
        } else {
            Color::TRANSPARENT
        };
        renderer.fill_rect(min_x, self.y, bw, bh, min_bg);
        renderer.draw_text(min_x + (bw as i32 - 8) / 2, self.y + (bh as i32 - 8) / 2,
                           "_", Text::SECONDARY);

        // ── Border ──
        let border_c = if is_active {
            Surface::WINDOW_BORDER_ACTIVE
        } else {
            Surface::WINDOW_BORDER_INACTIVE
        };
        renderer.draw_rounded_rect_aa(self.x, self.y, self.width, self.height, r, border_c);

        // ── Content area ──
        let content_y = self.y + tb as i32;
        let content_height = self.height.saturating_sub(tb);
        self.render_content(renderer, self.x, content_y, self.width, content_height);
    }

    /// Render window content
    fn render_content(&self, renderer: &mut Renderer, x: i32, y: i32, w: u32, h: u32) {
        match &self.content {
            WindowContent::Text(text) => {
                renderer.draw_text(x + Spacing::MD as i32, y + Spacing::MD as i32, text, Text::PRIMARY);
            }
            WindowContent::Browser { url, content } => {
                // ── Address bar ──
                let bar_h = Size::INPUT_HEIGHT;
                let bar_x = x + Spacing::SM as i32;
                let bar_y = y + Spacing::SM as i32;
                let bar_w = w - Spacing::LG;
                renderer.fill_rounded_rect_aa(bar_x, bar_y, bar_w, bar_h, Radius::INPUT,
                                               Surface::INPUT_BG);
                renderer.draw_rounded_rect_aa(bar_x, bar_y, bar_w, bar_h, Radius::INPUT,
                                               Surface::INPUT_BORDER);

                let display_url = if !self.input_buffer.is_empty() {
                    alloc::format!("{}|", self.input_buffer)
                } else {
                    url.clone()
                };
                renderer.draw_text(bar_x + Spacing::SM as i32,
                                   bar_y + (bar_h as i32 - 8) / 2,
                                   &display_url, Text::PRIMARY);

                // ── Page content ──
                let mut line_y = y + Spacing::SM as i32 + bar_h as i32 + Spacing::SM as i32;
                for line in content.lines() {
                    renderer.draw_text(x + Spacing::MD as i32, line_y, line, Text::PRIMARY);
                    line_y += 16;
                }
            }
            WindowContent::Terminal { lines, .. } => {
                // ── Terminal background ──
                let r_bot = Radius::WINDOW;
                renderer.fill_rounded_rect_aa(x, y, w, h, 0, TermTheme::BG);
                // Re-round bottom corners only by drawing a small rounded rect at bottom
                renderer.fill_rounded_rect_aa(x, y + h as i32 - r_bot as i32 * 2,
                                               w, r_bot * 2, r_bot, TermTheme::BG);

                // Visible lines
                let max_lines = (h / 14) as usize;
                let visible: Vec<&String> = lines.iter().rev().take(max_lines.saturating_sub(1)).collect();

                let mut ly = y + Spacing::SM as i32;
                for line in visible.iter().rev() {
                    let c = if line.starts_with("$") { TermTheme::PROMPT }
                            else if line.starts_with("kpio:") { TermTheme::ERROR }
                            else { TermTheme::FG };
                    renderer.draw_text(x + Spacing::SM as i32, ly, line, c);
                    ly += 14;
                }

                // Current input
                let input_line = alloc::format!("$ {}|", self.input_buffer);
                renderer.draw_text(x + Spacing::SM as i32, ly, &input_line, TermTheme::PROMPT);
            }
            WindowContent::FileManager { path, items } => {
                // ── Path / toolbar bar ──
                let bar_h = Size::INPUT_HEIGHT;
                let bar_x = x + Spacing::SM as i32;
                let bar_y = y + Spacing::SM as i32;
                let bar_w = w - Spacing::LG;
                renderer.fill_rounded_rect_aa(bar_x, bar_y, bar_w, bar_h, Radius::INPUT,
                                               Surface::INPUT_BG);
                renderer.draw_rounded_rect_aa(bar_x, bar_y, bar_w, bar_h, Radius::INPUT,
                                               Surface::INPUT_BORDER);

                // Back button
                renderer.fill_rounded_rect_aa(bar_x + 2, bar_y + 2, 22, bar_h - 4,
                                               Radius::SM, Surface::PANEL);
                renderer.draw_text(bar_x + 8, bar_y + (bar_h as i32 - 8) / 2, "<", Text::SECONDARY);

                // Path
                renderer.draw_text(bar_x + 30, bar_y + (bar_h as i32 - 8) / 2, path, Text::PRIMARY);

                // ── Column headers ──
                let hdr_y = bar_y + bar_h as i32 + Spacing::XXS as i32;
                renderer.fill_rect(x + Spacing::SM as i32, hdr_y, bar_w, 20, Surface::PANEL);
                renderer.draw_text(x + Spacing::LG as i32, hdr_y + 4, "Name", Text::SECONDARY);
                renderer.draw_text(x + 350, hdr_y + 4, "Type", Text::SECONDARY);
                renderer.draw_text(x + 500, hdr_y + 4, "Size", Text::SECONDARY);

                // ── Items ──
                let mut iy = hdr_y + 24;
                for (i, item) in items.iter().enumerate() {
                    let is_folder = item == ".." || !item.contains('.');
                    let row_bg = if i % 2 == 0 { Surface::WINDOW_BG } else { Surface::PANEL };
                    renderer.fill_rect(x + Spacing::SM as i32, iy - 2, bar_w, 22, row_bg);

                    // Mini icon
                    if item == ".." {
                        renderer.fill_rounded_rect_aa(x + 12, iy, 14, 14, 3, Accent::PRIMARY.with_alpha(60));
                        renderer.draw_text(x + 15, iy + 1, "^", Text::ON_ACCENT);
                    } else if is_folder {
                        renderer.fill_rounded_rect_aa(x + 12, iy, 14, 14, 3, IconColor::FOLDER);
                    } else {
                        renderer.fill_rounded_rect_aa(x + 12, iy, 14, 14, 3, IconColor::FILE);
                    }

                    renderer.draw_text(x + 32, iy + 1, item, Text::PRIMARY);

                    let ftype = if item == ".." { "Parent" }
                        else if is_folder { "Folder" }
                        else if item.ends_with(".txt") { "Text" }
                        else if item.ends_with(".pdf") { "PDF" }
                        else if item.ends_with(".jpg") || item.ends_with(".png") { "Image" }
                        else if item.ends_with(".mp3") { "Audio" }
                        else if item.ends_with(".mp4") { "Video" }
                        else if item.ends_with(".zip") { "Archive" }
                        else if item.ends_with(".exe") { "Executable" }
                        else { "File" };
                    renderer.draw_text(x + 350, iy + 1, ftype, Text::SECONDARY);

                    let size = if is_folder { "-" } else { "4 KB" };
                    renderer.draw_text(x + 500, iy + 1, size, Text::SECONDARY);

                    iy += 24;
                }

                // Status bar
                let status_y = (y + h as i32 - 24).max(iy + 8);
                renderer.fill_rect(x, status_y, w, 22, Surface::PANEL);
                renderer.draw_text(x + Spacing::MD as i32, status_y + 4,
                    &alloc::format!("{} items", items.len()), Text::SECONDARY);
            }
            WindowContent::Settings => {
                // ── Header ──
                renderer.draw_text_scaled(x + Spacing::LG as i32, y + Spacing::MD as i32,
                                          "Settings", Text::PRIMARY, 2);

                // ── Category cards ──
                let categories = [
                    ("Display",  "Resolution, brightness, theme"),
                    ("Sound",    "Volume, output device"),
                    ("Network",  "WiFi, Ethernet, VPN"),
                    ("System",   "Updates, backup, security"),
                    ("About",    "KPIO OS version 1.0"),
                ];
                let card_w = w - Spacing::XL * 2;
                let card_h = 46u32;
                let mut cy = y + 44;
                for (name, desc) in categories {
                    renderer.fill_rounded_rect_aa(
                        x + Spacing::LG as i32, cy, card_w, card_h,
                        Radius::MD, Surface::PANEL,
                    );
                    renderer.draw_rounded_rect_aa(
                        x + Spacing::LG as i32, cy, card_w, card_h,
                        Radius::MD, Color::rgba(0, 0, 0, 10),
                    );
                    renderer.draw_text(x + Spacing::XL as i32, cy + 8, name, Text::PRIMARY);
                    renderer.draw_text(x + Spacing::XL as i32, cy + 24, desc, Text::MUTED);
                    cy += card_h as i32 + Spacing::SM as i32;
                }
            }
        }
    }
}
