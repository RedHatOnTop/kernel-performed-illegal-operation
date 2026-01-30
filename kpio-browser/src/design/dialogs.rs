//! Dialog Components
//!
//! Modal dialogs, alerts, and popups for KPIO Browser.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{
    tokens::{Color, spacing, radius, shadows, z_index},
    theme::Theme,
    components::{Button, ButtonVariant, Size},
    layout::{Flex, JustifyContent, AlignItems, EdgeInsets},
    icons::Icon,
};

/// Dialog size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogSize {
    Small,
    #[default]
    Medium,
    Large,
    FullScreen,
}

impl DialogSize {
    /// Get width in pixels
    pub fn width(&self) -> u32 {
        match self {
            Self::Small => 320,
            Self::Medium => 480,
            Self::Large => 640,
            Self::FullScreen => u32::MAX,
        }
    }

    /// Get max height
    pub fn max_height(&self) -> u32 {
        match self {
            Self::Small => 300,
            Self::Medium => 480,
            Self::Large => 640,
            Self::FullScreen => u32::MAX,
        }
    }
}

/// Modal dialog
#[derive(Debug, Clone)]
pub struct Dialog {
    /// Dialog ID
    pub id: String,
    /// Title
    pub title: String,
    /// Size
    pub size: DialogSize,
    /// Is open
    pub open: bool,
    /// Close on backdrop click
    pub close_on_backdrop: bool,
    /// Close on escape key
    pub close_on_escape: bool,
    /// Show close button
    pub show_close: bool,
    /// Custom close button text
    pub close_text: Option<String>,
}

impl Dialog {
    /// Create new dialog
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            size: DialogSize::Medium,
            open: false,
            close_on_backdrop: true,
            close_on_escape: true,
            show_close: true,
            close_text: None,
        }
    }

    /// Set size
    pub fn size(mut self, size: DialogSize) -> Self {
        self.size = size;
        self
    }

    /// Open dialog
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Close dialog
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Toggle dialog
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
}

/// Alert dialog
#[derive(Debug, Clone)]
pub struct Alert {
    /// Alert type
    pub alert_type: AlertType,
    /// Title
    pub title: String,
    /// Message
    pub message: String,
    /// Primary button text
    pub primary_text: String,
    /// Secondary button text
    pub secondary_text: Option<String>,
    /// Is open
    pub open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertType {
    #[default]
    Info,
    Success,
    Warning,
    Error,
    Confirm,
}

impl AlertType {
    /// Get icon
    pub fn icon(&self) -> Icon {
        match self {
            Self::Info => Icon::Info,
            Self::Success => Icon::Check,
            Self::Warning => Icon::Warning,
            Self::Error => Icon::Error,
            Self::Confirm => Icon::Question,
        }
    }

    /// Get primary button variant
    pub fn button_variant(&self) -> ButtonVariant {
        match self {
            Self::Error | Self::Warning => ButtonVariant::Danger,
            _ => ButtonVariant::Primary,
        }
    }
}

impl Alert {
    /// Create info alert
    pub fn info(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            alert_type: AlertType::Info,
            title: title.into(),
            message: message.into(),
            primary_text: String::from("확인"),
            secondary_text: None,
            open: true,
        }
    }

    /// Create success alert
    pub fn success(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            alert_type: AlertType::Success,
            title: title.into(),
            message: message.into(),
            primary_text: String::from("확인"),
            secondary_text: None,
            open: true,
        }
    }

    /// Create warning alert
    pub fn warning(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            alert_type: AlertType::Warning,
            title: title.into(),
            message: message.into(),
            primary_text: String::from("확인"),
            secondary_text: None,
            open: true,
        }
    }

    /// Create error alert
    pub fn error(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            alert_type: AlertType::Error,
            title: title.into(),
            message: message.into(),
            primary_text: String::from("확인"),
            secondary_text: None,
            open: true,
        }
    }

    /// Create confirm dialog
    pub fn confirm(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            alert_type: AlertType::Confirm,
            title: title.into(),
            message: message.into(),
            primary_text: String::from("확인"),
            secondary_text: Some(String::from("취소")),
            open: true,
        }
    }

    /// Set button texts
    pub fn buttons(mut self, primary: impl Into<String>, secondary: Option<String>) -> Self {
        self.primary_text = primary.into();
        self.secondary_text = secondary;
        self
    }
}

/// Toast notification
#[derive(Debug, Clone)]
pub struct Toast {
    /// Toast ID
    pub id: u64,
    /// Toast type
    pub toast_type: ToastType,
    /// Message
    pub message: String,
    /// Duration in ms (0 = manual dismiss)
    pub duration: u32,
    /// Action button text
    pub action: Option<String>,
    /// Is dismissible
    pub dismissible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastType {
    #[default]
    Default,
    Success,
    Warning,
    Error,
    Info,
}

impl ToastType {
    /// Get icon
    pub fn icon(&self) -> Option<Icon> {
        match self {
            Self::Default => None,
            Self::Success => Some(Icon::Check),
            Self::Warning => Some(Icon::Warning),
            Self::Error => Some(Icon::Error),
            Self::Info => Some(Icon::Info),
        }
    }
}

impl Toast {
    /// Create new toast
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            id: 0,
            toast_type: ToastType::Default,
            message: message.into(),
            duration: 3000,
            action: None,
            dismissible: true,
        }
    }

    /// Success toast
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message).toast_type(ToastType::Success)
    }

    /// Error toast
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message).toast_type(ToastType::Error).duration(5000)
    }

    /// Warning toast
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message).toast_type(ToastType::Warning)
    }

    /// Set toast type
    pub fn toast_type(mut self, toast_type: ToastType) -> Self {
        self.toast_type = toast_type;
        self
    }

    /// Set duration
    pub fn duration(mut self, duration: u32) -> Self {
        self.duration = duration;
        self
    }

    /// Add action button
    pub fn action(mut self, text: impl Into<String>) -> Self {
        self.action = Some(text.into());
        self
    }

    /// Make persistent (no auto-dismiss)
    pub fn persistent(mut self) -> Self {
        self.duration = 0;
        self
    }
}

