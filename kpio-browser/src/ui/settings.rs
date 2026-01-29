//! Settings Panel
//!
//! Privacy settings, appearance themes, and browser configuration.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use spin::RwLock;

use super::Theme;

/// Settings category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    /// General settings.
    General,
    /// Appearance.
    Appearance,
    /// Privacy and security.
    Privacy,
    /// Search engine.
    Search,
    /// Default browser.
    DefaultBrowser,
    /// Downloads.
    Downloads,
    /// Languages.
    Languages,
    /// Accessibility.
    Accessibility,
    /// Advanced.
    Advanced,
}

/// Setting value.
#[derive(Debug, Clone)]
pub enum SettingValue {
    Bool(bool),
    String(String),
    Int(i64),
    Float(f64),
    Enum(String),
    List(Vec<String>),
}

impl SettingValue {
    /// Get as bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }
    
    /// Get as string.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(v) => Some(v),
            _ => None,
        }
    }
    
    /// Get as int.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            _ => None,
        }
    }
}

/// Setting definition.
#[derive(Debug, Clone)]
pub struct SettingDef {
    /// Setting key.
    pub key: String,
    /// Display name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Category.
    pub category: SettingsCategory,
    /// Default value.
    pub default: SettingValue,
    /// Whether it requires restart.
    pub requires_restart: bool,
}

/// Privacy settings.
#[derive(Debug, Clone)]
pub struct PrivacySettings {
    /// Send Do Not Track.
    pub do_not_track: bool,
    /// Block third-party cookies.
    pub block_third_party_cookies: bool,
    /// Block all cookies.
    pub block_all_cookies: bool,
    /// Clear cookies on exit.
    pub clear_cookies_on_exit: bool,
    /// Clear history on exit.
    pub clear_history_on_exit: bool,
    /// Clear cache on exit.
    pub clear_cache_on_exit: bool,
    /// Safe browsing enabled.
    pub safe_browsing: bool,
    /// Enhanced safe browsing.
    pub enhanced_safe_browsing: bool,
    /// Send usage statistics.
    pub send_usage_stats: bool,
    /// Hardware security keys.
    pub use_security_keys: bool,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            do_not_track: true,
            block_third_party_cookies: true,
            block_all_cookies: false,
            clear_cookies_on_exit: false,
            clear_history_on_exit: false,
            clear_cache_on_exit: false,
            safe_browsing: true,
            enhanced_safe_browsing: false,
            send_usage_stats: false,
            use_security_keys: true,
        }
    }
}

/// Appearance settings.
#[derive(Debug, Clone)]
pub struct AppearanceSettings {
    /// Theme.
    pub theme: Theme,
    /// Font size.
    pub font_size: FontSize,
    /// Page zoom (100 = 100%).
    pub page_zoom: u32,
    /// Show home button.
    pub show_home_button: bool,
    /// Home page URL.
    pub home_page: String,
    /// Show bookmarks bar.
    pub show_bookmarks_bar: bool,
    /// Tab position.
    pub tab_position: TabPosition,
    /// Use system title bar.
    pub use_system_titlebar: bool,
}

/// Font size preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontSize {
    VerySmall,
    Small,
    #[default]
    Medium,
    Large,
    VeryLarge,
}

impl FontSize {
    /// Get pixel size.
    pub fn to_pixels(&self) -> u32 {
        match self {
            Self::VerySmall => 12,
            Self::Small => 14,
            Self::Medium => 16,
            Self::Large => 18,
            Self::VeryLarge => 20,
        }
    }
}

/// Tab position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabPosition {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            font_size: FontSize::Medium,
            page_zoom: 100,
            show_home_button: true,
            home_page: "about:newtab".to_string(),
            show_bookmarks_bar: true,
            tab_position: TabPosition::Top,
            use_system_titlebar: false,
        }
    }
}

/// Search engine.
#[derive(Debug, Clone)]
pub struct SearchEngine {
    /// Name.
    pub name: String,
    /// Short name.
    pub short_name: String,
    /// Search URL template ({searchTerms} is replaced).
    pub search_url: String,
    /// Suggest URL template.
    pub suggest_url: Option<String>,
    /// Favicon URL.
    pub favicon_url: Option<String>,
    /// Whether it's the default.
    pub is_default: bool,
}

impl SearchEngine {
    /// Create Google search engine.
    pub fn google() -> Self {
        Self {
            name: "Google".to_string(),
            short_name: "google".to_string(),
            search_url: "https://www.google.com/search?q={searchTerms}".to_string(),
            suggest_url: Some("https://www.google.com/complete/search?q={searchTerms}&output=firefox".to_string()),
            favicon_url: Some("https://www.google.com/favicon.ico".to_string()),
            is_default: true,
        }
    }
    
