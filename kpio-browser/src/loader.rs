//! HTTP Page Loader
//!
//! This module provides functionality to load web pages via HTTP
//! and integrate with the rendering pipeline.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::format;
use alloc::boxed::Box;

use kpio_network::{
    HttpClient, HttpRequest, HttpResponse, HttpError, Url, StatusCode,
};

use crate::pipeline::{RenderPipeline, PipelineError};

/// Resource type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    /// HTML document.
    Document,
    /// CSS stylesheet.
    Stylesheet,
    /// JavaScript.
    Script,
    /// Image.
    Image,
    /// Font.
    Font,
    /// Other resource.
    Other,
}

impl ResourceType {
    /// Determine resource type from Content-Type header.
    pub fn from_content_type(content_type: &str) -> Self {
        let ct = content_type.to_ascii_lowercase();
        if ct.contains("text/html") || ct.contains("application/xhtml") {
            ResourceType::Document
        } else if ct.contains("text/css") {
            ResourceType::Stylesheet
        } else if ct.contains("javascript") || ct.contains("application/json") {
            ResourceType::Script
        } else if ct.contains("image/") {
            ResourceType::Image
        } else if ct.contains("font/") || ct.contains("application/font") {
            ResourceType::Font
        } else {
            ResourceType::Other
        }
    }
    
    /// Get Accept header value for this resource type.
    pub fn accept_header(&self) -> &'static str {
        match self {
            ResourceType::Document => "text/html,application/xhtml+xml,*/*;q=0.8",
            ResourceType::Stylesheet => "text/css,*/*;q=0.1",
            ResourceType::Script => "application/javascript,*/*;q=0.1",
            ResourceType::Image => "image/*,*/*;q=0.1",
            ResourceType::Font => "font/*,application/font-*,*/*;q=0.1",
            ResourceType::Other => "*/*",
        }
    }
}

/// Load result.
#[derive(Debug, Clone)]
pub struct LoadResult {
    /// Final URL (after redirects).
    pub url: String,
    /// HTTP status code.
    pub status: u16,
    /// Content type.
    pub content_type: Option<String>,
    /// Response body.
    pub body: Vec<u8>,
    /// Resource type.
    pub resource_type: ResourceType,
}

impl LoadResult {
    /// Get body as text.
    pub fn text(&self) -> Option<String> {
        String::from_utf8(self.body.clone()).ok()
    }
    
    /// Check if load was successful.
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
}

/// Loader error.
#[derive(Debug, Clone)]
pub enum LoaderError {
    /// HTTP error.
    Http(String),
    /// Invalid URL.
    InvalidUrl(String),
    /// Network error.
    Network(String),
    /// Too many redirects.
    TooManyRedirects,
    /// Timeout.
    Timeout,
    /// Resource not found.
    NotFound,
    /// Access denied.
    Forbidden,
    /// Server error.
    ServerError(u16),
}

impl From<HttpError> for LoaderError {
    fn from(err: HttpError) -> Self {
        match err {
            HttpError::InvalidUrl(s) => LoaderError::InvalidUrl(s),
            HttpError::Timeout => LoaderError::Timeout,
            HttpError::TooManyRedirects => LoaderError::TooManyRedirects,
            HttpError::Network(s) => LoaderError::Network(s),
            HttpError::InvalidResponse(s) => LoaderError::Http(s),
            HttpError::ConnectionClosed => LoaderError::Network("Connection closed".to_string()),
            HttpError::DnsError(s) => LoaderError::Network(format!("DNS: {}", s)),
        }
    }
}

/// Navigation entry for history.
#[derive(Debug, Clone)]
pub struct NavigationEntry {
    /// URL.
    pub url: String,
    /// Page title.
    pub title: String,
    /// Scroll position.
    pub scroll_x: i32,
    pub scroll_y: i32,
}

