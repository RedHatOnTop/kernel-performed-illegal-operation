//! Terminal Emulator
//!
//! Shell and command execution terminal.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::collections::VecDeque;

/// Terminal emulator
#[derive(Debug, Clone)]
pub struct Terminal {
    /// Terminal ID
    pub id: u64,
    /// Title
    pub title: String,
    /// Current working directory
    pub cwd: String,
    /// Shell type
    pub shell: ShellType,
    /// Terminal buffer
    pub buffer: TerminalBuffer,
    /// Input line
    pub input: String,
    /// Cursor position in input
    pub cursor: usize,
    /// Command history
    pub history: VecDeque<String>,
    /// History index (for navigation)
    pub history_index: Option<usize>,
    /// Environment variables
    pub env: Vec<EnvVar>,
    /// Terminal settings
    pub settings: TerminalSettings,
    /// Is running command
    pub running: bool,
    /// Current process ID
    pub current_process: Option<u64>,
}

impl Terminal {
    /// Create new terminal
    pub fn new(id: u64) -> Self {
        Self {
            id,
            title: String::from("Terminal"),
            cwd: String::from("/home"),
            shell: ShellType::Ksh,
            buffer: TerminalBuffer::new(1000),
            input: String::new(),
            cursor: 0,
            history: VecDeque::with_capacity(500),
            history_index: None,
            env: Self::default_env(),
            settings: TerminalSettings::default(),
            running: false,
            current_process: None,
        }
    }

    /// Default environment variables
    fn default_env() -> Vec<EnvVar> {
        alloc::vec![
            EnvVar::new("HOME", "/home"),
            EnvVar::new("USER", "user"),
            EnvVar::new("SHELL", "/bin/ksh"),
            EnvVar::new("PATH", "/bin:/usr/bin:/usr/local/bin"),
            EnvVar::new("TERM", "xterm-256color"),
            EnvVar::new("LANG", "ko_KR.UTF-8"),
        ]
    }

    /// Get prompt string
    pub fn prompt(&self) -> String {
        let home = self.env.iter()
            .find(|e| e.key == "HOME")
            .map(|e| e.value.as_str())
            .unwrap_or("/home");
        
        let display_path = if self.cwd.starts_with(home) {
            alloc::format!("~{}", &self.cwd[home.len()..])
        } else {
            self.cwd.clone()
        };

        alloc::format!("{}$ ", display_path)
    }

