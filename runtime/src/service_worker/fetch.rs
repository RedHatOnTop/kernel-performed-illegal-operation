//! Fetch Event Handling
//!
//! Implements fetch event interception for service workers.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use super::ServiceWorkerId;

/// Fetch event ID counter
static NEXT_FETCH_ID: AtomicU64 = AtomicU64::new(1);

/// Fetch event ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FetchEventId(u64);

impl FetchEventId {
    /// Create a new ID
    pub fn new() -> Self {
        Self(NEXT_FETCH_ID.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for FetchEventId {
    fn default() -> Self {
        Self::new()
    }
}

/// HTTP request method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl Default for RequestMethod {
    fn default() -> Self {
        Self::Get
    }
}

impl RequestMethod {
    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Patch => "PATCH",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

/// Request destination
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestDestination {
    /// Unknown
    Empty,
    /// Audio resource
    Audio,
    /// Document
    Document,
    /// Embed
    Embed,
    /// Font
    Font,
    /// Frame
    Frame,
    /// IFrame
    IFrame,
    /// Image
    Image,
    /// Manifest
    Manifest,
    /// Object
    Object,
    /// Report
    Report,
    /// Script
    Script,
    /// ServiceWorker
    ServiceWorker,
    /// SharedWorker
    SharedWorker,
    /// Style
    Style,
    /// Track
    Track,
    /// Video
    Video,
    /// Worker
    Worker,
    /// XMLHttpRequest
    Xslt,
}

impl Default for RequestDestination {
    fn default() -> Self {
        Self::Empty
    }
}

/// Request mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestMode {
    /// Same-origin only
    SameOrigin,
    /// No CORS
    NoCors,
    /// CORS
    Cors,
    /// Navigate
    Navigate,
    /// WebSocket
    WebSocket,
}

impl Default for RequestMode {
    fn default() -> Self {
        Self::NoCors
    }
}

/// Request credentials mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestCredentials {
    /// Never include credentials
    Omit,
    /// Include for same-origin only
    SameOrigin,
    /// Always include
    Include,
}

impl Default for RequestCredentials {
    fn default() -> Self {
        Self::SameOrigin
    }
}

/// Request cache mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestCache {
    /// Default browser behavior
    Default,
    /// No store
    NoStore,
    /// Reload
    Reload,
    /// No cache
    NoCache,
    /// Force cache
    ForceCache,
    /// Only if cached
    OnlyIfCached,
}

impl Default for RequestCache {
    fn default() -> Self {
        Self::Default
    }
}

/// Request redirect mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestRedirect {
    /// Follow redirects
    Follow,
    /// Error on redirect
    Error,
    /// Return opaque redirect response
    Manual,
}

impl Default for RequestRedirect {
    fn default() -> Self {
        Self::Follow
    }
}

/// Fetch request
#[derive(Debug, Clone)]
pub struct Request {
    /// Request URL
    pub url: String,
    /// HTTP method
    pub method: RequestMethod,
    /// Request headers
    pub headers: BTreeMap<String, String>,
    /// Request body (if any)
    pub body: Option<Vec<u8>>,
    /// Request destination
    pub destination: RequestDestination,
    /// Request mode
    pub mode: RequestMode,
    /// Credentials mode
    pub credentials: RequestCredentials,
    /// Cache mode
    pub cache: RequestCache,
    /// Redirect mode
    pub redirect: RequestRedirect,
    /// Referrer
    pub referrer: String,
    /// Referrer policy
    pub referrer_policy: String,
    /// Integrity
    pub integrity: String,
    /// Is reload navigation
    pub is_reload_navigation: bool,
    /// Is history navigation
    pub is_history_navigation: bool,
    /// Client ID
    pub client_id: Option<String>,
}

impl Request {
    /// Create a new request
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: RequestMethod::Get,
            headers: BTreeMap::new(),
            body: None,
            destination: RequestDestination::Empty,
            mode: RequestMode::Cors,
            credentials: RequestCredentials::SameOrigin,
            cache: RequestCache::Default,
            redirect: RequestRedirect::Follow,
            referrer: "about:client".to_string(),
            referrer_policy: String::new(),
            integrity: String::new(),
            is_reload_navigation: false,
            is_history_navigation: false,
            client_id: None,
        }
    }

    /// Clone the request
    pub fn clone_request(&self) -> Self {
        self.clone()
    }
}

/// Response type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseType {
    /// Basic
    Basic,
    /// CORS
    Cors,
    /// Default
    Default,
    /// Error
    Error,
    /// Opaque
    Opaque,
    /// Opaque redirect
    OpaqueRedirect,
}

impl Default for ResponseType {
    fn default() -> Self {
        Self::Default
    }
}

