//! Origin Isolation
//!
//! Implements site isolation to ensure each origin runs in a separate process,
//! providing strong security boundaries between different websites.
//!
//! # Security Features
//!
//! - **Site Isolation**: Each origin gets its own process
//! - **CORB**: Cross-Origin Read Blocking
//! - **COOP**: Cross-Origin Opener Policy
//! - **COEP**: Cross-Origin Embedder Policy
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────┐
//! │                    Origin Isolation Manager                       │
//! ├──────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
//! │  │  Origin: A.com  │  │  Origin: B.com  │  │  Origin: C.com  │  │
//! │  │  Process: 100   │  │  Process: 101   │  │  Process: 102   │  │
//! │  │  ┌───────────┐  │  │  ┌───────────┐  │  │  ┌───────────┐  │  │
//! │  │  │ Renderer  │  │  │  │ Renderer  │  │  │  │ Renderer  │  │  │
//! │  │  └───────────┘  │  │  └───────────┘  │  │  └───────────┘  │  │
//! │  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
//! │           │                   │                   │             │
//! │           └───────────────────┼───────────────────┘             │
//! │                               │                                 │
//! │                    ┌──────────▼──────────┐                      │
//! │                    │  Cross-Origin IPC   │                      │
//! │                    │   (message-only)    │                      │
//! │                    └─────────────────────┘                      │
//! │                                                                  │
//! └──────────────────────────────────────────────────────────────────┘
//! ```

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use core::sync::atomic::{AtomicU64, Ordering};
use spin::{Mutex, RwLock};

use crate::process::ProcessId;

/// Origin identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Origin {
    /// Scheme (http, https, file, etc.).
    pub scheme: String,
    /// Host (domain name or IP).
    pub host: String,
    /// Port (default ports may be None).
    pub port: Option<u16>,
}

impl Origin {
    /// Create a new origin.
    pub fn new(scheme: &str, host: &str, port: Option<u16>) -> Self {
        Self {
            scheme: scheme.to_string(),
            host: host.to_string(),
            port,
        }
    }

    /// Parse an origin from a URL string.
    pub fn parse(url: &str) -> Option<Self> {
        // Simple URL parsing (scheme://host:port/path)
        let (scheme, rest) = url.split_once("://")?;
        let host_port = rest.split('/').next()?;

        let (host, port) = if let Some((h, p)) = host_port.split_once(':') {
            (h.to_string(), p.parse().ok())
        } else {
            (host_port.to_string(), Self::default_port(scheme))
        };

        Some(Self {
            scheme: scheme.to_string(),
            host,
            port,
        })
    }

    /// Get default port for scheme.
    fn default_port(scheme: &str) -> Option<u16> {
        match scheme {
            "http" => Some(80),
            "https" => Some(443),
            "ftp" => Some(21),
            _ => None,
        }
    }

    /// Check if origin is opaque (cannot be compared).
    pub fn is_opaque(&self) -> bool {
        matches!(self.scheme.as_str(), "data" | "javascript" | "blob")
    }

    /// Get origin as a string for serialization.
    pub fn serialize(&self) -> String {
        match self.port {
            Some(port) if Some(port) != Self::default_port(&self.scheme) => {
                alloc::format!("{}://{}:{}", self.scheme, self.host, port)
            }
            _ => alloc::format!("{}://{}", self.scheme, self.host),
        }
    }

    /// Check if this origin is same-site with another.
    pub fn is_same_site(&self, other: &Origin) -> bool {
        // Same-site includes same host and subdomains
        if self.scheme != other.scheme {
            return false;
        }

        // Check if same host or subdomain
        if self.host == other.host {
            return true;
        }

        // Check if one is subdomain of the other
        let (shorter, longer) = if self.host.len() < other.host.len() {
            (&self.host, &other.host)
        } else {
            (&other.host, &self.host)
        };

        longer.ends_with(&alloc::format!(".{}", shorter))
    }

    /// Check if this origin can access another (same-origin policy).
    pub fn can_access(&self, other: &Origin) -> bool {
        self.scheme == other.scheme && self.host == other.host && self.port == other.port
    }
}

/// Site isolation context ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SiteId(pub u64);

/// Cross-Origin Opener Policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoopPolicy {
    /// No restrictions.
    UnsafeNone,
    /// Same origin only.
    SameOrigin,
    /// Same origin but allow popups.
    SameOriginAllowPopups,
}

