//! Internationalization Module (i18n)
//!
//! Unicode support, RTL text, and localization.

pub mod locale;
pub mod messages;
pub mod formatting;
pub mod bidi;

pub use locale::*;
pub use messages::*;
pub use formatting::*;
pub use bidi::*;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

/// i18n error
#[derive(Debug, Clone)]
pub enum I18nError {
    /// Locale not found
    LocaleNotFound(String),
    /// Message not found
    MessageNotFound(String),
    /// Invalid format
    InvalidFormat(String),
    /// Missing placeholder
    MissingPlaceholder(String),
}

/// Language direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Left-to-right
    Ltr,
    /// Right-to-left
    Rtl,
}

impl Default for Direction {
    fn default() -> Self {
        Self::Ltr
    }
}

/// Language info
#[derive(Debug, Clone)]
pub struct LanguageInfo {
    /// Language code (e.g., "en", "ko", "ar")
    pub code: String,
    /// Region code (e.g., "US", "KR", "SA")
    pub region: Option<String>,
    /// Script (e.g., "Latn", "Hang", "Arab")
    pub script: Option<String>,
    /// Native name
    pub native_name: String,
    /// English name
    pub english_name: String,
    /// Direction
    pub direction: Direction,
}

impl LanguageInfo {
    /// Create new language info
    pub fn new(code: &str, native_name: &str, english_name: &str) -> Self {
        Self {
            code: code.to_string(),
            region: None,
            script: None,
            native_name: native_name.to_string(),
            english_name: english_name.to_string(),
            direction: Direction::Ltr,
        }
    }

    /// Set RTL direction
    pub fn rtl(mut self) -> Self {
        self.direction = Direction::Rtl;
        self
    }

    /// Set region
    pub fn with_region(mut self, region: &str) -> Self {
        self.region = Some(region.to_string());
        self
    }

    /// Set script
    pub fn with_script(mut self, script: &str) -> Self {
        self.script = Some(script.to_string());
        self
    }

    /// Get full locale tag (e.g., "en-US", "ko-KR")
    pub fn locale_tag(&self) -> String {
        if let Some(ref region) = self.region {
            alloc::format!("{}-{}", self.code, region)
        } else {
            self.code.clone()
        }
    }
}

/// Supported languages
pub struct SupportedLanguages;

impl SupportedLanguages {
    /// English (US)
    pub fn english() -> LanguageInfo {
        LanguageInfo::new("en", "English", "English")
            .with_region("US")
    }

    /// Korean
    pub fn korean() -> LanguageInfo {
        LanguageInfo::new("ko", "한국어", "Korean")
            .with_region("KR")
            .with_script("Hang")
    }

    /// Japanese
    pub fn japanese() -> LanguageInfo {
        LanguageInfo::new("ja", "日本語", "Japanese")
            .with_region("JP")
    }

    /// Chinese (Simplified)
    pub fn chinese_simplified() -> LanguageInfo {
        LanguageInfo::new("zh", "简体中文", "Chinese (Simplified)")
            .with_region("CN")
            .with_script("Hans")
    }

    /// Chinese (Traditional)
    pub fn chinese_traditional() -> LanguageInfo {
        LanguageInfo::new("zh", "繁體中文", "Chinese (Traditional)")
            .with_region("TW")
            .with_script("Hant")
    }

    /// Arabic
    pub fn arabic() -> LanguageInfo {
        LanguageInfo::new("ar", "العربية", "Arabic")
            .with_region("SA")
            .rtl()
    }

    /// Hebrew
    pub fn hebrew() -> LanguageInfo {
        LanguageInfo::new("he", "עברית", "Hebrew")
            .with_region("IL")
            .rtl()
    }

    /// German
    pub fn german() -> LanguageInfo {
        LanguageInfo::new("de", "Deutsch", "German")
            .with_region("DE")
    }

    /// French
    pub fn french() -> LanguageInfo {
        LanguageInfo::new("fr", "Français", "French")
            .with_region("FR")
    }

    /// Spanish
    pub fn spanish() -> LanguageInfo {
        LanguageInfo::new("es", "Español", "Spanish")
            .with_region("ES")
    }

    /// Get all supported languages
    pub fn all() -> Vec<LanguageInfo> {
        alloc::vec![
            Self::english(),
            Self::korean(),
            Self::japanese(),
            Self::chinese_simplified(),
            Self::chinese_traditional(),
            Self::arabic(),
            Self::hebrew(),
            Self::german(),
            Self::french(),
            Self::spanish(),
        ]
    }
}

