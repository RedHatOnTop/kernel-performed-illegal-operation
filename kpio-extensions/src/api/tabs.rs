//! chrome.tabs API
//!
//! Provides tab management for extensions.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use spin::RwLock;

use super::{ApiResult, ApiError, ApiContext, EventEmitter};

/// Tab ID.
pub type TabId = u32;

/// Window ID.
pub type WindowId = u32;

/// Tab information.
#[derive(Debug, Clone)]
pub struct Tab {
    /// Tab ID.
    pub id: TabId,
    /// Tab index in window.
    pub index: i32,
    /// Window ID.
    pub window_id: WindowId,
    /// Opener tab ID.
    pub opener_tab_id: Option<TabId>,
    /// Whether the tab is highlighted.
    pub highlighted: bool,
    /// Whether the tab is active.
    pub active: bool,
    /// Whether the tab is pinned.
    pub pinned: bool,
    /// Whether the tab has been audible recently.
    pub audible: Option<bool>,
    /// Whether the tab is discarded.
    pub discarded: bool,
    /// Whether the tab can be discarded.
    pub auto_discardable: bool,
    /// Muted info.
    pub muted_info: Option<MutedInfo>,
    /// URL.
    pub url: Option<String>,
    /// Pending URL.
    pub pending_url: Option<String>,
    /// Title.
    pub title: Option<String>,
    /// Favicon URL.
    pub fav_icon_url: Option<String>,
    /// Status.
    pub status: Option<TabStatus>,
    /// Incognito.
    pub incognito: bool,
    /// Width.
    pub width: Option<i32>,
    /// Height.
    pub height: Option<i32>,
    /// Session ID.
    pub session_id: Option<String>,
    /// Group ID.
    pub group_id: i32,
}

impl Tab {
    /// Create a new tab.
    pub fn new(id: TabId, window_id: WindowId, index: i32) -> Self {
        Self {
            id,
            index,
            window_id,
            opener_tab_id: None,
            highlighted: false,
            active: false,
            pinned: false,
            audible: None,
            discarded: false,
            auto_discardable: true,
            muted_info: None,
            url: None,
            pending_url: None,
            title: None,
            fav_icon_url: None,
            status: Some(TabStatus::Loading),
            incognito: false,
            width: None,
            height: None,
            session_id: None,
            group_id: -1, // TAB_GROUP_ID_NONE
        }
    }
}

/// Tab status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabStatus {
    Loading,
    Complete,
    Unloaded,
}

/// Muted info.
#[derive(Debug, Clone)]
pub struct MutedInfo {
    /// Whether the tab is muted.
    pub muted: bool,
    /// Reason for muting.
    pub reason: Option<MutedReason>,
    /// Extension ID that muted the tab.
    pub extension_id: Option<String>,
}

/// Muted reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutedReason {
    User,
    Capture,
    Extension,
}

/// Tab query options.
#[derive(Debug, Clone, Default)]
pub struct QueryInfo {
    /// Active tabs only.
    pub active: Option<bool>,
    /// Pinned tabs only.
    pub pinned: Option<bool>,
    /// Audible tabs only.
    pub audible: Option<bool>,
    /// Muted tabs only.
    pub muted: Option<bool>,
    /// Highlighted tabs only.
    pub highlighted: Option<bool>,
    /// Discarded tabs only.
    pub discarded: Option<bool>,
    /// Auto-discardable tabs only.
    pub auto_discardable: Option<bool>,
    /// Current window only.
    pub current_window: Option<bool>,
    /// Last focused window only.
    pub last_focused_window: Option<bool>,
    /// Status filter.
    pub status: Option<TabStatus>,
    /// Title pattern.
    pub title: Option<String>,
    /// URL pattern.
    pub url: Option<Vec<String>>,
    /// Window ID.
    pub window_id: Option<WindowId>,
    /// Window type.
    pub window_type: Option<WindowType>,
    /// Index.
    pub index: Option<i32>,
    /// Group ID.
    pub group_id: Option<i32>,
}

