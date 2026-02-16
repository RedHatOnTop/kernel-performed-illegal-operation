//! chrome.webRequest API
//!
//! Provides network request interception and modification.

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

use super::{ApiContext, ApiError, ApiResult, EventEmitter};
use crate::api::tabs::TabId;

/// Request ID.
pub type RequestId = String;

/// Request filter.
#[derive(Debug, Clone)]
pub struct RequestFilter {
    /// URL patterns to match.
    pub urls: Vec<String>,
    /// Types of resources to match.
    pub types: Option<Vec<ResourceType>>,
    /// Tab ID to match.
    pub tab_id: Option<TabId>,
    /// Window ID to match.
    pub window_id: Option<i32>,
}

impl Default for RequestFilter {
    fn default() -> Self {
        Self {
            urls: Vec::new(),
            types: None,
            tab_id: None,
            window_id: None,
        }
    }
}

/// Resource type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    MainFrame,
    SubFrame,
    Stylesheet,
    Script,
    Image,
    Font,
    Object,
    XmlHttpRequest,
    Ping,
    CspReport,
    Media,
    WebSocket,
    WebTransport,
    Webbundle,
    Other,
}

impl ResourceType {
    /// Parse from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "main_frame" => Some(Self::MainFrame),
            "sub_frame" => Some(Self::SubFrame),
            "stylesheet" => Some(Self::Stylesheet),
            "script" => Some(Self::Script),
            "image" => Some(Self::Image),
            "font" => Some(Self::Font),
            "object" => Some(Self::Object),
            "xmlhttprequest" => Some(Self::XmlHttpRequest),
            "ping" => Some(Self::Ping),
            "csp_report" => Some(Self::CspReport),
            "media" => Some(Self::Media),
            "websocket" => Some(Self::WebSocket),
            "webtransport" => Some(Self::WebTransport),
            "webbundle" => Some(Self::Webbundle),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    /// Convert to string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MainFrame => "main_frame",
            Self::SubFrame => "sub_frame",
            Self::Stylesheet => "stylesheet",
            Self::Script => "script",
            Self::Image => "image",
            Self::Font => "font",
            Self::Object => "object",
            Self::XmlHttpRequest => "xmlhttprequest",
            Self::Ping => "ping",
            Self::CspReport => "csp_report",
            Self::Media => "media",
            Self::WebSocket => "websocket",
            Self::WebTransport => "webtransport",
            Self::Webbundle => "webbundle",
            Self::Other => "other",
        }
    }
}

/// HTTP header.
#[derive(Debug, Clone)]
pub struct HttpHeader {
    /// Header name.
    pub name: String,
    /// Header value.
    pub value: Option<String>,
    /// Binary value.
    pub binary_value: Option<Vec<u8>>,
}

impl HttpHeader {
    /// Create a new header.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: Some(value.into()),
            binary_value: None,
        }
    }

    /// Create a header with binary value.
    pub fn new_binary(name: impl Into<String>, binary: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            value: None,
            binary_value: Some(binary),
        }
    }
}

/// Request details.
#[derive(Debug, Clone)]
pub struct RequestDetails {
    /// Request ID.
    pub request_id: RequestId,
    /// URL.
    pub url: String,
    /// Method.
    pub method: String,
    /// Frame ID.
    pub frame_id: i32,
    /// Parent frame ID.
    pub parent_frame_id: i32,
    /// Document ID.
    pub document_id: Option<String>,
    /// Parent document ID.
    pub parent_document_id: Option<String>,
    /// Document lifecycle.
    pub document_lifecycle: Option<DocumentLifecycle>,
    /// Frame type.
    pub frame_type: Option<FrameType>,
    /// Tab ID.
    pub tab_id: i32,
    /// Resource type.
    pub resource_type: ResourceType,
    /// Initiator.
    pub initiator: Option<String>,
    /// Timestamp.
    pub time_stamp: f64,
}

/// Document lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentLifecycle {
    Prerender,
    Active,
    Cached,
    PendingDeletion,
}

/// Frame type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    OutermostFrame,
    FencedFrame,
    SubFrame,
}

/// Request body details.
#[derive(Debug, Clone)]
pub struct RequestBody {
    /// Error if any.
    pub error: Option<String>,
    /// Form data.
    pub form_data: Option<BTreeMap<String, Vec<String>>>,
    /// Raw data.
    pub raw: Option<Vec<UploadData>>,
}

/// Upload data.
#[derive(Debug, Clone)]
pub struct UploadData {
    /// Bytes.
    pub bytes: Option<Vec<u8>>,
    /// File.
    pub file: Option<String>,
}

