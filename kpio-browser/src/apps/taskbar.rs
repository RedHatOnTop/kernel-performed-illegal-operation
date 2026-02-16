//! Taskbar / Dock
//!
//! Bottom taskbar with app launcher, running apps, and system tray.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::design::{Color, Icon};

/// Taskbar position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskbarPosition {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

/// Taskbar style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskbarStyle {
    /// Windows-like taskbar
    #[default]
    Taskbar,
    /// macOS-like dock
    Dock,
    /// Minimal panel
    Panel,
}

/// Taskbar component
#[derive(Debug, Clone)]
pub struct Taskbar {
    /// Position
    pub position: TaskbarPosition,
    /// Style
    pub style: TaskbarStyle,
    /// Height/width (depending on position)
    pub size: u32,
    /// Auto-hide
    pub auto_hide: bool,
    /// Is visible (when auto-hide enabled)
    pub visible: bool,
    /// Pinned apps
    pub pinned: Vec<TaskbarItem>,
    /// Running apps
    pub running: Vec<TaskbarItem>,
    /// System tray items
    pub tray: SystemTray,
    /// Show app labels
    pub show_labels: bool,
    /// Centered icons (dock style)
    pub center_icons: bool,
    /// Small icons
    pub small_icons: bool,
}

impl Taskbar {
    /// Create new taskbar
    pub fn new() -> Self {
        Self {
            position: TaskbarPosition::Bottom,
            style: TaskbarStyle::Taskbar,
            size: 48,
            auto_hide: false,
            visible: true,
            pinned: Self::default_pinned(),
            running: Vec::new(),
            tray: SystemTray::new(),
            show_labels: true,
            center_icons: false,
            small_icons: false,
        }
    }

    /// Create dock style
    pub fn dock() -> Self {
        Self {
            position: TaskbarPosition::Bottom,
            style: TaskbarStyle::Dock,
            size: 64,
            auto_hide: false,
            visible: true,
            pinned: Self::default_pinned(),
            running: Vec::new(),
            tray: SystemTray::new(),
            show_labels: false,
            center_icons: true,
            small_icons: false,
        }
    }

    /// Default pinned apps
    fn default_pinned() -> Vec<TaskbarItem> {
        alloc::vec![
            TaskbarItem {
                id: 1,
                app_id: String::from("kpio.files"),
                name: String::from("Files"),
                icon: String::from("folder"),
                state: TaskbarItemState::Pinned,
                windows: Vec::new(),
                progress: None,
                badge: None,
            },
            TaskbarItem {
                id: 2,
                app_id: String::from("kpio.browser"),
                name: String::from("Browser"),
                icon: String::from("globe"),
                state: TaskbarItemState::Pinned,
                windows: Vec::new(),
                progress: None,
                badge: None,
            },
            TaskbarItem {
                id: 3,
                app_id: String::from("kpio.terminal"),
                name: String::from("Terminal"),
                icon: String::from("terminal"),
                state: TaskbarItemState::Pinned,
                windows: Vec::new(),
                progress: None,
                badge: None,
            },
            TaskbarItem {
                id: 4,
                app_id: String::from("kpio.settings"),
                name: String::from("Settings"),
                icon: String::from("settings"),
                state: TaskbarItemState::Pinned,
                windows: Vec::new(),
                progress: None,
                badge: None,
            },
        ]
    }

    /// Add running app
    pub fn add_running(&mut self, app_id: &str, name: &str, icon: &str, window_id: u64) {
        // Check if already in pinned
        if let Some(item) = self.pinned.iter_mut().find(|i| i.app_id == app_id) {
            item.windows.push(window_id);
            item.state = TaskbarItemState::Running;
            return;
        }

        // Check if already running
        if let Some(item) = self.running.iter_mut().find(|i| i.app_id == app_id) {
            item.windows.push(window_id);
            return;
        }

        // Add new
        let id = self.running.len() as u64 + 100;
        self.running.push(TaskbarItem {
            id,
            app_id: app_id.to_string(),
            name: name.to_string(),
            icon: icon.to_string(),
            state: TaskbarItemState::Running,
            windows: alloc::vec![window_id],
            progress: None,
            badge: None,
        });
    }

