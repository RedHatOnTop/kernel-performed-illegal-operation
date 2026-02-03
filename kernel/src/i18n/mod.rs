//! Internationalization (i18n) Module
//!
//! Provides multi-language support for KPIO OS including:
//! - Translation key management
//! - Locale detection and switching
//! - Date/time/number formatting
//! - RTL support

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use spin::RwLock;

/// Supported locales
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    English,
    Korean,
    Japanese,
    ChineseSimplified,
    Spanish,
    German,
}

impl Locale {
    /// Get locale code (BCP 47)
    pub fn code(&self) -> &'static str {
        match self {
            Locale::English => "en",
            Locale::Korean => "ko",
            Locale::Japanese => "ja",
            Locale::ChineseSimplified => "zh-CN",
            Locale::Spanish => "es",
            Locale::German => "de",
        }
    }

    /// Get locale display name (in its own language)
    pub fn native_name(&self) -> &'static str {
        match self {
            Locale::English => "English",
            Locale::Korean => "한국어",
            Locale::Japanese => "日本語",
            Locale::ChineseSimplified => "简体中文",
            Locale::Spanish => "Español",
            Locale::German => "Deutsch",
        }
    }

    /// Check if locale is RTL (right-to-left)
    pub fn is_rtl(&self) -> bool {
        // None of our currently supported locales are RTL
        false
    }

    /// Parse from code
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "en" | "en-US" | "en-GB" => Some(Locale::English),
            "ko" | "ko-KR" => Some(Locale::Korean),
            "ja" | "ja-JP" => Some(Locale::Japanese),
            "zh-CN" | "zh-Hans" => Some(Locale::ChineseSimplified),
            "es" | "es-ES" | "es-MX" => Some(Locale::Spanish),
            "de" | "de-DE" => Some(Locale::German),
            _ => None,
        }
    }
}

impl Default for Locale {
    fn default() -> Self {
        Locale::English
    }
}

/// Translation storage
pub struct TranslationStore {
    /// Current locale
    current: Locale,
    /// Translation maps per locale
    translations: BTreeMap<&'static str, BTreeMap<&'static str, &'static str>>,
}

impl TranslationStore {
    /// Create new translation store with default locale
    pub fn new() -> Self {
        let mut store = Self {
            current: Locale::English,
            translations: BTreeMap::new(),
        };
        
        // Load built-in translations
        store.load_english();
        store.load_korean();
        
        store
    }

    /// Load English translations
    fn load_english(&mut self) {
        let mut en = BTreeMap::new();
        
        // Desktop
        en.insert("desktop.welcome", "Welcome to KPIO OS");
        en.insert("desktop.logout", "Log Out");
        en.insert("desktop.shutdown", "Shut Down");
        en.insert("desktop.restart", "Restart");
        en.insert("desktop.lock", "Lock Screen");
        en.insert("desktop.search", "Search");
        
        // Browser
        en.insert("browser.new_tab", "New Tab");
        en.insert("browser.close_tab", "Close Tab");
        en.insert("browser.bookmarks", "Bookmarks");
        en.insert("browser.history", "History");
        en.insert("browser.downloads", "Downloads");
        en.insert("browser.settings", "Settings");
        en.insert("browser.private_mode", "Private Browsing");
        en.insert("browser.address_bar", "Enter URL or search");
        
        // Settings
        en.insert("settings.title", "Settings");
        en.insert("settings.general", "General");
        en.insert("settings.appearance", "Appearance");
        en.insert("settings.network", "Network");
        en.insert("settings.security", "Security");
        en.insert("settings.privacy", "Privacy");
        en.insert("settings.language", "Language");
        en.insert("settings.about", "About");
        
        // File Manager
        en.insert("files.new_folder", "New Folder");
        en.insert("files.new_file", "New File");
        en.insert("files.copy", "Copy");
        en.insert("files.paste", "Paste");
        en.insert("files.delete", "Delete");
        en.insert("files.rename", "Rename");
        en.insert("files.properties", "Properties");
        
        // Common
        en.insert("common.ok", "OK");
        en.insert("common.cancel", "Cancel");
        en.insert("common.apply", "Apply");
        en.insert("common.save", "Save");
        en.insert("common.close", "Close");
        en.insert("common.yes", "Yes");
        en.insert("common.no", "No");
        en.insert("common.error", "Error");
        en.insert("common.warning", "Warning");
        en.insert("common.info", "Information");
        
        self.translations.insert("en", en);
    }