/// Toast container
#[derive(Debug, Clone)]
pub struct ToastContainer {
    /// Active toasts
    pub toasts: Vec<Toast>,
    /// Position
    pub position: ToastPosition,
    /// Next toast ID
    next_id: u64,
    /// Max visible toasts
    pub max_visible: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastPosition {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    #[default]
    BottomRight,
}

impl ToastContainer {
    /// Create new container
    pub fn new() -> Self {
        Self {
            toasts: Vec::new(),
            position: ToastPosition::BottomRight,
            next_id: 1,
            max_visible: 5,
        }
    }

    /// Show toast
    pub fn show(&mut self, mut toast: Toast) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        toast.id = id;
        
        // Remove oldest if at max
        while self.toasts.len() >= self.max_visible {
            self.toasts.remove(0);
        }
        
        self.toasts.push(toast);
        id
    }

    /// Dismiss toast
    pub fn dismiss(&mut self, id: u64) {
        self.toasts.retain(|t| t.id != id);
    }

    /// Dismiss all
    pub fn dismiss_all(&mut self) {
        self.toasts.clear();
    }
}

impl Default for ToastContainer {
    fn default() -> Self {
        Self::new()
    }
}

/// Context menu
#[derive(Debug, Clone)]
pub struct ContextMenu {
    /// Menu items
    pub items: Vec<MenuItem>,
    /// Position X
    pub x: i32,
    /// Position Y
    pub y: i32,
    /// Is open
    pub open: bool,
}

impl ContextMenu {
    /// Create new context menu
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            x: 0,
            y: 0,
            open: false,
        }
    }

    /// Add item
    pub fn item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    /// Add separator
    pub fn separator(mut self) -> Self {
        self.items.push(MenuItem::separator());
        self
    }

    /// Show at position
    pub fn show_at(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
        self.open = true;
    }

    /// Close menu
    pub fn close(&mut self) {
        self.open = false;
    }
}

impl Default for ContextMenu {
    fn default() -> Self {
        Self::new()
    }
}

/// Menu item
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Item type
    pub item_type: MenuItemType,
    /// Label
    pub label: String,
    /// Icon
    pub icon: Option<Icon>,
    /// Keyboard shortcut
    pub shortcut: Option<String>,
    /// Is disabled
    pub disabled: bool,
    /// Is checked (for checkbox items)
    pub checked: bool,
    /// Submenu
    pub submenu: Option<Vec<MenuItem>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuItemType {
    #[default]
    Normal,
    Checkbox,
    Radio,
    Separator,
}

impl MenuItem {
    /// Create new menu item
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            item_type: MenuItemType::Normal,
            label: label.into(),
            icon: None,
            shortcut: None,
            disabled: false,
            checked: false,
            submenu: None,
        }
    }

    /// Create separator
    pub fn separator() -> Self {
        Self {
            item_type: MenuItemType::Separator,
            label: String::new(),
            icon: None,
            shortcut: None,
            disabled: false,
            checked: false,
            submenu: None,
        }
    }

    /// Set icon
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Set shortcut
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Set disabled
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set as checkbox
    pub fn checkbox(mut self, checked: bool) -> Self {
        self.item_type = MenuItemType::Checkbox;
        self.checked = checked;
        self
    }

    /// Add submenu
    pub fn submenu(mut self, items: Vec<MenuItem>) -> Self {
        self.submenu = Some(items);
        self
    }
}

/// Dropdown menu
#[derive(Debug, Clone)]
pub struct Dropdown {
    /// Trigger label
    pub label: String,
    /// Items
    pub items: Vec<MenuItem>,
    /// Is open
    pub open: bool,
    /// Selected value
    pub selected: Option<String>,
    /// Placeholder
    pub placeholder: String,
}

impl Dropdown {
    /// Create new dropdown
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            label: String::new(),
            items: Vec::new(),
            open: false,
            selected: None,
            placeholder: placeholder.into(),
        }
    }

    /// Add option
    pub fn option(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.items.push(MenuItem::new(label));
        self
    }

    /// Select value
    pub fn select(&mut self, value: impl Into<String>) {
        let value = value.into();
        self.selected = Some(value.clone());
        self.label = self.items.iter()
            .find(|i| i.label == value)
            .map(|i| i.label.clone())
            .unwrap_or(value);
    }

    /// Toggle open
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
}

/// Popover
#[derive(Debug, Clone)]
pub struct Popover {
    /// Is open
    pub open: bool,
    /// Anchor position
    pub anchor: PopoverAnchor,
    /// Offset X
    pub offset_x: i32,
    /// Offset Y
    pub offset_y: i32,
    /// Arrow visible
    pub show_arrow: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopoverAnchor {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
    BottomStart,
    BottomEnd,
    TopStart,
    TopEnd,
}

impl Popover {
    /// Create new popover
    pub fn new() -> Self {
        Self {
            open: false,
            anchor: PopoverAnchor::Bottom,
            offset_x: 0,
            offset_y: 8,
            show_arrow: true,
        }
    }

    /// Set anchor
    pub fn anchor(mut self, anchor: PopoverAnchor) -> Self {
        self.anchor = anchor;
        self
    }

    /// Toggle
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
}

impl Default for Popover {
    fn default() -> Self {
        Self::new()
    }
}
