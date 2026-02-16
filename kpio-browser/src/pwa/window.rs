//! PWA Window Management
//!
//! Handles standalone/fullscreen window modes for installed PWAs.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{DisplayMode, PwaError};

/// PWA Window
pub struct PwaWindow {
    /// App ID
    app_id: String,
    /// Display mode
    display_mode: DisplayMode,
    /// Window bounds
    bounds: WindowBounds,
    /// Window state
    state: WindowState,
    /// Window controls overlay
    controls_overlay: Option<WindowControlsOverlay>,
    /// Title bar style
    title_bar_style: TitleBarStyle,
    /// Theme color
    theme_color: Option<u32>,
    /// Is focused
    focused: bool,
    /// Is visible
    visible: bool,
}

impl PwaWindow {
    /// Create new PWA window
    pub fn new(app_id: String, display_mode: DisplayMode) -> Self {
        Self {
            app_id,
            display_mode,
            bounds: WindowBounds::default(),
            state: WindowState::Normal,
            controls_overlay: None,
            title_bar_style: TitleBarStyle::Default,
            theme_color: None,
            focused: false,
            visible: false,
        }
    }

    /// Get app ID
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// Get display mode
    pub fn display_mode(&self) -> DisplayMode {
        self.display_mode
    }

    /// Set display mode
    pub fn set_display_mode(&mut self, mode: DisplayMode) {
        self.display_mode = mode;
    }

    /// Get window bounds
    pub fn bounds(&self) -> &WindowBounds {
        &self.bounds
    }

    /// Set window bounds
    pub fn set_bounds(&mut self, bounds: WindowBounds) {
        self.bounds = bounds;
    }

    /// Get window state
    pub fn state(&self) -> WindowState {
        self.state
    }

    /// Set window state
    pub fn set_state(&mut self, state: WindowState) {
        self.state = state;
    }

    /// Minimize window
    pub fn minimize(&mut self) {
        self.state = WindowState::Minimized;
    }

    /// Maximize window
    pub fn maximize(&mut self) {
        self.state = WindowState::Maximized;
    }

    /// Restore window
    pub fn restore(&mut self) {
        self.state = WindowState::Normal;
    }

    /// Enter fullscreen
    pub fn enter_fullscreen(&mut self) {
        self.state = WindowState::Fullscreen;
    }

    /// Exit fullscreen
    pub fn exit_fullscreen(&mut self) {
        self.state = WindowState::Normal;
    }

    /// Check if minimized
    pub fn is_minimized(&self) -> bool {
        self.state == WindowState::Minimized
    }

    /// Check if maximized
    pub fn is_maximized(&self) -> bool {
        self.state == WindowState::Maximized
    }

    /// Check if fullscreen
    pub fn is_fullscreen(&self) -> bool {
        self.state == WindowState::Fullscreen
    }

    /// Get window controls overlay
    pub fn controls_overlay(&self) -> Option<&WindowControlsOverlay> {
        self.controls_overlay.as_ref()
    }

    /// Enable window controls overlay
    pub fn enable_controls_overlay(&mut self) {
        self.controls_overlay = Some(WindowControlsOverlay::default());
    }

    /// Disable window controls overlay
    pub fn disable_controls_overlay(&mut self) {
        self.controls_overlay = None;
    }

    /// Set title bar style
    pub fn set_title_bar_style(&mut self, style: TitleBarStyle) {
        self.title_bar_style = style;
    }

    /// Get title bar style
    pub fn title_bar_style(&self) -> TitleBarStyle {
        self.title_bar_style
    }

    /// Set theme color
    pub fn set_theme_color(&mut self, color: u32) {
        self.theme_color = Some(color);
    }

    /// Get theme color
    pub fn theme_color(&self) -> Option<u32> {
        self.theme_color
    }

    /// Focus window
    pub fn focus(&mut self) {
        self.focused = true;
    }

    /// Blur window
    pub fn blur(&mut self) {
        self.focused = false;
    }

    /// Check if focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Show window
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide window
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Close window
    pub fn close(&mut self) {
        self.visible = false;
    }
}

/// Window bounds
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowBounds {
    /// X position
    pub x: i32,
    /// Y position
    pub y: i32,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
}

impl WindowBounds {
    /// Create new bounds
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Get center point
    pub fn center(&self) -> (i32, i32) {
        (
            self.x + (self.width as i32 / 2),
            self.y + (self.height as i32 / 2),
        )
    }

    /// Check if point is inside
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width as i32
            && y >= self.y
            && y < self.y + self.height as i32
    }

    /// Check if overlaps with another bounds
    pub fn overlaps(&self, other: &WindowBounds) -> bool {
        self.x < other.x + other.width as i32
            && self.x + self.width as i32 > other.x
            && self.y < other.y + other.height as i32
            && self.y + self.height as i32 > other.y
    }
}