/// I18n manager
pub struct I18nManager {
    /// Current locale
    current_locale: String,
    /// Fallback locale
    fallback_locale: String,
    /// Available locales
    available_locales: Vec<String>,
    /// Language info
    languages: BTreeMap<String, LanguageInfo>,
    /// Message bundles
    bundles: BTreeMap<String, MessageBundle>,
    /// Number formatter
    number_formatter: NumberFormatter,
    /// Date formatter
    date_formatter: DateFormatter,
}

impl I18nManager {
    /// Create new manager
    pub fn new() -> Self {
        let mut manager = Self {
            current_locale: "en-US".to_string(),
            fallback_locale: "en-US".to_string(),
            available_locales: Vec::new(),
            languages: BTreeMap::new(),
            bundles: BTreeMap::new(),
            number_formatter: NumberFormatter::new(),
            date_formatter: DateFormatter::new(),
        };

        // Register supported languages
        for lang in SupportedLanguages::all() {
            let tag = lang.locale_tag();
            manager.available_locales.push(tag.clone());
            manager.languages.insert(tag, lang);
        }

        // Load built-in message bundles for all supported languages
        manager.bundles.insert("en-US".into(), english_bundle());
        manager.bundles.insert("ko-KR".into(), korean_bundle());
        manager.bundles.insert("ja-JP".into(), japanese_bundle());
        manager.bundles.insert("zh-CN".into(), chinese_simplified_bundle());
        manager.bundles.insert("es-ES".into(), spanish_bundle());
        manager.bundles.insert("de-DE".into(), german_bundle());
        manager.bundles.insert("fr-FR".into(), french_bundle());

        manager
    }

    /// Set current locale
    pub fn set_locale(&mut self, locale: &str) -> Result<(), I18nError> {
        if self.languages.contains_key(locale) || self.available_locales.contains(&locale.to_string()) {
            self.current_locale = locale.to_string();
            Ok(())
        } else {
            Err(I18nError::LocaleNotFound(locale.to_string()))
        }
    }

    /// Get current locale
    pub fn locale(&self) -> &str {
        &self.current_locale
    }

    /// Get current direction
    pub fn direction(&self) -> Direction {
        self.languages.get(&self.current_locale)
            .map(|l| l.direction)
            .unwrap_or(Direction::Ltr)
    }

    /// Get message
    pub fn get(&self, key: &str) -> Result<&str, I18nError> {
        // Try current locale
        if let Some(bundle) = self.bundles.get(&self.current_locale) {
            if let Some(msg) = bundle.get(key) {
                return Ok(msg);
            }
        }

        // Try fallback
        if let Some(bundle) = self.bundles.get(&self.fallback_locale) {
            if let Some(msg) = bundle.get(key) {
                return Ok(msg);
            }
        }

        Err(I18nError::MessageNotFound(key.to_string()))
    }

    /// Get message with formatting
    pub fn format(&self, key: &str, args: &BTreeMap<String, String>) -> Result<String, I18nError> {
        let template = self.get(key)?;
        self.format_string(template, args)
    }

    /// Format string with arguments
    fn format_string(&self, template: &str, args: &BTreeMap<String, String>) -> Result<String, I18nError> {
        let mut result = template.to_string();
        
        for (key, value) in args {
            let placeholder = alloc::format!("{{{}}}", key);
            result = result.replace(&placeholder, value);
        }

        // Check for missing placeholders
        if result.contains('{') && result.contains('}') {
            // Simple check - might have unresolved placeholders
        }

        Ok(result)
    }

    /// Load message bundle
    pub fn load_bundle(&mut self, locale: &str, bundle: MessageBundle) {
        self.bundles.insert(locale.to_string(), bundle);
    }

    /// Get number formatter
    pub fn number(&self) -> &NumberFormatter {
        &self.number_formatter
    }

    /// Get date formatter
    pub fn date(&self) -> &DateFormatter {
        &self.date_formatter
    }

    /// Available locales
    pub fn available_locales(&self) -> &[String] {
        &self.available_locales
    }

    /// Get language info
    pub fn language_info(&self, locale: &str) -> Option<&LanguageInfo> {
        self.languages.get(locale)
    }
}

impl Default for I18nManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global i18n manager
pub static I18N: RwLock<I18nManager> = RwLock::new(I18nManager {
    current_locale: String::new(),
    fallback_locale: String::new(),
    available_locales: Vec::new(),
    languages: BTreeMap::new(),
    bundles: BTreeMap::new(),
    number_formatter: NumberFormatter {
        decimal_separator: '.',
        thousands_separator: ',',
        grouping_size: 3,
    },
    date_formatter: DateFormatter {
        date_format: DateFormat::Short,
        time_format: TimeFormat::Short,
        first_day_of_week: 0,
        locale: String::new(),
    },
});
