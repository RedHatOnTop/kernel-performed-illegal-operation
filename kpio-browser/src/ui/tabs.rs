//! Tab Management UI
//!
//! Tab strip with drag-and-drop, tab grouping, and session restore.

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use spin::RwLock;

use super::{Color, KeyCode, KeyModifiers, MouseButton, Rect, UiEvent};

/// Tab ID.
pub type TabId = u32;

/// Tab Group ID.
pub type TabGroupId = u32;

/// Tab strip position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabStripPosition {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

/// Tab state.
#[derive(Debug, Clone)]
pub struct Tab {
    /// Tab ID.
    pub id: TabId,
    /// Title.
    pub title: String,
    /// URL.
    pub url: String,
    /// Favicon URL or data URI.
    pub favicon: Option<String>,
    /// Whether loading.
    pub loading: bool,
    /// Load progress (0-100).
    pub progress: u8,
    /// Whether audible (playing audio).
    pub audible: bool,
    /// Whether muted.
    pub muted: bool,
    /// Whether pinned.
    pub pinned: bool,
    /// Tab group.
    pub group_id: Option<TabGroupId>,
    /// Last access time.
    pub last_accessed: u64,
}

impl Tab {
    /// Create a new tab.
    pub fn new(id: TabId, url: &str) -> Self {
        Self {
            id,
            title: url.to_string(),
            url: url.to_string(),
            favicon: None,
            loading: true,
            progress: 0,
            audible: false,
            muted: false,
            pinned: false,
            group_id: None,
            last_accessed: 0,
        }
    }
}

/// Tab group.
#[derive(Debug, Clone)]
pub struct TabGroup {
    /// Group ID.
    pub id: TabGroupId,
    /// Group name.
    pub name: String,
    /// Group color.
    pub color: TabGroupColor,
    /// Whether collapsed.
    pub collapsed: bool,
}

/// Tab group color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabGroupColor {
    #[default]
    Grey,
    Blue,
    Red,
    Yellow,
    Green,
    Pink,
    Purple,
    Cyan,
    Orange,
}

impl TabGroupColor {
    /// Get RGB color.
    pub fn to_color(&self) -> Color {
        match self {
            Self::Grey => Color::rgb(100, 100, 100),
            Self::Blue => Color::rgb(66, 133, 244),
            Self::Red => Color::rgb(234, 67, 53),
            Self::Yellow => Color::rgb(251, 188, 5),
            Self::Green => Color::rgb(52, 168, 83),
            Self::Pink => Color::rgb(233, 30, 99),
            Self::Purple => Color::rgb(156, 39, 176),
            Self::Cyan => Color::rgb(0, 188, 212),
            Self::Orange => Color::rgb(255, 152, 0),
        }
    }
}

/// Drag state.
#[derive(Debug, Clone)]
struct DragState {
    /// Tab being dragged.
    tab_id: TabId,
    /// Start position.
    start_x: i32,
    start_y: i32,
    /// Current position.
    current_x: i32,
    current_y: i32,
    /// Original index.
    original_index: usize,
}

/// Tab strip widget.
pub struct TabStrip {
    /// Tabs.
    tabs: RwLock<Vec<Tab>>,
    /// Tab groups.
    groups: RwLock<BTreeMap<TabGroupId, TabGroup>>,
    /// Active tab ID.
    active_tab: RwLock<Option<TabId>>,
    /// Next tab ID.
    next_tab_id: RwLock<TabId>,
    /// Next group ID.
    next_group_id: RwLock<TabGroupId>,
    /// Strip position.
    position: RwLock<TabStripPosition>,
    /// Bounds.
    bounds: RwLock<Rect>,
    /// Tab width.
    tab_width: RwLock<u32>,
    /// Min tab width.
    min_tab_width: u32,
    /// Max tab width.
    max_tab_width: u32,
    /// Pinned tab width.
    pinned_tab_width: u32,
    /// Drag state.
    drag_state: RwLock<Option<DragState>>,
    /// Scroll offset.
    scroll_offset: RwLock<i32>,
    /// Show close buttons on hover.
    show_close_on_hover: RwLock<bool>,
}

