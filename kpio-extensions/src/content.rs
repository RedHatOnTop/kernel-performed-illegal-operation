//! Content Script Injection
//!
//! Handles content script matching, injection, and isolated world management.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;
use spin::RwLock;

use crate::ExtensionId;
use crate::manifest::{ContentScript, RunAt};

/// Frame ID type.
pub type FrameId = i32;

/// World ID for isolated worlds.
pub type WorldId = u32;

/// Script execution result.
#[derive(Debug, Clone)]
pub struct InjectionResult {
    /// Frame ID.
    pub frame_id: FrameId,
    /// Execution result (JSON).
    pub result: Option<String>,
    /// Error if any.
    pub error: Option<String>,
}

/// Script injection target.
#[derive(Debug, Clone)]
pub struct InjectionTarget {
    /// Tab ID.
    pub tab_id: u32,
    /// Frame IDs (if empty, all frames).
    pub frame_ids: Option<Vec<FrameId>>,
    /// All frames.
    pub all_frames: bool,
    /// Document IDs.
    pub document_ids: Option<Vec<String>>,
}

/// Script injection details.
#[derive(Debug, Clone)]
pub struct ScriptInjection {
    /// Target.
    pub target: InjectionTarget,
    /// Files to inject.
    pub files: Option<Vec<String>>,
    /// Inline code to inject.
    pub func: Option<String>,
    /// Arguments for function.
    pub args: Option<Vec<String>>,
    /// World to inject into.
    pub world: ExecutionWorld,
    /// Inject immediately without waiting for DOM.
    pub inject_immediately: bool,
}

/// CSS injection details.
#[derive(Debug, Clone)]
pub struct CssInjection {
    /// Target.
    pub target: InjectionTarget,
    /// CSS files.
    pub files: Option<Vec<String>>,
    /// Inline CSS.
    pub css: Option<String>,
    /// CSS origin.
    pub origin: CssOrigin,
}

/// CSS origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssOrigin {
    Author,
    User,
}

impl Default for CssOrigin {
    fn default() -> Self {
        Self::Author
    }
}

/// Execution world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionWorld {
    /// Main world (page context).
    Main,
    /// Isolated world (extension context).
    Isolated,
}

impl Default for ExecutionWorld {
    fn default() -> Self {
        Self::Isolated
    }
}

/// Match pattern for URL matching.
#[derive(Debug, Clone)]
pub struct MatchPattern {
    /// Original pattern string.
    pattern: String,
    /// Scheme part.
    scheme: MatchScheme,
    /// Host part.
    host: MatchHost,
    /// Path part.
    path: String,
}

/// Match scheme.
#[derive(Debug, Clone)]
enum MatchScheme {
    /// Match any scheme.
    All,
    /// Specific scheme.
    Specific(String),
}

/// Match host.
#[derive(Debug, Clone)]
enum MatchHost {
    /// Match all hosts.
    All,
    /// Match specific host.
    Specific(String),
    /// Match host suffix (*.example.com).
    Suffix(String),
}

impl MatchPattern {
    /// Parse a match pattern.
    pub fn parse(pattern: &str) -> Option<Self> {
        if pattern == "<all_urls>" {
            return Some(Self {
                pattern: pattern.to_string(),
                scheme: MatchScheme::All,
                host: MatchHost::All,
                path: "/*".to_string(),
            });
        }
        
        // Parse scheme://host/path
        let scheme_end = pattern.find("://")?;
        let scheme_str = &pattern[..scheme_end];
        let rest = &pattern[scheme_end + 3..];
        
        let scheme = if scheme_str == "*" {
            MatchScheme::All
        } else {
            MatchScheme::Specific(scheme_str.to_string())
        };
        
        // Find path separator
        let path_start = rest.find('/').unwrap_or(rest.len());
        let host_str = &rest[..path_start];
        let path = if path_start < rest.len() {
            rest[path_start..].to_string()
        } else {
            "/*".to_string()
        };
        
        let host = if host_str == "*" {
            MatchHost::All
        } else if host_str.starts_with("*.") {
            MatchHost::Suffix(host_str[2..].to_string())
        } else {
            MatchHost::Specific(host_str.to_string())
        };
        
        Some(Self {
            pattern: pattern.to_string(),
            scheme,
            host,
            path,
        })
    }
    
