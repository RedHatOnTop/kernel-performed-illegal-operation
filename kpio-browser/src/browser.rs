//! Browser main controller.
//!
//! Manages browser state and coordinates components.

use alloc::string::{String, ToString};

use crate::BrowserConfig;
use crate::tabs::TabManager;
use crate::navigation::Navigator;

/// Main browser controller.
pub struct Browser {
    /// Browser configuration.
    config: BrowserConfig,
    /// Tab manager.
    tabs: TabManager,
    /// Current active tab index.
    active_tab: usize,
    /// Navigation history manager.
    navigator: Navigator,
    /// Browser state.
    state: BrowserState,
}

/// Browser state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserState {
    /// Browser is idle.
    Idle,
    /// Loading a page.
    Loading,
    /// Page loaded.
    Ready,
    /// Error state.
    Error,
}

impl Browser {
    /// Create a new browser instance.
    pub fn new() -> Self {
        Self::with_config(BrowserConfig::default())
    }
    
    /// Create a browser with custom configuration.
    pub fn with_config(config: BrowserConfig) -> Self {
        let mut tabs = TabManager::new();
        tabs.new_tab();
        
        Self {
            config,
            tabs,
            active_tab: 0,
            navigator: Navigator::new(),
            state: BrowserState::Idle,
        }
    }
    
    /// Navigate to a URL.
    pub fn navigate(&mut self, url: &str) -> Result<(), BrowserError> {
        self.state = BrowserState::Loading;
        
        // Parse URL
        let parsed_url = self.navigator.parse_url(url)?;
        
        // Get active tab
        if let Some(tab) = self.tabs.get_active_mut() {
            tab.load_url(&parsed_url)?;
        }
        
        self.navigator.push_history(parsed_url);
        self.state = BrowserState::Ready;
        
        Ok(())
    }
    
    /// Go back in history.
    pub fn back(&mut self) -> Result<(), BrowserError> {
        if let Some(url) = self.navigator.go_back() {
            if let Some(tab) = self.tabs.get_active_mut() {
                tab.load_url(&url)?;
            }
        }
        Ok(())
    }
    
    /// Go forward in history.
    pub fn forward(&mut self) -> Result<(), BrowserError> {
        if let Some(url) = self.navigator.go_forward() {
            if let Some(tab) = self.tabs.get_active_mut() {
                tab.load_url(&url)?;
            }
        }
        Ok(())
    }
    
    /// Reload the current page.
    pub fn reload(&mut self) -> Result<(), BrowserError> {
        if let Some(url) = self.navigator.current_url() {
            if let Some(tab) = self.tabs.get_active_mut() {
                tab.load_url(&url)?;
            }
        }
        Ok(())
    }
    
    /// Stop loading.
    pub fn stop(&mut self) {
        if let Some(tab) = self.tabs.get_active_mut() {
            tab.stop_loading();
        }
        self.state = BrowserState::Idle;
    }
    
    /// Create a new tab.
    pub fn new_tab(&mut self) -> usize {
        let idx = self.tabs.new_tab();
        self.active_tab = idx;
        idx
    }
    
    /// Close a tab.
    pub fn close_tab(&mut self, index: usize) -> Result<(), BrowserError> {
        self.tabs.close_tab(index)?;
        if self.active_tab >= self.tabs.count() && self.tabs.count() > 0 {
            self.active_tab = self.tabs.count() - 1;
        }
        Ok(())
    }
    
    /// Switch to a tab.
    pub fn switch_tab(&mut self, index: usize) -> Result<(), BrowserError> {
        if index < self.tabs.count() {
            self.active_tab = index;
            self.tabs.set_active(index);
            Ok(())
        } else {
            Err(BrowserError::InvalidTabIndex(index))
        }
    }
    
    /// Get current URL.
    pub fn current_url(&self) -> Option<String> {
        self.navigator.current_url().map(|u| u.to_string())
    }
    
    /// Get page title.
    pub fn title(&self) -> Option<String> {
        self.tabs.get_active().and_then(|t| t.title())
    }
    
    /// Get browser state.
    pub fn state(&self) -> BrowserState {
        self.state
    }
    
    /// Get tab count.
    pub fn tab_count(&self) -> usize {
        self.tabs.count()
    }
    
    /// Get active tab index.
    pub fn active_tab(&self) -> usize {
        self.active_tab
    }
    
    /// Render the current page to a framebuffer.
    pub fn render(&mut self, framebuffer: &mut [u32], width: u32, height: u32) -> Result<(), BrowserError> {
        if let Some(tab) = self.tabs.get_active_mut() {
            tab.render(framebuffer, width, height)?;
        }
        Ok(())
    }
    
    /// Process a mouse event.
    pub fn on_mouse(&mut self, x: i32, y: i32, button: MouseButton, state: MouseState) {
        if let Some(tab) = self.tabs.get_active_mut() {
            tab.on_mouse(x, y, button, state);
        }
    }
    
    /// Process a keyboard event.
    pub fn on_key(&mut self, key: Key, state: KeyState, modifiers: Modifiers) {
        if let Some(tab) = self.tabs.get_active_mut() {
            tab.on_key(key, state, modifiers);
        }
    }
    
    /// Tick - process pending events and animations.
    pub fn tick(&mut self) {
        if let Some(tab) = self.tabs.get_active_mut() {
            tab.tick();
        }
    }
    
    /// Execute JavaScript.
    pub fn execute_script(&mut self, script: &str) -> Result<String, BrowserError> {
        if !self.config.javascript_enabled {
            return Err(BrowserError::JavaScriptDisabled);
        }
        
        if let Some(tab) = self.tabs.get_active_mut() {
            tab.execute_script(script)
        } else {
            Err(BrowserError::NoActiveTab)
        }
    }
}

impl Default for Browser {
    fn default() -> Self {
        Self::new()
    }
}

/// Browser error.
#[derive(Debug, Clone)]
pub enum BrowserError {
    /// Network error.
    NetworkError(String),
    /// Invalid URL.
    InvalidUrl(String),
    /// Parse error.
    ParseError(String),
    /// JavaScript error.
    JavaScriptError(String),
    /// Invalid tab index.
    InvalidTabIndex(usize),
    /// JavaScript disabled.
    JavaScriptDisabled,
    /// No active tab.
    NoActiveTab,
    /// Render error.
    RenderError(String),
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// Mouse state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseState {
    Pressed,
    Released,
    Moved,
}

/// Keyboard key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key(pub u32);

/// Key state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
}

/// Keyboard modifiers.
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}
