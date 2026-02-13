//! Window System
//!
//! Modern window manager with rounded corners, soft shadows,
//! themed title bars and polished application content areas.

use super::html_render::{self, RenderCmd, RenderedPage};
use super::render::{Color, Renderer};
use super::theme::{
    Accent, IconColor, Radius, Shadow, Shadows, Size, Spacing, Surface, TermTheme, Text,
};
use crate::terminal;
use crate::terminal::ansi::{self, AnsiAction, AnsiColor, StyledLine, StyledSpan, TextStyle};
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
                if path != "/" {
                    items.push(String::from(".."));
                }
                let mut sorted: Vec<(String, bool)> = entries
                    .iter()
                    .map(|(name, child_ino)| {
                        let is_dir = fs.get(*child_ino).map(|n| n.mode.is_dir()).unwrap_or(false);
                        (name.clone(), is_dir)
                    })
                    .collect();
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
    /// Cursor position within input_buffer (byte offset)
    pub cursor_pos: usize,
    /// Terminal command history (local copy)
    terminal_history: Vec<String>,
    /// History browsing index (0 = not browsing, 1..=len)
    history_idx: usize,
    /// Saved input when browsing history
    history_saved_input: String,
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

/// PWA display mode (determines window chrome)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PwaDisplayMode {
    /// Title bar only — no address bar
    Standalone,
    /// Title bar + minimal nav buttons (back/forward/reload)
    MinimalUi,
    /// No chrome at all — full-screen content
    Fullscreen,
}