impl Default for CoopPolicy {
    fn default() -> Self {
        Self::UnsafeNone
    }
}

/// Cross-Origin Embedder Policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoepPolicy {
    /// No restrictions.
    UnsafeNone,
    /// Require CORS or CORP.
    RequireCorp,
    /// Credentialless (no cookies for cross-origin).
    Credentialless,
}

impl Default for CoepPolicy {
    fn default() -> Self {
        Self::UnsafeNone
    }
}

/// Cross-Origin Resource Policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpPolicy {
    /// Cross-origin resources allowed.
    CrossOrigin,
    /// Same-site only.
    SameSite,
    /// Same-origin only.
    SameOrigin,
}

impl Default for CorpPolicy {
    fn default() -> Self {
        Self::CrossOrigin
    }
}

/// Site isolation context.
#[derive(Debug)]
pub struct SiteContext {
    /// Site ID.
    pub id: SiteId,
    /// Origin.
    pub origin: Origin,
    /// Process ID.
    pub process: ProcessId,
    /// Cross-Origin Opener Policy.
    pub coop: CoopPolicy,
    /// Cross-Origin Embedder Policy.
    pub coep: CoepPolicy,
    /// Cross-origin isolation enabled.
    pub cross_origin_isolated: bool,
    /// Child frames (for out-of-process iframes).
    pub child_frames: Vec<SiteId>,
    /// Parent frame (if any).
    pub parent: Option<SiteId>,
}

impl SiteContext {
    /// Create a new site context.
    pub fn new(id: SiteId, origin: Origin, process: ProcessId) -> Self {
        Self {
            id,
            origin,
            process,
            coop: CoopPolicy::default(),
            coep: CoepPolicy::default(),
            cross_origin_isolated: false,
            child_frames: Vec::new(),
            parent: None,
        }
    }

    /// Check if cross-origin isolation is enabled.
    /// Requires COOP: same-origin and COEP: require-corp.
    pub fn update_cross_origin_isolation(&mut self) {
        self.cross_origin_isolated =
            self.coop == CoopPolicy::SameOrigin && self.coep == CoepPolicy::RequireCorp;
    }

    /// Set COOP policy from header value.
    pub fn set_coop(&mut self, value: &str) {
        self.coop = match value.trim() {
            "same-origin" => CoopPolicy::SameOrigin,
            "same-origin-allow-popups" => CoopPolicy::SameOriginAllowPopups,
            _ => CoopPolicy::UnsafeNone,
        };
        self.update_cross_origin_isolation();
    }

    /// Set COEP policy from header value.
    pub fn set_coep(&mut self, value: &str) {
        self.coep = match value.trim() {
            "require-corp" => CoepPolicy::RequireCorp,
            "credentialless" => CoepPolicy::Credentialless,
            _ => CoepPolicy::UnsafeNone,
        };
        self.update_cross_origin_isolation();
    }
}

/// Cross-Origin Read Blocking (CORB) result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorbResult {
    /// Allow the response.
    Allow,
    /// Block and replace with empty response.
    Block,
    /// Sniff content to determine if blocking needed.
    Sniff,
}

/// CORB content type detector.
pub struct CorbChecker {
    /// Known HTML content type patterns.
    html_patterns: Vec<&'static [u8]>,
    /// Known XML content type patterns.
    xml_patterns: Vec<&'static [u8]>,
    /// Known JSON content type patterns.
    json_patterns: Vec<&'static [u8]>,
}

impl CorbChecker {
    /// Create a new CORB checker.
    pub fn new() -> Self {
        Self {
            html_patterns: alloc::vec![b"<!DOCTYPE", b"<html", b"<HTML", b"<script", b"<SCRIPT",],
            xml_patterns: alloc::vec![b"<?xml", b"<rss",],
            json_patterns: alloc::vec![b"{", b"[",],
        }
    }