/// Blocking response.
#[derive(Debug, Clone, Default)]
pub struct BlockingResponse {
    /// Cancel the request.
    pub cancel: Option<bool>,
    /// Redirect URL.
    pub redirect_url: Option<String>,
    /// Request headers to modify.
    pub request_headers: Option<Vec<HttpHeader>>,
    /// Response headers to modify.
    pub response_headers: Option<Vec<HttpHeader>>,
    /// Authentication credentials.
    pub auth_credentials: Option<AuthCredentials>,
}

/// Authentication credentials.
#[derive(Debug, Clone)]
pub struct AuthCredentials {
    /// Username.
    pub username: String,
    /// Password.
    pub password: String,
}

/// Response details.
#[derive(Debug, Clone)]
pub struct ResponseDetails {
    /// Request ID.
    pub request_id: RequestId,
    /// URL.
    pub url: String,
    /// Method.
    pub method: String,
    /// Frame ID.
    pub frame_id: i32,
    /// Parent frame ID.
    pub parent_frame_id: i32,
    /// Tab ID.
    pub tab_id: i32,
    /// Resource type.
    pub resource_type: ResourceType,
    /// Initiator.
    pub initiator: Option<String>,
    /// Timestamp.
    pub time_stamp: f64,
    /// Status code.
    pub status_code: i32,
    /// Status line.
    pub status_line: String,
    /// Response headers.
    pub response_headers: Option<Vec<HttpHeader>>,
    /// From cache.
    pub from_cache: bool,
    /// IP address.
    pub ip: Option<String>,
}

/// Auth challenge details.
#[derive(Debug, Clone)]
pub struct AuthChallenge {
    /// Whether from proxy.
    pub is_proxy: bool,
    /// Scheme.
    pub scheme: String,
    /// Host.
    pub host: String,
    /// Port.
    pub port: i32,
    /// Realm.
    pub realm: Option<String>,
}

/// Error details.
#[derive(Debug, Clone)]
pub struct ErrorDetails {
    /// Request ID.
    pub request_id: RequestId,
    /// URL.
    pub url: String,
    /// Method.
    pub method: String,
    /// Frame ID.
    pub frame_id: i32,
    /// Parent frame ID.
    pub parent_frame_id: i32,
    /// Tab ID.
    pub tab_id: i32,
    /// Resource type.
    pub resource_type: ResourceType,
    /// Initiator.
    pub initiator: Option<String>,
    /// Timestamp.
    pub time_stamp: f64,
    /// Error.
    pub error: String,
    /// From cache.
    pub from_cache: bool,
    /// IP address.
    pub ip: Option<String>,
}

/// Listener options.
#[derive(Debug, Clone, Default)]
pub struct ListenerOptions {
    /// Block until response.
    pub blocking: bool,
    /// Include request headers.
    pub request_headers: bool,
    /// Include response headers.
    pub response_headers: bool,
    /// Extra info spec.
    pub extra_info_spec: Vec<String>,
}

/// Registered listener.
struct RegisteredListener {
    /// Listener ID.
    id: u64,
    /// Filter.
    filter: RequestFilter,
    /// Options.
    options: ListenerOptions,
    /// Extension ID.
    extension_id: String,
}

/// Web Request API.
pub struct WebRequestApi {
    /// Next listener ID.
    next_listener_id: RwLock<u64>,
    /// On before request listeners.
    before_request_listeners: RwLock<Vec<RegisteredListener>>,
    /// On before send headers listeners.
    before_send_headers_listeners: RwLock<Vec<RegisteredListener>>,
    /// On send headers listeners.
    send_headers_listeners: RwLock<Vec<RegisteredListener>>,
    /// On headers received listeners.
    headers_received_listeners: RwLock<Vec<RegisteredListener>>,
    /// On auth required listeners.
    auth_required_listeners: RwLock<Vec<RegisteredListener>>,
    /// On response started listeners.
    response_started_listeners: RwLock<Vec<RegisteredListener>>,
    /// On before redirect listeners.
    before_redirect_listeners: RwLock<Vec<RegisteredListener>>,
    /// On completed listeners.
    completed_listeners: RwLock<Vec<RegisteredListener>>,
    /// On error listeners.
    error_listeners: RwLock<Vec<RegisteredListener>>,
    /// Events.
    pub on_before_request: RwLock<EventEmitter<RequestDetails>>,
    pub on_before_send_headers: RwLock<EventEmitter<RequestDetails>>,
    pub on_send_headers: RwLock<EventEmitter<RequestDetails>>,
    pub on_headers_received: RwLock<EventEmitter<ResponseDetails>>,
    pub on_auth_required: RwLock<EventEmitter<(RequestDetails, AuthChallenge)>>,
    pub on_response_started: RwLock<EventEmitter<ResponseDetails>>,
    pub on_before_redirect: RwLock<EventEmitter<ResponseDetails>>,
    pub on_completed: RwLock<EventEmitter<ResponseDetails>>,
    pub on_error_occurred: RwLock<EventEmitter<ErrorDetails>>,
}

