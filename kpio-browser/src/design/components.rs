//! UI Component System
//!
//! Reusable UI components for KPIO Browser.

use alloc::string::String;
use alloc::vec::Vec;
use super::tokens::{Color, spacing, radius, Typography};
use super::theme::Theme;

/// Component size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Size {
    XSmall,
    Small,
    #[default]
    Medium,
    Large,
    XLarge,
}

impl Size {
    /// Get height in pixels
    pub fn height(&self) -> u32 {
        match self {
            Self::XSmall => 24,
            Self::Small => 32,
            Self::Medium => 40,
            Self::Large => 48,
            Self::XLarge => 56,
        }
    }

    /// Get padding
    pub fn padding(&self) -> u32 {
        match self {
            Self::XSmall => spacing::XS,
            Self::Small => spacing::SM,
            Self::Medium => spacing::MD,
            Self::Large => spacing::LG,
            Self::XLarge => spacing::XL,
        }
    }

    /// Get icon size
    pub fn icon_size(&self) -> u32 {
        match self {
            Self::XSmall => 14,
            Self::Small => 16,
            Self::Medium => 20,
            Self::Large => 24,
            Self::XLarge => 28,
        }
    }
}

/// Button component style
#[derive(Debug, Clone)]
pub struct ButtonStyle {
    /// Background color
    pub background: Color,
    /// Text color
    pub foreground: Color,
    /// Border color
    pub border: Color,
    /// Border width
    pub border_width: u32,
    /// Border radius
    pub radius: u32,
    /// Is disabled
    pub disabled: bool,
}

/// Button variant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Filled primary button
    #[default]
    Primary,
    /// Secondary/outlined button
    Secondary,
    /// Ghost/text button
    Ghost,
    /// Danger button
    Danger,
    /// Success button
    Success,
}

impl ButtonVariant {
    /// Get style for theme
    pub fn style(&self, theme: &Theme) -> ButtonStyle {
        let c = &theme.colors;
        match self {
            Self::Primary => ButtonStyle {
                background: c.primary,
                foreground: c.text_on_primary,
                border: c.primary,
                border_width: 0,
                radius: radius::MD,
                disabled: false,
            },
            Self::Secondary => ButtonStyle {
                background: Color::transparent(),
                foreground: c.text_primary,
                border: c.border,
                border_width: 1,
                radius: radius::MD,
                disabled: false,
            },
            Self::Ghost => ButtonStyle {
                background: Color::transparent(),
                foreground: c.text_primary,
                border: Color::transparent(),
                border_width: 0,
                radius: radius::MD,
                disabled: false,
            },
            Self::Danger => ButtonStyle {
                background: c.error,
                foreground: Color::white(),
                border: c.error,
                border_width: 0,
                radius: radius::MD,
                disabled: false,
            },
            Self::Success => ButtonStyle {
                background: c.success,
                foreground: Color::white(),
                border: c.success,
                border_width: 0,
                radius: radius::MD,
                disabled: false,
            },
        }
    }
}

/// Button component
#[derive(Debug, Clone)]
pub struct Button {
    /// Button label
    pub label: String,
    /// Icon (optional, before label)
    pub icon: Option<String>,
    /// Trailing icon (optional, after label)
    pub trailing_icon: Option<String>,
    /// Button variant
    pub variant: ButtonVariant,
    /// Button size
    pub size: Size,
    /// Is disabled
    pub disabled: bool,
    /// Is loading
    pub loading: bool,
    /// Full width
    pub full_width: bool,
}

impl Button {
    /// Create new button
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            trailing_icon: None,
            variant: ButtonVariant::Primary,
            size: Size::Medium,
            disabled: false,
            loading: false,
            full_width: false,
        }
    }

    /// Set variant
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set size
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Set icon
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set disabled
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Input field style
#[derive(Debug, Clone)]
pub struct InputStyle {
    pub background: Color,
    pub foreground: Color,
    pub placeholder: Color,
    pub border: Color,
    pub border_focus: Color,
    pub radius: u32,
}

/// Input component
#[derive(Debug, Clone)]
pub struct Input {
    /// Input value
    pub value: String,
    /// Placeholder text
    pub placeholder: String,
    /// Input type
    pub input_type: InputType,
    /// Label
    pub label: Option<String>,
    /// Helper text
    pub helper: Option<String>,
    /// Error message
    pub error: Option<String>,
    /// Is disabled
    pub disabled: bool,
    /// Is readonly
    pub readonly: bool,
    /// Size
    pub size: Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputType {
    #[default]
    Text,
    Password,
    Email,
    Number,
    Search,
    Url,
}

impl Input {
    /// Create new input
    pub fn new() -> Self {
        Self {
            value: String::new(),
            placeholder: String::new(),
            input_type: InputType::Text,
            label: None,
            helper: None,
            error: None,
            disabled: false,
            readonly: false,
            size: Size::Medium,
        }
    }