/// Window type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowType {
    Normal,
    Popup,
    Panel,
    App,
    Devtools,
}

/// Tab create properties.
#[derive(Debug, Clone, Default)]
pub struct CreateProperties {
    /// Window ID.
    pub window_id: Option<WindowId>,
    /// Index.
    pub index: Option<i32>,
    /// URL.
    pub url: Option<String>,
    /// Active.
    pub active: Option<bool>,
    /// Selected (deprecated, use active).
    pub selected: Option<bool>,
    /// Pinned.
    pub pinned: Option<bool>,
    /// Opener tab ID.
    pub opener_tab_id: Option<TabId>,
}

/// Tab update properties.
#[derive(Debug, Clone, Default)]
pub struct UpdateProperties {
    /// URL.
    pub url: Option<String>,
    /// Active.
    pub active: Option<bool>,
    /// Highlighted.
    pub highlighted: Option<bool>,
    /// Selected (deprecated).
    pub selected: Option<bool>,
    /// Pinned.
    pub pinned: Option<bool>,
    /// Muted.
    pub muted: Option<bool>,
    /// Opener tab ID.
    pub opener_tab_id: Option<TabId>,
    /// Auto discardable.
    pub auto_discardable: Option<bool>,
}

/// Move properties.
#[derive(Debug, Clone)]
pub struct MoveProperties {
    /// Window ID.
    pub window_id: Option<WindowId>,
    /// Index.
    pub index: i32,
}

/// Tab change info.
#[derive(Debug, Clone)]
pub struct TabChangeInfo {
    /// Status change.
    pub status: Option<TabStatus>,
    /// URL change.
    pub url: Option<String>,
    /// Group ID change.
    pub group_id: Option<i32>,
    /// Pinned change.
    pub pinned: Option<bool>,
    /// Audible change.
    pub audible: Option<bool>,
    /// Discarded change.
    pub discarded: Option<bool>,
    /// Auto discardable change.
    pub auto_discardable: Option<bool>,
    /// Muted info change.
    pub muted_info: Option<MutedInfo>,
    /// Favicon URL change.
    pub fav_icon_url: Option<String>,
    /// Title change.
    pub title: Option<String>,
}

/// Tab active info.
#[derive(Debug, Clone)]
pub struct TabActiveInfo {
    /// Previous tab ID.
    pub previous_tab_id: Option<TabId>,
    /// Tab ID.
    pub tab_id: TabId,
    /// Window ID.
    pub window_id: WindowId,
}

/// Tab remove info.
#[derive(Debug, Clone)]
pub struct TabRemoveInfo {
    /// Window ID.
    pub window_id: WindowId,
    /// Whether window is closing.
    pub is_window_closing: bool,
}

/// Tab move info.
#[derive(Debug, Clone)]
pub struct TabMoveInfo {
    /// Window ID.
    pub window_id: WindowId,
    /// From index.
    pub from_index: i32,
    /// To index.
    pub to_index: i32,
}

/// Tab zoom change info.
#[derive(Debug, Clone)]
pub struct ZoomChangeInfo {
    /// Tab ID.
    pub tab_id: TabId,
    /// Old zoom factor.
    pub old_zoom_factor: f64,
    /// New zoom factor.
    pub new_zoom_factor: f64,
    /// Zoom settings.
    pub zoom_settings: ZoomSettings,
}

/// Zoom settings.
#[derive(Debug, Clone)]
pub struct ZoomSettings {
    /// Mode.
    pub mode: Option<ZoomSettingsMode>,
    /// Scope.
    pub scope: Option<ZoomSettingsScope>,
    /// Default zoom factor.
    pub default_zoom_factor: Option<f64>,
}

/// Zoom settings mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomSettingsMode {
    Automatic,
    Manual,
    Disabled,
}