    /// Create DuckDuckGo search engine.
    pub fn duckduckgo() -> Self {
        Self {
            name: "DuckDuckGo".to_string(),
            short_name: "duckduckgo".to_string(),
            search_url: "https://duckduckgo.com/?q={searchTerms}".to_string(),
            suggest_url: Some("https://duckduckgo.com/ac/?q={searchTerms}".to_string()),
            favicon_url: Some("https://duckduckgo.com/favicon.ico".to_string()),
            is_default: false,
        }
    }
    
    /// Create Bing search engine.
    pub fn bing() -> Self {
        Self {
            name: "Bing".to_string(),
            short_name: "bing".to_string(),
            search_url: "https://www.bing.com/search?q={searchTerms}".to_string(),
            suggest_url: Some("https://www.bing.com/osjson.aspx?query={searchTerms}".to_string()),
            favicon_url: Some("https://www.bing.com/favicon.ico".to_string()),
            is_default: false,
        }
    }
    
    /// Get search URL for query.
    pub fn get_search_url(&self, query: &str) -> String {
        self.search_url.replace("{searchTerms}", &url_encode(query))
    }
}

/// URL encode a string.
fn url_encode(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                result.push(c);
            }
            ' ' => result.push('+'),
            _ => {
                for b in c.to_string().as_bytes() {
                    result.push('%');
                    result.push_str(&alloc::format!("{:02X}", b));
                }
            }
        }
    }
    result
}

/// Search settings.
#[derive(Debug, Clone)]
pub struct SearchSettings {
    /// Available search engines.
    pub engines: Vec<SearchEngine>,
    /// Default search engine index.
    pub default_engine_index: usize,
    /// Show search suggestions.
    pub show_suggestions: bool,
    /// Preload search results.
    pub preload_results: bool,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            engines: vec![
                SearchEngine::google(),
                SearchEngine::duckduckgo(),
                SearchEngine::bing(),
            ],
            default_engine_index: 0,
            show_suggestions: true,
            preload_results: false,
        }
    }
}

impl SearchSettings {
    /// Get default search engine.
    pub fn default_engine(&self) -> &SearchEngine {
        &self.engines[self.default_engine_index.min(self.engines.len() - 1)]
    }
    
    /// Set default search engine by name.
    pub fn set_default(&mut self, short_name: &str) {
        if let Some(idx) = self.engines.iter().position(|e| e.short_name == short_name) {
            self.default_engine_index = idx;
        }
    }
    
    /// Add custom search engine.
    pub fn add_engine(&mut self, engine: SearchEngine) {
        self.engines.push(engine);
    }
}

/// Download settings.
#[derive(Debug, Clone)]
pub struct DownloadSettings {
    /// Default download path.
    pub download_path: String,
    /// Ask where to save.
    pub ask_where_to_save: bool,
    /// Open safe files automatically.
    pub open_safe_files: bool,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            download_path: "/home/user/Downloads".to_string(),
            ask_where_to_save: false,
            open_safe_files: true,
        }
    }
}

/// Language settings.
#[derive(Debug, Clone)]
pub struct LanguageSettings {
    /// UI language.
    pub ui_language: String,
    /// Spell check enabled.
    pub spell_check: bool,
    /// Spell check languages.
    pub spell_check_languages: Vec<String>,
    /// Offer translation.
    pub offer_translation: bool,
    /// Never translate languages.
    pub never_translate: Vec<String>,
}

impl Default for LanguageSettings {
    fn default() -> Self {
        Self {
            ui_language: "en-US".to_string(),
            spell_check: true,
            spell_check_languages: vec!["en-US".to_string()],
            offer_translation: true,
            never_translate: Vec::new(),
        }
    }
}

/// All browser settings.
#[derive(Debug, Clone)]
pub struct BrowserSettings {
    /// Privacy settings.
    pub privacy: PrivacySettings,
    /// Appearance settings.
    pub appearance: AppearanceSettings,
    /// Search settings.
    pub search: SearchSettings,
    /// Download settings.
    pub downloads: DownloadSettings,
    /// Language settings.
    pub language: LanguageSettings,
    /// Startup behavior.
    pub startup: StartupBehavior,
    /// Is default browser.
    pub is_default_browser: bool,
}

/// Startup behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StartupBehavior {
    /// Open new tab page.
    #[default]
    NewTabPage,
    /// Continue where you left off.
    RestoreSession,
    /// Open specific pages.
    SpecificPages,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            privacy: PrivacySettings::default(),
            appearance: AppearanceSettings::default(),
            search: SearchSettings::default(),
            downloads: DownloadSettings::default(),
            language: LanguageSettings::default(),
            startup: StartupBehavior::NewTabPage,
            is_default_browser: false,
        }
    }
}

/// Settings manager.
pub struct SettingsManager {
    /// Current settings.
    settings: RwLock<BrowserSettings>,
    /// Custom values (overrides).
    custom: RwLock<BTreeMap<String, SettingValue>>,
    /// Observers.
    observers: RwLock<Vec<SettingsObserver>>,
}

/// Settings observer callback.
type SettingsObserver = alloc::boxed::Box<dyn Fn(&str, &SettingValue) + Send + Sync>;

