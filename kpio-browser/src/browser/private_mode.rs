//! Private Browsing Mode
//!
//! Incognito/private browsing with enhanced privacy.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Private browsing session
#[derive(Debug, Clone)]
pub struct PrivateSession {
    /// Session ID
    pub id: u64,
    /// Created timestamp
    pub created_at: u64,
    /// Temporary storage
    pub storage: PrivateStorage,
    /// Temporary cookies
    pub cookies: Vec<PrivateCookie>,
    /// Visited URLs (only in memory)
    pub history: Vec<PrivateHistoryEntry>,
    /// Form data
    pub form_data: BTreeMap<String, String>,
    /// Settings
    pub settings: PrivateSettings,
    /// Active window IDs
    pub windows: Vec<u64>,
    /// Downloads (paths only, for cleanup)
    pub downloads: Vec<String>,
}

impl PrivateSession {
    /// Create new private session
    pub fn new(id: u64, timestamp: u64) -> Self {
        Self {
            id,
            created_at: timestamp,
            storage: PrivateStorage::new(),
            cookies: Vec::new(),
            history: Vec::new(),
            form_data: BTreeMap::new(),
            settings: PrivateSettings::default(),
            windows: Vec::new(),
            downloads: Vec::new(),
        }
    }

    /// Add visited URL
    pub fn add_visit(&mut self, url: &str, title: &str, timestamp: u64) {
        self.history.push(PrivateHistoryEntry {
            url: url.to_string(),
            title: title.to_string(),
            visited_at: timestamp,
        });
    }

    /// Add cookie
    pub fn add_cookie(&mut self, cookie: PrivateCookie) {
        // Remove existing cookie with same name/domain
        self.cookies
            .retain(|c| !(c.name == cookie.name && c.domain == cookie.domain));
        self.cookies.push(cookie);
    }

    /// Get cookies for domain
    pub fn get_cookies(&self, domain: &str) -> Vec<&PrivateCookie> {
        self.cookies
            .iter()
            .filter(|c| domain == c.domain || domain.ends_with(&alloc::format!(".{}", c.domain)))
            .collect()
    }

    /// Remove cookie
    pub fn remove_cookie(&mut self, name: &str, domain: &str) {
        self.cookies
            .retain(|c| !(c.name == name && c.domain == domain));
    }

    /// Clear expired cookies
    pub fn clear_expired_cookies(&mut self, current_time: u64) {
        self.cookies
            .retain(|c| c.expires.map(|e| e > current_time).unwrap_or(true));
    }

    /// Add window
    pub fn add_window(&mut self, window_id: u64) {
        if !self.windows.contains(&window_id) {
            self.windows.push(window_id);
        }
    }

    /// Remove window
    pub fn remove_window(&mut self, window_id: u64) {
        self.windows.retain(|&id| id != window_id);
    }

    /// Is empty (no windows)
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.storage.clear();
        self.cookies.clear();
        self.history.clear();
        self.form_data.clear();
        self.downloads.clear();
    }

    /// Get session age in seconds
    pub fn age(&self, current_time: u64) -> u64 {
        current_time.saturating_sub(self.created_at)
    }

    /// Track download
    pub fn track_download(&mut self, path: &str) {
        self.downloads.push(path.to_string());
    }
}

/// Private browsing manager
#[derive(Debug, Clone)]
pub struct PrivateBrowsingManager {
    /// Active sessions
    pub sessions: BTreeMap<u64, PrivateSession>,
    /// Next session ID
    next_id: u64,
    /// Global settings
    pub settings: PrivateSettings,
}

impl PrivateBrowsingManager {
    /// Create new manager
    pub fn new() -> Self {
        Self {
            sessions: BTreeMap::new(),
            next_id: 1,
            settings: PrivateSettings::default(),
        }
    }

    /// Create new private session
    pub fn create_session(&mut self, timestamp: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.sessions.insert(id, PrivateSession::new(id, timestamp));
        id
    }

    /// Get session
    pub fn get_session(&self, id: u64) -> Option<&PrivateSession> {
        self.sessions.get(&id)
    }

    /// Get session mutable
    pub fn get_session_mut(&mut self, id: u64) -> Option<&mut PrivateSession> {
        self.sessions.get_mut(&id)
    }