/// Fetch response
#[derive(Debug, Clone)]
pub struct Response {
    /// Response type
    pub response_type: ResponseType,
    /// URL
    pub url: String,
    /// Redirected
    pub redirected: bool,
    /// Status code
    pub status: u16,
    /// Status text
    pub status_text: String,
    /// Response headers
    pub headers: BTreeMap<String, String>,
    /// Response body
    pub body: Option<Vec<u8>>,
    /// Whether body was used
    pub body_used: bool,
}

impl Response {
    /// Create a new response
    pub fn new(status: u16) -> Self {
        Self {
            response_type: ResponseType::Default,
            url: String::new(),
            redirected: false,
            status,
            status_text: status_text_for(status).to_string(),
            headers: BTreeMap::new(),
            body: None,
            body_used: false,
        }
    }

    /// Create error response
    pub fn error() -> Self {
        Self {
            response_type: ResponseType::Error,
            url: String::new(),
            redirected: false,
            status: 0,
            status_text: String::new(),
            headers: BTreeMap::new(),
            body: None,
            body_used: false,
        }
    }

    /// Create redirect response
    pub fn redirect(url: impl Into<String>, status: u16) -> Self {
        let mut response = Self::new(status);
        response.headers.insert("Location".to_string(), url.into());
        response
    }

    /// Check if response is OK
    pub fn ok(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// Clone the response
    pub fn clone_response(&self) -> Self {
        self.clone()
    }
}

/// Get status text for status code
fn status_text_for(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        101 => "Switching Protocols",
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Unknown",
    }
}

/// Fetch event
#[derive(Debug)]
pub struct FetchEvent {
    /// Event ID
    id: FetchEventId,
    /// Request
    request: Request,
    /// Client ID
    client_id: Option<String>,
    /// Resulting client ID
    resulting_client_id: Option<String>,
    /// Preload response promise
    preload_response: Option<Response>,
    /// Whether respondWith was called
    responded: bool,
    /// Response (if respondWith was called)
    response: Option<Response>,
    /// Whether handled
    handled: bool,
}

impl FetchEvent {
    /// Create a new fetch event
    pub fn new(request: Request) -> Self {
        let client_id = request.client_id.clone();
        Self {
            id: FetchEventId::new(),
            request,
            client_id,
            resulting_client_id: None,
            preload_response: None,
            responded: false,
            response: None,
            handled: false,
        }
    }

    /// Get the event ID
    pub fn id(&self) -> FetchEventId {
        self.id
    }

    /// Get the request
    pub fn request(&self) -> &Request {
        &self.request
    }

    /// Get client ID
    pub fn client_id(&self) -> Option<&str> {
        self.client_id.as_deref()
    }

    /// Get resulting client ID
    pub fn resulting_client_id(&self) -> Option<&str> {
        self.resulting_client_id.as_deref()
    }

    /// Get preload response
    pub fn preload_response(&self) -> Option<&Response> {
        self.preload_response.as_ref()
    }

    /// Check if respondWith was called
    pub fn responded(&self) -> bool {
        self.responded
    }

    /// Respond with a response
    pub fn respond_with(&mut self, response: Response) {
        if !self.responded {
            self.responded = true;
            self.response = Some(response);
        }
    }

    /// Get the response
    pub fn take_response(&mut self) -> Option<Response> {
        self.response.take()
    }

    /// Mark as handled
    pub fn mark_handled(&mut self) {
        self.handled = true;
    }

    /// Check if handled
    pub fn handled(&self) -> bool {
        self.handled
    }
}

/// Fetch event handler
pub trait FetchHandler: Send + Sync {
    /// Handle a fetch event
    fn handle(&self, event: &mut FetchEvent);
}

/// Default fetch behavior (network fetch)
pub struct DefaultFetchHandler;

impl FetchHandler for DefaultFetchHandler {
    fn handle(&self, _event: &mut FetchEvent) {
        // Default: don't intercept, let browser handle
    }
}

/// Fetch interceptor
pub struct FetchInterceptor {
    /// Registered handlers by worker ID
    handlers: BTreeMap<ServiceWorkerId, Box<dyn FetchHandler>>,
}

impl FetchInterceptor {
    /// Create new interceptor
    pub fn new() -> Self {
        Self {
            handlers: BTreeMap::new(),
        }
    }

    /// Register a handler
    pub fn register(&mut self, worker_id: ServiceWorkerId, handler: Box<dyn FetchHandler>) {
        self.handlers.insert(worker_id, handler);
    }

    /// Unregister a handler
    pub fn unregister(&mut self, worker_id: ServiceWorkerId) {
        self.handlers.remove(&worker_id);
    }

