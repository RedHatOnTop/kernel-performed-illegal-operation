//! Desktop Environment
//!
//! Main desktop shell with wallpaper, icons, and window management.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::design::{Color, Theme, ThemeMode};

/// Desktop environment
#[derive(Debug, Clone)]
pub struct Desktop {
    /// Current wallpaper
    pub wallpaper: Wallpaper,
    /// Desktop icons
    pub icons: Vec<DesktopIcon>,
    /// Icon arrangement
    pub arrangement: IconArrangement,
    /// Icon size
    pub icon_size: IconSize,
    /// Show grid
    pub show_grid: bool,
    /// Active windows
    pub windows: Vec<DesktopWindow>,
    /// Focused window ID
    pub focused_window: Option<u64>,
    /// Screen resolution
    pub resolution: (u32, u32),
    /// Work area (excluding taskbar)
    pub work_area: WorkArea,
}

impl Desktop {
    /// Create new desktop
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            wallpaper: Wallpaper::default(),
            icons: Self::default_icons(),
            arrangement: IconArrangement::Grid,
            icon_size: IconSize::Medium,
            show_grid: false,
            windows: Vec::new(),
            focused_window: None,
            resolution: (width, height),
            work_area: WorkArea {
                x: 0,
                y: 0,
                width,
                height: height - 48, // Reserve space for taskbar
            },
        }
    }

    /// Default desktop icons
    fn default_icons() -> Vec<DesktopIcon> {
        alloc::vec![
            DesktopIcon {
                id: 1,
                name: String::from("Files"),
                icon: String::from("folder"),
                position: (0, 0),
                target: IconTarget::App(String::from("kpio.files")),
                selected: false,
            },
            DesktopIcon {
                id: 2,
                name: String::from("Trash"),
                icon: String::from("trash"),
                position: (0, 1),
                target: IconTarget::Special(SpecialFolder::Trash),
                selected: false,
            },
        ]
    }

    /// Add window
    pub fn add_window(&mut self, window: DesktopWindow) {
        let id = window.id;
        self.windows.push(window);
        self.focused_window = Some(id);
    }

    /// Remove window
    pub fn remove_window(&mut self, id: u64) {
        self.windows.retain(|w| w.id != id);
        if self.focused_window == Some(id) {
            self.focused_window = self.windows.last().map(|w| w.id);
        }
    }

    /// Focus window
    pub fn focus_window(&mut self, id: u64) {
        if let Some(idx) = self.windows.iter().position(|w| w.id == id) {
            let window = self.windows.remove(idx);
            self.windows.push(window);
            self.focused_window = Some(id);
        }
    }

    /// Get window by ID
    pub fn get_window(&self, id: u64) -> Option<&DesktopWindow> {
        self.windows.iter().find(|w| w.id == id)
    }

    /// Get window by ID (mutable)
    pub fn get_window_mut(&mut self, id: u64) -> Option<&mut DesktopWindow> {
        self.windows.iter_mut().find(|w| w.id == id)
    }

    /// Cascade windows
    pub fn cascade_windows(&mut self) {
        let mut x = 50i32;
        let mut y = 50i32;
        
        for window in &mut self.windows {
            window.x = x;
            window.y = y;
            x += 30;
            y += 30;
            
            if x > self.work_area.width as i32 - 200 {
                x = 50;
            }
            if y > self.work_area.height as i32 - 200 {
                y = 50;
            }
        }
    }

    /// Tile windows
    pub fn tile_windows(&mut self, direction: TileDirection) {
        if self.windows.is_empty() {
            return;
        }

        let count = self.windows.len() as u32;
        
        match direction {
            TileDirection::Horizontal => {
                let width = self.work_area.width / count;
                for (i, window) in self.windows.iter_mut().enumerate() {
                    window.x = (i as u32 * width) as i32;
                    window.y = 0;
                    window.width = width;
                    window.height = self.work_area.height;
                }
            }
            TileDirection::Vertical => {
                let height = self.work_area.height / count;
                for (i, window) in self.windows.iter_mut().enumerate() {
                    window.x = 0;
                    window.y = (i as u32 * height) as i32;
                    window.width = self.work_area.width;
                    window.height = height;
                }
            }
            TileDirection::Grid => {
                // Integer square root approximation
                let cols = {
                    let mut c = 1u32;
                    while c * c < count {
                        c += 1;
                    }
                    c
                };
                let rows = (count + cols - 1) / cols;
                let width = self.work_area.width / cols;
                let height = self.work_area.height / rows;
                
                for (i, window) in self.windows.iter_mut().enumerate() {
                    let col = i as u32 % cols;
                    let row = i as u32 / cols;
                    window.x = (col * width) as i32;
                    window.y = (row * height) as i32;
                    window.width = width;
                    window.height = height;
                }
            }
        }
    }
}

/// Wallpaper settings
#[derive(Debug, Clone)]
pub struct Wallpaper {
    /// Wallpaper type
    pub wallpaper_type: WallpaperType,
    /// Fit mode
    pub fit: WallpaperFit,
}

