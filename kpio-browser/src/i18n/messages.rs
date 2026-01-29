//! Message Bundles
//!
//! Localized message storage and lookup.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Message bundle
#[derive(Debug, Clone, Default)]
pub struct MessageBundle {
    /// Locale
    locale: String,
    /// Messages
    messages: BTreeMap<String, String>,
    /// Plural rules
    plurals: BTreeMap<String, PluralMessage>,
}

impl MessageBundle {
    /// Create new bundle
    pub fn new(locale: &str) -> Self {
        Self {
            locale: locale.to_string(),
            messages: BTreeMap::new(),
            plurals: BTreeMap::new(),
        }
    }

    /// Add message
    pub fn add(&mut self, key: &str, value: &str) {
        self.messages.insert(key.to_string(), value.to_string());
    }

    /// Add plural message
    pub fn add_plural(&mut self, key: &str, plural: PluralMessage) {
        self.plurals.insert(key.to_string(), plural);
    }

    /// Get message
    pub fn get(&self, key: &str) -> Option<&str> {
        self.messages.get(key).map(|s| s.as_str())
    }

    /// Get plural message
    pub fn get_plural(&self, key: &str, count: i64) -> Option<&str> {
        self.plurals.get(key).map(|p| p.get(count))
    }

    /// Has message
    pub fn has(&self, key: &str) -> bool {
        self.messages.contains_key(key)
    }

    /// Get all keys
    pub fn keys(&self) -> Vec<&str> {
        self.messages.keys().map(|s| s.as_str()).collect()
    }

    /// Locale
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// Message count
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Merge with another bundle
    pub fn merge(&mut self, other: &MessageBundle) {
        for (key, value) in &other.messages {
            self.messages.insert(key.clone(), value.clone());
        }
        for (key, plural) in &other.plurals {
            self.plurals.insert(key.clone(), plural.clone());
        }
    }
}

/// Plural message
#[derive(Debug, Clone)]
pub struct PluralMessage {
    /// Zero form
    pub zero: Option<String>,
    /// One form (singular)
    pub one: String,
    /// Two form (dual)
    pub two: Option<String>,
    /// Few form
    pub few: Option<String>,
    /// Many form
    pub many: Option<String>,
    /// Other form (plural default)
    pub other: String,
}

impl PluralMessage {
    /// Create simple singular/plural
    pub fn simple(one: &str, other: &str) -> Self {
        Self {
            zero: None,
            one: one.to_string(),
            two: None,
            few: None,
            many: None,
            other: other.to_string(),
        }
    }

    /// Get appropriate form
    pub fn get(&self, count: i64) -> &str {
        match count.abs() {
            0 => self.zero.as_deref().unwrap_or(&self.other),
            1 => &self.one,
            2 => self.two.as_deref().unwrap_or(&self.other),
            3..=10 => self.few.as_deref().unwrap_or(&self.other),
            11..=99 => self.many.as_deref().unwrap_or(&self.other),
            _ => &self.other,
        }
    }
}

/// Message formatter
pub struct MessageFormatter {
    /// Escape HTML
    escape_html: bool,
}

impl MessageFormatter {
    /// Create new formatter
    pub fn new() -> Self {
        Self { escape_html: true }
    }

    /// Format message with arguments
    pub fn format(&self, template: &str, args: &BTreeMap<String, String>) -> String {
        let mut result = template.to_string();

        for (key, value) in args {
            let placeholder = alloc::format!("{{{}}}", key);
            let safe_value = if self.escape_html {
                self.escape(value)
            } else {
                value.clone()
            };
            result = result.replace(&placeholder, &safe_value);
        }

        result
    }

    /// Escape HTML entities
    fn escape(&self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        for c in text.chars() {
            match c {
                '<' => result.push_str("&lt;"),
                '>' => result.push_str("&gt;"),
                '&' => result.push_str("&amp;"),
                '"' => result.push_str("&quot;"),
                '\'' => result.push_str("&#39;"),
                _ => result.push(c),
            }
        }
        result
    }

    /// Set escape HTML
    pub fn set_escape_html(&mut self, escape: bool) {
        self.escape_html = escape;
    }
}

impl Default for MessageFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard browser messages
pub struct BrowserMessages;