/// Page loader for fetching and rendering web pages.
pub struct PageLoader {
    /// HTTP client.
    client: HttpClient,
    /// Base URL for resolving relative URLs.
    base_url: Option<Url>,
    /// Navigation history.
    history: Vec<NavigationEntry>,
    /// Current history index.
    history_index: usize,
    /// Maximum redirects.
    max_redirects: u32,
    /// Pending requests (for simulation).
    pending_data: Option<Vec<u8>>,
}

impl PageLoader {
    /// Create a new page loader.
    pub fn new() -> Self {
        Self {
            client: HttpClient::new()
                .user_agent("KPIO-Browser/0.1 (KPIO OS)")
                .max_redirects(10)
                .timeout(30000),
            base_url: None,
            history: Vec::new(),
            history_index: 0,
            max_redirects: 10,
            pending_data: None,
        }
    }
    
    /// Set the base URL for resolving relative URLs.
    pub fn set_base_url(&mut self, url: &str) -> Result<(), LoaderError> {
        self.base_url = Some(Url::parse(url)?);
        Ok(())
    }
    
    /// Resolve a URL relative to the base URL.
    pub fn resolve_url(&self, url: &str) -> Result<String, LoaderError> {
        // Check if already absolute
        if url.starts_with("http://") || url.starts_with("https://") {
            return Ok(url.to_string());
        }
        
        // Check for protocol-relative URL
        if url.starts_with("//") {
            if let Some(base) = &self.base_url {
                return Ok(format!("{}:{}", base.scheme, url));
            }
            return Ok(format!("http:{}", url));
        }
        
        // Resolve relative URL
        if let Some(base) = &self.base_url {
            if url.starts_with('/') {
                // Absolute path
                let port_str = if (base.scheme == "http" && base.port == 80) 
                    || (base.scheme == "https" && base.port == 443) {
                    String::new()
                } else {
                    format!(":{}", base.port)
                };
                Ok(format!("{}://{}{}{}", base.scheme, base.host, port_str, url))
            } else {
                // Relative path - append to current directory
                let base_path = if let Some(pos) = base.path.rfind('/') {
                    &base.path[..=pos]
                } else {
                    "/"
                };
                let port_str = if (base.scheme == "http" && base.port == 80) 
                    || (base.scheme == "https" && base.port == 443) {
                    String::new()
                } else {
                    format!(":{}", base.port)
                };
                Ok(format!("{}://{}{}{}{}", base.scheme, base.host, port_str, base_path, url))
            }
        } else {
            Err(LoaderError::InvalidUrl("No base URL set for relative URL".to_string()))
        }
    }
    
    /// Build a request for a URL.
    pub fn build_request(&self, url: &str, resource_type: ResourceType) -> Result<(Url, HttpRequest), LoaderError> {
        let parsed = Url::parse(url)?;
        
        let request = HttpRequest::get(&parsed.path_and_query())
            .host(&parsed.host_port())
            .header("Accept", resource_type.accept_header())
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept-Encoding", "identity")
            .header("Connection", "close")
            .user_agent("KPIO-Browser/0.1 (KPIO OS)");
        
        Ok((parsed, request))
    }
    
    /// Simulate receiving response data.
    /// 
    /// In a real implementation, this would use the TCP stack.
    /// For now, we provide a way to feed data for testing.
    pub fn feed_response(&mut self, data: Vec<u8>) {
        self.pending_data = Some(data);
    }
    
    /// Parse a response from pending data.
    pub fn parse_response(&mut self, url: &str) -> Result<LoadResult, LoaderError> {
        let data = self.pending_data.take()
            .ok_or_else(|| LoaderError::Network("No response data".to_string()))?;
        
        let response = HttpClient::parse_response(&data)?;
        
        let content_type = response.content_type().cloned();
        let resource_type = content_type.as_ref()
            .map(|ct| ResourceType::from_content_type(ct))
            .unwrap_or(ResourceType::Other);
        
        // Check status
        if response.status == StatusCode::NOT_FOUND {
            return Err(LoaderError::NotFound);
        } else if response.status == StatusCode::FORBIDDEN || response.status == StatusCode::UNAUTHORIZED {
            return Err(LoaderError::Forbidden);
        } else if response.status.is_server_error() {
            return Err(LoaderError::ServerError(response.status.0));
        }
        
        Ok(LoadResult {
            url: url.to_string(),
            status: response.status.0,
            content_type,
            body: response.body,
            resource_type,
        })
    }
    
