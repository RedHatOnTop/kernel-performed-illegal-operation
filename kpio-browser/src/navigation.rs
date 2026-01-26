//! Navigation and URL handling.
//!
//! Manages browser navigation history.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::browser::BrowserError;

/// Navigator - handles URL parsing and history.
pub struct Navigator {
    /// Navigation history.
    history: Vec<Url>,
    /// Current history index.
    current_index: isize,
}

/// Parsed URL.
#[derive(Debug, Clone)]
pub struct Url {
    /// URL scheme (http, https, file, etc.).
    pub scheme: String,
    /// Host (domain).
    pub host: String,
    /// Port (optional).
    pub port: Option<u16>,
    /// Path.
    pub path: String,
    /// Query string.
    pub query: Option<String>,
    /// Fragment (hash).
    pub fragment: Option<String>,
    /// Original URL string.
    pub original: String,
}

impl Url {
    /// Create a new URL.
    pub fn new(original: &str) -> Self {
        Self {
            scheme: String::new(),
            host: String::new(),
            port: None,
            path: String::from("/"),
            query: None,
            fragment: None,
            original: original.into(),
        }
    }
    
    /// Check if this is an HTTP URL.
    pub fn is_http(&self) -> bool {
        self.scheme == "http" || self.scheme == "https"
    }
    
    /// Check if this is a file URL.
    pub fn is_file(&self) -> bool {
        self.scheme == "file"
    }
    
    /// Check if this is a special URL (about:, data:, etc.).
    pub fn is_special(&self) -> bool {
        self.scheme == "about" || self.scheme == "data" || self.scheme == "javascript"
    }
    
    /// Get origin.
    pub fn origin(&self) -> String {
        if let Some(port) = self.port {
            alloc::format!("{}://{}:{}", self.scheme, self.host, port)
        } else {
            alloc::format!("{}://{}", self.scheme, self.host)
        }
    }
    
    /// Get full URL.
    pub fn href(&self) -> String {
        let mut url = self.origin();
        url.push_str(&self.path);
        
        if let Some(query) = &self.query {
            url.push('?');
            url.push_str(query);
        }
        
        if let Some(fragment) = &self.fragment {
            url.push('#');
            url.push_str(fragment);
        }
        
        url
    }
}

impl core::fmt::Display for Url {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.href())
    }
}

impl Navigator {
    /// Create a new navigator.
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            current_index: -1,
        }
    }
    
    /// Parse a URL string.
    pub fn parse_url(&self, url_str: &str) -> Result<Url, BrowserError> {
        let mut url = Url::new(url_str);
        let input = url_str.trim();
        
        // Check for special URLs
        if input.starts_with("about:") {
            url.scheme = String::from("about");
            url.path = input[6..].into();
            return Ok(url);
        }
        
        if input.starts_with("data:") {
            url.scheme = String::from("data");
            url.path = input[5..].into();
            return Ok(url);
        }
        
        if input.starts_with("javascript:") {
            url.scheme = String::from("javascript");
            url.path = input[11..].into();
            return Ok(url);
        }
        
        // Parse scheme
        let (scheme, rest) = if let Some(pos) = input.find("://") {
            (input[..pos].to_lowercase(), &input[pos+3..])
        } else if input.starts_with("//") {
            (String::from("https"), &input[2..])
        } else {
            // No scheme - assume https
            (String::from("https"), input)
        };
        
        url.scheme = scheme;
        
        // Parse fragment
        let (rest, fragment) = if let Some(pos) = rest.find('#') {
            (&rest[..pos], Some(rest[pos+1..].into()))
        } else {
            (rest, None)
        };
        url.fragment = fragment;
        
        // Parse query
        let (rest, query) = if let Some(pos) = rest.find('?') {
            (&rest[..pos], Some(rest[pos+1..].into()))
        } else {
            (rest, None)
        };
        url.query = query;
        
        // Parse path
        let (host_port, path) = if let Some(pos) = rest.find('/') {
            (&rest[..pos], rest[pos..].into())
        } else {
            (rest, String::from("/"))
        };
        url.path = path;
        
        // Parse host and port
        if let Some(pos) = host_port.find(':') {
            url.host = host_port[..pos].to_lowercase();
            url.port = host_port[pos+1..].parse().ok();
        } else {
            url.host = host_port.to_lowercase();
        }
        
        if url.host.is_empty() && !url.is_special() {
            return Err(BrowserError::InvalidUrl(url_str.into()));
        }
        
        Ok(url)
    }
    
    /// Push URL to history.
    pub fn push_history(&mut self, url: Url) {
        // Remove forward history
        if self.current_index >= 0 {
            self.history.truncate((self.current_index + 1) as usize);
        }
        
        self.history.push(url);
        self.current_index = self.history.len() as isize - 1;
    }
    
    /// Go back in history.
    pub fn go_back(&mut self) -> Option<Url> {
        if self.current_index > 0 {
            self.current_index -= 1;
            Some(self.history[self.current_index as usize].clone())
        } else {
            None
        }
    }
    
    /// Go forward in history.
    pub fn go_forward(&mut self) -> Option<Url> {
        if self.current_index < self.history.len() as isize - 1 {
            self.current_index += 1;
            Some(self.history[self.current_index as usize].clone())
        } else {
            None
        }
    }
    
    /// Get current URL.
    pub fn current_url(&self) -> Option<Url> {
        if self.current_index >= 0 && self.current_index < self.history.len() as isize {
            Some(self.history[self.current_index as usize].clone())
        } else {
            None
        }
    }
    
    /// Can go back?
    pub fn can_go_back(&self) -> bool {
        self.current_index > 0
    }
    
    /// Can go forward?
    pub fn can_go_forward(&self) -> bool {
        self.current_index < self.history.len() as isize - 1
    }
    
    /// Get history length.
    pub fn history_length(&self) -> usize {
        self.history.len()
    }
    
    /// Clear history.
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.current_index = -1;
    }
}

impl Default for Navigator {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve a relative URL against a base URL.
pub fn resolve_url(base: &Url, relative: &str) -> Url {
    if relative.contains("://") {
        // Absolute URL
        Navigator::new().parse_url(relative).unwrap_or_else(|_| Url::new(relative))
    } else if relative.starts_with("//") {
        // Protocol-relative
        let url_str = alloc::format!("{}:{}", base.scheme, relative);
        Navigator::new().parse_url(&url_str).unwrap_or_else(|_| Url::new(relative))
    } else if relative.starts_with('/') {
        // Absolute path
        let mut url = base.clone();
        url.path = relative.into();
        url.query = None;
        url.fragment = None;
        url
    } else if relative.starts_with('?') {
        // Query only
        let mut url = base.clone();
        url.query = Some(relative[1..].into());
        url.fragment = None;
        url
    } else if relative.starts_with('#') {
        // Fragment only
        let mut url = base.clone();
        url.fragment = Some(relative[1..].into());
        url
    } else {
        // Relative path
        let mut url = base.clone();
        let base_path = if let Some(pos) = base.path.rfind('/') {
            &base.path[..=pos]
        } else {
            "/"
        };
        url.path = alloc::format!("{}{}", base_path, relative);
        url.query = None;
        url.fragment = None;
        url
    }
}