    /// Remove running app window
    pub fn remove_window(&mut self, window_id: u64) {
        // Remove from pinned
        for item in &mut self.pinned {
            item.windows.retain(|&w| w != window_id);
            if item.windows.is_empty() {
                item.state = TaskbarItemState::Pinned;
            }
        }

        // Remove from running
        self.running.retain(|item| {
            let mut windows = item.windows.clone();
            windows.retain(|&w| w != window_id);
            !windows.is_empty()
        });
    }

    /// Pin app
    pub fn pin(&mut self, app_id: &str, name: &str, icon: &str) {
        if self.pinned.iter().any(|i| i.app_id == app_id) {
            return;
        }

        // Move from running if exists
        if let Some(idx) = self.running.iter().position(|i| i.app_id == app_id) {
            let mut item = self.running.remove(idx);
            item.state = if item.windows.is_empty() {
                TaskbarItemState::Pinned
            } else {
                TaskbarItemState::Running
            };
            self.pinned.push(item);
            return;
        }

        // Add new pinned
        let id = self.pinned.len() as u64 + 1;
        self.pinned.push(TaskbarItem {
            id,
            app_id: app_id.to_string(),
            name: name.to_string(),
            icon: icon.to_string(),
            state: TaskbarItemState::Pinned,
            windows: Vec::new(),
            progress: None,
            badge: None,
        });
    }

    /// Unpin app
    pub fn unpin(&mut self, app_id: &str) {
        if let Some(idx) = self.pinned.iter().position(|i| i.app_id == app_id) {
            let item = self.pinned.remove(idx);

            // Move to running if has windows
            if !item.windows.is_empty() {
                self.running.push(item);
            }
        }
    }

    /// Get all items (pinned + running)
    pub fn all_items(&self) -> Vec<&TaskbarItem> {
        self.pinned.iter().chain(self.running.iter()).collect()
    }

    /// Set progress for app
    pub fn set_progress(&mut self, app_id: &str, progress: Option<f32>) {
        for item in self.pinned.iter_mut().chain(self.running.iter_mut()) {
            if item.app_id == app_id {
                item.progress = progress.map(|p| p.max(0.0).min(1.0));
            }
        }
    }

    /// Set badge for app
    pub fn set_badge(&mut self, app_id: &str, badge: Option<TaskbarBadge>) {
        for item in self.pinned.iter_mut().chain(self.running.iter_mut()) {
            if item.app_id == app_id {
                item.badge = badge.clone();
            }
        }
    }
}

impl Default for Taskbar {
    fn default() -> Self {
        Self::new()
    }
}

/// Taskbar item
#[derive(Debug, Clone)]
pub struct TaskbarItem {
    /// Item ID
    pub id: u64,
    /// App ID
    pub app_id: String,
    /// Display name
    pub name: String,
    /// Icon name
    pub icon: String,
    /// Item state
    pub state: TaskbarItemState,
    /// Window IDs
    pub windows: Vec<u64>,
    /// Progress indicator (0-1)
    pub progress: Option<f32>,
    /// Badge
    pub badge: Option<TaskbarBadge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarItemState {
    /// Pinned but not running
    Pinned,
    /// Running
    Running,
    /// Running and focused
    Active,
    /// Needs attention
    Attention,
}

/// Taskbar badge
#[derive(Debug, Clone)]
pub struct TaskbarBadge {
    /// Badge text
    pub text: String,
    /// Badge color
    pub color: Option<Color>,
}

impl TaskbarBadge {
    /// Count badge
    pub fn count(count: u32) -> Self {
        let text = if count > 99 {
            String::from("99+")
        } else {
            alloc::format!("{}", count)
        };
        Self { text, color: None }
    }