    /// Close session
    pub fn close_session(&mut self, id: u64) -> Option<PrivateSession> {
        self.sessions.remove(&id)
    }

    /// Close all sessions
    pub fn close_all(&mut self) -> Vec<PrivateSession> {
        let keys: Vec<u64> = self.sessions.keys().cloned().collect();
        let mut result = Vec::new();
        for key in keys {
            if let Some(session) = self.sessions.remove(&key) {
                result.push(session);
            }
        }
        result
    }

    /// Check if any private session is active
    pub fn has_active_sessions(&self) -> bool {
        self.sessions.values().any(|s| !s.is_empty())
    }

    /// Get session for window
    pub fn session_for_window(&self, window_id: u64) -> Option<u64> {
        for (session_id, session) in &self.sessions {
            if session.windows.contains(&window_id) {
                return Some(*session_id);
            }
        }
        None
    }

    /// Is window private
    pub fn is_private_window(&self, window_id: u64) -> bool {
        self.session_for_window(window_id).is_some()
    }
}

impl Default for PrivateBrowsingManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Private storage (in-memory only)
#[derive(Debug, Clone, Default)]
pub struct PrivateStorage {
    /// Local storage per origin
    pub local: BTreeMap<String, BTreeMap<String, String>>,
    /// Session storage per origin
    pub session: BTreeMap<String, BTreeMap<String, String>>,
    /// IndexedDB per origin (simplified - just track names)
    pub indexed_db: BTreeMap<String, Vec<String>>,
}

impl PrivateStorage {
    /// Create new storage
    pub fn new() -> Self {
        Self::default()
    }

    /// Get local storage for origin
    pub fn get_local(&self, origin: &str) -> Option<&BTreeMap<String, String>> {
        self.local.get(origin)
    }

    /// Set local storage item
    pub fn set_local(&mut self, origin: &str, key: &str, value: &str) {
        self.local
            .entry(origin.to_string())
            .or_insert_with(BTreeMap::new)
            .insert(key.to_string(), value.to_string());
    }

    /// Remove local storage item
    pub fn remove_local(&mut self, origin: &str, key: &str) {
        if let Some(storage) = self.local.get_mut(origin) {
            storage.remove(key);
        }
    }

    /// Clear local storage for origin
    pub fn clear_local(&mut self, origin: &str) {
        self.local.remove(origin);
    }

    /// Get session storage for origin
    pub fn get_session(&self, origin: &str) -> Option<&BTreeMap<String, String>> {
        self.session.get(origin)
    }

    /// Set session storage item
    pub fn set_session(&mut self, origin: &str, key: &str, value: &str) {
        self.session
            .entry(origin.to_string())
            .or_insert_with(BTreeMap::new)
            .insert(key.to_string(), value.to_string());
    }

    /// Clear all storage
    pub fn clear(&mut self) {
        self.local.clear();
        self.session.clear();
        self.indexed_db.clear();
    }
}

/// Private cookie
#[derive(Debug, Clone)]
pub struct PrivateCookie {
    /// Cookie name
    pub name: String,
    /// Cookie value
    pub value: String,
    /// Domain
    pub domain: String,
    /// Path
    pub path: String,
    /// Secure flag
    pub secure: bool,
    /// HttpOnly flag
    pub http_only: bool,
    /// SameSite
    pub same_site: SameSite,
    /// Expiration (None = session cookie)
    pub expires: Option<u64>,
}

/// SameSite attribute
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SameSite {
    Strict,
    #[default]
    Lax,
    None,
}

/// Private history entry
#[derive(Debug, Clone)]
pub struct PrivateHistoryEntry {
    /// URL
    pub url: String,
    /// Page title
    pub title: String,
    /// Visit timestamp
    pub visited_at: u64,
}

/// Private browsing settings
#[derive(Debug, Clone)]
pub struct PrivateSettings {
    /// Block all third-party cookies
    pub block_third_party_cookies: bool,
    /// Enhanced tracking protection in private
    pub enhanced_tracking_protection: bool,
    /// Disable all extensions in private
    pub disable_extensions: bool,
    /// Clear downloads on close
    pub clear_downloads_on_close: bool,
    /// Ask before closing multiple tabs
    pub confirm_close: bool,
    /// Use separate search engine
    pub separate_search_engine: Option<String>,
    /// Disable autofill
    pub disable_autofill: bool,
    /// Disable password saving prompts
    pub disable_password_save: bool,
    /// Force HTTPS
    pub force_https: bool,
    /// Block WebRTC IP leak
    pub block_webrtc_leak: bool,
}