impl BrowserMessages {
    // Navigation
    pub const BACK: &'static str = "navigation.back";
    pub const FORWARD: &'static str = "navigation.forward";
    pub const RELOAD: &'static str = "navigation.reload";
    pub const STOP: &'static str = "navigation.stop";
    pub const HOME: &'static str = "navigation.home";

    // Actions
    pub const COPY: &'static str = "action.copy";
    pub const PASTE: &'static str = "action.paste";
    pub const CUT: &'static str = "action.cut";
    pub const UNDO: &'static str = "action.undo";
    pub const REDO: &'static str = "action.redo";
    pub const SELECT_ALL: &'static str = "action.select_all";

    // Tabs
    pub const NEW_TAB: &'static str = "tabs.new";
    pub const CLOSE_TAB: &'static str = "tabs.close";
    pub const REOPEN_TAB: &'static str = "tabs.reopen";

    // Bookmarks
    pub const ADD_BOOKMARK: &'static str = "bookmarks.add";
    pub const REMOVE_BOOKMARK: &'static str = "bookmarks.remove";
    pub const EDIT_BOOKMARK: &'static str = "bookmarks.edit";

    // Errors
    pub const ERR_NOT_FOUND: &'static str = "error.not_found";
    pub const ERR_CONNECTION: &'static str = "error.connection";
    pub const ERR_CERTIFICATE: &'static str = "error.certificate";
    pub const ERR_TIMEOUT: &'static str = "error.timeout";

    // Dialogs
    pub const OK: &'static str = "dialog.ok";
    pub const CANCEL: &'static str = "dialog.cancel";
    pub const YES: &'static str = "dialog.yes";
    pub const NO: &'static str = "dialog.no";
    pub const SAVE: &'static str = "dialog.save";
    pub const CLOSE: &'static str = "dialog.close";
}

/// Create English message bundle
pub fn english_bundle() -> MessageBundle {
    let mut bundle = MessageBundle::new("en-US");

    // Navigation
    bundle.add(BrowserMessages::BACK, "Back");
    bundle.add(BrowserMessages::FORWARD, "Forward");
    bundle.add(BrowserMessages::RELOAD, "Reload");
    bundle.add(BrowserMessages::STOP, "Stop");
    bundle.add(BrowserMessages::HOME, "Home");

    // Actions
    bundle.add(BrowserMessages::COPY, "Copy");
    bundle.add(BrowserMessages::PASTE, "Paste");
    bundle.add(BrowserMessages::CUT, "Cut");
    bundle.add(BrowserMessages::UNDO, "Undo");
    bundle.add(BrowserMessages::REDO, "Redo");
    bundle.add(BrowserMessages::SELECT_ALL, "Select All");

    // Dialogs
    bundle.add(BrowserMessages::OK, "OK");
    bundle.add(BrowserMessages::CANCEL, "Cancel");
    bundle.add(BrowserMessages::YES, "Yes");
    bundle.add(BrowserMessages::NO, "No");
    bundle.add(BrowserMessages::SAVE, "Save");
    bundle.add(BrowserMessages::CLOSE, "Close");

    bundle
}

/// Create Korean message bundle
pub fn korean_bundle() -> MessageBundle {
    let mut bundle = MessageBundle::new("ko-KR");

    // Navigation
    bundle.add(BrowserMessages::BACK, "뒤로");
    bundle.add(BrowserMessages::FORWARD, "앞으로");
    bundle.add(BrowserMessages::RELOAD, "새로고침");
    bundle.add(BrowserMessages::STOP, "중지");
    bundle.add(BrowserMessages::HOME, "홈");

    // Actions
    bundle.add(BrowserMessages::COPY, "복사");
    bundle.add(BrowserMessages::PASTE, "붙여넣기");
    bundle.add(BrowserMessages::CUT, "잘라내기");
    bundle.add(BrowserMessages::UNDO, "실행 취소");
    bundle.add(BrowserMessages::REDO, "다시 실행");
    bundle.add(BrowserMessages::SELECT_ALL, "전체 선택");

    // Dialogs
    bundle.add(BrowserMessages::OK, "확인");
    bundle.add(BrowserMessages::CANCEL, "취소");
    bundle.add(BrowserMessages::YES, "예");
    bundle.add(BrowserMessages::NO, "아니오");
    bundle.add(BrowserMessages::SAVE, "저장");
    bundle.add(BrowserMessages::CLOSE, "닫기");

    bundle
}