    /// Load Korean translations
    fn load_korean(&mut self) {
        let mut ko = BTreeMap::new();
        
        // Desktop
        ko.insert("desktop.welcome", "KPIO OS에 오신 것을 환영합니다");
        ko.insert("desktop.logout", "로그아웃");
        ko.insert("desktop.shutdown", "시스템 종료");
        ko.insert("desktop.restart", "다시 시작");
        ko.insert("desktop.lock", "화면 잠금");
        ko.insert("desktop.search", "검색");
        
        // Browser
        ko.insert("browser.new_tab", "새 탭");
        ko.insert("browser.close_tab", "탭 닫기");
        ko.insert("browser.bookmarks", "북마크");
        ko.insert("browser.history", "방문 기록");
        ko.insert("browser.downloads", "다운로드");
        ko.insert("browser.settings", "설정");
        ko.insert("browser.private_mode", "시크릿 모드");
        ko.insert("browser.address_bar", "URL 입력 또는 검색");
        
        // Settings
        ko.insert("settings.title", "설정");
        ko.insert("settings.general", "일반");
        ko.insert("settings.appearance", "모양");
        ko.insert("settings.network", "네트워크");
        ko.insert("settings.security", "보안");
        ko.insert("settings.privacy", "개인정보");
        ko.insert("settings.language", "언어");
        ko.insert("settings.about", "정보");
        
        // File Manager
        ko.insert("files.new_folder", "새 폴더");
        ko.insert("files.new_file", "새 파일");
        ko.insert("files.copy", "복사");
        ko.insert("files.paste", "붙여넣기");
        ko.insert("files.delete", "삭제");
        ko.insert("files.rename", "이름 바꾸기");
        ko.insert("files.properties", "속성");
        
        // Common
        ko.insert("common.ok", "확인");
        ko.insert("common.cancel", "취소");
        ko.insert("common.apply", "적용");
        ko.insert("common.save", "저장");
        ko.insert("common.close", "닫기");
        ko.insert("common.yes", "예");
        ko.insert("common.no", "아니오");
        ko.insert("common.error", "오류");
        ko.insert("common.warning", "경고");
        ko.insert("common.info", "정보");
        
        self.translations.insert("ko", ko);
    }

    /// Set current locale
    pub fn set_locale(&mut self, locale: Locale) {
        self.current = locale;
    }

    /// Get current locale
    pub fn current_locale(&self) -> Locale {
        self.current
    }

    /// Translate a key
    pub fn translate<'a>(&'a self, key: &'a str) -> &'a str {
        // Try current locale first
        if let Some(translations) = self.translations.get(self.current.code()) {
            if let Some(text) = translations.get(key) {
                return text;
            }
        }

        // Fall back to English
        if let Some(translations) = self.translations.get("en") {
            if let Some(text) = translations.get(key) {
                return text;
            }
        }

        // Return key if not found
        key
    }

    /// Get all available locales
    pub fn available_locales(&self) -> Vec<Locale> {
        vec![
            Locale::English,
            Locale::Korean,
            Locale::Japanese,
            Locale::ChineseSimplified,
            Locale::Spanish,
            Locale::German,
        ]
    }
}

impl Default for TranslationStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global translation store
static TRANSLATIONS: RwLock<Option<TranslationStore>> = RwLock::new(None);

/// Initialize the i18n system
pub fn init() {
    let mut store = TRANSLATIONS.write();
    if store.is_none() {
        *store = Some(TranslationStore::new());
    }
}

/// Set the current locale
pub fn set_locale(locale: Locale) {
    if let Some(ref mut store) = *TRANSLATIONS.write() {
        store.set_locale(locale);
    }
}

/// Get current locale
pub fn current_locale() -> Locale {
    TRANSLATIONS.read().as_ref()
        .map(|s| s.current_locale())
        .unwrap_or_default()
}

