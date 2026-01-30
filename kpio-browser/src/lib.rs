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

pub mod browser;
pub mod document;
pub mod window;
pub mod navigation;
pub mod renderer;
pub mod events;
pub mod tabs;
pub mod pipeline;
pub mod input;
pub mod loader;
pub mod csp;
pub mod ui;
pub mod pwa;
pub mod account;
pub mod design;
pub mod a11y;
pub mod i18n;
pub mod apps;

pub use browser::Browser;
pub use document::Document;
pub use window::Window;
pub use navigation::Navigator;
pub use pipeline::{RenderPipeline, PipelineError};
pub use input::{InputManager, RawInputEvent, DomInputEvent, HitTestResult};
pub use csp::{CspPolicy, CspContext, CspCheck};
pub use loader::{PageLoader, DocumentLoader, LoadResult, LoaderError};

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