    /// Add a navigation entry.
    pub fn push_history(&mut self, url: &str, title: &str) {
        // Truncate forward history if we're not at the end
        if self.history_index < self.history.len() {
            self.history.truncate(self.history_index);
        }
        
        self.history.push(NavigationEntry {
            url: url.to_string(),
            title: title.to_string(),
            scroll_x: 0,
            scroll_y: 0,
        });
        self.history_index = self.history.len();
    }
    
    /// Update scroll position for current entry.
    pub fn update_scroll(&mut self, scroll_x: i32, scroll_y: i32) {
        if self.history_index > 0 && self.history_index <= self.history.len() {
            self.history[self.history_index - 1].scroll_x = scroll_x;
            self.history[self.history_index - 1].scroll_y = scroll_y;
        }
    }
    
    /// Check if can go back.
    pub fn can_go_back(&self) -> bool {
        self.history_index > 1
    }
    
    /// Check if can go forward.
    pub fn can_go_forward(&self) -> bool {
        self.history_index < self.history.len()
    }
    
    /// Go back in history.
    pub fn go_back(&mut self) -> Option<&NavigationEntry> {
        if self.can_go_back() {
            self.history_index -= 1;
            self.history.get(self.history_index - 1)
        } else {
            None
        }
    }
    
    /// Go forward in history.
    pub fn go_forward(&mut self) -> Option<&NavigationEntry> {
        if self.can_go_forward() {
            self.history_index += 1;
            self.history.get(self.history_index - 1)
        } else {
            None
        }
    }
    
    /// Get current navigation entry.
    pub fn current(&self) -> Option<&NavigationEntry> {
        if self.history_index > 0 {
            self.history.get(self.history_index - 1)
        } else {
            None
        }
    }
    
    /// Get history length.
    pub fn history_length(&self) -> usize {
        self.history.len()
    }
    
    /// Clear history.
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.history_index = 0;
    }
}

impl Default for PageLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource cache entry.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// URL.
    pub url: String,
    /// Content type.
    pub content_type: Option<String>,
    /// Cached data.
    pub data: Vec<u8>,
    /// Cache timestamp (ticks).
    pub timestamp: u64,
    /// Max age in seconds.
    pub max_age: Option<u32>,
}

/// Simple resource cache.
pub struct ResourceCache {
    /// Cached entries.
    entries: Vec<CacheEntry>,
    /// Maximum cache size in bytes.
    max_size: usize,
    /// Current size.
    current_size: usize,
}

impl ResourceCache {
    /// Create a new resource cache.
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_size,
            current_size: 0,
        }
    }
    
    /// Get a cached entry.
    pub fn get(&self, url: &str) -> Option<&CacheEntry> {
        self.entries.iter().find(|e| e.url == url)
    }
    
    /// Store an entry in the cache.
    pub fn store(&mut self, entry: CacheEntry) {
        let size = entry.data.len();
        
        // Evict entries if needed
        while self.current_size + size > self.max_size && !self.entries.is_empty() {
            if let Some(removed) = self.entries.pop() {
                self.current_size = self.current_size.saturating_sub(removed.data.len());
            }
        }
        
        // Remove existing entry for this URL
        if let Some(pos) = self.entries.iter().position(|e| e.url == entry.url) {
            let removed = self.entries.remove(pos);
            self.current_size = self.current_size.saturating_sub(removed.data.len());
        }
        
        self.current_size += size;
        self.entries.insert(0, entry);
    }
    
    /// Remove an entry from the cache.
    pub fn remove(&mut self, url: &str) {
        if let Some(pos) = self.entries.iter().position(|e| e.url == url) {
            let removed = self.entries.remove(pos);
            self.current_size = self.current_size.saturating_sub(removed.data.len());
        }
    }
    
    /// Clear the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_size = 0;
    }
    
    /// Get current cache size.
    pub fn size(&self) -> usize {
        self.current_size
    }
    
    /// Get entry count.
    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for ResourceCache {
    fn default() -> Self {
        Self::new(16 * 1024 * 1024) // 16 MB default
    }
}