impl TabStrip {
    /// Create a new tab strip.
    pub fn new() -> Self {
        Self {
            tabs: RwLock::new(Vec::new()),
            groups: RwLock::new(BTreeMap::new()),
            active_tab: RwLock::new(None),
            next_tab_id: RwLock::new(1),
            next_group_id: RwLock::new(1),
            position: RwLock::new(TabStripPosition::Top),
            bounds: RwLock::new(Rect::new(0, 0, 0, 40)),
            tab_width: RwLock::new(200),
            min_tab_width: 100,
            max_tab_width: 240,
            pinned_tab_width: 40,
            drag_state: RwLock::new(None),
            scroll_offset: RwLock::new(0),
            show_close_on_hover: RwLock::new(true),
        }
    }

    /// Create a new tab.
    pub fn new_tab(&self, url: &str) -> TabId {
        let mut next_id = self.next_tab_id.write();
        let id = *next_id;
        *next_id += 1;

        let tab = Tab::new(id, url);
        self.tabs.write().push(tab);
        *self.active_tab.write() = Some(id);

        self.recalculate_tab_widths();
        id
    }

    /// Close a tab.
    pub fn close_tab(&self, tab_id: TabId) -> bool {
        let mut tabs = self.tabs.write();
        if let Some(pos) = tabs.iter().position(|t| t.id == tab_id) {
            tabs.remove(pos);

            // Select another tab if this was active
            let mut active = self.active_tab.write();
            if *active == Some(tab_id) {
                // Select next tab, or previous if at end
                *active = if pos < tabs.len() {
                    Some(tabs[pos].id)
                } else if !tabs.is_empty() {
                    Some(tabs[tabs.len() - 1].id)
                } else {
                    None
                };
            }

            drop(tabs);
            self.recalculate_tab_widths();
            true
        } else {
            false
        }
    }

    /// Activate a tab.
    pub fn activate_tab(&self, tab_id: TabId) {
        if self.tabs.read().iter().any(|t| t.id == tab_id) {
            *self.active_tab.write() = Some(tab_id);
        }
    }

    /// Get active tab.
    pub fn active_tab(&self) -> Option<TabId> {
        *self.active_tab.read()
    }

    /// Get tab by ID.
    pub fn get_tab(&self, tab_id: TabId) -> Option<Tab> {
        self.tabs.read().iter().find(|t| t.id == tab_id).cloned()
    }

    /// Update tab title.
    pub fn set_tab_title(&self, tab_id: TabId, title: &str) {
        if let Some(tab) = self.tabs.write().iter_mut().find(|t| t.id == tab_id) {
            tab.title = title.to_string();
        }
    }

    /// Update tab URL.
    pub fn set_tab_url(&self, tab_id: TabId, url: &str) {
        if let Some(tab) = self.tabs.write().iter_mut().find(|t| t.id == tab_id) {
            tab.url = url.to_string();
        }
    }

    /// Update tab favicon.
    pub fn set_tab_favicon(&self, tab_id: TabId, favicon: Option<String>) {
        if let Some(tab) = self.tabs.write().iter_mut().find(|t| t.id == tab_id) {
            tab.favicon = favicon;
        }
    }

    /// Set tab loading state.
    pub fn set_tab_loading(&self, tab_id: TabId, loading: bool, progress: u8) {
        if let Some(tab) = self.tabs.write().iter_mut().find(|t| t.id == tab_id) {
            tab.loading = loading;
            tab.progress = progress;
        }
    }