    /// Intercept a fetch
    pub fn intercept(&self, worker_id: ServiceWorkerId, event: &mut FetchEvent) -> bool {
        if let Some(handler) = self.handlers.get(&worker_id) {
            handler.handle(event);
            event.responded()
        } else {
            false
        }
    }
}

impl Default for FetchInterceptor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_creation() {
        let req = Request::new("https://example.com/page");
        assert_eq!(req.url, "https://example.com/page");
        assert_eq!(req.method, RequestMethod::Get);
        assert!(req.body.is_none());
    }

    #[test]
    fn test_request_method_as_str() {
        assert_eq!(RequestMethod::Get.as_str(), "GET");
        assert_eq!(RequestMethod::Post.as_str(), "POST");
        assert_eq!(RequestMethod::Delete.as_str(), "DELETE");
    }

    #[test]
    fn test_response_new() {
        let resp = Response::new(200);
        assert_eq!(resp.status, 200);
        assert_eq!(resp.status_text, "OK");
        assert!(resp.ok());
    }

    #[test]
    fn test_response_ok_range() {
        assert!(Response::new(200).ok());
        assert!(Response::new(201).ok());
        assert!(Response::new(299).ok());
        assert!(!Response::new(300).ok());
        assert!(!Response::new(404).ok());
        assert!(!Response::new(500).ok());
    }

    #[test]
    fn test_response_error() {
        let resp = Response::error();
        assert_eq!(resp.response_type, ResponseType::Error);
        assert_eq!(resp.status, 0);
        assert!(!resp.ok());
    }

    #[test]
    fn test_response_redirect() {
        let resp = Response::redirect("https://other.com", 301);
        assert_eq!(resp.status, 301);
        assert_eq!(
            resp.headers.get("Location"),
            Some(&"https://other.com".to_string())
        );
    }

    #[test]
    fn test_fetch_event_respond_with() {
        let req = Request::new("https://example.com/api/data");
        let mut event = FetchEvent::new(req);
        assert!(!event.responded());
        event.respond_with(Response::new(200));
        assert!(event.responded());
        let resp = event.take_response().unwrap();
        assert_eq!(resp.status, 200);
    }

    #[test]
    fn test_fetch_event_respond_with_once() {
        let req = Request::new("https://example.com/");
        let mut event = FetchEvent::new(req);
        event.respond_with(Response::new(200));
        // Second call should be ignored
        event.respond_with(Response::new(404));
        assert!(event.responded());
        let resp = event.take_response().unwrap();
        assert_eq!(resp.status, 200);
    }

    #[test]
    fn test_fetch_event_handled() {
        let req = Request::new("https://example.com/");
        let mut event = FetchEvent::new(req);
        assert!(!event.handled());
        event.mark_handled();
        assert!(event.handled());
    }

    #[test]
    fn test_default_fetch_handler_no_response() {
        let handler = DefaultFetchHandler;
        let req = Request::new("https://example.com/");
        let mut event = FetchEvent::new(req);
        handler.handle(&mut event);
        assert!(!event.responded()); // Default does not respond
    }

    #[test]
    fn test_fetch_interceptor_register_unregister() {
        let mut interceptor = FetchInterceptor::new();
        let worker_id = ServiceWorkerId::new();
        interceptor.register(worker_id, Box::new(DefaultFetchHandler));

        let req = Request::new("https://example.com/");
        let mut event = FetchEvent::new(req);
        let result = interceptor.intercept(worker_id, &mut event);
        assert!(!result); // DefaultFetchHandler doesn't respond

        interceptor.unregister(worker_id);
        let req = Request::new("https://example.com/");
        let mut event = FetchEvent::new(req);
        let result = interceptor.intercept(worker_id, &mut event);
        assert!(!result); // Not registered
    }

    #[test]
    fn test_fetch_interceptor_custom_handler() {
        struct TestHandler;
        impl FetchHandler for TestHandler {
            fn handle(&self, event: &mut FetchEvent) {
                event.respond_with(Response::new(418)); // I'm a teapot
            }
        }

        let mut interceptor = FetchInterceptor::new();
        let worker_id = ServiceWorkerId::new();
        interceptor.register(worker_id, Box::new(TestHandler));

        let req = Request::new("https://example.com/");
        let mut event = FetchEvent::new(req);
        let result = interceptor.intercept(worker_id, &mut event);
        assert!(result);
        let resp = event.take_response().unwrap();
        assert_eq!(resp.status, 418);
    }

    #[test]
    fn test_fetch_event_id_unique() {
        let id1 = FetchEventId::new();
        let id2 = FetchEventId::new();
        assert_ne!(id1, id2);
    }
}