/// Document loader that coordinates loading and rendering.
pub struct DocumentLoader {
    /// Page loader.
    loader: PageLoader,
    /// Resource cache.
    cache: ResourceCache,
    /// Render pipeline.
    pipeline: RenderPipeline,
}

impl DocumentLoader {
    /// Create a new document loader.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            loader: PageLoader::new(),
            cache: ResourceCache::default(),
            pipeline: RenderPipeline::new(width as f32, height as f32),
        }
    }
    
    /// Get the render pipeline.
    pub fn pipeline(&self) -> &RenderPipeline {
        &self.pipeline
    }
    
    /// Get the render pipeline mutably.
    pub fn pipeline_mut(&mut self) -> &mut RenderPipeline {
        &mut self.pipeline
    }
    
    /// Get the page loader.
    pub fn loader(&self) -> &PageLoader {
        &self.loader
    }
    
    /// Get the page loader mutably.
    pub fn loader_mut(&mut self) -> &mut PageLoader {
        &mut self.loader
    }
    
    /// Get the resource cache.
    pub fn cache(&self) -> &ResourceCache {
        &self.cache
    }
    
    /// Get the resource cache mutably.
    pub fn cache_mut(&mut self) -> &mut ResourceCache {
        &mut self.cache
    }
    
    /// Navigate to a URL.
    /// 
    /// Returns the request data to send over the network.
    pub fn navigate(&mut self, url: &str) -> Result<(String, Vec<u8>), LoaderError> {
        let resolved = self.loader.resolve_url(url).unwrap_or_else(|_| url.to_string());
        let (parsed_url, request) = self.loader.build_request(&resolved, ResourceType::Document)?;
        
        self.loader.set_base_url(&resolved)?;
        
        Ok((resolved, request.to_bytes()))
    }
    
    /// Process a response and render the page.
    pub fn process_response(&mut self, url: &str, response_data: &[u8], framebuffer: &mut [u32]) 
        -> Result<(), LoaderError> 
    {
        self.loader.feed_response(response_data.to_vec());
        let result = self.loader.parse_response(url)?;
        
        if result.is_success() {
            // Cache the response
            self.cache.store(CacheEntry {
                url: url.to_string(),
                content_type: result.content_type.clone(),
                data: result.body.clone(),
                timestamp: 0, // Would use actual time
                max_age: None,
            });
            
            // Render the document
            if let Some(html) = result.text() {
                let width = self.pipeline.viewport_width() as u32;
                let height = self.pipeline.viewport_height() as u32;
                self.pipeline.render_to_framebuffer(&html, framebuffer, width, height)
                    .map_err(|e| LoaderError::Http(format!("Render error: {:?}", e)))?;
            }
            
            // Add to history
            self.loader.push_history(url, "Untitled"); // Would extract title from document
        }
        
        Ok(())
    }
    
    /// Render HTML directly (for testing).
    pub fn render_html(&mut self, html: &str, framebuffer: &mut [u32]) -> Result<(), LoaderError> {
        let width = self.pipeline.viewport_width() as u32;
        let height = self.pipeline.viewport_height() as u32;
        self.pipeline.render_to_framebuffer(html, framebuffer, width, height)
            .map_err(|e| LoaderError::Http(format!("Render error: {:?}", e)))
    }
    
    /// Resize the viewport.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.pipeline = RenderPipeline::new(width as f32, height as f32);
    }
}
