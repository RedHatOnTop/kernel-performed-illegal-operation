//! Tab Suspension
//!
//! Automatic tab suspension for resource saving.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Tab suspension manager
#[derive(Debug, Clone)]
pub struct TabSuspensionManager {
    /// Settings
    pub settings: SuspensionSettings,
    /// Suspended tabs (tab_id -> suspended info)
    pub suspended: BTreeMap<u64, SuspendedTab>,
    /// Tab activity (tab_id -> last activity timestamp)
    pub activity: BTreeMap<u64, u64>,
    /// Whitelist patterns
    pub whitelist: Vec<String>,
    /// Never suspend tabs (by ID)
    pub pinned_awake: Vec<u64>,
}

impl TabSuspensionManager {
    /// Create new manager
    pub fn new() -> Self {
        Self {
            settings: SuspensionSettings::default(),
            suspended: BTreeMap::new(),
            activity: BTreeMap::new(),
            whitelist: Self::default_whitelist(),
            pinned_awake: Vec::new(),
        }
    }

    /// Default whitelist
    fn default_whitelist() -> Vec<String> {
        alloc::vec![
            // Don't suspend these
            String::from("*://*/video/*"),
            String::from("*://*/watch*"),
            String::from("*://meet.google.com/*"),
            String::from("*://zoom.us/*"),
            String::from("*://teams.microsoft.com/*"),
            String::from("*://discord.com/*"),
            String::from("*://music.*"),
            String::from("*://spotify.com/*"),
        ]
    }

    /// Update tab activity
    pub fn update_activity(&mut self, tab_id: u64, timestamp: u64) {
        self.activity.insert(tab_id, timestamp);
    }

    /// Check if tab should be suspended
    pub fn should_suspend(&self, tab_id: u64, url: &str, current_time: u64) -> bool {
        // Check if already suspended
        if self.suspended.contains_key(&tab_id) {
            return false;
        }

        // Check if suspension is enabled
        if !self.settings.enabled {
            return false;
        }

        // Check if tab is pinned awake
        if self.pinned_awake.contains(&tab_id) {
            return false;
        }

        // Check whitelist
        if self.is_whitelisted(url) {
            return false;
        }

        // Check inactivity time
        if let Some(&last_activity) = self.activity.get(&tab_id) {
            let inactive_secs = current_time.saturating_sub(last_activity);
            inactive_secs >= self.settings.timeout_seconds
        } else {
            // No activity recorded, don't suspend yet
            false
        }
    }

    /// Check if URL is whitelisted
    pub fn is_whitelisted(&self, url: &str) -> bool {
        for pattern in &self.whitelist {
            if Self::match_pattern(pattern, url) {
                return true;
            }
        }
        false
    }

    /// Add whitelist pattern
    pub fn add_whitelist(&mut self, pattern: &str) {
        if !self.whitelist.contains(&pattern.to_string()) {
            self.whitelist.push(pattern.to_string());
        }
    }

    /// Remove whitelist pattern
    pub fn remove_whitelist(&mut self, pattern: &str) {
        self.whitelist.retain(|p| p != pattern);
    }

    /// Suspend tab
    pub fn suspend(&mut self, tab_id: u64, url: &str, title: &str, favicon: Option<&str>) {
        self.suspended.insert(
            tab_id,
            SuspendedTab {
                original_url: url.to_string(),
                original_title: title.to_string(),
                favicon: favicon.map(|s| s.to_string()),
                suspended_at: 0, // Would be set to current time
                scroll_position: 0,
            },
        );
    }

    /// Unsuspend tab (returns original URL)
    pub fn unsuspend(&mut self, tab_id: u64) -> Option<SuspendedTab> {
        self.suspended.remove(&tab_id)
    }

    /// Is tab suspended
    pub fn is_suspended(&self, tab_id: u64) -> bool {
        self.suspended.contains_key(&tab_id)
    }

    /// Get suspended tab info
    pub fn get_suspended(&self, tab_id: u64) -> Option<&SuspendedTab> {
        self.suspended.get(&tab_id)
    }

    /// Pin tab awake (never suspend)
    pub fn pin_awake(&mut self, tab_id: u64) {
        if !self.pinned_awake.contains(&tab_id) {
            self.pinned_awake.push(tab_id);
        }
    }

    /// Unpin tab awake
    pub fn unpin_awake(&mut self, tab_id: u64) {
        self.pinned_awake.retain(|&id| id != tab_id);
    }

    /// Suspend all inactive tabs
    pub fn suspend_all_inactive(
        &mut self,
        tabs: &[(u64, &str, &str, Option<&str>)],
        current_time: u64,
    ) {
        for &(tab_id, url, title, favicon) in tabs {
            if self.should_suspend(tab_id, url, current_time) {
                self.suspend(tab_id, url, title, favicon);
            }
        }
    }

    /// Unsuspend all tabs
    pub fn unsuspend_all(&mut self) -> Vec<(u64, SuspendedTab)> {
        let keys: Vec<u64> = self.suspended.keys().cloned().collect();
        let mut result = Vec::new();
        for key in keys {
            if let Some(tab) = self.suspended.remove(&key) {
                result.push((key, tab));
            }
        }
        result
    }