/// Window state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    /// Normal
    Normal,
    /// Minimized
    Minimized,
    /// Maximized
    Maximized,
    /// Fullscreen
    Fullscreen,
}

impl Default for WindowState {
    fn default() -> Self {
        Self::Normal
    }
}

/// Title bar style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleBarStyle {
    /// Default (system)
    Default,
    /// Hidden (no title bar)
    Hidden,
    /// Hidden inset (content extends into title bar area)
    HiddenInset,
    /// Custom title bar
    Custom,
}

impl Default for TitleBarStyle {
    fn default() -> Self {
        Self::Default
    }
}

/// Window Controls Overlay
///
/// Allows web content to be displayed over the title bar area.
#[derive(Debug, Clone)]
pub struct WindowControlsOverlay {
    /// Visible
    visible: bool,
    /// Bounding rect
    bounding_rect: ControlsRect,
    /// Title bar area rect (where app can draw)
    title_bar_area: ControlsRect,
}

impl Default for WindowControlsOverlay {
    fn default() -> Self {
        Self {
            visible: true,
            bounding_rect: ControlsRect::default(),
            title_bar_area: ControlsRect::default(),
        }
    }
}

impl WindowControlsOverlay {
    /// Is visible
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Set visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Get bounding rect
    pub fn bounding_rect(&self) -> &ControlsRect {
        &self.bounding_rect
    }

    /// Get title bar area rect
    pub fn title_bar_area(&self) -> &ControlsRect {
        &self.title_bar_area
    }

    /// Update rects based on window size
    pub fn update_rects(&mut self, window_width: u32, controls_width: u32, height: u32) {
        // Controls are on the right side
        self.bounding_rect = ControlsRect {
            x: (window_width - controls_width) as i32,
            y: 0,
            width: controls_width,
            height,
        };

        // Title bar area is the left portion
        self.title_bar_area = ControlsRect {
            x: 0,
            y: 0,
            width: window_width - controls_width,
            height,
        };
    }
}

/// Controls rect
#[derive(Debug, Clone, Copy, Default)]
pub struct ControlsRect {
    /// X position
    pub x: i32,
    /// Y position
    pub y: i32,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
}

/// PWA Window Manager
pub struct PwaWindowManager {
    /// Active windows
    windows: Vec<PwaWindow>,
    /// Next window ID
    next_id: u64,
}

impl PwaWindowManager {
    /// Create new window manager
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            next_id: 1,
        }
    }

    /// Create window for app
    pub fn create_window(&mut self, app_id: String, display_mode: DisplayMode) -> &mut PwaWindow {
        let window = PwaWindow::new(app_id, display_mode);
        self.windows.push(window);
        self.windows.last_mut().unwrap()
    }

    /// Find window by app ID
    pub fn find_window(&self, app_id: &str) -> Option<&PwaWindow> {
        self.windows.iter().find(|w| w.app_id() == app_id)
    }

    /// Find mutable window by app ID
    pub fn find_window_mut(&mut self, app_id: &str) -> Option<&mut PwaWindow> {
        self.windows.iter_mut().find(|w| w.app_id() == app_id)
    }

    /// Close window
    pub fn close_window(&mut self, app_id: &str) {
        if let Some(pos) = self.windows.iter().position(|w| w.app_id() == app_id) {
            self.windows.remove(pos);
        }
    }

    /// Get all windows
    pub fn windows(&self) -> &[PwaWindow] {
        &self.windows
    }

    /// Get focused window
    pub fn focused_window(&self) -> Option<&PwaWindow> {
        self.windows.iter().find(|w| w.is_focused())
    }

    /// Get focused window mut
    pub fn focused_window_mut(&mut self) -> Option<&mut PwaWindow> {
        self.windows.iter_mut().find(|w| w.is_focused())
    }

    /// Focus window
    pub fn focus_window(&mut self, app_id: &str) {
        for window in &mut self.windows {
            if window.app_id() == app_id {
                window.focus();
            } else {
                window.blur();
            }
        }
    }

    /// Minimize all windows
    pub fn minimize_all(&mut self) {
        for window in &mut self.windows {
            window.minimize();
        }
    }

    /// Restore all windows
    pub fn restore_all(&mut self) {
        for window in &mut self.windows {
            window.restore();
        }
    }

    /// Count visible windows
    pub fn visible_count(&self) -> usize {
        self.windows.iter().filter(|w| w.is_visible()).count()
    }
}

impl Default for PwaWindowManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global PWA window manager
use spin::RwLock;
pub static PWA_WINDOW_MANAGER: RwLock<PwaWindowManager> = RwLock::new(PwaWindowManager {
    windows: Vec::new(),
    next_id: 1,
});
