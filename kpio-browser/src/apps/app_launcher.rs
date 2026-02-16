//! App Launcher
//!
//! Application launcher / start menu.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{AppCategory, AppInfo};

/// App launcher
#[derive(Debug, Clone)]
pub struct AppLauncher {
    /// Is open
    pub open: bool,
    /// Search query
    pub search_query: String,
    /// Filtered apps
    pub filtered_apps: Vec<AppInfo>,
    /// All apps
    pub all_apps: Vec<AppInfo>,
    /// Pinned apps
    pub pinned: Vec<String>,
    /// Recent apps
    pub recent: Vec<String>,
    /// Selected index
    pub selected_index: usize,
    /// Current view
    pub view: LauncherView,
}

impl AppLauncher {
    /// Create new launcher
    pub fn new() -> Self {
        let all_apps = super::system_apps();
        Self {
            open: false,
            search_query: String::new(),
            filtered_apps: all_apps.clone(),
            all_apps,
            pinned: alloc::vec![
                String::from("kpio.browser"),
                String::from("kpio.files"),
                String::from("kpio.terminal"),
                String::from("kpio.settings"),
            ],
            recent: Vec::new(),
            selected_index: 0,
            view: LauncherView::Pinned,
        }
    }

    /// Toggle launcher
    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.reset();
        }
    }

    /// Open launcher
    pub fn show(&mut self) {
        self.open = true;
        self.reset();
    }

    /// Close launcher
    pub fn hide(&mut self) {
        self.open = false;
    }

    /// Reset state
    fn reset(&mut self) {
        self.search_query.clear();
        self.filtered_apps = self.all_apps.clone();
        self.selected_index = 0;
        self.view = LauncherView::Pinned;
    }

    /// Search apps
    pub fn search(&mut self, query: &str) {
        self.search_query = query.to_string();

        if query.is_empty() {
            self.filtered_apps = self.all_apps.clone();
            self.view = LauncherView::Pinned;
        } else {
            let query_lower = query.to_lowercase();
            self.filtered_apps = self
                .all_apps
                .iter()
                .filter(|app| {
                    app.name.to_lowercase().contains(&query_lower)
                        || app.description.to_lowercase().contains(&query_lower)
                })
                .cloned()
                .collect();
            self.view = LauncherView::Search;
        }

        self.selected_index = 0;
    }

    /// Navigate up
    pub fn navigate_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Navigate down
    pub fn navigate_down(&mut self) {
        let max = match self.view {
            LauncherView::Search => self.filtered_apps.len(),
            LauncherView::AllApps => self.all_apps.len(),
            LauncherView::Pinned => self.pinned.len(),
        };
        if self.selected_index + 1 < max {
            self.selected_index += 1;
        }
    }

    /// Get selected app
    pub fn selected_app(&self) -> Option<&AppInfo> {
        match self.view {
            LauncherView::Search => self.filtered_apps.get(self.selected_index),
            LauncherView::AllApps => self.all_apps.get(self.selected_index),
            LauncherView::Pinned => {
                let app_id = self.pinned.get(self.selected_index)?;
                self.all_apps.iter().find(|a| &a.id.0 == app_id)
            }
        }
    }

    /// Launch selected app
    pub fn launch_selected(&mut self) -> Option<String> {
        let app = self.selected_app()?.clone();
        self.add_recent(&app.id.0);
        self.hide();
        Some(app.id.0.clone())
    }

    /// Launch app by ID
    pub fn launch(&mut self, app_id: &str) -> bool {
        if self.all_apps.iter().any(|a| a.id.0 == app_id) {
            self.add_recent(app_id);
            self.hide();
            true
        } else {
            false
        }
    }

    /// Add to recent
    fn add_recent(&mut self, app_id: &str) {
        // Remove if exists
        self.recent.retain(|id| id != app_id);
        // Add to front
        self.recent.insert(0, app_id.to_string());
        // Limit size
        if self.recent.len() > 10 {
            self.recent.pop();
        }
    }

    /// Pin app
    pub fn pin(&mut self, app_id: &str) {
        if !self.pinned.contains(&app_id.to_string()) {
            self.pinned.push(app_id.to_string());
        }
    }

    /// Unpin app
    pub fn unpin(&mut self, app_id: &str) {
        self.pinned.retain(|id| id != app_id);
    }

    /// Set view
    pub fn set_view(&mut self, view: LauncherView) {
        self.view = view;
        self.selected_index = 0;
    }

    /// Get apps by category
    pub fn apps_by_category(&self, category: AppCategory) -> Vec<&AppInfo> {
        self.all_apps
            .iter()
            .filter(|app| app.category == category)
            .collect()
    }

    /// Get pinned apps
    pub fn pinned_apps(&self) -> Vec<&AppInfo> {
        self.pinned
            .iter()
            .filter_map(|id| self.all_apps.iter().find(|a| &a.id.0 == id))
            .collect()
    }

    /// Get recent apps
    pub fn recent_apps(&self) -> Vec<&AppInfo> {
        self.recent
            .iter()
            .filter_map(|id| self.all_apps.iter().find(|a| &a.id.0 == id))
            .collect()
    }
}