    /// Test if URL matches this pattern.
    pub fn matches(&self, url: &str) -> bool {
        // Parse URL
        let scheme_end = match url.find("://") {
            Some(pos) => pos,
            None => return false,
        };
        let url_scheme = &url[..scheme_end];
        let rest = &url[scheme_end + 3..];
        
        // Check scheme
        match &self.scheme {
            MatchScheme::All => {
                // * matches http, https, file, ftp
                if !matches!(url_scheme, "http" | "https" | "file" | "ftp") {
                    return false;
                }
            }
            MatchScheme::Specific(s) => {
                if url_scheme != s {
                    return false;
                }
            }
        }
        
        // Extract host and path
        let path_start = rest.find('/').unwrap_or(rest.len());
        let url_host = &rest[..path_start];
        let url_path = if path_start < rest.len() {
            &rest[path_start..]
        } else {
            "/"
        };
        
        // Check host
        match &self.host {
            MatchHost::All => {}
            MatchHost::Specific(h) => {
                if url_host != h {
                    return false;
                }
            }
            MatchHost::Suffix(suffix) => {
                if url_host != suffix && !url_host.ends_with(&alloc::format!(".{}", suffix)) {
                    return false;
                }
            }
        }
        
        // Check path (simple glob matching)
        self.path_matches(url_path, &self.path)
    }
    
    /// Simple path glob matching.
    fn path_matches(&self, url_path: &str, pattern: &str) -> bool {
        if pattern == "/*" || pattern == "*" {
            return true;
        }
        
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.is_empty() {
            return url_path == pattern;
        }
        
        let mut pos = 0;
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            if let Some(found) = url_path[pos..].find(part) {
                if i == 0 && found != 0 {
                    return false;
                }
                pos += found + part.len();
            } else {
                return false;
            }
        }
        
        // If pattern doesn't end with *, path must end at current position
        if !pattern.ends_with('*') && pos != url_path.len() {
            return false;
        }
        
        true
    }
}

/// Registered content script.
#[derive(Debug, Clone)]
pub struct RegisteredScript {
    /// Script ID.
    pub id: String,
    /// Match patterns.
    pub matches: Vec<MatchPattern>,
    /// Exclude matches.
    pub exclude_matches: Vec<MatchPattern>,
    /// Include globs.
    pub include_globs: Vec<String>,
    /// Exclude globs.
    pub exclude_globs: Vec<String>,
    /// JavaScript files.
    pub js: Vec<String>,
    /// CSS files.
    pub css: Vec<String>,
    /// Run at.
    pub run_at: RunAt,
    /// All frames.
    pub all_frames: bool,
    /// Match about:blank.
    pub match_about_blank: bool,
    /// Match origin as fallback.
    pub match_origin_as_fallback: bool,
    /// Execution world.
    pub world: ExecutionWorld,
    /// Persist across sessions.
    pub persist_across_sessions: bool,
}

impl RegisteredScript {
    /// Create from manifest content script.
    pub fn from_manifest(id: &str, cs: &ContentScript) -> Self {
        let matches: Vec<MatchPattern> = cs.matches.iter()
            .filter_map(|p| MatchPattern::parse(p))
            .collect();
        
        let exclude_matches: Vec<MatchPattern> = cs.exclude_matches.iter()
            .filter_map(|p| MatchPattern::parse(p))
            .collect();
        
        Self {
            id: id.to_string(),
            matches,
            exclude_matches,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            js: cs.js.clone(),
            css: cs.css.clone(),
            run_at: cs.run_at.clone(),
            all_frames: cs.all_frames,
            match_about_blank: cs.match_about_blank,
            match_origin_as_fallback: cs.match_origin_as_fallback,
            world: ExecutionWorld::Isolated,
            persist_across_sessions: true,
        }
    }
    