    /// Insert character at cursor
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += 1;
        self.history_index = None;
    }

    /// Delete character before cursor
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
        }
    }

    /// Delete character at cursor
    pub fn delete(&mut self) {
        if self.cursor < self.input.len() {
            self.input.remove(self.cursor);
        }
    }

    /// Move cursor left
    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor right
    pub fn cursor_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }

    /// Move cursor to start
    pub fn cursor_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end
    pub fn cursor_end(&mut self) {
        self.cursor = self.input.len();
    }

    /// Move word left
    pub fn cursor_word_left(&mut self) {
        while self.cursor > 0 {
            self.cursor -= 1;
            if self.cursor == 0 {
                break;
            }
            let c = self.input.chars().nth(self.cursor - 1);
            if c == Some(' ') {
                break;
            }
        }
    }

    /// Move word right
    pub fn cursor_word_right(&mut self) {
        let len = self.input.len();
        while self.cursor < len {
            self.cursor += 1;
            if self.cursor >= len {
                break;
            }
            let c = self.input.chars().nth(self.cursor);
            if c == Some(' ') {
                break;
            }
        }
    }

    /// Navigate history up
    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => 0,
            Some(i) if i + 1 < self.history.len() => i + 1,
            Some(i) => i,
        };

        self.history_index = Some(new_index);
        if let Some(cmd) = self.history.get(new_index) {
            self.input = cmd.clone();
            self.cursor = self.input.len();
        }
    }

    /// Navigate history down
    pub fn history_down(&mut self) {
        match self.history_index {
            None => {}
            Some(0) => {
                self.history_index = None;
                self.input.clear();
                self.cursor = 0;
            }
            Some(i) => {
                self.history_index = Some(i - 1);
                if let Some(cmd) = self.history.get(i - 1) {
                    self.input = cmd.clone();
                    self.cursor = self.input.len();
                }
            }
        }
    }

    /// Submit command
    pub fn submit(&mut self) -> Option<String> {
        let cmd = self.input.trim().to_string();
        
        if cmd.is_empty() {
            // Empty line
            self.buffer.write_line(&alloc::format!("{}", self.prompt()));
            return None;
        }

        // Add to history
        if self.history.front() != Some(&cmd) {
            self.history.push_front(cmd.clone());
            if self.history.len() > 500 {
                self.history.pop_back();
            }
        }

        // Echo command to buffer
        self.buffer.write_line(&alloc::format!("{}{}", self.prompt(), cmd));

        // Clear input
        self.input.clear();
        self.cursor = 0;
        self.history_index = None;

        Some(cmd)
    }

    /// Execute built-in command
    pub fn execute_builtin(&mut self, cmd: &str) -> Option<String> {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        match parts[0] {
            "cd" => {
                let path = parts.get(1).unwrap_or(&"~");
                self.cd(path);
                None
            }
            "pwd" => {
                Some(self.cwd.clone())
            }
            "echo" => {
                let output = parts[1..].join(" ");
                Some(output)
            }
            "clear" => {
                self.buffer.clear();
                None
            }
            "exit" => {
                Some(String::from("exit"))
            }
            "export" => {
                if let Some(arg) = parts.get(1) {
                    if let Some((key, value)) = arg.split_once('=') {
                        self.set_env(key, value);
                    }
                }
                None
            }
            "env" => {
                let output = self.env.iter()
                    .map(|e| alloc::format!("{}={}", e.key, e.value))
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(output)
            }
            "history" => {
                let output = self.history.iter()
                    .enumerate()
                    .map(|(i, cmd)| alloc::format!("{:4}  {}", i + 1, cmd))
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(output)
            }
            "help" => {
                Some(String::from(
                    "Built-in commands:\n\
                     cd [path]    - Change directory\n\
                     pwd          - Print current directory\n\
                     echo [text]  - Print text\n\
                     clear        - Clear screen\n\
                     export       - Set environment variable\n\
                     env          - List environment variables\n\
                     history      - Command history\n\
                     help         - Help\n\
                     exit         - Exit terminal"
                ))
            }
            _ => None, // Not a builtin
        }
    }

    /// Change directory
    pub fn cd(&mut self, path: &str) {
        let new_path = if path == "~" || path == "$HOME" {
            self.env.iter()
                .find(|e| e.key == "HOME")
                .map(|e| e.value.clone())
                .unwrap_or_else(|| String::from("/home"))
        } else if path.starts_with('/') {
            path.to_string()
        } else if path == ".." {
            if let Some(idx) = self.cwd.rfind('/') {
                if idx == 0 {
                    String::from("/")
                } else {
                    self.cwd[..idx].to_string()
                }
            } else {
                self.cwd.clone()
            }
        } else {
            alloc::format!("{}/{}", self.cwd, path)
        };

        self.cwd = new_path;
    }

    /// Set environment variable
    pub fn set_env(&mut self, key: &str, value: &str) {
        if let Some(var) = self.env.iter_mut().find(|e| e.key == key) {
            var.value = value.to_string();
        } else {
            self.env.push(EnvVar::new(key, value));
        }
    }

    /// Get environment variable
    pub fn get_env(&self, key: &str) -> Option<&str> {
        self.env.iter()
            .find(|e| e.key == key)
            .map(|e| e.value.as_str())
    }

    /// Write output to buffer
    pub fn write(&mut self, text: &str) {
        self.buffer.write(text);
    }

    /// Write line to buffer
    pub fn write_line(&mut self, text: &str) {
        self.buffer.write_line(text);
    }

    /// Write error to buffer
    pub fn write_error(&mut self, text: &str) {
        // In real implementation, would use different color
        self.buffer.write_line(&alloc::format!("Error: {}", text));
    }

    /// Clear screen
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Interrupt current process (Ctrl+C)
    pub fn interrupt(&mut self) {
        if self.running {
            self.running = false;
            self.current_process = None;
            self.buffer.write_line("^C");
        } else {
            self.input.clear();
            self.cursor = 0;
        }
    }
}

