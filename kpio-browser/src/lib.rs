//! KPIO Browser - Web browser for KPIO OS
//!
//! A lightweight web browser shell that integrates:
//! - HTML parsing (kpio-html)
//! - CSS parsing and styling (kpio-css)
//! - JavaScript execution (kpio-js)
//! - Layout engine (kpio-layout)
//! - Graphics rendering (kpio-graphics)

#![no_std]

extern crate alloc;

pub mod a11y;
pub mod account;
pub mod apps;
pub mod browser;
pub mod csp;
pub mod design;
pub mod document;
pub mod events;
pub mod fs_bridge;
pub mod i18n;
pub mod input;
pub mod input_bridge;
pub mod kernel_bridge;
pub mod loader;
pub mod navigation;
pub mod network_bridge;
pub mod pipeline;
pub mod pwa;
pub mod renderer;
pub mod tabs;
pub mod ui;
pub mod window;

#[cfg(test)]
mod tests;

pub use browser::Browser;
pub use csp::{CspCheck, CspContext, CspPolicy};
pub use document::Document;
pub use input::{DomInputEvent, HitTestResult, InputManager, RawInputEvent};
pub use loader::{DocumentLoader, LoadResult, LoaderError, PageLoader};
pub use navigation::Navigator;
pub use pipeline::{PipelineError, RenderPipeline};
pub use window::Window;

use alloc::string::String;

/// Initialize the browser.
pub fn init() -> Browser {
    Browser::new()
}

/// Browser configuration.
#[derive(Debug, Clone)]
pub struct BrowserConfig {
    /// Browser user agent string.
    pub user_agent: String,
    /// Default homepage URL.
    pub homepage: String,
    /// Enable JavaScript.
    pub javascript_enabled: bool,
    /// Enable cookies.
    pub cookies_enabled: bool,
    /// Viewport width.
    pub viewport_width: u32,
    /// Viewport height.
    pub viewport_height: u32,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            user_agent: String::from("KPIO-Browser/0.1.0"),
            homepage: String::from("about:blank"),
            javascript_enabled: true,
            cookies_enabled: true,
            viewport_width: 1024,
            viewport_height: 768,
        }
    }
}