    /// Pin/unpin tab.
    pub fn set_tab_pinned(&self, tab_id: TabId, pinned: bool) {
        let mut tabs = self.tabs.write();
        if let Some(pos) = tabs.iter().position(|t| t.id == tab_id) {
            tabs[pos].pinned = pinned;

            // Move pinned tabs to the left
            if pinned {
                let tab = tabs.remove(pos);
                let insert_pos = tabs.iter().take_while(|t| t.pinned).count();
                tabs.insert(insert_pos, tab);
            } else {
                let tab = tabs.remove(pos);
                let insert_pos = tabs.iter().take_while(|t| t.pinned).count();
                tabs.insert(insert_pos, tab);
            }
        }
        drop(tabs);
        self.recalculate_tab_widths();
    }

    /// Mute/unmute tab.
    pub fn set_tab_muted(&self, tab_id: TabId, muted: bool) {
        if let Some(tab) = self.tabs.write().iter_mut().find(|t| t.id == tab_id) {
            tab.muted = muted;
        }
    }

    /// Create a tab group.
    pub fn create_group(&self, name: &str, color: TabGroupColor) -> TabGroupId {
        let mut next_id = self.next_group_id.write();
        let id = *next_id;
        *next_id += 1;

        let group = TabGroup {
            id,
            name: name.to_string(),
            color,
            collapsed: false,
        };

        self.groups.write().insert(id, group);
        id
    }

    /// Add tab to group.
    pub fn add_tab_to_group(&self, tab_id: TabId, group_id: TabGroupId) {
        if let Some(tab) = self.tabs.write().iter_mut().find(|t| t.id == tab_id) {
            tab.group_id = Some(group_id);
        }
    }

    /// Remove tab from group.
    pub fn remove_tab_from_group(&self, tab_id: TabId) {
        if let Some(tab) = self.tabs.write().iter_mut().find(|t| t.id == tab_id) {
            tab.group_id = None;
        }
    }

    /// Collapse/expand group.
    pub fn set_group_collapsed(&self, group_id: TabGroupId, collapsed: bool) {
        if let Some(group) = self.groups.write().get_mut(&group_id) {
            group.collapsed = collapsed;
        }
    }

    /// Delete group (ungroup tabs).
    pub fn delete_group(&self, group_id: TabGroupId) {
        // Remove group from all tabs
        for tab in self.tabs.write().iter_mut() {
            if tab.group_id == Some(group_id) {
                tab.group_id = None;
            }
        }
        self.groups.write().remove(&group_id);
    }

    /// Move tab.
    pub fn move_tab(&self, tab_id: TabId, new_index: usize) {
        let mut tabs = self.tabs.write();
        if let Some(pos) = tabs.iter().position(|t| t.id == tab_id) {
            let tabs_len = tabs.len();
            if pos != new_index && new_index < tabs_len {
                let tab = tabs.remove(pos);
                let insert_pos = if new_index > pos {
                    new_index - 1
                } else {
                    new_index
                };
                let max_pos = tabs.len();
                tabs.insert(insert_pos.min(max_pos), tab);
            }
        }
    }

    /// Get all tabs.
    pub fn tabs(&self) -> Vec<Tab> {
        self.tabs.read().clone()
    }

    /// Get tab count.
    pub fn tab_count(&self) -> usize {
        self.tabs.read().len()
    }

    /// Handle UI event.
    pub fn handle_event(&self, event: &UiEvent) -> bool {
        match event {
            UiEvent::Click {
                x,
                y,
                button: MouseButton::Left,
            } => self.handle_click(*x, *y),
            UiEvent::Click {
                x,
                y,
                button: MouseButton::Middle,
            } => {
                // Middle click to close tab
                if let Some(tab_id) = self.tab_at_position(*x, *y) {
                    self.close_tab(tab_id);
                    true
                } else {
                    false
                }
            }
            UiEvent::MouseMove { x, y } => self.handle_drag(*x, *y),
            UiEvent::KeyPress { key, modifiers } => self.handle_key(*key, *modifiers),
            _ => false,
        }
    }