    /// Check if response should be blocked.
    pub fn check(
        &self,
        request_origin: &Origin,
        response_origin: &Origin,
        content_type: Option<&str>,
        content: &[u8],
    ) -> CorbResult {
        // Same-origin requests are always allowed
        if request_origin.can_access(response_origin) {
            return CorbResult::Allow;
        }

        // Check content type
        if let Some(ct) = content_type {
            let ct_lower = ct.to_lowercase();

            // Block sensitive content types
            if ct_lower.contains("text/html")
                || ct_lower.contains("application/json")
                || ct_lower.contains("text/xml")
                || ct_lower.contains("application/xml")
            {
                return CorbResult::Block;
            }

            // Allow known safe types
            if ct_lower.contains("image/")
                || ct_lower.contains("audio/")
                || ct_lower.contains("video/")
                || ct_lower.contains("text/css")
                || ct_lower.contains("text/javascript")
                || ct_lower.contains("application/javascript")
            {
                return CorbResult::Allow;
            }
        }

        // Sniff content for sensitive data
        if self.sniff_html(content) || self.sniff_xml(content) || self.sniff_json(content) {
            return CorbResult::Block;
        }

        CorbResult::Allow
    }

    /// Sniff for HTML content.
    fn sniff_html(&self, content: &[u8]) -> bool {
        let content = self.skip_whitespace(content);
        for pattern in &self.html_patterns {
            if content.starts_with(pattern) {
                return true;
            }
        }
        false
    }

    /// Sniff for XML content.
    fn sniff_xml(&self, content: &[u8]) -> bool {
        let content = self.skip_whitespace(content);
        for pattern in &self.xml_patterns {
            if content.starts_with(pattern) {
                return true;
            }
        }
        false
    }

    /// Sniff for JSON content.
    fn sniff_json(&self, content: &[u8]) -> bool {
        let content = self.skip_whitespace(content);
        for pattern in &self.json_patterns {
            if content.starts_with(pattern) {
                return true;
            }
        }
        false
    }

    /// Skip leading whitespace and BOM.
    fn skip_whitespace<'a>(&self, content: &'a [u8]) -> &'a [u8] {
        let mut start = 0;

        // Skip UTF-8 BOM
        if content.starts_with(&[0xEF, 0xBB, 0xBF]) {
            start = 3;
        }

        // Skip whitespace
        while start < content.len() {
            match content[start] {
                b' ' | b'\t' | b'\n' | b'\r' => start += 1,
                _ => break,
            }
        }

        &content[start..]
    }
}

impl Default for CorbChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Origin isolation manager.
pub struct OriginIsolationManager {
    /// Site contexts by ID.
    sites: BTreeMap<SiteId, SiteContext>,
    /// Origin to site ID mapping.
    origin_map: BTreeMap<Origin, SiteId>,
    /// Process to site ID mapping.
    process_map: BTreeMap<ProcessId, Vec<SiteId>>,
    /// Next site ID.
    next_id: AtomicU64,
    /// CORB checker.
    corb: CorbChecker,
}

impl OriginIsolationManager {
    /// Create a new origin isolation manager.
    pub fn new() -> Self {
        Self {
            sites: BTreeMap::new(),
            origin_map: BTreeMap::new(),
            process_map: BTreeMap::new(),
            next_id: AtomicU64::new(1),
            corb: CorbChecker::new(),
        }
    }

    /// Create or get site context for origin.
    pub fn get_or_create_site(&mut self, origin: Origin, process: ProcessId) -> SiteId {
        if let Some(&id) = self.origin_map.get(&origin) {
            return id;
        }

        let id = SiteId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let context = SiteContext::new(id, origin.clone(), process);

        self.sites.insert(id, context);
        self.origin_map.insert(origin, id);
        self.process_map.entry(process).or_default().push(id);

        id
    }

    /// Get site context.
    pub fn get_site(&self, id: SiteId) -> Option<&SiteContext> {
        self.sites.get(&id)
    }

    /// Get mutable site context.
    pub fn get_site_mut(&mut self, id: SiteId) -> Option<&mut SiteContext> {
        self.sites.get_mut(&id)
    }

    /// Get site ID for origin.
    pub fn site_for_origin(&self, origin: &Origin) -> Option<SiteId> {
        self.origin_map.get(origin).copied()
    }

    /// Check if cross-origin navigation is allowed (COOP check).
    pub fn can_navigate(&self, from: SiteId, to_origin: &Origin) -> NavigationDecision {
        let from_site = match self.sites.get(&from) {
            Some(s) => s,
            None => return NavigationDecision::Allow,
        };

        // Same origin - always allowed
        if from_site.origin.can_access(to_origin) {
            return NavigationDecision::Allow;
        }

        // Check COOP policy
        match from_site.coop {
            CoopPolicy::UnsafeNone => NavigationDecision::Allow,
            CoopPolicy::SameOrigin => NavigationDecision::NewBrowsingContextGroup,
            CoopPolicy::SameOriginAllowPopups => {
                if from_site.origin.is_same_site(to_origin) {
                    NavigationDecision::Allow
                } else {
                    NavigationDecision::NewBrowsingContextGroup
                }
            }
        }
    }