impl Default for Terminal {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Shell type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShellType {
    #[default]
    Ksh,  // KPIO Shell
    Bash,
    Zsh,
    Fish,
}

/// Environment variable
#[derive(Debug, Clone)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

impl EnvVar {
    pub fn new(key: &str, value: &str) -> Self {
        Self {
            key: key.to_string(),
            value: value.to_string(),
        }
    }
}

/// Terminal buffer
#[derive(Debug, Clone)]
pub struct TerminalBuffer {
    /// Lines
    pub lines: VecDeque<TerminalLine>,
    /// Max lines
    pub max_lines: usize,
    /// Scroll offset
    pub scroll_offset: usize,
    /// Columns
    pub cols: usize,
    /// Rows
    pub rows: usize,
}

impl TerminalBuffer {
    /// Create new buffer
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            max_lines,
            scroll_offset: 0,
            cols: 80,
            rows: 24,
        }
    }

    /// Write text
    pub fn write(&mut self, text: &str) {
        // Simple implementation - just append to last line or create new
        if self.lines.is_empty() {
            self.lines.push_back(TerminalLine::new());
        }
        
        if let Some(line) = self.lines.back_mut() {
            line.text.push_str(text);
        }
    }

    /// Write line
    pub fn write_line(&mut self, text: &str) {
        self.lines.push_back(TerminalLine {
            text: text.to_string(),
            wrapped: false,
        });

        while self.lines.len() > self.max_lines {
            self.lines.pop_front();
        }
    }

    /// Clear buffer
    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll_offset = 0;
    }

    /// Scroll up
    pub fn scroll_up(&mut self, lines: usize) {
        let max_scroll = self.lines.len().saturating_sub(self.rows);
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
    }

    /// Scroll down
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Scroll to bottom
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Get visible lines
    pub fn visible_lines(&self) -> impl Iterator<Item = &TerminalLine> {
        let start = self.lines.len().saturating_sub(self.rows + self.scroll_offset);
        let end = self.lines.len().saturating_sub(self.scroll_offset);
        self.lines.range(start..end)
    }
}

/// Terminal line
#[derive(Debug, Clone)]
pub struct TerminalLine {
    pub text: String,
    pub wrapped: bool,
}

impl TerminalLine {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            wrapped: false,
        }
    }
}

impl Default for TerminalLine {
    fn default() -> Self {
        Self::new()
    }
}

/// Terminal settings
#[derive(Debug, Clone)]
pub struct TerminalSettings {
    /// Font family
    pub font_family: String,
    /// Font size
    pub font_size: u32,
    /// Line height
    pub line_height: f32,
    /// Cursor style
    pub cursor_style: CursorStyle,
    /// Cursor blink
    pub cursor_blink: bool,
    /// Scrollback lines
    pub scrollback: usize,
    /// Copy on select
    pub copy_on_select: bool,
    /// Bell sound
    pub bell: bool,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            font_family: String::from("monospace"),
            font_size: 14,
            line_height: 1.2,
            cursor_style: CursorStyle::Block,
            cursor_blink: true,
            scrollback: 1000,
            copy_on_select: true,
            bell: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyle {
    #[default]
    Block,
    Underline,
    Bar,
}

/// Tab in terminal (multiple sessions)
#[derive(Debug, Clone)]
pub struct TerminalTab {
    pub id: u64,
    pub title: String,
    pub terminal: Terminal,
}

impl TerminalTab {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            title: String::from("Terminal"),
            terminal: Terminal::new(id),
        }
    }
}

/// Terminal window with tabs
#[derive(Debug, Clone)]
pub struct TerminalWindow {
    /// Tabs
    pub tabs: Vec<TerminalTab>,
    /// Active tab index
    pub active_tab: usize,
    /// Next tab ID
    next_id: u64,
}

impl TerminalWindow {
    /// Create new terminal window
    pub fn new() -> Self {
        let mut window = Self {
            tabs: Vec::new(),
            active_tab: 0,
            next_id: 1,
        };
        window.new_tab();
        window
    }

    /// Create new tab
    pub fn new_tab(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(TerminalTab::new(id));
        self.active_tab = self.tabs.len() - 1;
        id
    }

    /// Close tab
    pub fn close_tab(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        if index < self.tabs.len() {
            self.tabs.remove(index);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
            true
        } else {
            false
        }
    }

    /// Get active terminal
    pub fn active_terminal(&mut self) -> Option<&mut Terminal> {
        self.tabs.get_mut(self.active_tab).map(|t| &mut t.terminal)
    }
}

impl Default for TerminalWindow {
    fn default() -> Self {
        Self::new()
    }
}
