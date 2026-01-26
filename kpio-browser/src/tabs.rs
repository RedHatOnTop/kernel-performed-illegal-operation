//! Tab management.
//!
//! Browser tab system.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use kpio_js::Engine;

use crate::browser::{BrowserError, MouseButton, MouseState, Key, KeyState, Modifiers};
use crate::document::Document;
use crate::navigation::Url;
use crate::renderer::Renderer;
use crate::window::Window;

/// Browser tab.
pub struct Tab {
    /// Tab ID.
    id: usize,
    /// Tab title.
    title: Option<String>,
    /// Current URL.
    url: Option<Url>,
    /// Loading state.
    loading: bool,
    /// Document.
    document: Option<Document>,
    /// JavaScript engine.
    js_engine: Engine,
    /// Renderer.
    renderer: Renderer,
    /// Window.
    window: Window,
    /// Scroll position.
    scroll_x: i32,
    scroll_y: i32,
}

impl Tab {
    /// Create a new tab.
    pub fn new(id: usize) -> Self {
        Self {
            id,
            title: None,
            url: None,
            loading: false,
            document: None,
            js_engine: Engine::new(),
            renderer: Renderer::new(),
            window: Window::default(),
            scroll_x: 0,
            scroll_y: 0,
        }
    }
    
    /// Get tab ID.
    pub fn id(&self) -> usize {
        self.id
    }
    
    /// Get tab title.
    pub fn title(&self) -> Option<String> {
        self.title.clone()
    }
    
    /// Get current URL.
    pub fn url(&self) -> Option<&Url> {
        self.url.as_ref()
    }
    
    /// Is tab loading?
    pub fn is_loading(&self) -> bool {
        self.loading
    }
    
    /// Load a URL.
    pub fn load_url(&mut self, url: &Url) -> Result<(), BrowserError> {
        self.loading = true;
        self.url = Some(url.clone());
        
        // Handle special URLs
        if url.is_special() {
            match url.scheme.as_str() {
                "about" => self.load_about_page(&url.path),
                "javascript" => {
                    self.execute_script(&url.path)?;
                }
                "data" => self.load_data_url(&url.path),
                _ => {}
            }
            self.loading = false;
            return Ok(());
        }
        
        // For HTTP URLs, we would fetch here
        // For now, just create an empty document
        self.document = Some(Document::new(&url.href()));
        self.title = self.document.as_ref().map(|d| d.title().to_string());
        
        self.loading = false;
        Ok(())
    }
    
    /// Load HTML content directly.
    pub fn load_html(&mut self, html: &str, url: &str) -> Result<(), BrowserError> {
        self.loading = true;
        
        let mut document = Document::from_html(html, url);
        document.compute_styles();
        
        self.title = Some(document.title().to_string());
        self.document = Some(document);
        
        // Execute inline scripts
        self.execute_inline_scripts()?;
        
        self.loading = false;
        Ok(())
    }
    
    /// Load about: page.
    fn load_about_page(&mut self, page: &str) {
        let html = match page {
            "blank" => "<html><head><title>about:blank</title></head><body></body></html>",
            "newtab" => r#"
                <html>
                <head><title>New Tab</title></head>
                <body style="background: #f5f5f5; font-family: sans-serif;">
                    <h1 style="text-align: center; color: #333;">KPIO Browser</h1>
                    <p style="text-align: center;">Welcome to KPIO OS Web Browser</p>
                </body>
                </html>
            "#,
            _ => "<html><head><title>Not Found</title></head><body><h1>Page not found</h1></body></html>",
        };
        
        let _ = self.load_html(html, &alloc::format!("about:{}", page));
    }
    
    /// Load data: URL.
    fn load_data_url(&mut self, data: &str) {
        // Parse data URL format: media-type;base64,data
        if data.starts_with("text/html,") {
            let html = &data[10..];
            let _ = self.load_html(html, "data:text/html");
        }
    }
    
    /// Stop loading.
    pub fn stop_loading(&mut self) {
        self.loading = false;
    }
    
    /// Execute JavaScript.
    pub fn execute_script(&mut self, script: &str) -> Result<String, BrowserError> {
        self.js_engine.eval(script)
            .map(|v| v.to_string().unwrap_or_default())
            .map_err(|e| BrowserError::JavaScriptError(alloc::format!("{:?}", e)))
    }
    
    /// Execute inline scripts in document.
    fn execute_inline_scripts(&mut self) -> Result<(), BrowserError> {
        // Would extract and execute <script> tags
        // Simplified for now
        Ok(())
    }
    
    /// Render tab to framebuffer.
    pub fn render(&mut self, framebuffer: &mut [u32], width: u32, height: u32) -> Result<(), BrowserError> {
        if let Some(doc) = &self.document {
            self.renderer.render(doc, framebuffer, width, height);
        } else {
            // Clear to white
            for pixel in framebuffer.iter_mut() {
                *pixel = 0xFFFFFFFF;
            }
        }
        Ok(())
    }
    
    /// Handle mouse event.
    pub fn on_mouse(&mut self, x: i32, y: i32, button: MouseButton, state: MouseState) {
        // Would dispatch to DOM event system
        let _ = (x, y, button, state);
    }
    
    /// Handle keyboard event.
    pub fn on_key(&mut self, key: Key, state: KeyState, modifiers: Modifiers) {
        // Would dispatch to DOM event system
        let _ = (key, state, modifiers);
    }
    
    /// Tick - process pending work.
    pub fn tick(&mut self) {
        // Process timers, animations, etc.
    }
    
    /// Scroll page.
    pub fn scroll(&mut self, dx: i32, dy: i32) {
        self.scroll_x += dx;
        self.scroll_y += dy;
        
        // Clamp scroll position
        self.scroll_x = self.scroll_x.max(0);
        self.scroll_y = self.scroll_y.max(0);
    }
    
    /// Get scroll position.
    pub fn scroll_position(&self) -> (i32, i32) {
        (self.scroll_x, self.scroll_y)
    }
}

/// Tab manager.
pub struct TabManager {
    /// All tabs.
    tabs: Vec<Tab>,
    /// Active tab index.
    active: usize,
    /// Next tab ID.
    next_id: usize,
}

impl TabManager {
    /// Create a new tab manager.
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active: 0,
            next_id: 0,
        }
    }
    
    /// Create a new tab.
    pub fn new_tab(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        
        let tab = Tab::new(id);
        self.tabs.push(tab);
        
        let index = self.tabs.len() - 1;
        self.active = index;
        index
    }
    
    /// Close a tab.
    pub fn close_tab(&mut self, index: usize) -> Result<(), BrowserError> {
        if index >= self.tabs.len() {
            return Err(BrowserError::InvalidTabIndex(index));
        }
        
        if self.tabs.len() == 1 {
            // Don't close last tab, just clear it
            self.tabs[0] = Tab::new(self.next_id);
            self.next_id += 1;
            return Ok(());
        }
        
        self.tabs.remove(index);
        
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        
        Ok(())
    }
    
    /// Set active tab.
    pub fn set_active(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = index;
        }
    }
    
    /// Get active tab.
    pub fn get_active(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }
    
    /// Get active tab mutably.
    pub fn get_active_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }
    
    /// Get tab by index.
    pub fn get(&self, index: usize) -> Option<&Tab> {
        self.tabs.get(index)
    }
    
    /// Get tab by index mutably.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Tab> {
        self.tabs.get_mut(index)
    }
    
    /// Get tab count.
    pub fn count(&self) -> usize {
        self.tabs.len()
    }
    
    /// Get all tabs.
    pub fn all(&self) -> &[Tab] {
        &self.tabs
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}