/// Zoom settings scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomSettingsScope {
    PerOrigin,
    PerTab,
}

/// Tabs API.
pub struct TabsApi {
    /// Tab storage.
    tabs: RwLock<BTreeMap<TabId, Tab>>,
    /// Next tab ID.
    next_tab_id: RwLock<TabId>,
    /// Current window ID.
    current_window_id: RwLock<WindowId>,
    /// On created event.
    pub on_created: RwLock<EventEmitter<Tab>>,
    /// On updated event.
    pub on_updated: RwLock<EventEmitter<(TabId, TabChangeInfo, Tab)>>,
    /// On removed event.
    pub on_removed: RwLock<EventEmitter<(TabId, TabRemoveInfo)>>,
    /// On activated event.
    pub on_activated: RwLock<EventEmitter<TabActiveInfo>>,
    /// On moved event.
    pub on_moved: RwLock<EventEmitter<(TabId, TabMoveInfo)>>,
}

impl TabsApi {
    /// Create a new Tabs API.
    pub fn new() -> Self {
        Self {
            tabs: RwLock::new(BTreeMap::new()),
            next_tab_id: RwLock::new(1),
            current_window_id: RwLock::new(1),
            on_created: RwLock::new(EventEmitter::new()),
            on_updated: RwLock::new(EventEmitter::new()),
            on_removed: RwLock::new(EventEmitter::new()),
            on_activated: RwLock::new(EventEmitter::new()),
            on_moved: RwLock::new(EventEmitter::new()),
        }
    }
    
    /// Query tabs.
    pub fn query(&self, _ctx: &ApiContext, query: QueryInfo) -> ApiResult<Vec<Tab>> {
        let tabs = self.tabs.read();
        let mut results = Vec::new();
        
        for tab in tabs.values() {
            // Apply filters
            if let Some(active) = query.active {
                if tab.active != active {
                    continue;
                }
            }
            if let Some(pinned) = query.pinned {
                if tab.pinned != pinned {
                    continue;
                }
            }
            if let Some(window_id) = query.window_id {
                if tab.window_id != window_id {
                    continue;
                }
            }
            if let Some(ref url_patterns) = query.url {
                let matches = tab.url.as_ref().map(|url| {
                    url_patterns.iter().any(|pattern| matches_url_pattern(url, pattern))
                }).unwrap_or(false);
                if !matches {
                    continue;
                }
            }
            
            results.push(tab.clone());
        }
        
        Ok(results)
    }
    