/// Translate a key
pub fn t(key: &str) -> String {
    TRANSLATIONS.read().as_ref()
        .map(|s| String::from(s.translate(key)))
        .unwrap_or_else(|| String::from(key))
}

/// Date formatting
pub mod date {
    use super::Locale;

    /// Format a date according to locale
    pub fn format_date(year: u32, month: u32, day: u32, locale: Locale) -> alloc::string::String {
        match locale {
            Locale::English | Locale::Spanish => {
                alloc::format!("{:02}/{:02}/{:04}", month, day, year)
            }
            Locale::Korean | Locale::Japanese | Locale::ChineseSimplified => {
                alloc::format!("{:04}-{:02}-{:02}", year, month, day)
            }
            Locale::German => {
                alloc::format!("{:02}.{:02}.{:04}", day, month, year)
            }
        }
    }

    /// Format time according to locale
    pub fn format_time(hour: u32, minute: u32, locale: Locale, use_24h: bool) -> alloc::string::String {
        if use_24h {
            alloc::format!("{:02}:{:02}", hour, minute)
        } else {
            let period = if hour < 12 { "AM" } else { "PM" };
            let hour_12 = if hour == 0 { 12 } else if hour > 12 { hour - 12 } else { hour };
            
            match locale {
                Locale::Korean => {
                    let period_ko = if hour < 12 { "오전" } else { "오후" };
                    alloc::format!("{} {}:{:02}", period_ko, hour_12, minute)
                }
                _ => alloc::format!("{}:{:02} {}", hour_12, minute, period),
            }
        }
    }
}

/// Number formatting
pub mod number {
    use super::Locale;

    /// Format a number with locale-appropriate separators
    pub fn format_number(value: u64, locale: Locale) -> alloc::string::String {
        let s = alloc::format!("{}", value);
        let chars: alloc::vec::Vec<char> = s.chars().collect();
        
        let separator = match locale {
            Locale::German => '.',
            _ => ',',
        };

        let mut result = alloc::string::String::new();
        for (i, c) in chars.iter().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.insert(0, separator);
            }
            result.insert(0, *c);
        }
        
        result
    }

    /// Format currency
    pub fn format_currency(value: i64, locale: Locale) -> alloc::string::String {
        let (symbol, position, decimal_sep) = match locale {
            Locale::English => ("$", true, '.'),
            Locale::Korean => ("₩", true, '.'),
            Locale::Japanese => ("¥", true, '.'),
            Locale::ChineseSimplified => ("¥", true, '.'),
            Locale::Spanish | Locale::German => ("€", false, ','),
        };

        let whole = value / 100;
        let cents = (value.abs() % 100) as u32;
        
        if position {
            // Symbol before
            alloc::format!("{}{}{}{:02}", symbol, whole, decimal_sep, cents)
        } else {
            // Symbol after
            alloc::format!("{}{}{:02} {}", whole, decimal_sep, cents, symbol)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_codes() {
        assert_eq!(Locale::English.code(), "en");
        assert_eq!(Locale::Korean.code(), "ko");
    }

    #[test]
    fn test_locale_parsing() {
        assert_eq!(Locale::from_code("en-US"), Some(Locale::English));
        assert_eq!(Locale::from_code("ko"), Some(Locale::Korean));
        assert_eq!(Locale::from_code("invalid"), None);
    }

    #[test]
    fn test_translation() {
        let store = TranslationStore::new();
        assert_eq!(store.translate("common.ok"), "OK");
    }

    #[test]
    fn test_date_format() {
        assert_eq!(date::format_date(2026, 1, 15, Locale::English), "01/15/2026");
        assert_eq!(date::format_date(2026, 1, 15, Locale::Korean), "2026-01-15");
        assert_eq!(date::format_date(2026, 1, 15, Locale::German), "15.01.2026");
    }

    #[test]
    fn test_number_format() {
        assert_eq!(number::format_number(1234567, Locale::English), "1,234,567");
        assert_eq!(number::format_number(1234567, Locale::German), "1.234.567");
    }
}
