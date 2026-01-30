//! Browser Chrome Components
//!
//! Tab bar, address bar, and toolbar components.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::design::{
    tokens::{Color, spacing, radius, shadows},
    theme::Theme,
    components::{Size, Button, ButtonVariant, Input, InputType, Badge, Progress},
    layout::{Flex, JustifyContent, AlignItems, EdgeInsets},
    icons::{Icon, IconButton},
};

/// Browser tab
#[derive(Debug, Clone)]
pub struct BrowserTab {
    /// Tab ID
    pub id: u64,
    /// Tab title
    pub title: String,
    /// Tab URL
    pub url: String,
    /// Favicon URL
    pub favicon: Option<String>,
    /// Is active tab
    pub active: bool,
    /// Is loading
    pub loading: bool,
    /// Is pinned
    pub pinned: bool,
    /// Is playing audio
    pub playing_audio: bool,
    /// Is muted
    pub muted: bool,
    /// Close button visible
    pub show_close: bool,
}

impl BrowserTab {
    /// Create new tab
    pub fn new(id: u64, title: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            url: url.into(),
            favicon: None,
            active: false,
            loading: false,
            pinned: false,
            playing_audio: false,
            muted: false,
            show_close: true,
        }
    }

    /// New tab (blank)
    pub fn new_tab(id: u64) -> Self {
        Self::new(id, "새 탭", "kpio://newtab")
    }

    /// Set active
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Set loading
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Set pinned
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self.show_close = !pinned;
        self
    }

    /// Get display title (truncated)
    pub fn display_title(&self, max_len: usize) -> String {
        if self.title.len() <= max_len {
            self.title.clone()
        } else {
            let mut title = self.title.chars().take(max_len - 1).collect::<String>();
            title.push('…');
            title
        }
    }

    /// Get tab width based on state
    pub fn width(&self, available_width: u32, tab_count: usize) -> u32 {
        if self.pinned {
            return 40; // Pinned tabs are icon-only
        }

        let min_width = 100u32;
        let max_width = 240u32;
        
        let ideal_width = available_width / (tab_count as u32).max(1);
        ideal_width.max(min_width).min(max_width)
    }
}

/// Tab bar component
#[derive(Debug, Clone)]
pub struct TabBar {
    /// Tabs
    pub tabs: Vec<BrowserTab>,
    /// Active tab index
    pub active_index: usize,
    /// Show new tab button
    pub show_new_tab: bool,
    /// Tab overflow (scrollable)
    pub overflow: TabOverflow,
    /// Height
    pub height: u32,
}

impl TabBar {
    /// Create new tab bar
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_index: 0,
            show_new_tab: true,
            overflow: TabOverflow::Scroll,
            height: 36,
        }
    }

    /// Add tab
    pub fn add_tab(&mut self, tab: BrowserTab) {
        self.tabs.push(tab);
    }

    /// Close tab
    pub fn close_tab(&mut self, index: usize) -> Option<BrowserTab> {
        if index < self.tabs.len() {
            let tab = self.tabs.remove(index);
            if self.active_index >= self.tabs.len() && self.active_index > 0 {
                self.active_index -= 1;
            }
            Some(tab)
        } else {
            None
        }
    }

    /// Set active tab
    pub fn set_active(&mut self, index: usize) {
        if index < self.tabs.len() {
            // Deactivate current
            if let Some(tab) = self.tabs.get_mut(self.active_index) {
                tab.active = false;
            }
            // Activate new
            if let Some(tab) = self.tabs.get_mut(index) {
                tab.active = true;
            }
            self.active_index = index;
        }
    }

    /// Get active tab
    pub fn active_tab(&self) -> Option<&BrowserTab> {
        self.tabs.get(self.active_index)
    }

    /// Count
    pub fn count(&self) -> usize {
        self.tabs.len()
    }
}

impl Default for TabBar {
    fn default() -> Self {
        Self::new()
    }
}

/// Tab overflow behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabOverflow {
    #[default]
    Scroll,
    Shrink,
    Stack,
}

/// Address bar component
#[derive(Debug, Clone)]
pub struct AddressBar {
    /// Current URL
    pub url: String,
    /// Display URL (formatted)
    pub display_url: String,
    /// Is focused
    pub focused: bool,
    /// Is secure (HTTPS)
    pub secure: bool,
    /// Security level
    pub security: SecurityLevel,
    /// Is loading
    pub loading: bool,
    /// Load progress (0-100)
    pub progress: u32,
    /// Search suggestions visible
    pub suggestions_visible: bool,
    /// Suggestions
    pub suggestions: Vec<Suggestion>,
}

impl AddressBar {
    /// Create new address bar
    pub fn new() -> Self {
        Self {
            url: String::new(),
            display_url: String::new(),
            focused: false,
            secure: false,
            security: SecurityLevel::None,
            loading: false,
            progress: 0,
            suggestions_visible: false,
            suggestions: Vec::new(),
        }
    }

    /// Set URL
    pub fn set_url(&mut self, url: impl Into<String>) {
        let url = url.into();
        self.display_url = Self::format_url(&url);
        self.url = url;
        self.update_security();
    }

    /// Format URL for display
    fn format_url(url: &str) -> String {
        // Remove protocol for display
        url.strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(url)
            .to_string()
    }

    /// Update security status
    fn update_security(&mut self) {
        if self.url.starts_with("https://") {
            self.secure = true;
            self.security = SecurityLevel::Secure;
        } else if self.url.starts_with("http://") {
            self.secure = false;
            self.security = SecurityLevel::NotSecure;
        } else if self.url.starts_with("kpio://") {
            self.secure = true;
            self.security = SecurityLevel::Internal;
        } else {
            self.secure = false;
            self.security = SecurityLevel::None;
        }
    }

