//! Page Components
//!
//! Full-page components like new tab page, settings, etc.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{
    components::{Button, ButtonVariant, Card, Input, Size, Toggle},
    icons::Icon,
    layout::{AlignItems, EdgeInsets, Flex, Grid, JustifyContent},
    theme::Theme,
    tokens::{radius, spacing, Color},
};

/// New tab page
#[derive(Debug, Clone)]
pub struct NewTabPage {
    /// Search bar focused
    pub search_focused: bool,
    /// Quick links
    pub quick_links: Vec<QuickLink>,
    /// Recent tabs
    pub recent_tabs: Vec<RecentTab>,
    /// Show recent tabs
    pub show_recent: bool,
    /// Background image
    pub background: Option<String>,
    /// Clock visible
    pub show_clock: bool,
    /// Greeting visible
    pub show_greeting: bool,
}

impl NewTabPage {
    /// Create new tab page
    pub fn new() -> Self {
        Self {
            search_focused: false,
            quick_links: Self::default_links(),
            recent_tabs: Vec::new(),
            show_recent: true,
            background: None,
            show_clock: true,
            show_greeting: true,
        }
    }

    /// Default quick links
    fn default_links() -> Vec<QuickLink> {
        alloc::vec![
            QuickLink::new("Google", "https://google.com"),
            QuickLink::new("YouTube", "https://youtube.com"),
            QuickLink::new("GitHub", "https://github.com"),
            QuickLink::new("Reddit", "https://reddit.com"),
        ]
    }

    /// Get greeting based on time of day
    pub fn greeting(&self, hour: u8) -> &'static str {
        match hour {
            5..=11 => "Good morning",
            12..=17 => "Good afternoon",
            18..=21 => "Good evening",
            _ => "Good night",
        }
    }
}

impl Default for NewTabPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Quick link tile
#[derive(Debug, Clone)]
pub struct QuickLink {
    /// Title
    pub title: String,
    /// URL
    pub url: String,
    /// Favicon
    pub favicon: Option<String>,
    /// Custom color
    pub color: Option<Color>,
    /// Is pinned
    pub pinned: bool,
}

impl QuickLink {
    /// Create new quick link
    pub fn new(title: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            url: url.into(),
            favicon: None,
            color: None,
            pinned: false,
        }
    }

    /// Get initials for fallback icon
    pub fn initials(&self) -> String {
        self.title
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default()
    }
}

/// Recent tab entry
#[derive(Debug, Clone)]
pub struct RecentTab {
    /// Title
    pub title: String,
    /// URL
    pub url: String,
    /// Favicon
    pub favicon: Option<String>,
    /// Last accessed timestamp
    pub last_accessed: u64,
    /// Device name (for synced tabs)
    pub device: Option<String>,
}

/// Settings page
#[derive(Debug, Clone)]
pub struct SettingsPage {
    /// Current section
    pub section: SettingsSection,
    /// Search query
    pub search: String,
}

impl SettingsPage {
    /// Create new settings page
    pub fn new() -> Self {
        Self {
            section: SettingsSection::General,
            search: String::new(),
        }
    }

    /// Get sections
    pub fn sections() -> Vec<SettingsSectionInfo> {
        alloc::vec![
            SettingsSectionInfo {
                section: SettingsSection::General,
                title: String::from("General"),
                icon: Icon::Settings,
            },
            SettingsSectionInfo {
                section: SettingsSection::Appearance,
                title: String::from("Appearance"),
                icon: Icon::Sun,
            },
            SettingsSectionInfo {
                section: SettingsSection::Privacy,
                title: String::from("Privacy"),
                icon: Icon::Lock,
            },
            SettingsSectionInfo {
                section: SettingsSection::Search,
                title: String::from("Search"),
                icon: Icon::Search,
            },
            SettingsSectionInfo {
                section: SettingsSection::Downloads,
                title: String::from("Downloads"),
                icon: Icon::Download,
            },
            SettingsSectionInfo {
                section: SettingsSection::Languages,
                title: String::from("Languages"),
                icon: Icon::Globe,
            },
            SettingsSectionInfo {
                section: SettingsSection::Accessibility,
                title: String::from("Accessibility"),
                icon: Icon::Eye,
            },
            SettingsSectionInfo {
                section: SettingsSection::About,
                title: String::from("About KPIO"),
                icon: Icon::Info,
            },
        ]
    }
}

impl Default for SettingsPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Settings section
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsSection {
    #[default]
    General,
    Appearance,
    Privacy,
    Search,
    Downloads,
    Languages,
    Accessibility,
    About,
}

/// Settings section info
#[derive(Debug, Clone)]
pub struct SettingsSectionInfo {
    pub section: SettingsSection,
    pub title: String,
    pub icon: Icon,
}

/// Settings group
#[derive(Debug, Clone)]
pub struct SettingsGroup {
    /// Title
    pub title: String,
    /// Description
    pub description: Option<String>,
    /// Settings items
    pub items: Vec<SettingsItem>,
}

impl SettingsGroup {
    /// Create new group
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
            items: Vec::new(),
        }
    }

    /// Add description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add item
    pub fn item(mut self, item: SettingsItem) -> Self {
        self.items.push(item);
        self
    }
}

/// Settings item
#[derive(Debug, Clone)]
pub struct SettingsItem {
    /// Item type
    pub item_type: SettingsItemType,
    /// Label
    pub label: String,
    /// Description
    pub description: Option<String>,
    /// Current value
    pub value: SettingsValue,
    /// Is disabled
    pub disabled: bool,
}

#[derive(Debug, Clone)]
pub enum SettingsItemType {
    Toggle,
    Select,
    Input,
    Link,
    Button,
}