impl WebRequestApi {
    /// Create a new WebRequest API.
    pub fn new() -> Self {
        Self {
            next_listener_id: RwLock::new(1),
            before_request_listeners: RwLock::new(Vec::new()),
            before_send_headers_listeners: RwLock::new(Vec::new()),
            send_headers_listeners: RwLock::new(Vec::new()),
            headers_received_listeners: RwLock::new(Vec::new()),
            auth_required_listeners: RwLock::new(Vec::new()),
            response_started_listeners: RwLock::new(Vec::new()),
            before_redirect_listeners: RwLock::new(Vec::new()),
            completed_listeners: RwLock::new(Vec::new()),
            error_listeners: RwLock::new(Vec::new()),
            on_before_request: RwLock::new(EventEmitter::new()),
            on_before_send_headers: RwLock::new(EventEmitter::new()),
            on_send_headers: RwLock::new(EventEmitter::new()),
            on_headers_received: RwLock::new(EventEmitter::new()),
            on_auth_required: RwLock::new(EventEmitter::new()),
            on_response_started: RwLock::new(EventEmitter::new()),
            on_before_redirect: RwLock::new(EventEmitter::new()),
            on_completed: RwLock::new(EventEmitter::new()),
            on_error_occurred: RwLock::new(EventEmitter::new()),
        }
    }

    /// Add before request listener.
    pub fn add_before_request_listener(
        &self,
        ctx: &ApiContext,
        filter: RequestFilter,
        options: ListenerOptions,
    ) -> u64 {
        self.add_listener(ctx, &self.before_request_listeners, filter, options)
    }

    /// Add before send headers listener.
    pub fn add_before_send_headers_listener(
        &self,
        ctx: &ApiContext,
        filter: RequestFilter,
        options: ListenerOptions,
    ) -> u64 {
        self.add_listener(ctx, &self.before_send_headers_listeners, filter, options)
    }

    /// Add headers received listener.
    pub fn add_headers_received_listener(
        &self,
        ctx: &ApiContext,
        filter: RequestFilter,
        options: ListenerOptions,
    ) -> u64 {
        self.add_listener(ctx, &self.headers_received_listeners, filter, options)
    }

    /// Helper to add listener.
    fn add_listener(
        &self,
        ctx: &ApiContext,
        listeners: &RwLock<Vec<RegisteredListener>>,
        filter: RequestFilter,
        options: ListenerOptions,
    ) -> u64 {
        let mut next_id = self.next_listener_id.write();
        let id = *next_id;
        *next_id += 1;

        listeners.write().push(RegisteredListener {
            id,
            filter,
            options,
            extension_id: ctx.extension_id.as_str().to_string(),
        });

        id
    }

    /// Remove listener.
    pub fn remove_listener(&self, listener_id: u64) {
        // Remove from all listener lists
        self.before_request_listeners
            .write()
            .retain(|l| l.id != listener_id);
        self.before_send_headers_listeners
            .write()
            .retain(|l| l.id != listener_id);
        self.send_headers_listeners
            .write()
            .retain(|l| l.id != listener_id);
        self.headers_received_listeners
            .write()
            .retain(|l| l.id != listener_id);
        self.auth_required_listeners
            .write()
            .retain(|l| l.id != listener_id);
        self.response_started_listeners
            .write()
            .retain(|l| l.id != listener_id);
        self.before_redirect_listeners
            .write()
            .retain(|l| l.id != listener_id);
        self.completed_listeners
            .write()
            .retain(|l| l.id != listener_id);
        self.error_listeners.write().retain(|l| l.id != listener_id);
    }

    /// Handle before request (internal).
    pub fn handle_before_request(&self, details: &RequestDetails) -> Option<BlockingResponse> {
        let listeners = self.before_request_listeners.read();

        for listener in listeners.iter() {
            if self.matches_filter(&listener.filter, details) {
                // Emit event
                self.on_before_request.read().emit(details);

                if listener.options.blocking {
                    // Would wait for response
                    return Some(BlockingResponse::default());
                }
            }
        }

        None
    }

