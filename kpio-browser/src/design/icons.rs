//! Icon System
//!
//! Built-in icons for KPIO Browser UI.

use alloc::string::String;

/// Icon set
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    // Navigation
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronDown,
    Home,
    Refresh,
    ExternalLink,

    // Browser
    Globe,
    Search,
    Bookmark,
    BookmarkFilled,
    History,
    Download,
    Downloads,
    Tab,
    TabNew,
    TabClose,
    Window,
    WindowNew,
    Incognito,

    // Actions
    Plus,
    Minus,
    Close,
    Check,
    Menu,
    MoreHorizontal,
    MoreVertical,
    Edit,
    Copy,
    Paste,
    Cut,
    Trash,
    Share,
    Print,
    ZoomIn,
    ZoomOut,
    Fullscreen,
    ExitFullscreen,

    // Media
    Play,
    Pause,
    Stop,
    VolumeHigh,
    VolumeLow,
    VolumeMute,
    VolumeOff,

    // Files
    File,
    FileText,
    Folder,
    FolderOpen,
    Image,
    Video,
    Audio,
    Archive,

    // UI
    Settings,
    SettingsGear,
    User,
    Users,
    Lock,
    Unlock,
    Eye,
    EyeOff,
    Bell,
    BellOff,
    Info,
    Warning,
    Error,
    Help,
    Question,

    // System
    Sun,
    Moon,
    Monitor,
    Wifi,
    WifiOff,
    Battery,
    BatteryLow,
    BatteryCharging,
    Power,

    // Profile
    Profile,
    ProfileAdd,
    ProfileSwitch,
}

impl Icon {
    /// Get icon name
    pub fn name(&self) -> &'static str {
        match self {
            Self::ArrowLeft => "arrow-left",
            Self::ArrowRight => "arrow-right",
            Self::ArrowUp => "arrow-up",
            Self::ArrowDown => "arrow-down",
            Self::ChevronLeft => "chevron-left",
            Self::ChevronRight => "chevron-right",
            Self::ChevronUp => "chevron-up",
            Self::ChevronDown => "chevron-down",
            Self::Home => "home",
            Self::Refresh => "refresh",
            Self::ExternalLink => "external-link",

            Self::Globe => "globe",
            Self::Search => "search",
            Self::Bookmark => "bookmark",
            Self::BookmarkFilled => "bookmark-filled",
            Self::History => "history",
            Self::Download => "download",
            Self::Downloads => "downloads",
            Self::Tab => "tab",
            Self::TabNew => "tab-new",
            Self::TabClose => "tab-close",
            Self::Window => "window",
            Self::WindowNew => "window-new",
            Self::Incognito => "incognito",

            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::Close => "close",
            Self::Check => "check",
            Self::Menu => "menu",
            Self::MoreHorizontal => "more-horizontal",
            Self::MoreVertical => "more-vertical",
            Self::Edit => "edit",
            Self::Copy => "copy",
            Self::Paste => "paste",
            Self::Cut => "cut",
            Self::Trash => "trash",
            Self::Share => "share",
            Self::Print => "print",
            Self::ZoomIn => "zoom-in",
            Self::ZoomOut => "zoom-out",
            Self::Fullscreen => "fullscreen",
            Self::ExitFullscreen => "exit-fullscreen",

            Self::Play => "play",
            Self::Pause => "pause",
            Self::Stop => "stop",
            Self::VolumeHigh => "volume-high",
            Self::VolumeLow => "volume-low",
            Self::VolumeMute => "volume-mute",
            Self::VolumeOff => "volume-off",

            Self::File => "file",
            Self::FileText => "file-text",
            Self::Folder => "folder",
            Self::FolderOpen => "folder-open",
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Archive => "archive",

            Self::Settings => "settings",
            Self::SettingsGear => "settings-gear",
            Self::User => "user",
            Self::Users => "users",
            Self::Lock => "lock",
            Self::Unlock => "unlock",
            Self::Eye => "eye",
            Self::EyeOff => "eye-off",
            Self::Bell => "bell",
            Self::BellOff => "bell-off",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Help => "help",
            Self::Question => "question",

            Self::Sun => "sun",
            Self::Moon => "moon",
            Self::Monitor => "monitor",
            Self::Wifi => "wifi",
            Self::WifiOff => "wifi-off",
            Self::Battery => "battery",
            Self::BatteryLow => "battery-low",
            Self::BatteryCharging => "battery-charging",
            Self::Power => "power",

            Self::Profile => "profile",
            Self::ProfileAdd => "profile-add",
            Self::ProfileSwitch => "profile-switch",
        }
    }

    /// Get SVG path data (simplified icon paths)
    pub fn path_data(&self) -> &'static str {
        match self {
            Self::ArrowLeft => "M19 12H5M12 19l-7-7 7-7",
            Self::ArrowRight => "M5 12h14M12 5l7 7-7 7",
            Self::ChevronLeft => "M15 18l-6-6 6-6",
            Self::ChevronRight => "M9 18l6-6-6-6",
            Self::Home => "M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z",
            Self::Search => "M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z",
            Self::Close => "M18 6L6 18M6 6l12 12",
            Self::Plus => "M12 5v14M5 12h14",
            Self::Minus => "M5 12h14",
            Self::Check => "M20 6L9 17l-5-5",
            Self::Menu => "M4 6h16M4 12h16M4 18h16",
            Self::Settings => "M12 15a3 3 0 100-6 3 3 0 000 6z",
            Self::User => "M20 21v-2a4 4 0 00-4-4H8a4 4 0 00-4 4v2",
            Self::Globe => "M12 2a10 10 0 100 20 10 10 0 000-20z",
            Self::Bookmark => "M19 21l-7-5-7 5V5a2 2 0 012-2h10a2 2 0 012 2z",
            Self::Refresh => "M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15",
            _ => "M12 12m-10 0a10 10 0 1020 0 10 10 0 10-20 0", // Default circle
        }
    }
}

/// Icon component
#[derive(Debug, Clone)]
pub struct IconComponent {
    /// Icon to display
    pub icon: Icon,
    /// Size in pixels
    pub size: u32,
    /// Color (uses foreground if None)
    pub color: Option<super::tokens::Color>,
    /// Stroke width
    pub stroke_width: f32,
}

impl IconComponent {
    /// Create new icon
    pub fn new(icon: Icon) -> Self {
        Self {
            icon,
            size: 24,
            color: None,
            stroke_width: 2.0,
        }
    }

    /// Set size
    pub fn size(mut self, size: u32) -> Self {
        self.size = size;
        self
    }

    /// Set color
    pub fn color(mut self, color: super::tokens::Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Set stroke width
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }
}

/// Icon button
#[derive(Debug, Clone)]
pub struct IconButton {
    /// Icon to display
    pub icon: Icon,
    /// Size
    pub size: super::components::Size,
    /// Variant
    pub variant: IconButtonVariant,
    /// Is disabled
    pub disabled: bool,
    /// Tooltip
    pub tooltip: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconButtonVariant {
    #[default]
    Ghost,
    Filled,
    Outlined,
}

impl IconButton {
    /// Create new icon button
    pub fn new(icon: Icon) -> Self {
        Self {
            icon,
            size: super::components::Size::Medium,
            variant: IconButtonVariant::Ghost,
            disabled: false,
            tooltip: None,
        }
    }

    /// Set tooltip
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Set variant
    pub fn variant(mut self, variant: IconButtonVariant) -> Self {
        self.variant = variant;
        self
    }
}
