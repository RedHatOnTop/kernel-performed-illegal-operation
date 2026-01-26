//! Browser window.
//!
//! Manages the browser window and viewport.

use alloc::string::String;

/// Browser window.
pub struct Window {
    /// Window width.
    width: u32,
    /// Window height.
    height: u32,
    /// Device pixel ratio.
    pixel_ratio: f32,
    /// Is window focused.
    focused: bool,
    /// Window title.
    title: String,
}

impl Window {
    /// Create a new window.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixel_ratio: 1.0,
            focused: true,
            title: String::new(),
        }
    }
    
    /// Get window width.
    pub fn width(&self) -> u32 {
        self.width
    }
    
    /// Get window height.
    pub fn height(&self) -> u32 {
        self.height
    }
    
    /// Get inner width (viewport width).
    pub fn inner_width(&self) -> u32 {
        self.width
    }
    
    /// Get inner height (viewport height).
    pub fn inner_height(&self) -> u32 {
        self.height
    }
    
    /// Resize window.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
    
    /// Get device pixel ratio.
    pub fn device_pixel_ratio(&self) -> f32 {
        self.pixel_ratio
    }
    
    /// Set device pixel ratio.
    pub fn set_device_pixel_ratio(&mut self, ratio: f32) {
        self.pixel_ratio = ratio;
    }
    
    /// Check if window is focused.
    pub fn is_focused(&self) -> bool {
        self.focused
    }
    
    /// Set focus state.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
    
    /// Get window title.
    pub fn title(&self) -> &str {
        &self.title
    }
    
    /// Set window title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }
    
    /// Scroll by amount.
    pub fn scroll_by(&mut self, _x: i32, _y: i32) {
        // Would update scroll position
    }
    
    /// Scroll to position.
    pub fn scroll_to(&mut self, _x: i32, _y: i32) {
        // Would set scroll position
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::new(1024, 768)
    }
}