    /// Get a tab by ID.
    pub fn get(&self, _ctx: &ApiContext, tab_id: TabId) -> ApiResult<Tab> {
        self.tabs.read()
            .get(&tab_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Tab"))
    }
    
    /// Get current tab.
    pub fn get_current(&self, ctx: &ApiContext) -> ApiResult<Option<Tab>> {
        if let Some(tab_id) = ctx.tab_id {
            Ok(self.tabs.read().get(&tab_id).cloned())
        } else {
            Ok(None)
        }
    }
    
    /// Create a new tab.
    pub fn create(&self, _ctx: &ApiContext, props: CreateProperties) -> ApiResult<Tab> {
        let mut next_id = self.next_tab_id.write();
        let tab_id = *next_id;
        *next_id += 1;
        
        let window_id = props.window_id.unwrap_or(*self.current_window_id.read());
        let index = props.index.unwrap_or(0);
        
        let mut tab = Tab::new(tab_id, window_id, index);
        tab.url = props.url;
        tab.active = props.active.unwrap_or(true);
        tab.pinned = props.pinned.unwrap_or(false);
        tab.opener_tab_id = props.opener_tab_id;
        
        self.tabs.write().insert(tab_id, tab.clone());
        self.on_created.read().emit(&tab);
        
        Ok(tab)
    }
    
    /// Update a tab.
    pub fn update(&self, _ctx: &ApiContext, tab_id: TabId, props: UpdateProperties) -> ApiResult<Tab> {
        let mut tabs = self.tabs.write();
        let tab = tabs.get_mut(&tab_id)
            .ok_or_else(|| ApiError::not_found("Tab"))?;
        
        let mut change_info = TabChangeInfo {
            status: None,
            url: None,
            group_id: None,
            pinned: None,
            audible: None,
            discarded: None,
            auto_discardable: None,
            muted_info: None,
            fav_icon_url: None,
            title: None,
        };
        
        if let Some(url) = props.url {
            tab.url = Some(url.clone());
            change_info.url = Some(url);
        }
        if let Some(active) = props.active {
            tab.active = active;
        }
        if let Some(pinned) = props.pinned {
            if tab.pinned != pinned {
                tab.pinned = pinned;
                change_info.pinned = Some(pinned);
            }
        }
        if let Some(auto_discardable) = props.auto_discardable {
            if tab.auto_discardable != auto_discardable {
                tab.auto_discardable = auto_discardable;
                change_info.auto_discardable = Some(auto_discardable);
            }
        }
        
        let tab_clone = tab.clone();
        drop(tabs);
        
        self.on_updated.read().emit(&(tab_id, change_info, tab_clone.clone()));
        
        Ok(tab_clone)
    }
    
    /// Remove tabs.
    pub fn remove(&self, _ctx: &ApiContext, tab_ids: Vec<TabId>) -> ApiResult<()> {
        let mut tabs = self.tabs.write();
        
        for tab_id in tab_ids {
            if let Some(tab) = tabs.remove(&tab_id) {
                let remove_info = TabRemoveInfo {
                    window_id: tab.window_id,
                    is_window_closing: false,
                };
                self.on_removed.read().emit(&(tab_id, remove_info));
            }
        }
        
        Ok(())
    }
    
    /// Reload a tab.
    pub fn reload(&self, _ctx: &ApiContext, _tab_id: Option<TabId>, _bypass_cache: bool) -> ApiResult<()> {
        // Would trigger page reload
        Ok(())
    }
    
    /// Go back in history.
    pub fn go_back(&self, _ctx: &ApiContext, _tab_id: Option<TabId>) -> ApiResult<()> {
        // Would navigate back
        Ok(())
    }
    
    /// Go forward in history.
    pub fn go_forward(&self, _ctx: &ApiContext, _tab_id: Option<TabId>) -> ApiResult<()> {
        // Would navigate forward
        Ok(())
    }
}

impl Default for TabsApi {
    fn default() -> Self {
        Self::new()
    }
}

/// Match URL against pattern.
fn matches_url_pattern(url: &str, pattern: &str) -> bool {
    if pattern == "<all_urls>" {
        return true;
    }
    
    // Simple glob matching
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.is_empty() {
        return url == pattern;
    }
    
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExtensionId;
    
    #[test]
    fn test_tabs_api() {
        let api = TabsApi::new();
        let ctx = ApiContext::new(ExtensionId::new("test"));
        
        // Create a tab
        let props = CreateProperties {
            url: Some("https://example.com".to_string()),
            ..Default::default()
        };
        let tab = api.create(&ctx, props).unwrap();
        assert_eq!(tab.url, Some("https://example.com".to_string()));
        
        // Query tabs
        let tabs = api.query(&ctx, QueryInfo::default()).unwrap();
        assert_eq!(tabs.len(), 1);
        
        // Get tab
        let tab = api.get(&ctx, tab.id).unwrap();
        assert_eq!(tab.url, Some("https://example.com".to_string()));
    }
    
    #[test]
    fn test_url_pattern_matching() {
        assert!(matches_url_pattern("https://example.com/path", "<all_urls>"));
        assert!(matches_url_pattern("https://example.com/path", "https://example.com/*"));
        assert!(matches_url_pattern("https://example.com/path", "*://example.com/*"));
        assert!(!matches_url_pattern("https://other.com/path", "https://example.com/*"));
    }
}