impl Default for PrivateSettings {
    fn default() -> Self {
        Self {
            block_third_party_cookies: true,
            enhanced_tracking_protection: true,
            disable_extensions: true,
            clear_downloads_on_close: false,
            confirm_close: true,
            separate_search_engine: None,
            disable_autofill: true,
            disable_password_save: true,
            force_https: true,
            block_webrtc_leak: true,
        }
    }
}

/// Private window info
#[derive(Debug, Clone)]
pub struct PrivateWindowInfo {
    /// Window ID
    pub window_id: u64,
    /// Session ID
    pub session_id: u64,
    /// Tab count
    pub tab_count: usize,
    /// Created at
    pub created_at: u64,
}

/// On-boarding info for private mode
#[derive(Debug, Clone)]
pub struct PrivateModeInfo {
    /// What is hidden
    pub hidden: Vec<String>,
    /// What is NOT hidden
    pub not_hidden: Vec<String>,
    /// Tips
    pub tips: Vec<String>,
}

impl Default for PrivateModeInfo {
    fn default() -> Self {
        Self {
            hidden: alloc::vec![
                String::from("Browsing history"),
                String::from("Search history"),
                String::from("Cookies and site data"),
                String::from("Information entered in forms"),
                String::from("Temporary files"),
            ],
            not_hidden: alloc::vec![
                String::from("Downloaded files"),
                String::from("Bookmarks"),
                String::from("Internet service provider"),
                String::from("Employer/school network"),
                String::from("Websites you visited"),
            ],
            tips: alloc::vec![
                String::from(
                    "All data from this session will be deleted when you exit incognito mode"
                ),
                String::from("Downloaded files will remain on your computer"),
                String::from("Extensions are disabled in incognito mode"),
            ],
        }
    }
}

/// Generate private mode welcome page HTML
pub fn private_mode_welcome_html() -> String {
    let info = PrivateModeInfo::default();

    alloc::format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Incognito Mode</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
            background: #1a1a2e;
            color: #eee;
            margin: 0;
            padding: 2rem;
            min-height: 100vh;
        }}
        .container {{
            max-width: 600px;
            margin: 0 auto;
        }}
        .header {{
            text-align: center;
            margin-bottom: 2rem;
        }}
        .icon {{
            font-size: 4rem;
            margin-bottom: 1rem;
        }}
        h1 {{
            margin: 0 0 0.5rem 0;
            font-size: 1.75rem;
        }}
        .subtitle {{
            color: #888;
        }}
        .section {{
            background: rgba(255,255,255,0.05);
            border-radius: 12px;
            padding: 1.5rem;
            margin-bottom: 1rem;
        }}
        .section h2 {{
            font-size: 1rem;
            margin: 0 0 1rem 0;
            color: #3b82f6;
        }}
        .section ul {{
            margin: 0;
            padding-left: 1.5rem;
        }}
        .section li {{
            margin-bottom: 0.5rem;
            color: #ccc;
        }}
        .warning {{
            border-left: 3px solid #f59e0b;
        }}
        .warning h2 {{
            color: #f59e0b;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <div class="icon">🕵️</div>
            <h1>Incognito Mode</h1>
            <p class="subtitle">You can now browse privately</p>
        </div>
        
        <div class="section">
            <h2>✓ Items not saved</h2>
            <ul>
                {}
            </ul>
        </div>
        
        <div class="section warning">
            <h2>⚠ Who can still see your activity</h2>
            <ul>
                {}
            </ul>
        </div>
        
        <div class="section">
            <h2>💡 Note</h2>
            <ul>
                {}
            </ul>
        </div>
    </div>
</body>
</html>"#,
        info.hidden
            .iter()
            .map(|s| alloc::format!("<li>{}</li>", s))
            .collect::<Vec<_>>()
            .join("\n                "),
        info.not_hidden
            .iter()
            .map(|s| alloc::format!("<li>{}</li>", s))
            .collect::<Vec<_>>()
            .join("\n                "),
        info.tips
            .iter()
            .map(|s| alloc::format!("<li>{}</li>", s))
            .collect::<Vec<_>>()
            .join("\n                ")
    )
}