impl Default for AppLauncher {
    fn default() -> Self {
        Self::new()
    }
}

/// Launcher view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LauncherView {
    #[default]
    Pinned,
    AllApps,
    Search,
}

/// Quick action
#[derive(Debug, Clone)]
pub struct QuickAction {
    /// Action ID
    pub id: String,
    /// Name
    pub name: String,
    /// Icon
    pub icon: String,
    /// Action type
    pub action_type: QuickActionType,
}

#[derive(Debug, Clone)]
pub enum QuickActionType {
    /// Open settings page
    Settings(String),
    /// Run command
    Command(String),
    /// Open URL
    Url(String),
    /// Open path
    Path(String),
}

/// Get quick actions for search
pub fn quick_actions() -> Vec<QuickAction> {
    alloc::vec![
        QuickAction {
            id: String::from("shutdown"),
            name: String::from("Shutdown"),
            icon: String::from("power"),
            action_type: QuickActionType::Command(String::from("shutdown")),
        },
        QuickAction {
            id: String::from("restart"),
            name: String::from("Restart"),
            icon: String::from("refresh-cw"),
            action_type: QuickActionType::Command(String::from("restart")),
        },
        QuickAction {
            id: String::from("lock"),
            name: String::from("Lock Screen"),
            icon: String::from("lock"),
            action_type: QuickActionType::Command(String::from("lock")),
        },
        QuickAction {
            id: String::from("sleep"),
            name: String::from("Sleep"),
            icon: String::from("moon"),
            action_type: QuickActionType::Command(String::from("sleep")),
        },
        QuickAction {
            id: String::from("display"),
            name: String::from("Display Settings"),
            icon: String::from("monitor"),
            action_type: QuickActionType::Settings(String::from("display")),
        },
        QuickAction {
            id: String::from("sound"),
            name: String::from("Sound Settings"),
            icon: String::from("volume"),
            action_type: QuickActionType::Settings(String::from("sound")),
        },
        QuickAction {
            id: String::from("network"),
            name: String::from("Network Settings"),
            icon: String::from("wifi"),
            action_type: QuickActionType::Settings(String::from("network")),
        },
        QuickAction {
            id: String::from("bluetooth"),
            name: String::from("Bluetooth Settings"),
            icon: String::from("bluetooth"),
            action_type: QuickActionType::Settings(String::from("bluetooth")),
        },
    ]
}

/// Search result type
#[derive(Debug, Clone)]
pub enum SearchResult {
    /// App
    App(AppInfo),
    /// Quick action
    Action(QuickAction),
    /// File
    File { name: String, path: String },
    /// Web search suggestion
    WebSearch(String),
}

/// Search all
pub fn search_all(query: &str, apps: &[AppInfo]) -> Vec<SearchResult> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    // Search apps
    for app in apps {
        if app.name.to_lowercase().contains(&query_lower)
            || app.description.to_lowercase().contains(&query_lower)
        {
            results.push(SearchResult::App(app.clone()));
        }
    }

    // Search quick actions
    for action in quick_actions() {
        if action.name.to_lowercase().contains(&query_lower) {
            results.push(SearchResult::Action(action));
        }
    }

    // Add web search as fallback
    if !query.is_empty() {
        results.push(SearchResult::WebSearch(query.to_string()));
    }

    results
}