    /// Handle click.
    fn handle_click(&self, x: i32, y: i32) -> bool {
        let bounds = *self.bounds.read();
        if !bounds.contains(x, y) {
            return false;
        }

        // Check if clicking on a tab
        if let Some(tab_id) = self.tab_at_position(x, y) {
            // Check if clicking on close button
            if self.is_close_button(x, y, tab_id) {
                self.close_tab(tab_id);
            } else {
                self.activate_tab(tab_id);
            }
            return true;
        }

        // Check if clicking on new tab button
        if self.is_new_tab_button(x, y) {
            self.new_tab("about:newtab");
            return true;
        }

        false
    }

    /// Handle drag.
    fn handle_drag(&self, x: i32, y: i32) -> bool {
        let mut drag = self.drag_state.write();
        if let Some(ref mut state) = *drag {
            state.current_x = x;
            state.current_y = y;

            // Calculate new position
            let tab_width = *self.tab_width.read() as i32;
            let new_index = ((x - self.bounds.read().x) / tab_width) as usize;

            if new_index != state.original_index {
                self.move_tab(state.tab_id, new_index);
            }

            true
        } else {
            false
        }
    }

    /// Handle keyboard shortcut.
    fn handle_key(&self, key: KeyCode, modifiers: KeyModifiers) -> bool {
        if modifiers.ctrl {
            match key {
                KeyCode::T => {
                    // Ctrl+T: New tab
                    self.new_tab("about:newtab");
                    true
                }
                KeyCode::W => {
                    // Ctrl+W: Close current tab
                    if let Some(tab_id) = self.active_tab() {
                        self.close_tab(tab_id);
                    }
                    true
                }
                KeyCode::Tab => {
                    // Ctrl+Tab: Next tab
                    self.next_tab();
                    true
                }
                KeyCode::Num1
                | KeyCode::Num2
                | KeyCode::Num3
                | KeyCode::Num4
                | KeyCode::Num5
                | KeyCode::Num6
                | KeyCode::Num7
                | KeyCode::Num8
                | KeyCode::Num9 => {
                    // Ctrl+1-9: Switch to tab
                    let index = match key {
                        KeyCode::Num1 => 0,
                        KeyCode::Num2 => 1,
                        KeyCode::Num3 => 2,
                        KeyCode::Num4 => 3,
                        KeyCode::Num5 => 4,
                        KeyCode::Num6 => 5,
                        KeyCode::Num7 => 6,
                        KeyCode::Num8 => 7,
                        KeyCode::Num9 => {
                            // Ctrl+9 goes to last tab
                            self.tab_count().saturating_sub(1)
                        }
                        _ => 0,
                    };
                    self.activate_tab_by_index(index);
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// Get tab at position.
    fn tab_at_position(&self, x: i32, _y: i32) -> Option<TabId> {
        let bounds = *self.bounds.read();
        let scroll = *self.scroll_offset.read();
        let tab_width = *self.tab_width.read() as i32;

        let relative_x = x - bounds.x + scroll;
        let tabs = self.tabs.read();

        let mut current_x = 0;
        for tab in tabs.iter() {
            let width = if tab.pinned {
                self.pinned_tab_width as i32
            } else {
                tab_width
            };
            if relative_x >= current_x && relative_x < current_x + width {
                return Some(tab.id);
            }
            current_x += width;
        }

        None
    }

    /// Check if position is close button.
    fn is_close_button(&self, x: i32, _y: i32, tab_id: TabId) -> bool {
        // Close button is at the right side of the tab
        let tabs = self.tabs.read();
        let tab = match tabs.iter().find(|t| t.id == tab_id) {
            Some(t) => t,
            None => return false,
        };

        if tab.pinned {
            return false; // Pinned tabs don't show close button
        }

        // Calculate tab position
        let bounds = *self.bounds.read();
        let scroll = *self.scroll_offset.read();
        let tab_width = *self.tab_width.read() as i32;

        let mut current_x = bounds.x - scroll;
        for t in tabs.iter() {
            if t.id == tab_id {
                let close_x = current_x + tab_width - 24;
                return x >= close_x && x < close_x + 16;
            }
            current_x += if t.pinned {
                self.pinned_tab_width as i32
            } else {
                tab_width
            };
        }

        false
    }

    /// Check if position is new tab button.
    fn is_new_tab_button(&self, x: i32, _y: i32) -> bool {
        let bounds = *self.bounds.read();
        let tabs = self.tabs.read();
        let tab_width = *self.tab_width.read() as i32;

        // New tab button is after all tabs
        let total_width: i32 = tabs
            .iter()
            .map(|t| {
                if t.pinned {
                    self.pinned_tab_width as i32
                } else {
                    tab_width
                }
            })
            .sum();

        let button_x = bounds.x + total_width;
        x >= button_x && x < button_x + 28
    }

    /// Go to next tab.
    fn next_tab(&self) {
        let tabs = self.tabs.read();
        if tabs.is_empty() {
            return;
        }

        let active = *self.active_tab.read();
        let current_idx = active
            .and_then(|id| tabs.iter().position(|t| t.id == id))
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % tabs.len();

        drop(tabs);
        self.activate_tab_by_index(next_idx);
    }

    /// Activate tab by index.
    fn activate_tab_by_index(&self, index: usize) {
        let tabs = self.tabs.read();
        if let Some(tab) = tabs.get(index) {
            let id = tab.id;
            drop(tabs);
            self.activate_tab(id);
        }
    }

    /// Recalculate tab widths.
    fn recalculate_tab_widths(&self) {
        let tabs = self.tabs.read();
        let bounds = *self.bounds.read();

        let pinned_count = tabs.iter().filter(|t| t.pinned).count() as u32;
        let regular_count = (tabs.len() as u32).saturating_sub(pinned_count);

        let pinned_width_total = pinned_count * self.pinned_tab_width;
        let available_width = bounds
            .width
            .saturating_sub(pinned_width_total)
            .saturating_sub(28); // 28 for new tab button

        let width = if regular_count > 0 {
            (available_width / regular_count).clamp(self.min_tab_width, self.max_tab_width)
        } else {
            self.max_tab_width
        };

        *self.tab_width.write() = width;
    }

    /// Set bounds.
    pub fn set_bounds(&self, bounds: Rect) {
        *self.bounds.write() = bounds;
        self.recalculate_tab_widths();
    }
}

impl Default for TabStrip {
    fn default() -> Self {
        Self::new()
    }
}

/// Session restore data.
#[derive(Debug, Clone)]
pub struct SessionData {
    /// Windows.
    pub windows: Vec<WindowData>,
    /// Last modified time.
    pub last_modified: u64,
}

/// Window data.
#[derive(Debug, Clone)]
pub struct WindowData {
    /// Window bounds.
    pub bounds: Rect,
    /// Tabs.
    pub tabs: Vec<TabData>,
    /// Active tab index.
    pub active_tab_index: usize,
    /// Tab groups.
    pub groups: Vec<TabGroup>,
}

/// Tab data for session restore.
#[derive(Debug, Clone)]
pub struct TabData {
    /// URL.
    pub url: String,
    /// Title.
    pub title: String,
    /// Pinned.
    pub pinned: bool,
    /// Group ID.
    pub group_id: Option<TabGroupId>,
    /// Scroll position.
    pub scroll_x: i32,
    pub scroll_y: i32,
    /// History entries.
    pub history: Vec<HistoryEntry>,
    /// Current history index.
    pub history_index: usize,
}

/// History entry.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// URL.
    pub url: String,
    /// Title.
    pub title: String,
}

/// Session manager.
pub struct SessionManager {
    /// Current session.
    session: RwLock<SessionData>,
    /// Auto save interval (ms).
    auto_save_interval: u64,
    /// Last save time.
    last_save_time: RwLock<u64>,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new() -> Self {
        Self {
            session: RwLock::new(SessionData {
                windows: Vec::new(),
                last_modified: 0,
            }),
            auto_save_interval: 30_000, // 30 seconds
            last_save_time: RwLock::new(0),
        }
    }

    /// Save current session from tab strips.
    pub fn save_session(&self, windows: &[(Rect, &TabStrip)]) {
        let mut session = self.session.write();
        session.windows.clear();

        for (bounds, strip) in windows {
            let tabs = strip.tabs();
            let tab_data: Vec<TabData> = tabs
                .iter()
                .map(|t| TabData {
                    url: t.url.clone(),
                    title: t.title.clone(),
                    pinned: t.pinned,
                    group_id: t.group_id,
                    scroll_x: 0,
                    scroll_y: 0,
                    history: vec![HistoryEntry {
                        url: t.url.clone(),
                        title: t.title.clone(),
                    }],
                    history_index: 0,
                })
                .collect();

            let active_idx = strip
                .active_tab()
                .and_then(|id| tabs.iter().position(|t| t.id == id))
                .unwrap_or(0);

            session.windows.push(WindowData {
                bounds: *bounds,
                tabs: tab_data,
                active_tab_index: active_idx,
                groups: Vec::new(),
            });
        }

        session.last_modified = 0; // Would use actual timestamp
    }

    /// Restore session to tab strips.
    pub fn restore_session(&self, strip: &TabStrip) {
        let session = self.session.read();

        for window in &session.windows {
            for (idx, tab_data) in window.tabs.iter().enumerate() {
                let tab_id = strip.new_tab(&tab_data.url);
                strip.set_tab_title(tab_id, &tab_data.title);
                strip.set_tab_pinned(tab_id, tab_data.pinned);

                if let Some(group_id) = tab_data.group_id {
                    strip.add_tab_to_group(tab_id, group_id);
                }

                if idx == window.active_tab_index {
                    strip.activate_tab(tab_id);
                }
            }
        }
    }

    /// Check if should auto-save.
    pub fn should_auto_save(&self, current_time: u64) -> bool {
        let last = *self.last_save_time.read();
        current_time - last >= self.auto_save_interval
    }

    /// Get session data for serialization.
    pub fn get_session(&self) -> SessionData {
        self.session.read().clone()
    }

    /// Load session from data.
    pub fn load_session(&self, data: SessionData) {
        *self.session.write() = data;
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_strip() {
        let strip = TabStrip::new();

        // Create tabs
        let tab1 = strip.new_tab("https://example.com");
        let tab2 = strip.new_tab("https://google.com");

        assert_eq!(strip.tab_count(), 2);
        assert_eq!(strip.active_tab(), Some(tab2));

        // Activate tab
        strip.activate_tab(tab1);
        assert_eq!(strip.active_tab(), Some(tab1));

        // Close tab
        strip.close_tab(tab1);
        assert_eq!(strip.tab_count(), 1);
        assert_eq!(strip.active_tab(), Some(tab2));
    }

    #[test]
    fn test_tab_groups() {
        let strip = TabStrip::new();

        let tab1 = strip.new_tab("https://example.com");
        let tab2 = strip.new_tab("https://google.com");

        let group = strip.create_group("Work", TabGroupColor::Blue);
        strip.add_tab_to_group(tab1, group);
        strip.add_tab_to_group(tab2, group);

        let tabs = strip.tabs();
        assert_eq!(tabs[0].group_id, Some(group));
        assert_eq!(tabs[1].group_id, Some(group));
    }

    #[test]
    fn test_pinned_tabs() {
        let strip = TabStrip::new();

        let tab1 = strip.new_tab("https://example.com");
        let _tab2 = strip.new_tab("https://google.com");

        strip.set_tab_pinned(tab1, true);

        let tabs = strip.tabs();
        assert!(tabs[0].pinned);
        assert!(!tabs[1].pinned);
    }
}