    /// Check if script should be injected for URL.
    pub fn should_inject(&self, url: &str) -> bool {
        // Check exclude first
        for pattern in &self.exclude_matches {
            if pattern.matches(url) {
                return false;
            }
        }
        
        // Check include
        for pattern in &self.matches {
            if pattern.matches(url) {
                return true;
            }
        }
        
        false
    }
}

/// Content script manager.
pub struct ContentScriptManager {
    /// Registered scripts by extension.
    scripts: RwLock<BTreeMap<ExtensionId, Vec<RegisteredScript>>>,
    /// Dynamic scripts.
    dynamic_scripts: RwLock<BTreeMap<String, RegisteredScript>>,
    /// Next world ID.
    next_world_id: RwLock<WorldId>,
    /// Extension world IDs.
    extension_worlds: RwLock<BTreeMap<ExtensionId, WorldId>>,
}

impl ContentScriptManager {
    /// Create a new content script manager.
    pub fn new() -> Self {
        Self {
            scripts: RwLock::new(BTreeMap::new()),
            dynamic_scripts: RwLock::new(BTreeMap::new()),
            next_world_id: RwLock::new(1),
            extension_worlds: RwLock::new(BTreeMap::new()),
        }
    }
    
    /// Register extension content scripts from manifest.
    pub fn register_extension(&self, extension_id: ExtensionId, content_scripts: Vec<ContentScript>) {
        let scripts: Vec<RegisteredScript> = content_scripts.iter()
            .enumerate()
            .map(|(i, cs)| RegisteredScript::from_manifest(&alloc::format!("manifest_{}", i), cs))
            .collect();
        
        // Allocate isolated world
        if !scripts.is_empty() {
            let mut next_id = self.next_world_id.write();
            self.extension_worlds.write().insert(extension_id.clone(), *next_id);
            *next_id += 1;
        }
        
        self.scripts.write().insert(extension_id, scripts);
    }
    
    /// Unregister extension.
    pub fn unregister_extension(&self, extension_id: &ExtensionId) {
        self.scripts.write().remove(extension_id);
        self.extension_worlds.write().remove(extension_id);
    }
    
    /// Register dynamic script.
    pub fn register_script(&self, extension_id: &ExtensionId, script: RegisteredScript) -> String {
        let id = alloc::format!("{}_{}", extension_id.as_str(), script.id);
        self.dynamic_scripts.write().insert(id.clone(), script);
        id
    }
    
    /// Unregister dynamic script.
    pub fn unregister_script(&self, script_id: &str) {
        self.dynamic_scripts.write().remove(script_id);
    }
    
    /// Get scripts to inject for URL.
    pub fn get_scripts_for_url(&self, url: &str) -> Vec<(ExtensionId, RegisteredScript)> {
        let mut results = Vec::new();
        
        // Check manifest scripts
        let scripts = self.scripts.read();
        for (ext_id, ext_scripts) in scripts.iter() {
            for script in ext_scripts {
                if script.should_inject(url) {
                    results.push((ext_id.clone(), script.clone()));
                }
            }
        }
        
        // Check dynamic scripts
        let dynamic = self.dynamic_scripts.read();
        for (_id, script) in dynamic.iter() {
            if script.should_inject(url) {
                // Extract extension ID from script ID
                // This is simplified
                results.push((ExtensionId::new("dynamic"), script.clone()));
            }
        }
        
        results
    }
    
    /// Get world ID for extension.
    pub fn get_world_id(&self, extension_id: &ExtensionId) -> Option<WorldId> {
        self.extension_worlds.read().get(extension_id).copied()
    }
    
    /// Inject script into frame.
    pub fn inject_script(
        &self,
        extension_id: &ExtensionId,
        injection: ScriptInjection,
    ) -> Vec<InjectionResult> {
        // Would perform actual injection through browser APIs
        vec![InjectionResult {
            frame_id: 0,
            result: Some("undefined".to_string()),
            error: None,
        }]
    }
    
    /// Inject CSS into frame.
    pub fn inject_css(
        &self,
        extension_id: &ExtensionId,
        injection: CssInjection,
    ) -> Vec<InjectionResult> {
        // Would perform actual CSS injection
        vec![InjectionResult {
            frame_id: 0,
            result: None,
            error: None,
        }]
    }
    