    /// Start loading
    pub fn start_loading(&mut self) {
        self.loading = true;
        self.progress = 0;
    }

    /// Update progress
    pub fn set_progress(&mut self, progress: u32) {
        self.progress = progress.min(100);
    }

    /// Finish loading
    pub fn finish_loading(&mut self) {
        self.loading = false;
        self.progress = 100;
    }

    /// Get security icon
    pub fn security_icon(&self) -> Icon {
        match self.security {
            SecurityLevel::Secure => Icon::Lock,
            SecurityLevel::NotSecure => Icon::Unlock,
            SecurityLevel::Dangerous => Icon::Warning,
            SecurityLevel::Internal => Icon::Globe,
            SecurityLevel::None => Icon::Globe,
        }
    }
}

impl Default for AddressBar {
    fn default() -> Self {
        Self::new()
    }
}

/// Security level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SecurityLevel {
    #[default]
    None,
    Secure,
    NotSecure,
    Dangerous,
    Internal,
}

/// URL/Search suggestion
#[derive(Debug, Clone)]
pub struct Suggestion {
    /// Suggestion type
    pub suggestion_type: SuggestionType,
    /// Display text
    pub text: String,
    /// Secondary text
    pub secondary: Option<String>,
    /// URL (for history/bookmark)
    pub url: Option<String>,
    /// Icon
    pub icon: Icon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionType {
    History,
    Bookmark,
    Search,
    Url,
}

/// Toolbar component
#[derive(Debug, Clone)]
pub struct Toolbar {
    /// Navigation buttons visible
    pub show_navigation: bool,
    /// Can go back
    pub can_go_back: bool,
    /// Can go forward
    pub can_go_forward: bool,
    /// Bookmarked
    pub bookmarked: bool,
    /// Extensions visible
    pub show_extensions: bool,
    /// Extension icons
    pub extensions: Vec<ExtensionIcon>,
    /// Menu button visible
    pub show_menu: bool,
}

impl Toolbar {
    /// Create new toolbar
    pub fn new() -> Self {
        Self {
            show_navigation: true,
            can_go_back: false,
            can_go_forward: false,
            bookmarked: false,
            show_extensions: true,
            extensions: Vec::new(),
            show_menu: true,
        }
    }

    /// Update navigation state
    pub fn update_navigation(&mut self, can_back: bool, can_forward: bool) {
        self.can_go_back = can_back;
        self.can_go_forward = can_forward;
    }
}

impl Default for Toolbar {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension icon in toolbar
#[derive(Debug, Clone)]
pub struct ExtensionIcon {
    /// Extension ID
    pub id: String,
    /// Extension name
    pub name: String,
    /// Icon URL
    pub icon_url: String,
    /// Badge text
    pub badge: Option<String>,
    /// Badge color
    pub badge_color: Option<Color>,
}

/// Status bar (bottom)
#[derive(Debug, Clone)]
pub struct StatusBar {
    /// Visible
    pub visible: bool,
    /// Status text
    pub text: String,
    /// Hover URL
    pub hover_url: Option<String>,
    /// Zoom level
    pub zoom: u32,
    /// Height
    pub height: u32,
}

impl StatusBar {
    /// Create new status bar
    pub fn new() -> Self {
        Self {
            visible: true,
            text: String::new(),
            hover_url: None,
            zoom: 100,
            height: 24,
        }
    }

    /// Set hover URL
    pub fn set_hover_url(&mut self, url: Option<String>) {
        self.hover_url = url.clone();
        if let Some(url) = url {
            self.text = Self::format_url(&url);
        } else {
            self.text.clear();
        }
    }

    fn format_url(url: &str) -> String {
        // Truncate long URLs
        if url.len() > 80 {
            let mut truncated = url.chars().take(77).collect::<String>();
            truncated.push_str("...");
            truncated
        } else {
            url.to_string()
        }
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete browser chrome
#[derive(Debug, Clone)]
pub struct BrowserChrome {
    /// Tab bar
    pub tab_bar: TabBar,
    /// Toolbar
    pub toolbar: Toolbar,
    /// Address bar
    pub address_bar: AddressBar,
    /// Status bar
    pub status_bar: StatusBar,
    /// Bookmarks bar visible
    pub bookmarks_bar_visible: bool,
    /// Fullscreen mode
    pub fullscreen: bool,
}

impl BrowserChrome {
    /// Create new browser chrome
    pub fn new() -> Self {
        let mut chrome = Self {
            tab_bar: TabBar::new(),
            toolbar: Toolbar::new(),
            address_bar: AddressBar::new(),
            status_bar: StatusBar::new(),
            bookmarks_bar_visible: true,
            fullscreen: false,
        };

        // Add initial tab
        let tab = BrowserTab::new_tab(0).active(true);
        chrome.tab_bar.add_tab(tab);

        chrome
    }

    /// Get total chrome height
    pub fn chrome_height(&self) -> u32 {
        if self.fullscreen {
            return 0;
        }

        let mut height = self.tab_bar.height + 40; // tab bar + toolbar

        if self.bookmarks_bar_visible {
            height += 32;
        }

        if self.status_bar.visible {
            height += self.status_bar.height;
        }

        height
    }

    /// Enter fullscreen
    pub fn enter_fullscreen(&mut self) {
        self.fullscreen = true;
    }

    /// Exit fullscreen
    pub fn exit_fullscreen(&mut self) {
        self.fullscreen = false;
    }
}

impl Default for BrowserChrome {
    fn default() -> Self {
        Self::new()
    }
}