/// Window content types
pub enum WindowContent {
    /// Simple text content
    Text(String),
    /// Browser window
    Browser {
        url: String,
        content: String,
        rendered: Option<RenderedPage>,
    },
    /// Terminal window
    Terminal {
        lines: Vec<StyledLine>,
        scroll_offset: usize,
    },
    /// File manager
    FileManager { path: String, items: Vec<String> },
    /// Settings
    Settings,
    /// Installed PWA / Web App window
    WebApp {
        app_id: u64,
        url: String,
        content: String,
        rendered: Option<RenderedPage>,
        display_mode: PwaDisplayMode,
        theme_color: Option<u32>,
        scope: String,
    },
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
            cursor_pos: 0,
            terminal_history: Vec::new(),
            history_idx: 0,
            history_saved_input: String::new(),
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
            cursor_pos: 0,
            terminal_history: Vec::new(),
            history_idx: 0,
            history_saved_input: String::new(),
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
        let _prompt = terminal::shell::with_shell(|sh| sh.prompt());
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
                    StyledLine::from_plain("KPIO Shell v2.0 \u{2014} Type 'help' for commands"),
                    StyledLine::new(),
                ],
                scroll_offset: 0,
            },
            input_buffer: String::new(),
            cursor_pos: 0,
            terminal_history: Vec::new(),
            history_idx: 0,
            history_saved_input: String::new(),
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
            cursor_pos: 0,
            terminal_history: Vec::new(),
            history_idx: 0,
            history_saved_input: String::new(),
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
            cursor_pos: 0,
            terminal_history: Vec::new(),
            history_idx: 0,
            history_saved_input: String::new(),
            saved_x: x,
            saved_y: y,
            saved_width: 600,
            saved_height: 450,
            hovered_button: -1,
            scroll_y: 0,
        }
    }

    /// Create a PWA / Web App window
    ///
    /// `app_id` — kernel-level app id (u64)
    /// `name` — display name from manifest (short_name or name)
    /// `start_url` — initial URL to load
    /// `scope` — navigation scope (URLs outside this open in a normal browser)
    /// `theme_color` — optional ARGB title-bar colour from manifest
    /// `display_mode` — Standalone / MinimalUi / Fullscreen
    pub fn new_webapp(
        id: WindowId,
        app_id: u64,
        name: &str,
        start_url: &str,
        scope: &str,
        theme_color: Option<u32>,
        display_mode: PwaDisplayMode,
        x: i32,
        y: i32,
    ) -> Self {
        Self {
            id,
            title: String::from(name),
            x,
            y,
            width: 800,
            height: 600,
            state: WindowState::Normal,
            content: WindowContent::WebApp {
                app_id,
                url: String::from(start_url),
                content: String::new(),
                rendered: Some(navigate_to_url(start_url)),
                display_mode,
                theme_color,
                scope: String::from(scope),
            },
            input_buffer: String::new(),
            cursor_pos: 0,
            terminal_history: Vec::new(),
            history_idx: 0,
            history_saved_input: String::new(),
            saved_x: x,
            saved_y: y,
            saved_width: 800,
            saved_height: 600,
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
        px >= self.x
            && px < self.x + self.width as i32
            && py >= self.y
            && py < self.y + self.height as i32
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
            WindowContent::WebApp { display_mode, .. } => {
                // MinimalUi mode has a small toolbar area
                if *display_mode == PwaDisplayMode::MinimalUi && content_y < 29 {
                    // Click on mini nav buttons (handled later)
                }
                // Otherwise clicks go to rendered content
            }
            _ => {}
        }
    }

    /// Handle key input (rich version with modifiers)
    pub fn on_key_event(&mut self, event: &super::input::KeyEvent) {
        if !event.pressed {
            return;
        }

        let key = event.key;

        match &mut self.content {
            WindowContent::Terminal {
                lines,
                scroll_offset,
            } => {
                match key {
                    // ── Enter: execute command ──
                    '\n' => {
                        let cmd = self.input_buffer.clone();
                        let prompt = terminal::shell::with_shell(|sh| sh.prompt());
                        let prompt_line = alloc::format!("{}{}", prompt, cmd);
                        let parsed_prompt = ansi::parse_ansi_lines(&prompt_line);
                        if let Some(pl) = parsed_prompt.into_iter().next() {
                            lines.push(pl);
                        } else {
                            lines.push(StyledLine::from_plain(&prompt_line));
                        }

                        if !cmd.trim().is_empty() {
                            // Save to local history
                            self.terminal_history.push(cmd.clone());
                            if self.terminal_history.len() > 200 {
                                self.terminal_history.remove(0);
                            }

                            let result = terminal::shell::execute(&cmd);
                            for raw_line in &result {
                                let actions = ansi::parse_ansi(raw_line);
                                for action in actions {
                                    match action {
                                        AnsiAction::ClearScreen => {
                                            lines.clear();
                                        }
                                        AnsiAction::Line(styled) => {
                                            lines.push(styled);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }

                        self.input_buffer.clear();
                        self.cursor_pos = 0;
                        self.history_idx = 0;
                        *scroll_offset = 0;

                        while lines.len() > 500 {
                            lines.remove(0);
                        }
                    }

                    // ── Ctrl+C: cancel current input ──
                    _ if event.ctrl && (key == 'c' || key == 'C') => {
                        let prompt = terminal::shell::with_shell(|sh| sh.prompt());
                        let cancel_line = alloc::format!("{}{}^C", prompt, self.input_buffer);
                        lines.push(StyledLine::from_plain(&cancel_line));
                        self.input_buffer.clear();
                        self.cursor_pos = 0;
                        self.history_idx = 0;
                    }

                    // ── Ctrl+L: clear screen ──
                    _ if event.ctrl && (key == 'l' || key == 'L') => {
                        lines.clear();
                    }

                    // ── Ctrl+A: beginning of line ──
                    _ if event.ctrl && (key == 'a' || key == 'A') => {
                        self.cursor_pos = 0;
                    }

                    // ── Ctrl+E: end of line ──
                    _ if event.ctrl && (key == 'e' || key == 'E') => {
                        self.cursor_pos = self.input_buffer.len();
                    }

                    // ── Ctrl+U: kill line (clear input) ──
                    _ if event.ctrl && (key == 'u' || key == 'U') => {
                        self.input_buffer.clear();
                        self.cursor_pos = 0;
                    }

                    // ── Ctrl+W: kill word backwards ──
                    _ if event.ctrl && (key == 'w' || key == 'W') => {
                        // Remove chars backward to the previous space
                        let before = &self.input_buffer[..self.cursor_pos];
                        let trimmed = before.trim_end();
                        let new_end = trimmed.rfind(' ').map(|i| i + 1).unwrap_or(0);
                        let after = self.input_buffer[self.cursor_pos..].to_string();
                        self.input_buffer =
                            alloc::format!("{}{}", &self.input_buffer[..new_end], after);
                        self.cursor_pos = new_end;
                    }

                    // ── Backspace ──
                    '\x08' => {
                        if self.cursor_pos > 0 {
                            self.input_buffer.remove(self.cursor_pos - 1);
                            self.cursor_pos -= 1;
                        }
                    }

                    // ── Delete (0x7F) ──
                    '\x7F' => {
                        if self.cursor_pos < self.input_buffer.len() {
                            self.input_buffer.remove(self.cursor_pos);
                        }
                    }

                    // ── Left arrow (\x13 = DC3) ──
                    '\x13' => {
                        if self.cursor_pos > 0 {
                            self.cursor_pos -= 1;
                        }
                    }

                    // ── Right arrow (\x14 = DC4) ──
                    '\x14' => {
                        if self.cursor_pos < self.input_buffer.len() {
                            self.cursor_pos += 1;
                        }
                    }

                    // ── Home (\x01 = SOH) ──
                    '\x01' => {
                        self.cursor_pos = 0;
                    }

                    // ── End (\x05 = ENQ) ──
                    '\x05' => {
                        self.cursor_pos = self.input_buffer.len();
                    }

                    // ── Up arrow (\x11 = DC1) ── history prev
                    '\x11' => {
                        if !self.terminal_history.is_empty() {
                            if self.history_idx == 0 {
                                // Save current input before browsing
                                self.history_saved_input = self.input_buffer.clone();
                            }
                            if self.history_idx < self.terminal_history.len() {
                                self.history_idx += 1;
                                let idx = self.terminal_history.len() - self.history_idx;
                                self.input_buffer = self.terminal_history[idx].clone();
                                self.cursor_pos = self.input_buffer.len();
                            }
                        }
                    }

                    // ── Down arrow (\x12 = DC2) ── history next
                    '\x12' => {
                        if self.history_idx > 0 {
                            self.history_idx -= 1;
                            if self.history_idx == 0 {
                                // Restore saved input
                                self.input_buffer = self.history_saved_input.clone();
                            } else {
                                let idx = self.terminal_history.len() - self.history_idx;
                                self.input_buffer = self.terminal_history[idx].clone();
                            }
                            self.cursor_pos = self.input_buffer.len();
                        }
                    }

                    // ── Page Up (\x15) ── scrollback up
                    '\x15' => {
                        let page = 10;
                        *scroll_offset = (*scroll_offset + page).min(lines.len().saturating_sub(1));
                    }

                    // ── Page Down (\x16) ── scrollback down
                    '\x16' => {
                        if *scroll_offset >= 10 {
                            *scroll_offset -= 10;
                        } else {
                            *scroll_offset = 0;
                        }
                    }

                    // ── Tab: simple completion ──
                    '\t' => {
                        let completed = terminal::shell::tab_complete(&self.input_buffer);
                        if let Some(c) = completed {
                            self.input_buffer = c;
                            self.cursor_pos = self.input_buffer.len();
                        }
                    }

                    // ── Escape ──
                    '\x1B' => {
                        // Cancel history browsing
                        if self.history_idx > 0 {
                            self.history_idx = 0;
                            self.input_buffer = self.history_saved_input.clone();
                            self.cursor_pos = self.input_buffer.len();
                        }
                    }

                    // ── Normal printable character ──
                    c if c >= ' ' && !c.is_control() && (c as u32) < 0xF000 => {
                        self.input_buffer.insert(self.cursor_pos, c);
                        self.cursor_pos += 1;
                        self.history_idx = 0; // reset history browse on typing
                    }

                    _ => {} // Ignore other keys
                }
            }
            WindowContent::Browser {
                url,
                content,
                rendered,
            } => {
                if key == '\n' {
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
                    self.cursor_pos = 0;
                } else if key == '\x08' {
                    if self.cursor_pos > 0 {
                        self.input_buffer.remove(self.cursor_pos - 1);
                        self.cursor_pos -= 1;
                    }
                } else if key >= ' ' && !key.is_control() && (key as u32) < 0xF000 {
                    self.input_buffer.insert(self.cursor_pos, key);
                    self.cursor_pos += 1;
                }
            }
            WindowContent::WebApp {
                url,
                content,
                rendered,
                display_mode,
                scope,
                ..
            } => {
                // MinimalUi allows address-bar-like input for within-scope navigation
                if *display_mode == PwaDisplayMode::MinimalUi {
                    if key == '\n' && !self.input_buffer.is_empty() {
                        let target = self.input_buffer.clone();
                        // Enforce scope: only navigate if within scope
                        if target.starts_with(scope.as_str()) {
                            *url = target;
                            *rendered = Some(navigate_to_url(url));
                            *content = String::new();
                            self.scroll_y = 0;
                        }
                        self.input_buffer.clear();
                        self.cursor_pos = 0;
                    } else if key == '\x08' {
                        if self.cursor_pos > 0 {
                            self.input_buffer.remove(self.cursor_pos - 1);
                            self.cursor_pos -= 1;
                        }
                    } else if key >= ' ' && !key.is_control() && (key as u32) < 0xF000 {
                        self.input_buffer.insert(self.cursor_pos, key);
                        self.cursor_pos += 1;
                    }
                }
                // Standalone / Fullscreen: no user URL input
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

        // ── Flat shadow (single layer) ──
        let shadow = Shadows::WINDOW;
        renderer.fill_rounded_rect_aa(
            self.x + shadow.offset_x,
            self.y + shadow.offset_y,
            self.width,
            self.height,
            r + 2,
            shadow.color,
        );

        // ── Window body ──
        renderer.fill_rounded_rect_aa(
            self.x,
            self.y,
            self.width,
            self.height,
            r,
            Surface::WINDOW_BG,
        );

        // ── Title bar (flat solid) ──
        // WebApp windows can override the title bar color with theme_color
        let title_color = if let WindowContent::WebApp { theme_color: Some(tc), .. } = &self.content {
            Color::rgba(
                (*tc >> 16) as u8,
                (*tc >> 8) as u8,
                *tc as u8,
                255,
            )
        } else if is_active {
            Surface::WINDOW_TITLE_ACTIVE
        } else {
            Surface::WINDOW_TITLE_INACTIVE
        };
        renderer.fill_rounded_rect_aa(self.x, self.y, self.width, tb, r, title_color);
        renderer.fill_rect(self.x, self.y + r as i32, self.width, tb - r, title_color);

        // 1px separator
        renderer.draw_hline(
            self.x,
            self.y + tb as i32 - 1,
            self.width,
            Color::rgba(0, 0, 0, 10),
        );

        // ── Centred title text ──
        let title_len = self.title.len() as i32 * 8;
        let title_x = self.x + (self.width as i32 - title_len) / 2;
        let title_y = self.y + (tb as i32 - 8) / 2;
        renderer.draw_text(
            title_x,
            title_y,
            &self.title,
            if is_active {
                Text::PRIMARY
            } else {
                Text::SECONDARY
            },
        );

        // ── Window control buttons (flat, geometric glyphs) ──
        let close_x = self.x + self.width as i32 - bw as i32;
        let max_x = close_x - bw as i32;
        let min_x = max_x - bw as i32;

        // Close button — red hover, × glyph
        if self.hovered_button == 0 {
            // Fill hover bg (top-right corner gets rounded to match window)
            renderer.fill_rounded_rect_aa(close_x, self.y, bw, bh, r, Surface::CLOSE_HOVER);
            renderer.fill_rect(close_x, self.y + r as i32, bw, bh - r, Surface::CLOSE_HOVER);
        }
        {
            let gc = if self.hovered_button == 0 {
                Text::ON_ACCENT
            } else {
                Text::SECONDARY
            };
            let cx = close_x + bw as i32 / 2;
            let cy = self.y + bh as i32 / 2;
            // Draw × using two AA lines
            renderer.draw_line_aa(cx - 4, cy - 4, cx + 4, cy + 4, gc);
            renderer.draw_line_aa(cx + 4, cy - 4, cx - 4, cy + 4, gc);
        }

        // Maximize button — subtle hover, □ glyph
        if self.hovered_button == 1 {
            renderer.fill_rect(max_x, self.y, bw, bh, Surface::BUTTON_HOVER);
        }
        {
            let gc = Text::SECONDARY;
            let cx = max_x + bw as i32 / 2;
            let cy = self.y + bh as i32 / 2;
            renderer.draw_rect(cx - 4, cy - 4, 9, 9, gc);
        }

        // Minimize button — subtle hover, ─ glyph
        if self.hovered_button == 2 {
            renderer.fill_rect(min_x, self.y, bw, bh, Surface::BUTTON_HOVER);
        }
        {
            let gc = Text::SECONDARY;
            let cx = min_x + bw as i32 / 2;
            let cy = self.y + bh as i32 / 2;
            renderer.draw_hline(cx - 4, cy, 9, gc);
        }

        // ── Border (1px solid) ──
        let border_c = if is_active {
            Surface::WINDOW_BORDER_ACTIVE
        } else {
            Surface::WINDOW_BORDER_INACTIVE
        };
        renderer.draw_rounded_rect_aa(self.x, self.y, self.width, self.height, r, border_c);

        // ── Resize grip (bottom-right, 3 diagonal lines) ──
        let grip_x = self.x + self.width as i32 - 14;
        let grip_y = self.y + self.height as i32 - 14;
        let grip_c = Color::rgba(0, 0, 0, 30);
        renderer.draw_line_aa(grip_x + 10, grip_y, grip_x, grip_y + 10, grip_c);
        renderer.draw_line_aa(grip_x + 10, grip_y + 4, grip_x + 4, grip_y + 10, grip_c);
        renderer.draw_line_aa(grip_x + 10, grip_y + 8, grip_x + 8, grip_y + 10, grip_c);

        // ── Content area ──
        let content_y = self.y + tb as i32;
        let content_height = self.height.saturating_sub(tb);
        self.render_content(renderer, self.x, content_y, self.width, content_height);
    }

    /// Render window content
    fn render_content(&self, renderer: &mut Renderer, x: i32, y: i32, w: u32, h: u32) {
        match &self.content {
            WindowContent::Text(text) => {
                renderer.draw_text(
                    x + Spacing::MD as i32,
                    y + Spacing::MD as i32,
                    text,
                    Text::PRIMARY,
                );
            }
            WindowContent::Browser { url, rendered, .. } => {
                // ── Address bar ──
                let bar_h = Size::INPUT_HEIGHT;
                let bar_x = x + Spacing::SM as i32;
                let bar_y = y + Spacing::SM as i32;
                let bar_w = w - Spacing::LG;
                renderer.fill_rounded_rect_aa(
                    bar_x,
                    bar_y,
                    bar_w,
                    bar_h,
                    Radius::INPUT,
                    Surface::INPUT_BG,
                );
                renderer.draw_rounded_rect_aa(
                    bar_x,
                    bar_y,
                    bar_w,
                    bar_h,
                    Radius::INPUT,
                    Surface::INPUT_BORDER,
                );

                let display_url = if !self.input_buffer.is_empty() {
                    alloc::format!("{}|", self.input_buffer)
                } else {
                    url.clone()
                };
                renderer.draw_text(
                    bar_x + Spacing::SM as i32,
                    bar_y + (bar_h as i32 - 8) / 2,
                    &display_url,
                    Text::PRIMARY,
                );

                // ── Rendered HTML content ──
                let content_top = y + Spacing::SM as i32 + bar_h as i32 + Spacing::SM as i32;
                let scroll = self.scroll_y;
                if let Some(page) = rendered {
                    for cmd in &page.commands {
                        match cmd {
                            RenderCmd::FillRect {
                                x: rx,
                                y: ry,
                                w: rw,
                                h: rh,
                                color,
                            } => {
                                let cy = content_top + ry - scroll;
                                renderer.fill_rect(
                                    x + rx,
                                    cy,
                                    *rw,
                                    *rh,
                                    Color::rgba(
                                        (*color >> 16) as u8,
                                        (*color >> 8) as u8,
                                        *color as u8,
                                        (*color >> 24) as u8,
                                    ),
                                );
                            }
                            RenderCmd::Text {
                                x: tx,
                                y: ty,
                                text,
                                color,
                                ..
                            } => {
                                let cy = content_top + ty - scroll;
                                if cy >= content_top - 16 && cy < y + h as i32 {
                                    renderer.draw_text(
                                        x + tx,
                                        cy,
                                        text,
                                        Color::rgba(
                                            (*color >> 16) as u8,
                                            (*color >> 8) as u8,
                                            *color as u8,
                                            255,
                                        ),
                                    );
                                }
                            }
                            RenderCmd::HRule {
                                x: hx,
                                y: hy,
                                w: hw,
                                color,
                            } => {
                                let cy = content_top + hy - scroll;
                                if cy >= content_top && cy < y + h as i32 {
                                    renderer.draw_hline(
                                        x + hx,
                                        cy,
                                        *hw,
                                        Color::rgba(
                                            (*color >> 16) as u8,
                                            (*color >> 8) as u8,
                                            *color as u8,
                                            255,
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            WindowContent::Terminal {
                lines,
                scroll_offset,
            } => {
                // ── Terminal background ──
                let r_bot = Radius::WINDOW;
                renderer.fill_rounded_rect_aa(x, y, w, h, 0, TermTheme::BG);
                // Re-round bottom corners only by drawing a small rounded rect at bottom
                renderer.fill_rounded_rect_aa(
                    x,
                    y + h as i32 - r_bot as i32 * 2,
                    w,
                    r_bot * 2,
                    r_bot,
                    TermTheme::BG,
                );

                // Visible lines
                let line_height = 14u32;
                let max_lines =
                    ((h.saturating_sub(Spacing::SM + Spacing::SM)) / line_height) as usize;
                let prompt = terminal::shell::with_shell(|sh| sh.prompt());

                // Reserve 1 line for current input
                let available = max_lines.saturating_sub(1);
                let total = lines.len();
                let start_idx = if total > available {
                    total - available - (*scroll_offset).min(total.saturating_sub(available))
                } else {
                    0
                };
                let end_idx = (start_idx + available).min(total);
                let visible = &lines[start_idx..end_idx];

                let mut ly = y + Spacing::SM as i32;
                for sline in visible {
                    let mut lx = x + Spacing::SM as i32;
                    for span in &sline.spans {
                        let c = ansi_style_to_color(&span.style, true);
                        renderer.draw_text(lx, ly, &span.text, c);
                        // Approximate text width: 7px per character (monospace)
                        lx += (span.text.len() as i32) * 7;
                    }
                    ly += line_height as i32;
                }

                // Current input with prompt + positioned cursor
                let prompt_len = prompt.len();
                let before_cursor = &self.input_buffer[..self.cursor_pos];
                let after_cursor = &self.input_buffer[self.cursor_pos..];
                let input_prefix = alloc::format!("{}{}", prompt, before_cursor);

                let px = x + Spacing::SM as i32;
                renderer.draw_text(px, ly, &input_prefix, TermTheme::PROMPT);

                // Draw blinking block cursor
                let cursor_x = px + (input_prefix.len() as i32) * 7;
                renderer.fill_rect(cursor_x, ly, 7, 14, TermTheme::CURSOR);

                // Draw text after cursor
                if !after_cursor.is_empty() {
                    renderer.draw_text(cursor_x + 7, ly, after_cursor, TermTheme::PROMPT);
                }

                // Scrollback indicator
                if *scroll_offset > 0 && total > available {
                    let bar_h = 4u32;
                    let bar_y = y + Spacing::SM as i32;
                    renderer.fill_rect(x + w as i32 - 10, bar_y, bar_h, 20, TermTheme::PROMPT);
                }
            }
            WindowContent::FileManager { path, items } => {
                // ── Path / toolbar bar ──
                let bar_h = Size::INPUT_HEIGHT;
                let bar_x = x + Spacing::SM as i32;
                let bar_y = y + Spacing::SM as i32;
                let bar_w = w - Spacing::LG;
                renderer.fill_rounded_rect_aa(
                    bar_x,
                    bar_y,
                    bar_w,
                    bar_h,
                    Radius::INPUT,
                    Surface::INPUT_BG,
                );
                renderer.draw_rounded_rect_aa(
                    bar_x,
                    bar_y,
                    bar_w,
                    bar_h,
                    Radius::INPUT,
                    Surface::INPUT_BORDER,
                );

                // Back button
                renderer.fill_rounded_rect_aa(
                    bar_x + 2,
                    bar_y + 2,
                    22,
                    bar_h - 4,
                    Radius::SM,
                    Surface::PANEL,
                );
                renderer.draw_text(
                    bar_x + 8,
                    bar_y + (bar_h as i32 - 8) / 2,
                    "<",
                    Text::SECONDARY,
                );

                // Path
                renderer.draw_text(
                    bar_x + 30,
                    bar_y + (bar_h as i32 - 8) / 2,
                    path,
                    Text::PRIMARY,
                );

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
                    let row_bg = if i % 2 == 0 {
                        Surface::WINDOW_BG
                    } else {
                        Surface::PANEL
                    };
                    renderer.fill_rect(x + Spacing::SM as i32, iy - 2, bar_w, 22, row_bg);

                    // Mini icon
                    if item == ".." {
                        renderer.fill_rounded_rect_aa(
                            x + 12,
                            iy,
                            14,
                            14,
                            3,
                            Accent::PRIMARY.with_alpha(60),
                        );
                        renderer.draw_text(x + 15, iy + 1, "^", Text::ON_ACCENT);
                    } else if is_folder {
                        renderer.fill_rounded_rect_aa(x + 12, iy, 14, 14, 3, IconColor::FOLDER);
                    } else {
                        renderer.fill_rounded_rect_aa(x + 12, iy, 14, 14, 3, IconColor::FILE);
                    }

                    renderer.draw_text(x + 32, iy + 1, item, Text::PRIMARY);

                    let ftype = if item == ".." {
                        "Parent"
                    } else if is_folder {
                        "Folder"
                    } else if item.ends_with(".txt") {
                        "Text"
                    } else if item.ends_with(".pdf") {
                        "PDF"
                    } else if item.ends_with(".jpg") || item.ends_with(".png") {
                        "Image"
                    } else if item.ends_with(".mp3") {
                        "Audio"
                    } else if item.ends_with(".mp4") {
                        "Video"
                    } else if item.ends_with(".zip") {
                        "Archive"
                    } else if item.ends_with(".exe") {
                        "Executable"
                    } else {
                        "File"
                    };
                    renderer.draw_text(x + 350, iy + 1, ftype, Text::SECONDARY);

                    let size = if is_folder { "-" } else { "4 KB" };
                    renderer.draw_text(x + 500, iy + 1, size, Text::SECONDARY);

                    iy += 24;
                }

                // Status bar
                let status_y = (y + h as i32 - 24).max(iy + 8);
                renderer.fill_rect(x, status_y, w, 22, Surface::PANEL);
                renderer.draw_text(
                    x + Spacing::MD as i32,
                    status_y + 4,
                    &alloc::format!("{} items", items.len()),
                    Text::SECONDARY,
                );
            }
            WindowContent::Settings => {
                // ── Header ──
                renderer.draw_text_scaled(
                    x + Spacing::LG as i32,
                    y + Spacing::MD as i32,
                    "Settings",
                    Text::PRIMARY,
                    2,
                );

                // ── Category cards ──
                let categories = [
                    ("Display", "Resolution, brightness, theme"),
                    ("Sound", "Volume, output device"),
                    ("Network", "WiFi, Ethernet, VPN"),
                    ("System", "Updates, backup, security"),
                    ("About", "KPIO OS version 1.0"),
                ];
                let card_w = w - Spacing::XL * 2;
                let card_h = 46u32;
                let mut cy = y + 44;
                for (name, desc) in categories {
                    renderer.fill_rounded_rect_aa(
                        x + Spacing::LG as i32,
                        cy,
                        card_w,
                        card_h,
                        Radius::MD,
                        Surface::PANEL,
                    );
                    renderer.draw_rounded_rect_aa(
                        x + Spacing::LG as i32,
                        cy,
                        card_w,
                        card_h,
                        Radius::MD,
                        Color::rgba(0, 0, 0, 10),
                    );
                    renderer.draw_text(x + Spacing::XL as i32, cy + 8, name, Text::PRIMARY);
                    renderer.draw_text(x + Spacing::XL as i32, cy + 24, desc, Text::MUTED);
                    cy += card_h as i32 + Spacing::SM as i32;
                }
            }
            WindowContent::WebApp {
                url,
                rendered,
                display_mode,
                theme_color,
                ..
            } => {
                // MinimalUi: small nav bar at top
                let content_top = if *display_mode == PwaDisplayMode::MinimalUi {
                    let bar_h = 24u32;
                    let bar_x = x + Spacing::SM as i32;
                    let bar_y = y + 2;
                    let bar_w = w - Spacing::LG;

                    // Mini toolbar background
                    renderer.fill_rounded_rect_aa(
                        bar_x,
                        bar_y,
                        bar_w,
                        bar_h,
                        Radius::SM,
                        Surface::INPUT_BG,
                    );

                    // Back / Forward / Reload buttons
                    let btn_w = 22u32;
                    renderer.draw_text(bar_x + 4, bar_y + 4, "<", Text::SECONDARY);
                    renderer.draw_text(bar_x + 4 + btn_w as i32, bar_y + 4, ">", Text::SECONDARY);
                    renderer.draw_text(
                        bar_x + 4 + btn_w as i32 * 2,
                        bar_y + 4,
                        "\u{21BB}",
                        Text::SECONDARY,
                    );

                    // URL display (truncated)
                    let url_x = bar_x + 4 + btn_w as i32 * 3 + 6;
                    renderer.draw_text(url_x, bar_y + 4, url, Text::MUTED);

                    y + bar_h as i32 + 4
                } else {
                    // Standalone / Fullscreen: no nav bar
                    y
                };

                // Render HTML content (identical to Browser minus address bar)
                let scroll = self.scroll_y;
                if let Some(page) = rendered {
                    for cmd in &page.commands {
                        match cmd {
                            RenderCmd::FillRect {
                                x: rx,
                                y: ry,
                                w: rw,
                                h: rh,
                                color,
                            } => {
                                let cy = content_top + ry - scroll;
                                renderer.fill_rect(
                                    x + rx,
                                    cy,
                                    *rw,
                                    *rh,
                                    Color::rgba(
                                        (*color >> 16) as u8,
                                        (*color >> 8) as u8,
                                        *color as u8,
                                        (*color >> 24) as u8,
                                    ),
                                );
                            }
                            RenderCmd::Text {
                                x: tx,
                                y: ty,
                                text,
                                color,
                                ..
                            } => {
                                let cy = content_top + ty - scroll;
                                if cy >= content_top - 16 && cy < y + h as i32 {
                                    renderer.draw_text(
                                        x + tx,
                                        cy,
                                        text,
                                        Color::rgba(
                                            (*color >> 16) as u8,
                                            (*color >> 8) as u8,
                                            *color as u8,
                                            255,
                                        ),
                                    );
                                }
                            }
                            RenderCmd::HRule {
                                x: hx,
                                y: hy,
                                w: hw,
                                color,
                            } => {
                                let cy = content_top + hy - scroll;
                                if cy >= content_top && cy < y + h as i32 {
                                    renderer.draw_hline(
                                        x + hx,
                                        cy,
                                        *hw,
                                        Color::rgba(
                                            (*color >> 16) as u8,
                                            (*color >> 8) as u8,
                                            *color as u8,
                                            255,
                                        ),
                                    );
                                }
                            }
                        }
                    }
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
            "<html><body><h1>Page Not Found</h1>\
             <p>Could not connect to <b>{}</b>.</p>\
             <p>Try the following:</p>\
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

/// Convert an ANSI `TextStyle` to a GUI `Color`.
///
/// If `is_fg` is true, resolves the foreground color (with dim/bold adjustments).
/// Otherwise resolves the background color, returning the terminal default BG
/// when the ANSI color is `Default`.
fn ansi_style_to_color(style: &TextStyle, is_fg: bool) -> Color {
    if is_fg {
        let (r, g, b) = style.fg_rgb();
        // If the style has the default foreground and no special attrs,
        // use the terminal theme foreground for consistency
        if style.fg == AnsiColor::Default && !style.bold && !style.dim {
            TermTheme::FG
        } else if style.bold && style.fg == AnsiColor::Default {
            // Bold default = slightly brighter
            Color::rgb(230, 235, 240)
        } else {
            Color::rgb(r, g, b)
        }
    } else {
        match style.bg_rgb() {
            Some((r, g, b)) => Color::rgb(r, g, b),
            None => TermTheme::BG, // transparent → terminal bg
        }
    }
}