impl SettingsManager {
    /// Create a new settings manager.
    pub fn new() -> Self {
        Self {
            settings: RwLock::new(BrowserSettings::default()),
            custom: RwLock::new(BTreeMap::new()),
            observers: RwLock::new(Vec::new()),
        }
    }
    
    /// Get current settings.
    pub fn settings(&self) -> BrowserSettings {
        self.settings.read().clone()
    }
    
    /// Update privacy settings.
    pub fn set_privacy(&self, privacy: PrivacySettings) {
        self.settings.write().privacy = privacy;
    }
    
    /// Update appearance settings.
    pub fn set_appearance(&self, appearance: AppearanceSettings) {
        self.settings.write().appearance = appearance;
    }
    
    /// Update search settings.
    pub fn set_search(&self, search: SearchSettings) {
        self.settings.write().search = search;
    }
    
    /// Set custom value.
    pub fn set_custom(&self, key: &str, value: SettingValue) {
        self.custom.write().insert(key.to_string(), value.clone());
        for observer in self.observers.read().iter() {
            observer(key, &value);
        }
    }
    
    /// Get custom value.
    pub fn get_custom(&self, key: &str) -> Option<SettingValue> {
        self.custom.read().get(key).cloned()
    }
    
    /// Add observer.
    pub fn observe<F>(&self, callback: F)
    where
        F: Fn(&str, &SettingValue) + Send + Sync + 'static,
    {
        self.observers.write().push(alloc::boxed::Box::new(callback));
    }
    
    /// Export settings as JSON-like structure.
    pub fn export(&self) -> BTreeMap<String, SettingValue> {
        let settings = self.settings.read();
        let mut map = BTreeMap::new();
        
        // Privacy
        map.insert("privacy.do_not_track".to_string(), SettingValue::Bool(settings.privacy.do_not_track));
        map.insert("privacy.block_third_party_cookies".to_string(), SettingValue::Bool(settings.privacy.block_third_party_cookies));
        map.insert("privacy.safe_browsing".to_string(), SettingValue::Bool(settings.privacy.safe_browsing));
        
        // Appearance
        map.insert("appearance.theme".to_string(), SettingValue::Enum(match settings.appearance.theme {
            Theme::Light => "light".to_string(),
            Theme::Dark => "dark".to_string(),
            Theme::System => "system".to_string(),
        }));
        map.insert("appearance.page_zoom".to_string(), SettingValue::Int(settings.appearance.page_zoom as i64));
        
        // Search
        map.insert("search.default_engine".to_string(), SettingValue::String(
            settings.search.default_engine().short_name.clone()
        ));
        
        // Custom values
        let custom = self.custom.read();
        for (k, v) in custom.iter() {
            map.insert(k.clone(), v.clone());
        }
        
        map
    }
    
    /// Import settings.
    pub fn import(&self, data: BTreeMap<String, SettingValue>) {
        let mut settings = self.settings.write();
        
        if let Some(SettingValue::Bool(v)) = data.get("privacy.do_not_track") {
            settings.privacy.do_not_track = *v;
        }
        if let Some(SettingValue::Bool(v)) = data.get("privacy.block_third_party_cookies") {
            settings.privacy.block_third_party_cookies = *v;
        }
        if let Some(SettingValue::Bool(v)) = data.get("privacy.safe_browsing") {
            settings.privacy.safe_browsing = *v;
        }
        
        if let Some(SettingValue::Enum(theme)) = data.get("appearance.theme") {
            settings.appearance.theme = match theme.as_str() {
                "light" => Theme::Light,
                "dark" => Theme::Dark,
                _ => Theme::System,
            };
        }
        if let Some(SettingValue::Int(zoom)) = data.get("appearance.page_zoom") {
            settings.appearance.page_zoom = *zoom as u32;
        }
        
        if let Some(SettingValue::String(engine)) = data.get("search.default_engine") {
            settings.search.set_default(engine);
        }
    }
}

impl Default for SettingsManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_search_engine() {
        let engine = SearchEngine::google();
        let url = engine.get_search_url("hello world");
        assert!(url.contains("hello+world"));
    }
    
    #[test]
    fn test_settings_manager() {
        let manager = SettingsManager::new();
        
        let settings = manager.settings();
        assert_eq!(settings.appearance.theme, Theme::System);
        
        let mut appearance = settings.appearance.clone();
        appearance.theme = Theme::Dark;
        manager.set_appearance(appearance);
        
        assert_eq!(manager.settings().appearance.theme, Theme::Dark);
    }
    
    #[test]
    fn test_export_import() {
        let manager = SettingsManager::new();
        
        let mut appearance = manager.settings().appearance.clone();
        appearance.page_zoom = 125;
        manager.set_appearance(appearance);
        
        let exported = manager.export();
        
        let manager2 = SettingsManager::new();
        manager2.import(exported);
        
        assert_eq!(manager2.settings().appearance.page_zoom, 125);
    }
}
