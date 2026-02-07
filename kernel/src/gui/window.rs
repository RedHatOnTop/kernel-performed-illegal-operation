//! Window System
//!
//! Modern window manager with rounded corners, soft shadows,
//! themed title bars and polished application content areas.

use super::render::{Color, Renderer};
use super::theme::{Surface, Text, Accent, Shadow, Spacing, Radius, Size, TermTheme, IconColor, Shadows};
use super::html_render::{self, RenderedPage, RenderCmd};
use crate::terminal;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Get folder contents from the in-memory filesystem
fn get_folder_contents(path: &str) -> Vec<String> {
    terminal::fs::with_fs(|fs| {
        let ino = match fs.resolve(path) {
            Some(i) => i,
            None => return alloc::vec![String::from("..")],
        };
        match fs.readdir(ino) {
            Some(entries) => {
                let mut items = Vec::new();
                if path != "/" { items.push(String::from("..")); }
                let mut sorted: Vec<(String, bool)> = entries.iter().map(|(name, child_ino)| {
                    let is_dir = fs.get(*child_ino).map(|n| n.mode.is_dir()).unwrap_or(false);
                    (name.clone(), is_dir)
                }).collect();
                sorted.sort_by(|a, b| {
                    // Directories first, then by name
                    b.1.cmp(&a.1).then(a.0.cmp(&b.0))
                });
                for (name, _) in sorted {
                    items.push(name);
                }
                items
            }
            None => alloc::vec![String::from("..")],
        }
    })
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
    Browser { url: String, content: String, rendered: Option<RenderedPage> },
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
                content: String::new(),
                rendered: Some(navigate_to_url("kpio://home")),
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
        let prompt = terminal::shell::with_shell(|sh| sh.prompt());
        Self {
            id,
            title: String::from("Terminal"),
            x,
            y,
            width: 720,
            height: 460,
            state: WindowState::Normal,
            content: WindowContent::Terminal {
                lines: alloc::vec![
                    String::from("KPIO Shell v2.0 — Type 'help' for commands"),
                    String::from(""),
                ],
                cursor_pos: 0,
            },
            input_buffer: String::new(),
            saved_x: x,
            saved_y: y,
            saved_width: 720,
            saved_height: 460,
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
                    // Execute command via shell engine
                    let cmd = self.input_buffer.clone();
                    let prompt = terminal::shell::with_shell(|sh| sh.prompt());
                    lines.push(alloc::format!("{}{}", prompt, cmd));

                    if !cmd.trim().is_empty() {
                        let result = terminal::shell::execute(&cmd);
                        for line in &result {
                            if line == "\x1B[CLEAR]" {
                                lines.clear();
                            } else {
                                lines.push(line.clone());
                            }
                        }
                    }

                    self.input_buffer.clear();

                    // Keep only last 500 lines for scrollback
                    while lines.len() > 500 {
                        lines.remove(0);
                    }
                } else if key == '\x08' {
                    // Backspace
                    self.input_buffer.pop();
                } else {
                    self.input_buffer.push(key);
                }
            }
            WindowContent::Browser { url, content, rendered } => {
                if key == '\n' {
                    // Navigate to URL
                    let new_url = if self.input_buffer.is_empty() {
                        url.clone()
                    } else {
                        self.input_buffer.clone()
                    };
                    
                    *url = new_url.clone();
                    *rendered = Some(navigate_to_url(&new_url));
                    *content = String::new();
                    self.scroll_y = 0;
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
            WindowContent::Browser { url, rendered, .. } => {
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

                // ── Rendered HTML content ──
                let content_top = y + Spacing::SM as i32 + bar_h as i32 + Spacing::SM as i32;
                let scroll = self.scroll_y;
                if let Some(page) = rendered {
                    for cmd in &page.commands {
                        match cmd {
                            RenderCmd::FillRect { x: rx, y: ry, w: rw, h: rh, color } => {
                                let cy = content_top + ry - scroll;
                                renderer.fill_rect(x + rx, cy, *rw, *rh,
                                    Color::rgba((*color >> 16) as u8, (*color >> 8) as u8, *color as u8, (*color >> 24) as u8));
                            }
                            RenderCmd::Text { x: tx, y: ty, text, color, .. } => {
                                let cy = content_top + ty - scroll;
                                if cy >= content_top - 16 && cy < y + h as i32 {
                                    renderer.draw_text(x + tx, cy, text,
                                        Color::rgba((*color >> 16) as u8, (*color >> 8) as u8, *color as u8, 255));
                                }
                            }
                            RenderCmd::HRule { x: hx, y: hy, w: hw, color } => {
                                let cy = content_top + hy - scroll;
                                if cy >= content_top && cy < y + h as i32 {
                                    renderer.draw_hline(x + hx, cy, *hw,
                                        Color::rgba((*color >> 16) as u8, (*color >> 8) as u8, *color as u8, 255));
                                }
                            }
                        }
                    }
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
                let line_height = 14u32;
                let max_lines = ((h.saturating_sub(Spacing::SM + Spacing::SM)) / line_height) as usize;
                let prompt = terminal::shell::with_shell(|sh| sh.prompt());

                // Reserve 1 line for current input
                let available = max_lines.saturating_sub(1);
                let visible: Vec<&String> = if lines.len() > available {
                    lines[lines.len() - available..].iter().collect()
                } else {
                    lines.iter().collect()
                };

                let mut ly = y + Spacing::SM as i32;
                for line in &visible {
                    let c = if line.contains("@") && line.contains("$") { TermTheme::PROMPT }
                            else if line.starts_with("  ") || line.starts_with("──") { TermTheme::FG }
                            else if line.contains(": command not found")
                                 || line.contains(": No such file")
                                 || line.contains(": cannot ")
                                 || line.contains(": missing ")
                                 || line.contains(": error")
                                 || line.contains(": invalid") { TermTheme::ERROR }
                            else { TermTheme::FG };
                    renderer.draw_text(x + Spacing::SM as i32, ly, line, c);
                    ly += line_height as i32;
                }

                // Current input with prompt
                let input_line = alloc::format!("{}{}|", prompt, self.input_buffer);
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

/// Navigate to a URL and return a rendered page.
///
/// Supports:
/// - `kpio://` internal pages (mapped to HTTP server routes)
/// - `http://localhost/` routes to the built-in HTTP server
/// - Other URLs show a "not reachable" page
fn navigate_to_url(url: &str) -> RenderedPage {
    use crate::net::http;

    let path = if url.starts_with("kpio://") {
        // Map kpio:// URLs to HTTP server routes
        let route = &url[7..]; // strip "kpio://"
        match route {
            "home" | "" => "/",
            "about" => "/about",
            "settings" => "/status",
            "help" => "/about",
            other => {
                let p = alloc::format!("/{}", other);
                return render_http_page(&http::fetch(&p));
            }
        }
    } else if url.starts_with("http://localhost") || url.starts_with("http://127.0.0.1") {
        // Extract path from http://localhost/path
        let rest = if url.starts_with("http://localhost") {
            &url[16..] // "http://localhost".len()
        } else {
            &url[17..] // "http://127.0.0.1:".len() roughly
        };
        if rest.is_empty() || rest == "/" {
            "/"
        } else {
            rest
        }
    } else if url.is_empty() {
        "/"
    } else {
        // Unknown URL — show error page
        let error_html = alloc::format!(
            "<html><body><h1>페이지를 찾을 수 없습니다</h1>\
             <p>주소 <b>{}</b>에 연결할 수 없습니다.</p>\
             <p>다음을 시도해 보세요:</p>\
             <ul><li>kpio://home</li><li>http://localhost/</li><li>kpio://about</li></ul>\
             </body></html>",
            url
        );
        return html_render::render_html(&error_html, 760);
    };

    let response = http::fetch(path);
    render_http_page(&response)
}

/// Convert an HTTP response body into a rendered page.
fn render_http_page(response: &crate::net::http::HttpResponse) -> RenderedPage {
    let body = core::str::from_utf8(&response.body).unwrap_or("(binary content)");
    if response.content_type.contains("html") {
        html_render::render_html(body, 760)
    } else {
        // Plain text / JSON — wrap in <pre>
        let wrapped = alloc::format!("<html><body><pre>{}</pre></body></html>", body);
        html_render::render_html(&wrapped, 760)
    }
}