    /// Set placeholder
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Set label
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set error
    pub fn error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

/// Card component
#[derive(Debug, Clone)]
pub struct Card {
    /// Card title
    pub title: Option<String>,
    /// Card subtitle
    pub subtitle: Option<String>,
    /// Padding
    pub padding: u32,
    /// Border radius
    pub radius: u32,
    /// Has shadow
    pub shadow: bool,
    /// Is clickable
    pub clickable: bool,
}

impl Card {
    /// Create new card
    pub fn new() -> Self {
        Self {
            title: None,
            subtitle: None,
            padding: spacing::LG,
            radius: radius::LG,
            shadow: true,
            clickable: false,
        }
    }

    /// Set title
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set clickable
    pub fn clickable(mut self, clickable: bool) -> Self {
        self.clickable = clickable;
        self
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

/// Badge component
#[derive(Debug, Clone)]
pub struct Badge {
    /// Badge text
    pub text: String,
    /// Variant
    pub variant: BadgeVariant,
    /// Size
    pub size: Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeVariant {
    #[default]
    Default,
    Primary,
    Success,
    Warning,
    Error,
    Info,
}

impl Badge {
    /// Create new badge
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            variant: BadgeVariant::Default,
            size: Size::Small,
        }
    }

    /// Set variant
    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }
}

/// Avatar component
#[derive(Debug, Clone)]
pub struct Avatar {
    /// Image source (optional)
    pub src: Option<String>,
    /// Alt text / initials
    pub alt: String,
    /// Size in pixels
    pub size: u32,
    /// Is circular
    pub circular: bool,
    /// Status indicator
    pub status: Option<AvatarStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvatarStatus {
    Online,
    Away,
    Busy,
    Offline,
}

impl Avatar {
    /// Create new avatar
    pub fn new(alt: impl Into<String>) -> Self {
        Self {
            src: None,
            alt: alt.into(),
            size: 40,
            circular: true,
            status: None,
        }
    }

    /// Set image source
    pub fn src(mut self, src: impl Into<String>) -> Self {
        self.src = Some(src.into());
        self
    }

    /// Set size
    pub fn size(mut self, size: u32) -> Self {
        self.size = size;
        self
    }

    /// Get initials from alt text
    pub fn initials(&self) -> String {
        let words: Vec<&str> = self.alt.split_whitespace().take(2).collect();
        words.iter()
            .filter_map(|w| w.chars().next())
            .map(|c| c.to_uppercase().next().unwrap_or(c))
            .collect()
    }
}

/// Tab item
#[derive(Debug, Clone)]
pub struct TabItem {
    /// Tab ID
    pub id: String,
    /// Tab label
    pub label: String,
    /// Icon
    pub icon: Option<String>,
    /// Is active
    pub active: bool,
    /// Is disabled
    pub disabled: bool,
    /// Badge count
    pub badge: Option<u32>,
}

impl TabItem {
    /// Create new tab
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            active: false,
            disabled: false,
            badge: None,
        }
    }

    /// Set active
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Set icon
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// Toggle/Switch component
#[derive(Debug, Clone)]
pub struct Toggle {
    /// Is checked
    pub checked: bool,
    /// Is disabled
    pub disabled: bool,
    /// Size
    pub size: Size,
    /// Label
    pub label: Option<String>,
}

impl Toggle {
    /// Create new toggle
    pub fn new() -> Self {
        Self {
            checked: false,
            disabled: false,
            size: Size::Medium,
            label: None,
        }
    }

    /// Set checked
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Set label
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl Default for Toggle {
    fn default() -> Self {
        Self::new()
    }
}

/// Progress bar
#[derive(Debug, Clone)]
pub struct Progress {
    /// Value (0-100)
    pub value: u32,
    /// Max value
    pub max: u32,
    /// Is indeterminate
    pub indeterminate: bool,
    /// Size
    pub size: Size,
    /// Show label
    pub show_label: bool,
}

impl Progress {
    /// Create new progress bar
    pub fn new(value: u32) -> Self {
        Self {
            value: value.min(100),
            max: 100,
            indeterminate: false,
            size: Size::Medium,
            show_label: false,
        }
    }

    /// Create indeterminate progress
    pub fn indeterminate() -> Self {
        Self {
            value: 0,
            max: 100,
            indeterminate: true,
            size: Size::Medium,
            show_label: false,
        }
    }

    /// Get percentage
    pub fn percentage(&self) -> f32 {
        if self.max == 0 {
            0.0
        } else {
            (self.value as f32 / self.max as f32) * 100.0
        }
    }
}

/// Spinner/Loader
#[derive(Debug, Clone)]
pub struct Spinner {
    /// Size in pixels
    pub size: u32,
    /// Color (uses primary if None)
    pub color: Option<Color>,
}

impl Spinner {
    /// Create new spinner
    pub fn new() -> Self {
        Self {
            size: 24,
            color: None,
        }
    }

    /// Set size
    pub fn size(mut self, size: u32) -> Self {
        self.size = size;
        self
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

/// Tooltip
#[derive(Debug, Clone)]
pub struct Tooltip {
    /// Content
    pub content: String,
    /// Position
    pub position: TooltipPosition,
    /// Delay in ms
    pub delay: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TooltipPosition {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

impl Tooltip {
    /// Create new tooltip
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            position: TooltipPosition::Top,
            delay: 500,
        }
    }
}