impl Default for Wallpaper {
    fn default() -> Self {
        Self {
            wallpaper_type: WallpaperType::SolidColor(Color::from_hex(0x1a1a2e)),
            fit: WallpaperFit::Fill,
        }
    }
}

#[derive(Debug, Clone)]
pub enum WallpaperType {
    /// Solid color
    SolidColor(Color),
    /// Gradient
    Gradient { start: Color, end: Color, angle: f32 },
    /// Image path
    Image(String),
    /// Slideshow directory
    Slideshow { path: String, interval_secs: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WallpaperFit {
    #[default]
    Fill,
    Fit,
    Stretch,
    Tile,
    Center,
    Span,
}

/// Desktop icon
#[derive(Debug, Clone)]
pub struct DesktopIcon {
    /// Icon ID
    pub id: u64,
    /// Display name
    pub name: String,
    /// Icon image
    pub icon: String,
    /// Grid position (col, row)
    pub position: (u32, u32),
    /// Target
    pub target: IconTarget,
    /// Is selected
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub enum IconTarget {
    /// Application
    App(String),
    /// File or folder
    Path(String),
    /// URL
    Url(String),
    /// Special folder
    Special(SpecialFolder),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialFolder {
    Home,
    Documents,
    Downloads,
    Pictures,
    Videos,
    Music,
    Trash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconArrangement {
    #[default]
    Grid,
    Free,
    AutoArrange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconSize {
    Small,
    #[default]
    Medium,
    Large,
    ExtraLarge,
}

impl IconSize {
    pub fn pixels(&self) -> u32 {
        match self {
            Self::Small => 32,
            Self::Medium => 48,
            Self::Large => 64,
            Self::ExtraLarge => 96,
        }
    }
}

/// Work area (usable screen space)
#[derive(Debug, Clone, Copy)]
pub struct WorkArea {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Desktop window
#[derive(Debug, Clone)]
pub struct DesktopWindow {
    /// Window ID
    pub id: u64,
    /// Window title
    pub title: String,
    /// App ID
    pub app_id: String,
    /// Position X
    pub x: i32,
    /// Position Y
    pub y: i32,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
    /// Window state
    pub state: WindowState,
    /// Is resizable
    pub resizable: bool,
    /// Minimum size
    pub min_size: Option<(u32, u32)>,
    /// Maximum size
    pub max_size: Option<(u32, u32)>,
    /// Window decorations
    pub decorations: bool,
    /// Is modal
    pub modal: bool,
    /// Parent window ID
    pub parent: Option<u64>,
}

impl DesktopWindow {
    /// Create new window
    pub fn new(id: u64, title: impl Into<String>, app_id: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            app_id: app_id.into(),
            x: 100,
            y: 100,
            width: 800,
            height: 600,
            state: WindowState::Normal,
            resizable: true,
            min_size: Some((200, 150)),
            max_size: None,
            decorations: true,
            modal: false,
            parent: None,
        }
    }

    /// Minimize window
    pub fn minimize(&mut self) {
        self.state = WindowState::Minimized;
    }

    /// Maximize window
    pub fn maximize(&mut self, work_area: &WorkArea) {
        self.state = WindowState::Maximized;
        self.x = work_area.x as i32;
        self.y = work_area.y as i32;
        self.width = work_area.width;
        self.height = work_area.height;
    }

    /// Restore window
    pub fn restore(&mut self) {
        self.state = WindowState::Normal;
    }

    /// Toggle maximize
    pub fn toggle_maximize(&mut self, work_area: &WorkArea) {
        match self.state {
            WindowState::Maximized => self.restore(),
            _ => self.maximize(work_area),
        }
    }

    /// Center on screen
    pub fn center(&mut self, screen_width: u32, screen_height: u32) {
        self.x = ((screen_width - self.width) / 2) as i32;
        self.y = ((screen_height - self.height) / 2) as i32;
    }

    /// Check if point is inside window
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width as i32 &&
        y >= self.y && y < self.y + self.height as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowState {
    #[default]
    Normal,
    Minimized,
    Maximized,
    Fullscreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileDirection {
    Horizontal,
    Vertical,
    Grid,
}

/// Window snap zones
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapZone {
    None,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Maximize,
}

impl SnapZone {
    /// Get snap zone from screen position
    pub fn from_position(x: i32, y: i32, width: u32, height: u32, margin: i32) -> Self {
        let left = x < margin;
        let right = x > width as i32 - margin;
        let top = y < margin;
        let bottom = y > height as i32 - margin;

        match (left, right, top, bottom) {
            (true, _, true, _) => Self::TopLeft,
            (true, _, _, true) => Self::BottomLeft,
            (_, true, true, _) => Self::TopRight,
            (_, true, _, true) => Self::BottomRight,
            (true, _, _, _) => Self::Left,
            (_, true, _, _) => Self::Right,
            (_, _, true, _) => Self::Maximize,
            _ => Self::None,
        }
    }
}