#[derive(Debug, Clone)]
pub enum SettingsValue {
    Bool(bool),
    String(String),
    Number(i64),
    Option(String, Vec<String>),
    None,
}

impl SettingsItem {
    /// Create toggle item
    pub fn toggle(label: impl Into<String>, value: bool) -> Self {
        Self {
            item_type: SettingsItemType::Toggle,
            label: label.into(),
            description: None,
            value: SettingsValue::Bool(value),
            disabled: false,
        }
    }

    /// Create select item
    pub fn select(
        label: impl Into<String>,
        value: impl Into<String>,
        options: Vec<String>,
    ) -> Self {
        Self {
            item_type: SettingsItemType::Select,
            label: label.into(),
            description: None,
            value: SettingsValue::Option(value.into(), options),
            disabled: false,
        }
    }

    /// Create link item
    pub fn link(label: impl Into<String>) -> Self {
        Self {
            item_type: SettingsItemType::Link,
            label: label.into(),
            description: None,
            value: SettingsValue::None,
            disabled: false,
        }
    }

    /// Add description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// History page
#[derive(Debug, Clone)]
pub struct HistoryPage {
    /// Search query
    pub search: String,
    /// Date filter
    pub date_filter: DateFilter,
    /// History entries
    pub entries: Vec<HistoryEntry>,
    /// Selected entries
    pub selected: Vec<u64>,
}

impl HistoryPage {
    /// Create new history page
    pub fn new() -> Self {
        Self {
            search: String::new(),
            date_filter: DateFilter::All,
            entries: Vec::new(),
            selected: Vec::new(),
        }
    }
}

impl Default for HistoryPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Date filter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateFilter {
    #[default]
    All,
    Today,
    Yesterday,
    LastWeek,
    LastMonth,
    Custom,
}

/// History entry
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// Entry ID
    pub id: u64,
    /// Title
    pub title: String,
    /// URL
    pub url: String,
    /// Favicon
    pub favicon: Option<String>,
    /// Visit timestamp
    pub visited_at: u64,
    /// Visit count
    pub visit_count: u32,
}

/// Bookmarks page
#[derive(Debug, Clone)]
pub struct BookmarksPage {
    /// Current folder
    pub current_folder: u64,
    /// Breadcrumb path
    pub path: Vec<BookmarkFolder>,
    /// Folders in current location
    pub folders: Vec<BookmarkFolder>,
    /// Bookmarks in current location
    pub bookmarks: Vec<Bookmark>,
    /// View mode
    pub view_mode: BookmarkViewMode,
    /// Search query
    pub search: String,
}

impl BookmarksPage {
    /// Create new bookmarks page
    pub fn new() -> Self {
        Self {
            current_folder: 0, // Root
            path: alloc::vec![BookmarkFolder::root()],
            folders: Vec::new(),
            bookmarks: Vec::new(),
            view_mode: BookmarkViewMode::List,
            search: String::new(),
        }
    }
}

impl Default for BookmarksPage {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BookmarkViewMode {
    #[default]
    List,
    Grid,
}

/// Bookmark folder
#[derive(Debug, Clone)]
pub struct BookmarkFolder {
    /// Folder ID
    pub id: u64,
    /// Folder name
    pub name: String,
    /// Parent folder ID
    pub parent_id: Option<u64>,
    /// Item count
    pub item_count: u32,
}

impl BookmarkFolder {
    /// Create root folder
    pub fn root() -> Self {
        Self {
            id: 0,
            name: String::from("Bookmarks"),
            parent_id: None,
            item_count: 0,
        }
    }
}

/// Bookmark
#[derive(Debug, Clone)]
pub struct Bookmark {
    /// Bookmark ID
    pub id: u64,
    /// Title
    pub title: String,
    /// URL
    pub url: String,
    /// Favicon
    pub favicon: Option<String>,
    /// Folder ID
    pub folder_id: u64,
    /// Added timestamp
    pub added_at: u64,
}

/// Downloads page
#[derive(Debug, Clone)]
pub struct DownloadsPage {
    /// Downloads list
    pub downloads: Vec<DownloadItem>,
    /// Search query
    pub search: String,
    /// Show completed
    pub show_completed: bool,
}

impl DownloadsPage {
    /// Create new downloads page
    pub fn new() -> Self {
        Self {
            downloads: Vec::new(),
            search: String::new(),
            show_completed: true,
        }
    }
}

impl Default for DownloadsPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Download item
#[derive(Debug, Clone)]
pub struct DownloadItem {
    /// Download ID
    pub id: u64,
    /// File name
    pub filename: String,
    /// Source URL
    pub url: String,
    /// Download path
    pub path: String,
    /// Status
    pub status: DownloadStatus,
    /// Total size in bytes
    pub total_bytes: u64,
    /// Downloaded bytes
    pub downloaded_bytes: u64,
    /// Speed in bytes/sec
    pub speed: u64,
    /// Started timestamp
    pub started_at: u64,
}

impl DownloadItem {
    /// Get progress percentage
    pub fn progress(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.downloaded_bytes as f32 / self.total_bytes as f32) * 100.0
        }
    }

    /// Format size
    pub fn format_size(bytes: u64) -> String {
        if bytes < 1024 {
            alloc::format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            alloc::format!("{:.1} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            alloc::format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            alloc::format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

/// Download status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl DownloadStatus {
    /// Get icon
    pub fn icon(&self) -> Icon {
        match self {
            Self::Pending => Icon::Download,
            Self::Downloading => Icon::Download,
            Self::Paused => Icon::Pause,
            Self::Completed => Icon::Check,
            Self::Failed => Icon::Error,
            Self::Cancelled => Icon::Close,
        }
    }
}