    /// Dot badge
    pub fn dot() -> Self {
        Self {
            text: String::new(),
            color: None,
        }
    }
}

/// System tray
#[derive(Debug, Clone)]
pub struct SystemTray {
    /// Tray items
    pub items: Vec<TrayItem>,
    /// Show clock
    pub show_clock: bool,
    /// Show date
    pub show_date: bool,
    /// Battery indicator
    pub battery: Option<BatteryInfo>,
    /// Network indicator
    pub network: Option<NetworkInfo>,
    /// Volume indicator
    pub volume: Option<VolumeInfo>,
}

impl SystemTray {
    /// Create new system tray
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            show_clock: true,
            show_date: true,
            battery: Some(BatteryInfo::default()),
            network: Some(NetworkInfo::default()),
            volume: Some(VolumeInfo::default()),
        }
    }

    /// Add tray item
    pub fn add_item(&mut self, item: TrayItem) {
        self.items.push(item);
    }

    /// Remove tray item
    pub fn remove_item(&mut self, id: &str) {
        self.items.retain(|i| i.id != id);
    }
}

impl Default for SystemTray {
    fn default() -> Self {
        Self::new()
    }
}

/// Tray item
#[derive(Debug, Clone)]
pub struct TrayItem {
    /// Item ID
    pub id: String,
    /// Tooltip
    pub tooltip: String,
    /// Icon
    pub icon: String,
    /// Is visible
    pub visible: bool,
}

/// Battery information
#[derive(Debug, Clone)]
pub struct BatteryInfo {
    /// Battery level (0-100)
    pub level: u8,
    /// Is charging
    pub charging: bool,
    /// Time remaining (minutes)
    pub time_remaining: Option<u32>,
    /// Is power saver on
    pub power_saver: bool,
}

impl Default for BatteryInfo {
    fn default() -> Self {
        Self {
            level: 100,
            charging: true,
            time_remaining: None,
            power_saver: false,
        }
    }
}

impl BatteryInfo {
    /// Get battery icon
    pub fn icon(&self) -> &'static str {
        if self.charging {
            "battery-charging"
        } else if self.level <= 10 {
            "battery-low"
        } else {
            "battery"
        }
    }
}

/// Network information
#[derive(Debug, Clone)]
pub struct NetworkInfo {
    /// Connection type
    pub connection_type: ConnectionType,
    /// Signal strength (0-100)
    pub signal_strength: u8,
    /// Network name (SSID)
    pub network_name: Option<String>,
    /// Is connected
    pub connected: bool,
}

impl Default for NetworkInfo {
    fn default() -> Self {
        Self {
            connection_type: ConnectionType::Ethernet,
            signal_strength: 100,
            network_name: None,
            connected: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionType {
    #[default]
    Ethernet,
    Wifi,
    Cellular,
    None,
}

impl NetworkInfo {
    /// Get network icon
    pub fn icon(&self) -> &'static str {
        if !self.connected {
            return "wifi-off";
        }
        match self.connection_type {
            ConnectionType::Ethernet => "wifi",
            ConnectionType::Wifi => "wifi",
            ConnectionType::Cellular => "wifi",
            ConnectionType::None => "wifi-off",
        }
    }
}

/// Volume information
#[derive(Debug, Clone)]
pub struct VolumeInfo {
    /// Volume level (0-100)
    pub level: u8,
    /// Is muted
    pub muted: bool,
}

impl Default for VolumeInfo {
    fn default() -> Self {
        Self {
            level: 50,
            muted: false,
        }
    }
}

impl VolumeInfo {
    /// Get volume icon
    pub fn icon(&self) -> &'static str {
        if self.muted || self.level == 0 {
            "volume-mute"
        } else if self.level < 50 {
            "volume-low"
        } else {
            "volume-high"
        }
    }
}

/// Start menu / App launcher button
#[derive(Debug, Clone)]
pub struct StartButton {
    /// Button style
    pub style: StartButtonStyle,
    /// Custom icon
    pub icon: Option<String>,
    /// Custom text
    pub text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StartButtonStyle {
    #[default]
    Icon,
    IconText,
    Text,
}

impl Default for StartButton {
    fn default() -> Self {
        Self {
            style: StartButtonStyle::Icon,
            icon: None,
            text: None,
        }
    }
}