    /// Get count of suspended tabs
    pub fn suspended_count(&self) -> usize {
        self.suspended.len()
    }

    /// Estimated memory saved (rough estimate)
    pub fn estimated_memory_saved_mb(&self) -> u64 {
        // Assume ~50MB per suspended tab
        (self.suspended.len() as u64) * 50
    }

    /// Tab removed
    pub fn tab_removed(&mut self, tab_id: u64) {
        self.suspended.remove(&tab_id);
        self.activity.remove(&tab_id);
        self.pinned_awake.retain(|&id| id != tab_id);
    }

    /// Simple pattern matching
    fn match_pattern(pattern: &str, url: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            let mut pos = 0;

            for (i, part) in parts.iter().enumerate() {
                if part.is_empty() {
                    continue;
                }

                if let Some(found) = url[pos..].find(part) {
                    if i == 0 && found != 0 {
                        return false;
                    }
                    pos += found + part.len();
                } else {
                    return false;
                }
            }
            true
        } else {
            url == pattern
        }
    }
}

impl Default for TabSuspensionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Suspended tab info
#[derive(Debug, Clone)]
pub struct SuspendedTab {
    /// Original URL
    pub original_url: String,
    /// Original title
    pub original_title: String,
    /// Favicon
    pub favicon: Option<String>,
    /// Suspended timestamp
    pub suspended_at: u64,
    /// Scroll position
    pub scroll_position: u32,
}

/// Suspension settings
#[derive(Debug, Clone)]
pub struct SuspensionSettings {
    /// Enable auto-suspension
    pub enabled: bool,
    /// Timeout in seconds before suspension
    pub timeout_seconds: u64,
    /// Suspend pinned tabs
    pub suspend_pinned: bool,
    /// Suspend tabs playing audio
    pub suspend_audio: bool,
    /// Suspend tabs with unsaved forms
    pub suspend_forms: bool,
    /// Suspend tabs with active downloads
    pub suspend_downloads: bool,
    /// Show suspended page instead of blank
    pub show_suspended_page: bool,
    /// Auto-unsuspend on focus
    pub auto_unsuspend: bool,
    /// Unsuspend on hover (delay ms)
    pub unsuspend_on_hover: Option<u32>,
    /// Keep favicon
    pub keep_favicon: bool,
    /// Memory threshold (MB) to trigger aggressive suspension
    pub memory_threshold_mb: Option<u64>,
}

impl Default for SuspensionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_seconds: 30 * 60, // 30 minutes
            suspend_pinned: false,
            suspend_audio: false,
            suspend_forms: false,
            suspend_downloads: false,
            show_suspended_page: true,
            auto_unsuspend: true,
            unsuspend_on_hover: Some(1000),
            keep_favicon: true,
            memory_threshold_mb: None,
        }
    }
}

/// Suspension strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SuspensionStrategy {
    /// Suspend oldest inactive tabs first
    #[default]
    OldestFirst,
    /// Suspend least recently used tabs first
    LeastRecentlyUsed,
    /// Suspend highest memory tabs first
    HighestMemoryFirst,
    /// Suspend background tabs first
    BackgroundFirst,
}

/// Tab memory info (for memory-based suspension)
#[derive(Debug, Clone)]
pub struct TabMemoryInfo {
    pub tab_id: u64,
    pub memory_mb: u64,
    pub cpu_percent: f32,
}

/// Generate suspended page HTML
pub fn suspended_page_html(tab: &SuspendedTab) -> String {
    alloc::format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>{} (Suspended)</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            height: 100vh;
            margin: 0;
            background: #1a1a2e;
            color: #eee;
        }}
        .container {{
            text-align: center;
            padding: 2rem;
        }}
        .favicon {{
            width: 48px;
            height: 48px;
            margin-bottom: 1rem;
        }}
        .title {{
            font-size: 1.5rem;
            margin-bottom: 0.5rem;
        }}
        .url {{
            color: #888;
            font-size: 0.9rem;
            margin-bottom: 2rem;
            word-break: break-all;
            max-width: 500px;
        }}
        .message {{
            color: #666;
            margin-bottom: 2rem;
        }}
        .resume {{
            background: #3b82f6;
            color: white;
            border: none;
            padding: 0.75rem 2rem;
            font-size: 1rem;
            border-radius: 8px;
            cursor: pointer;
            transition: background 0.2s;
        }}
        .resume:hover {{
            background: #2563eb;
        }}
        .info {{
            margin-top: 2rem;
            color: #666;
            font-size: 0.8rem;
        }}
    </style>
</head>
<body>
    <div class="container">
        {}
        <div class="title">{}</div>
        <div class="url">{}</div>
        <div class="message">This tab has been suspended to save memory</div>
        <button class="resume" onclick="location.href='{}'">Reload Tab</button>
        <div class="info">Click to return to the original page</div>
    </div>
</body>
</html>"#,
        tab.original_title,
        tab.favicon
            .as_ref()
            .map(|f| alloc::format!(r#"<img class="favicon" src="{}" alt="">"#, f))
            .unwrap_or_default(),
        tab.original_title,
        tab.original_url,
        tab.original_url
    )
}