    /// Remove CSS from frame.
    pub fn remove_css(
        &self,
        extension_id: &ExtensionId,
        injection: CssInjection,
    ) -> Vec<InjectionResult> {
        // Would remove injected CSS
        vec![InjectionResult {
            frame_id: 0,
            result: None,
            error: None,
        }]
    }
}

impl Default for ContentScriptManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a URL matches a pattern (public interface for Extension::has_host_permission).
pub fn match_pattern(pattern: &str, url: &str) -> bool {
    MatchPattern::parse(pattern)
        .map(|p| p.matches(url))
        .unwrap_or(false)
}

/// Global globs matching.
pub fn matches_glob(text: &str, pattern: &str) -> bool {
    let mut text_chars = text.chars().peekable();
    let mut pattern_chars = pattern.chars().peekable();
    
    while let Some(p) = pattern_chars.next() {
        match p {
            '*' => {
                // Match zero or more characters
                if pattern_chars.peek().is_none() {
                    return true; // * at end matches everything
                }
                
                // Try matching rest of pattern at each position
                let remaining: String = pattern_chars.collect();
                while text_chars.peek().is_some() {
                    let remaining_text: String = text_chars.clone().collect();
                    if matches_glob(&remaining_text, &remaining) {
                        return true;
                    }
                    text_chars.next();
                }
                return matches_glob("", &remaining);
            }
            '?' => {
                // Match exactly one character
                if text_chars.next().is_none() {
                    return false;
                }
            }
            c => {
                // Match literal character
                if text_chars.next() != Some(c) {
                    return false;
                }
            }
        }
    }
    
    // Pattern consumed, text should also be consumed
    text_chars.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_match_pattern() {
        // All URLs
        let pattern = MatchPattern::parse("<all_urls>").unwrap();
        assert!(pattern.matches("https://example.com/path"));
        assert!(pattern.matches("http://test.org/"));
        
        // Specific host
        let pattern = MatchPattern::parse("https://example.com/*").unwrap();
        assert!(pattern.matches("https://example.com/path"));
        assert!(pattern.matches("https://example.com/"));
        assert!(!pattern.matches("https://other.com/path"));
        assert!(!pattern.matches("http://example.com/path"));
        
        // Wildcard scheme
        let pattern = MatchPattern::parse("*://example.com/*").unwrap();
        assert!(pattern.matches("https://example.com/path"));
        assert!(pattern.matches("http://example.com/path"));
        
        // Subdomain wildcard
        let pattern = MatchPattern::parse("*://*.example.com/*").unwrap();
        assert!(pattern.matches("https://sub.example.com/path"));
        assert!(pattern.matches("https://example.com/path"));
        assert!(!pattern.matches("https://notexample.com/path"));
    }
    
    #[test]
    fn test_glob_matching() {
        assert!(matches_glob("hello", "hello"));
        assert!(matches_glob("hello", "h*o"));
        assert!(matches_glob("hello", "*"));
        assert!(matches_glob("hello", "h?llo"));
        assert!(matches_glob("hello", "h*l*o"));
        assert!(!matches_glob("hello", "world"));
        assert!(!matches_glob("hello", "h?o"));
    }
    
    #[test]
    fn test_content_script_manager() {
        use crate::manifest::ContentScriptWorld;
        
        let manager = ContentScriptManager::new();
        
        let cs = ContentScript {
            matches: vec!["*://example.com/*".to_string()],
            exclude_matches: Vec::new(),
            js: vec!["content.js".to_string()],
            css: Vec::new(),
            run_at: RunAt::DocumentIdle,
            all_frames: false,
            match_about_blank: false,
            match_origin_as_fallback: false,
            world: ContentScriptWorld::Isolated,
        };
        
        let ext_id = ExtensionId::new("test");
        manager.register_extension(ext_id.clone(), vec![cs]);
        
        let scripts = manager.get_scripts_for_url("https://example.com/page");
        assert_eq!(scripts.len(), 1);
        
        let scripts = manager.get_scripts_for_url("https://other.com/page");
        assert!(scripts.is_empty());
        
        manager.unregister_extension(&ext_id);
    }
}