    /// Check if resource can be embedded (COEP/CORP check).
    pub fn can_embed(
        &self,
        embedder: SiteId,
        resource_origin: &Origin,
        resource_corp: CorpPolicy,
    ) -> bool {
        let embedder_site = match self.sites.get(&embedder) {
            Some(s) => s,
            None => return true,
        };

        // Same origin - always allowed
        if embedder_site.origin.can_access(resource_origin) {
            return true;
        }

        // Check COEP policy
        match embedder_site.coep {
            CoepPolicy::UnsafeNone => true,
            CoepPolicy::RequireCorp => {
                // Resource must have CORP: cross-origin or same-site
                match resource_corp {
                    CorpPolicy::CrossOrigin => true,
                    CorpPolicy::SameSite => embedder_site.origin.is_same_site(resource_origin),
                    CorpPolicy::SameOrigin => embedder_site.origin.can_access(resource_origin),
                }
            }
            CoepPolicy::Credentialless => {
                // Allow but without credentials
                true
            }
        }
    }

    /// Perform CORB check.
    pub fn corb_check(
        &self,
        request_site: SiteId,
        response_origin: &Origin,
        content_type: Option<&str>,
        content: &[u8],
    ) -> CorbResult {
        let request_origin = match self.sites.get(&request_site) {
            Some(s) => &s.origin,
            None => return CorbResult::Allow,
        };

        self.corb
            .check(request_origin, response_origin, content_type, content)
    }

    /// Add child frame.
    pub fn add_child_frame(&mut self, parent: SiteId, child: SiteId) {
        if let Some(parent_site) = self.sites.get_mut(&parent) {
            parent_site.child_frames.push(child);
        }
        if let Some(child_site) = self.sites.get_mut(&child) {
            child_site.parent = Some(parent);
        }
    }

    /// Remove site.
    pub fn remove_site(&mut self, id: SiteId) {
        if let Some(site) = self.sites.remove(&id) {
            self.origin_map.remove(&site.origin);

            if let Some(sites) = self.process_map.get_mut(&site.process) {
                sites.retain(|&s| s != id);
            }

            // Remove from parent's children
            if let Some(parent_id) = site.parent {
                if let Some(parent) = self.sites.get_mut(&parent_id) {
                    parent.child_frames.retain(|&c| c != id);
                }
            }
        }
    }

    /// Get all sites for process.
    pub fn sites_for_process(&self, process: ProcessId) -> &[SiteId] {
        self.process_map
            .get(&process)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}

impl Default for OriginIsolationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Navigation decision result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationDecision {
    /// Allow navigation in same browsing context.
    Allow,
    /// Navigation requires new browsing context group.
    NewBrowsingContextGroup,
    /// Block navigation.
    Block,
}

/// Global origin isolation manager.
static ORIGIN_MANAGER: RwLock<Option<OriginIsolationManager>> = RwLock::new(None);

/// Initialize origin isolation.
pub fn init() {
    let mut mgr = ORIGIN_MANAGER.write();
    *mgr = Some(OriginIsolationManager::new());
    crate::serial_println!("[Origin] Isolation manager initialized");
}

/// Get or create site for origin.
pub fn get_or_create_site(origin: Origin, process: ProcessId) -> Option<SiteId> {
    ORIGIN_MANAGER
        .write()
        .as_mut()
        .map(|m| m.get_or_create_site(origin, process))
}

/// Check if navigation is allowed.
pub fn can_navigate(from: SiteId, to_origin: &Origin) -> NavigationDecision {
    ORIGIN_MANAGER
        .read()
        .as_ref()
        .map(|m| m.can_navigate(from, to_origin))
        .unwrap_or(NavigationDecision::Allow)
}

/// Perform CORB check.
pub fn corb_check(
    request_site: SiteId,
    response_origin: &Origin,
    content_type: Option<&str>,
    content: &[u8],
) -> CorbResult {
    ORIGIN_MANAGER
        .read()
        .as_ref()
        .map(|m| m.corb_check(request_site, response_origin, content_type, content))
        .unwrap_or(CorbResult::Allow)
}