    /// Handle headers received (internal).
    pub fn handle_headers_received(&self, details: &ResponseDetails) -> Option<BlockingResponse> {
        let listeners = self.headers_received_listeners.read();

        for listener in listeners.iter() {
            if self.matches_response_filter(&listener.filter, details) {
                self.on_headers_received.read().emit(details);

                if listener.options.blocking {
                    return Some(BlockingResponse::default());
                }
            }
        }

        None
    }

    /// Check if filter matches request.
    fn matches_filter(&self, filter: &RequestFilter, details: &RequestDetails) -> bool {
        // Check URL patterns
        if !filter.urls.is_empty() {
            let matches = filter
                .urls
                .iter()
                .any(|pattern| matches_url_pattern(&details.url, pattern));
            if !matches {
                return false;
            }
        }

        // Check resource types
        if let Some(ref types) = filter.types {
            if !types.contains(&details.resource_type) {
                return false;
            }
        }

        // Check tab ID
        if let Some(tab_id) = filter.tab_id {
            if details.tab_id != tab_id as i32 {
                return false;
            }
        }

        true
    }

    /// Check if filter matches response.
    fn matches_response_filter(&self, filter: &RequestFilter, details: &ResponseDetails) -> bool {
        // Check URL patterns
        if !filter.urls.is_empty() {
            let matches = filter
                .urls
                .iter()
                .any(|pattern| matches_url_pattern(&details.url, pattern));
            if !matches {
                return false;
            }
        }

        // Check resource types
        if let Some(ref types) = filter.types {
            if !types.contains(&details.resource_type) {
                return false;
            }
        }

        true
    }

    /// Get maximum redirects constant.
    pub const fn max_handler_behavior_changed_calls_per_10_minutes() -> usize {
        20
    }
}

impl Default for WebRequestApi {
    fn default() -> Self {
        Self::new()
    }
}

/// Match URL against pattern.
fn matches_url_pattern(url: &str, pattern: &str) -> bool {
    if pattern == "<all_urls>" {
        return true;
    }

    // Parse pattern: <scheme>://<host>/<path>
    let parts: Vec<&str> = pattern.splitn(2, "://").collect();
    if parts.len() != 2 {
        return false;
    }

    let scheme_pattern = parts[0];
    let rest = parts[1];

    // Check scheme
    if scheme_pattern != "*" {
        let url_scheme = url.split("://").next().unwrap_or("");
        if scheme_pattern != url_scheme {
            return false;
        }
    }

    // Extract URL host and path
    let url_rest = url.split("://").nth(1).unwrap_or("");
    let url_host = url_rest.split('/').next().unwrap_or("");

    // Parse pattern host and path
    let pattern_host = rest.split('/').next().unwrap_or("");

    // Check host
    if pattern_host != "*" {
        if pattern_host.starts_with("*.") {
            // Subdomain wildcard
            let base = &pattern_host[2..];
            if !url_host.ends_with(base) && url_host != &base[..] {
                return false;
            }
        } else if pattern_host != url_host {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExtensionId;

    #[test]
    fn test_url_pattern_matching() {
        // All URLs
        assert!(matches_url_pattern(
            "https://example.com/path",
            "<all_urls>"
        ));

        // Specific scheme and host
        assert!(matches_url_pattern(
            "https://example.com/path",
            "https://example.com/*"
        ));
        assert!(!matches_url_pattern(
            "http://example.com/path",
            "https://example.com/*"
        ));

        // Wildcard scheme
        assert!(matches_url_pattern(
            "https://example.com/path",
            "*://example.com/*"
        ));
        assert!(matches_url_pattern(
            "http://example.com/path",
            "*://example.com/*"
        ));

        // Subdomain wildcard
        assert!(matches_url_pattern(
            "https://sub.example.com/path",
            "*://*.example.com/*"
        ));
        assert!(matches_url_pattern(
            "https://example.com/path",
            "*://*.example.com/*"
        ));
    }

    #[test]
    fn test_web_request_api() {
        let api = WebRequestApi::new();
        let ctx = ApiContext::new(ExtensionId::new("test"));

        // Add listener
        let filter = RequestFilter {
            urls: vec!["<all_urls>".to_string()],
            ..Default::default()
        };
        let options = ListenerOptions::default();
        let id = api.add_before_request_listener(&ctx, filter, options);

        assert!(id > 0);

        // Remove listener
        api.remove_listener(id);
    }
}
