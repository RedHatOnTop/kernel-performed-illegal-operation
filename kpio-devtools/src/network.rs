//! Network Panel
//!
//! Provides network request monitoring, timing, and analysis for DevTools.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

/// Request ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RequestId(pub String);

/// Loader ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoaderId(pub String);

/// Frame ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FrameId(pub String);

/// Monotonic time (seconds since an arbitrary epoch).
pub type MonotonicTime = f64;

/// Wall time (seconds since Unix epoch).
pub type TimeSinceEpoch = f64;

/// Resource type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    /// Document (HTML).
    Document,
    /// Stylesheet (CSS).
    Stylesheet,
    /// Image.
    Image,
    /// Media (audio/video).
    Media,
    /// Font.
    Font,
    /// Script (JavaScript).
    Script,
    /// TextTrack (subtitles).
    TextTrack,
    /// XHR.
    XHR,
    /// Fetch API.
    Fetch,
    /// Prefetch.
    Prefetch,
    /// EventSource.
    EventSource,
    /// WebSocket.
    WebSocket,
    /// Manifest.
    Manifest,
    /// Signed Exchange.
    SignedExchange,
    /// Ping.
    Ping,
    /// CSP Violation Report.
    CspViolationReport,
    /// Preflight (CORS).
    Preflight,
    /// Other.
    Other,
}

impl ResourceType {
    /// Get MIME type hint.
    pub fn mime_type_hint(&self) -> &'static str {
        match self {
            Self::Document => "text/html",
            Self::Stylesheet => "text/css",
            Self::Image => "image/*",
            Self::Media => "video/*",
            Self::Font => "font/*",
            Self::Script => "application/javascript",
            Self::XHR | Self::Fetch => "application/json",
            Self::Manifest => "application/manifest+json",
            _ => "application/octet-stream",
        }
    }
}

/// HTTP request method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
    Connect,
    Trace,
}

impl HttpMethod {
    /// Parse from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "GET" => Some(Self::Get),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            "DELETE" => Some(Self::Delete),
            "PATCH" => Some(Self::Patch),
            "HEAD" => Some(Self::Head),
            "OPTIONS" => Some(Self::Options),
            "CONNECT" => Some(Self::Connect),
            "TRACE" => Some(Self::Trace),
            _ => None,
        }
    }
    
    /// Convert to string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Patch => "PATCH",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
            Self::Connect => "CONNECT",
            Self::Trace => "TRACE",
        }
    }
}

/// HTTP headers.
#[derive(Debug, Clone, Default)]
pub struct Headers {
    headers: BTreeMap<String, String>,
}

impl Headers {
    /// Create new empty headers.
    pub fn new() -> Self {
        Self {
            headers: BTreeMap::new(),
        }
    }
    
    /// Set a header.
    pub fn set(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_lowercase(), value.to_string());
    }
    
    /// Get a header.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_lowercase()).map(|s| s.as_str())
    }
    
    /// Remove a header.
    pub fn remove(&mut self, name: &str) -> Option<String> {
        self.headers.remove(&name.to_lowercase())
    }
    
    /// Iterate over headers.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.headers.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
    
    /// Get content type.
    pub fn content_type(&self) -> Option<&str> {
        self.get("content-type")
    }
    
    /// Get content length.
    pub fn content_length(&self) -> Option<u64> {
        self.get("content-length")?.parse().ok()
    }
}

/// Request data.
#[derive(Debug, Clone)]
pub struct Request {
    /// URL.
    pub url: String,
    /// URL fragment (after #).
    pub url_fragment: Option<String>,
    /// HTTP method.
    pub method: HttpMethod,
    /// HTTP headers.
    pub headers: Headers,
    /// POST data.
    pub post_data: Option<String>,
    /// Whether POST data has binary content.
    pub has_post_data: bool,
    /// POST data entries.
    pub post_data_entries: Option<Vec<PostDataEntry>>,
    /// Mixed content type.
    pub mixed_content_type: MixedContentType,
    /// Initial priority.
    pub initial_priority: ResourcePriority,
    /// Referrer policy.
    pub referrer_policy: ReferrerPolicy,
    /// Is link preload.
    pub is_link_preload: bool,
    /// Trust token params.
    pub trust_token_params: Option<TrustTokenParams>,
    /// Is same site.
    pub is_same_site: bool,
}

impl Request {
    /// Create a new request.
    pub fn new(url: &str, method: HttpMethod) -> Self {
        Self {
            url: url.to_string(),
            url_fragment: None,
            method,
            headers: Headers::new(),
            post_data: None,
            has_post_data: false,
            post_data_entries: None,
            mixed_content_type: MixedContentType::None,
            initial_priority: ResourcePriority::Medium,
            referrer_policy: ReferrerPolicy::StrictOriginWhenCrossOrigin,
            is_link_preload: false,
            trust_token_params: None,
            is_same_site: true,
        }
    }
}

/// POST data entry.
#[derive(Debug, Clone)]
pub struct PostDataEntry {
    /// Bytes (base64 encoded).
    pub bytes: Option<String>,
}

/// Mixed content type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixedContentType {
    /// Not mixed content.
    None,
    /// Blockable mixed content.
    Blockable,
    /// Optionally blockable.
    OptionallyBlockable,
}

/// Resource priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourcePriority {
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
}

/// Referrer policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferrerPolicy {
    NoReferrer,
    NoReferrerWhenDowngrade,
    Origin,
    OriginWhenCrossOrigin,
    SameOrigin,
    StrictOrigin,
    StrictOriginWhenCrossOrigin,
    UnsafeUrl,
}

/// Trust token params.
#[derive(Debug, Clone)]
pub struct TrustTokenParams {
    /// Operation type.
    pub operation_type: TrustTokenOperationType,
    /// Refresh policy.
    pub refresh_policy: TrustTokenRefreshPolicy,
    /// Issuers.
    pub issuers: Option<Vec<String>>,
}

/// Trust token operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustTokenOperationType {
    Issuance,
    Redemption,
    Signing,
}

/// Trust token refresh policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustTokenRefreshPolicy {
    UseCached,
    Refresh,
}

/// Response data.
#[derive(Debug, Clone)]
pub struct Response {
    /// URL.
    pub url: String,
    /// HTTP status code.
    pub status: u16,
    /// HTTP status text.
    pub status_text: String,
    /// Response headers.
    pub headers: Headers,
    /// MIME type.
    pub mime_type: String,
    /// Request headers (as sent).
    pub request_headers: Option<Headers>,
    /// Connection reused.
    pub connection_reused: bool,
    /// Connection ID.
    pub connection_id: f64,
    /// Remote IP address.
    pub remote_ip_address: Option<String>,
    /// Remote port.
    pub remote_port: Option<u16>,
    /// From disk cache.
    pub from_disk_cache: bool,
    /// From service worker.
    pub from_service_worker: bool,
    /// From prefetch cache.
    pub from_prefetch_cache: bool,
    /// Encoded data length.
    pub encoded_data_length: i64,
    /// Timing info.
    pub timing: Option<ResourceTiming>,
    /// Service worker response source.
    pub service_worker_response_source: Option<ServiceWorkerResponseSource>,
    /// Response time.
    pub response_time: Option<TimeSinceEpoch>,
    /// Cache storage cache name.
    pub cache_storage_cache_name: Option<String>,
    /// Protocol.
    pub protocol: Option<String>,
    /// Security state.
    pub security_state: SecurityState,
    /// Security details.
    pub security_details: Option<SecurityDetails>,
}

impl Response {
    /// Create a new response.
    pub fn new(url: &str, status: u16) -> Self {
        Self {
            url: url.to_string(),
            status,
            status_text: Self::status_text(status).to_string(),
            headers: Headers::new(),
            mime_type: String::new(),
            request_headers: None,
            connection_reused: false,
            connection_id: 0.0,
            remote_ip_address: None,
            remote_port: None,
            from_disk_cache: false,
            from_service_worker: false,
            from_prefetch_cache: false,
            encoded_data_length: 0,
            timing: None,
            service_worker_response_source: None,
            response_time: None,
            cache_storage_cache_name: None,
            protocol: None,
            security_state: SecurityState::Unknown,
            security_details: None,
        }
    }
    
    /// Get status text for status code.
    fn status_text(status: u16) -> &'static str {
        match status {
            100 => "Continue",
            101 => "Switching Protocols",
            200 => "OK",
            201 => "Created",
            202 => "Accepted",
            204 => "No Content",
            206 => "Partial Content",
            301 => "Moved Permanently",
            302 => "Found",
            303 => "See Other",
            304 => "Not Modified",
            307 => "Temporary Redirect",
            308 => "Permanent Redirect",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            408 => "Request Timeout",
            409 => "Conflict",
            410 => "Gone",
            413 => "Payload Too Large",
            414 => "URI Too Long",
            415 => "Unsupported Media Type",
            429 => "Too Many Requests",
            500 => "Internal Server Error",
            501 => "Not Implemented",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            504 => "Gateway Timeout",
            _ => "",
        }
    }
    
    /// Check if response is successful.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
    
    /// Check if response is redirect.
    pub fn is_redirect(&self) -> bool {
        matches!(self.status, 301 | 302 | 303 | 307 | 308)
    }
    
    /// Check if response is from cache.
    pub fn is_from_cache(&self) -> bool {
        self.from_disk_cache || self.from_prefetch_cache
    }
}

/// Resource timing.
#[derive(Debug, Clone, Copy)]
pub struct ResourceTiming {
    /// Request time.
    pub request_time: f64,
    /// Proxy start.
    pub proxy_start: f64,
    /// Proxy end.
    pub proxy_end: f64,
    /// DNS lookup start.
    pub dns_start: f64,
    /// DNS lookup end.
    pub dns_end: f64,
    /// Connection start.
    pub connect_start: f64,
    /// Connection end.
    pub connect_end: f64,
    /// SSL/TLS handshake start.
    pub ssl_start: f64,
    /// SSL/TLS handshake end.
    pub ssl_end: f64,
    /// Worker start.
    pub worker_start: f64,
    /// Worker ready.
    pub worker_ready: f64,
    /// Worker fetch start.
    pub worker_fetch_start: f64,
    /// Worker respond with settled.
    pub worker_respond_with_settled: f64,
    /// Send start.
    pub send_start: f64,
    /// Send end.
    pub send_end: f64,
    /// Push start.
    pub push_start: f64,
    /// Push end.
    pub push_end: f64,
    /// Receive headers start.
    pub receive_headers_start: f64,
    /// Receive headers end.
    pub receive_headers_end: f64,
}

impl ResourceTiming {
    /// Create timing from start time.
    pub fn from_start(request_time: f64) -> Self {
        Self {
            request_time,
            proxy_start: -1.0,
            proxy_end: -1.0,
            dns_start: -1.0,
            dns_end: -1.0,
            connect_start: -1.0,
            connect_end: -1.0,
            ssl_start: -1.0,
            ssl_end: -1.0,
            worker_start: -1.0,
            worker_ready: -1.0,
            worker_fetch_start: -1.0,
            worker_respond_with_settled: -1.0,
            send_start: 0.0,
            send_end: 0.0,
            push_start: 0.0,
            push_end: 0.0,
            receive_headers_start: 0.0,
            receive_headers_end: 0.0,
        }
    }
    
    /// Calculate DNS time.
    pub fn dns_time(&self) -> Option<f64> {
        if self.dns_start >= 0.0 && self.dns_end >= 0.0 {
            Some(self.dns_end - self.dns_start)
        } else {
            None
        }
    }
    
    /// Calculate connection time.
    pub fn connect_time(&self) -> Option<f64> {
        if self.connect_start >= 0.0 && self.connect_end >= 0.0 {
            Some(self.connect_end - self.connect_start)
        } else {
            None
        }
    }
    
    /// Calculate SSL time.
    pub fn ssl_time(&self) -> Option<f64> {
        if self.ssl_start >= 0.0 && self.ssl_end >= 0.0 {
            Some(self.ssl_end - self.ssl_start)
        } else {
            None
        }
    }
    
    /// Calculate time to first byte.
    pub fn ttfb(&self) -> f64 {
        self.receive_headers_start
    }
    
    /// Calculate total time.
    pub fn total_time(&self, end_time: f64) -> f64 {
        (end_time - self.request_time) * 1000.0
    }
}

/// Service worker response source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceWorkerResponseSource {
    CacheStorage,
    HttpCache,
    FallbackCode,
    Network,
}

/// Security state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityState {
    Unknown,
    Neutral,
    Insecure,
    Secure,
    Info,
    InsecureBroken,
}

/// Security details.
#[derive(Debug, Clone)]
pub struct SecurityDetails {
    /// Protocol.
    pub protocol: String,
    /// Key exchange.
    pub key_exchange: String,
    /// Key exchange group.
    pub key_exchange_group: Option<String>,
    /// Cipher.
    pub cipher: String,
    /// MAC.
    pub mac: Option<String>,
    /// Certificate ID.
    pub certificate_id: i32,
    /// Subject name.
    pub subject_name: String,
    /// SAN list.
    pub san_list: Vec<String>,
    /// Issuer.
    pub issuer: String,
    /// Valid from.
    pub valid_from: TimeSinceEpoch,
    /// Valid to.
    pub valid_to: TimeSinceEpoch,
    /// Certificate transparency compliance.
    pub certificate_transparency_compliance: CertificateTransparencyCompliance,
}

/// Certificate transparency compliance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateTransparencyCompliance {
    Unknown,
    NotCompliant,
    Compliant,
}

/// WebSocket frame.
#[derive(Debug, Clone)]
pub struct WebSocketFrame {
    /// Opcode.
    pub opcode: u8,
    /// Mask.
    pub mask: bool,
    /// Payload data (base64 for binary).
    pub payload_data: String,
}

/// WebSocket request.
#[derive(Debug, Clone)]
pub struct WebSocketRequest {
    /// Request headers.
    pub headers: Headers,
}

/// WebSocket response.
#[derive(Debug, Clone)]
pub struct WebSocketResponse {
    /// Status code.
    pub status: u16,
    /// Status text.
    pub status_text: String,
    /// Response headers.
    pub headers: Headers,
    /// Request headers.
    pub headers_text: Option<String>,
    /// Request headers.
    pub request_headers: Option<Headers>,
}

/// Network request entry.
#[derive(Debug, Clone)]
pub struct NetworkEntry {
    /// Request ID.
    pub request_id: RequestId,
    /// Loader ID.
    pub loader_id: LoaderId,
    /// Frame ID.
    pub frame_id: Option<FrameId>,
    /// Resource type.
    pub resource_type: ResourceType,
    /// Request.
    pub request: Request,
    /// Response.
    pub response: Option<Response>,
    /// Initiator.
    pub initiator: Initiator,
    /// Redirect response.
    pub redirect_response: Option<Response>,
    /// Wall time.
    pub wall_time: TimeSinceEpoch,
    /// Timestamp (monotonic).
    pub timestamp: MonotonicTime,
    /// Response received timestamp.
    pub response_received_timestamp: Option<MonotonicTime>,
    /// Loading finished timestamp.
    pub loading_finished_timestamp: Option<MonotonicTime>,
    /// Encoded data length (total).
    pub encoded_data_length: i64,
    /// Decoded body length.
    pub decoded_body_length: i64,
    /// Has been blocked.
    pub blocked_reason: Option<BlockedReason>,
    /// CORS error status.
    pub cors_error_status: Option<CorsErrorStatus>,
    /// Response body.
    pub response_body: Option<Vec<u8>>,
}

impl NetworkEntry {
    /// Create a new network entry.
    pub fn new(
        request_id: RequestId,
        loader_id: LoaderId,
        request: Request,
        resource_type: ResourceType,
        timestamp: MonotonicTime,
        wall_time: TimeSinceEpoch,
    ) -> Self {
        Self {
            request_id,
            loader_id,
            frame_id: None,
            resource_type,
            request,
            response: None,
            initiator: Initiator::Other,
            redirect_response: None,
            wall_time,
            timestamp,
            response_received_timestamp: None,
            loading_finished_timestamp: None,
            encoded_data_length: 0,
            decoded_body_length: 0,
            blocked_reason: None,
            cors_error_status: None,
            response_body: None,
        }
    }
    
    /// Set response.
    pub fn set_response(&mut self, response: Response, timestamp: MonotonicTime) {
        self.response = Some(response);
        self.response_received_timestamp = Some(timestamp);
    }
    
    /// Mark as finished.
    pub fn finish(&mut self, timestamp: MonotonicTime, encoded_length: i64, decoded_length: i64) {
        self.loading_finished_timestamp = Some(timestamp);
        self.encoded_data_length = encoded_length;
        self.decoded_body_length = decoded_length;
    }
    
    /// Calculate duration.
    pub fn duration(&self) -> Option<f64> {
        self.loading_finished_timestamp
            .map(|end| (end - self.timestamp) * 1000.0)
    }
    
    /// Is completed.
    pub fn is_completed(&self) -> bool {
        self.loading_finished_timestamp.is_some()
    }
}

/// Request initiator.
#[derive(Debug, Clone)]
pub enum Initiator {
    /// Parser (HTML parser).
    Parser {
        url: String,
        line_number: Option<i32>,
        column_number: Option<i32>,
    },
    /// Script.
    Script {
        stack_trace: Option<crate::console::StackTrace>,
    },
    /// Preload.
    Preload,
    /// Signed exchange.
    SignedExchange {
        url: String,
    },
    /// Preflight.
    Preflight {
        request_id: RequestId,
    },
    /// Other.
    Other,
}

/// Blocked reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockedReason {
    Other,
    Csp,
    MixedContent,
    Origin,
    Inspector,
    SubresourceFilter,
    ContentType,
    CoepFrameResourceNeedsCoepHeader,
    CoopSandboxedIframeCannotNavigateToCoopPage,
    CorpNotSameOrigin,
    CorpNotSameOriginAfterDefaultedToSameOriginByCoep,
    CorpNotSameSite,
}

/// CORS error status.
#[derive(Debug, Clone)]
pub struct CorsErrorStatus {
    /// CORS error.
    pub cors_error: CorsError,
    /// Failed parameter.
    pub failed_parameter: String,
}

/// CORS error type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorsError {
    DisallowedByMode,
    InvalidResponse,
    WildcardOriginNotAllowed,
    MissingAllowOriginHeader,
    MultipleAllowOriginValues,
    InvalidAllowOriginValue,
    AllowOriginMismatch,
    InvalidAllowCredentials,
    CorsDisabledScheme,
    PreflightInvalidStatus,
    PreflightDisallowedRedirect,
    PreflightWildcardOriginNotAllowed,
    PreflightMissingAllowOriginHeader,
    PreflightMultipleAllowOriginValues,
    PreflightInvalidAllowOriginValue,
    PreflightAllowOriginMismatch,
    PreflightInvalidAllowCredentials,
    PreflightMissingAllowPrivateNetwork,
    PreflightInvalidAllowPrivateNetwork,
    InvalidPrivateNetworkAccess,
    UnexpectedPrivateNetworkAccess,
    NoCorsRedirectModeNotFollow,
}

/// Network panel.
pub struct NetworkPanel {
    /// Entries.
    entries: BTreeMap<String, NetworkEntry>,
    /// Next request ID.
    next_request_id: u64,
    /// Next loader ID.
    next_loader_id: u64,
    /// Is recording.
    is_recording: bool,
    /// Preserve log.
    preserve_log: bool,
    /// Disable cache.
    disable_cache: bool,
    /// Throttling.
    throttling: Option<NetworkThrottling>,
    /// Blocked URLs.
    blocked_urls: Vec<String>,
}

impl NetworkPanel {
    /// Create a new network panel.
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            next_request_id: 1,
            next_loader_id: 1,
            is_recording: true,
            preserve_log: false,
            disable_cache: false,
            throttling: None,
            blocked_urls: Vec::new(),
        }
    }
    
    /// Generate a new request ID.
    pub fn new_request_id(&mut self) -> RequestId {
        let id = RequestId(alloc::format!("{}.{}", self.next_loader_id, self.next_request_id));
        self.next_request_id += 1;
        id
    }
    
    /// Generate a new loader ID.
    pub fn new_loader_id(&mut self) -> LoaderId {
        let id = LoaderId(alloc::format!("{}", self.next_loader_id));
        self.next_loader_id += 1;
        id
    }
    
    /// Record a request.
    pub fn record_request(&mut self, entry: NetworkEntry) {
        if self.is_recording {
            self.entries.insert(entry.request_id.0.clone(), entry);
        }
    }
    
    /// Get an entry.
    pub fn get_entry(&self, request_id: &RequestId) -> Option<&NetworkEntry> {
        self.entries.get(&request_id.0)
    }
    
    /// Get an entry mutably.
    pub fn get_entry_mut(&mut self, request_id: &RequestId) -> Option<&mut NetworkEntry> {
        self.entries.get_mut(&request_id.0)
    }
    
    /// Get all entries.
    pub fn entries(&self) -> impl Iterator<Item = &NetworkEntry> {
        self.entries.values()
    }
    
    /// Clear entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    
    /// Set recording state.
    pub fn set_recording(&mut self, recording: bool) {
        self.is_recording = recording;
    }
    
    /// Set preserve log.
    pub fn set_preserve_log(&mut self, preserve: bool) {
        self.preserve_log = preserve;
    }
    
    /// Set cache disabled.
    pub fn set_cache_disabled(&mut self, disabled: bool) {
        self.disable_cache = disabled;
    }
    
    /// Is cache disabled?
    pub fn is_cache_disabled(&self) -> bool {
        self.disable_cache
    }
    
    /// Set throttling.
    pub fn set_throttling(&mut self, throttling: Option<NetworkThrottling>) {
        self.throttling = throttling;
    }
    
    /// Block URL.
    pub fn block_url(&mut self, url: &str) {
        if !self.blocked_urls.contains(&url.to_string()) {
            self.blocked_urls.push(url.to_string());
        }
    }
    
    /// Unblock URL.
    pub fn unblock_url(&mut self, url: &str) {
        self.blocked_urls.retain(|u| u != url);
    }
    
    /// Check if URL is blocked.
    pub fn is_url_blocked(&self, url: &str) -> bool {
        self.blocked_urls.iter().any(|pattern| {
            // Simple pattern matching
            if pattern.contains('*') {
                let parts: Vec<&str> = pattern.split('*').collect();
                if parts.is_empty() {
                    return true;
                }
                let mut pos = 0;
                for (i, part) in parts.iter().enumerate() {
                    if part.is_empty() {
                        continue;
                    }
                    if let Some(found) = url[pos..].find(part) {
                        if i == 0 && found != 0 {
                            return false;
                        }
                        pos += found + part.len();
                    } else {
                        return false;
                    }
                }
                true
            } else {
                url == pattern
            }
        })
    }
    
    /// Export to HAR format.
    pub fn export_har(&self) -> HarLog {
        let entries: Vec<HarEntry> = self.entries.values().map(|e| {
            HarEntry {
                started_date_time: format_iso8601(e.wall_time),
                time: e.duration().unwrap_or(0.0),
                request: HarRequest {
                    method: e.request.method.as_str().to_string(),
                    url: e.request.url.clone(),
                    http_version: "HTTP/1.1".to_string(),
                    cookies: Vec::new(),
                    headers: e.request.headers.iter()
                        .map(|(k, v)| HarHeader { name: k.to_string(), value: v.to_string() })
                        .collect(),
                    query_string: Vec::new(),
                    post_data: e.request.post_data.as_ref().map(|data| HarPostData {
                        mime_type: e.request.headers.content_type().unwrap_or("").to_string(),
                        text: data.clone(),
                        params: Vec::new(),
                    }),
                    headers_size: -1,
                    body_size: e.request.post_data.as_ref().map(|d| d.len() as i64).unwrap_or(-1),
                },
                response: HarResponse {
                    status: e.response.as_ref().map(|r| r.status).unwrap_or(0),
                    status_text: e.response.as_ref().map(|r| r.status_text.clone()).unwrap_or_default(),
                    http_version: "HTTP/1.1".to_string(),
                    cookies: Vec::new(),
                    headers: e.response.as_ref().map(|r| {
                        r.headers.iter()
                            .map(|(k, v)| HarHeader { name: k.to_string(), value: v.to_string() })
                            .collect()
                    }).unwrap_or_default(),
                    content: HarContent {
                        size: e.decoded_body_length,
                        compression: Some(e.decoded_body_length - e.encoded_data_length),
                        mime_type: e.response.as_ref().map(|r| r.mime_type.clone()).unwrap_or_default(),
                        text: None,
                        encoding: None,
                    },
                    redirect_url: String::new(),
                    headers_size: -1,
                    body_size: e.encoded_data_length,
                },
                cache: HarCache {},
                timings: HarTimings {
                    blocked: -1.0,
                    dns: e.response.as_ref()
                        .and_then(|r| r.timing.as_ref())
                        .and_then(|t| t.dns_time())
                        .unwrap_or(-1.0),
                    connect: e.response.as_ref()
                        .and_then(|r| r.timing.as_ref())
                        .and_then(|t| t.connect_time())
                        .unwrap_or(-1.0),
                    send: e.response.as_ref()
                        .and_then(|r| r.timing.as_ref())
                        .map(|t| t.send_end - t.send_start)
                        .unwrap_or(-1.0),
                    wait: e.response.as_ref()
                        .and_then(|r| r.timing.as_ref())
                        .map(|t| t.receive_headers_start - t.send_end)
                        .unwrap_or(-1.0),
                    receive: e.response.as_ref()
                        .and_then(|r| r.timing.as_ref())
                        .map(|t| t.receive_headers_end - t.receive_headers_start)
                        .unwrap_or(-1.0),
                    ssl: e.response.as_ref()
                        .and_then(|r| r.timing.as_ref())
                        .and_then(|t| t.ssl_time())
                        .unwrap_or(-1.0),
                },
                server_ip_address: e.response.as_ref().and_then(|r| r.remote_ip_address.clone()),
                connection: None,
            }
        }).collect();
        
        HarLog {
            version: "1.2".to_string(),
            creator: HarCreator {
                name: "KPIO DevTools".to_string(),
                version: "1.0.0".to_string(),
            },
            entries,
        }
    }
}

impl Default for NetworkPanel {
    fn default() -> Self {
        Self::new()
    }
}

/// Network throttling preset.
#[derive(Debug, Clone)]
pub struct NetworkThrottling {
    /// Download throughput (bytes per second).
    pub download_throughput: i64,
    /// Upload throughput (bytes per second).
    pub upload_throughput: i64,
    /// Latency (milliseconds).
    pub latency: f64,
    /// Offline.
    pub offline: bool,
}

impl NetworkThrottling {
    /// No throttling.
    pub fn none() -> Self {
        Self {
            download_throughput: -1,
            upload_throughput: -1,
            latency: 0.0,
            offline: false,
        }
    }
    
    /// Slow 3G.
    pub fn slow_3g() -> Self {
        Self {
            download_throughput: 500 * 1024 / 8,  // 500 kbps
            upload_throughput: 500 * 1024 / 8,    // 500 kbps
            latency: 400.0,
            offline: false,
        }
    }
    
    /// Fast 3G.
    pub fn fast_3g() -> Self {
        Self {
            download_throughput: 1500 * 1024 / 8,  // 1.5 Mbps
            upload_throughput: 750 * 1024 / 8,     // 750 kbps
            latency: 150.0,
            offline: false,
        }
    }
    
    /// Offline.
    pub fn offline() -> Self {
        Self {
            download_throughput: 0,
            upload_throughput: 0,
            latency: 0.0,
            offline: true,
        }
    }
}

/// Format timestamp as ISO 8601.
fn format_iso8601(timestamp: TimeSinceEpoch) -> String {
    // Simplified - would use proper date formatting
    alloc::format!("{}Z", timestamp)
}

// HAR format types

/// HAR log.
#[derive(Debug, Clone)]
pub struct HarLog {
    pub version: String,
    pub creator: HarCreator,
    pub entries: Vec<HarEntry>,
}

/// HAR creator.
#[derive(Debug, Clone)]
pub struct HarCreator {
    pub name: String,
    pub version: String,
}

/// HAR entry.
#[derive(Debug, Clone)]
pub struct HarEntry {
    pub started_date_time: String,
    pub time: f64,
    pub request: HarRequest,
    pub response: HarResponse,
    pub cache: HarCache,
    pub timings: HarTimings,
    pub server_ip_address: Option<String>,
    pub connection: Option<String>,
}

/// HAR request.
#[derive(Debug, Clone)]
pub struct HarRequest {
    pub method: String,
    pub url: String,
    pub http_version: String,
    pub cookies: Vec<HarCookie>,
    pub headers: Vec<HarHeader>,
    pub query_string: Vec<HarQueryParam>,
    pub post_data: Option<HarPostData>,
    pub headers_size: i64,
    pub body_size: i64,
}

/// HAR response.
#[derive(Debug, Clone)]
pub struct HarResponse {
    pub status: u16,
    pub status_text: String,
    pub http_version: String,
    pub cookies: Vec<HarCookie>,
    pub headers: Vec<HarHeader>,
    pub content: HarContent,
    pub redirect_url: String,
    pub headers_size: i64,
    pub body_size: i64,
}

/// HAR header.
#[derive(Debug, Clone)]
pub struct HarHeader {
    pub name: String,
    pub value: String,
}

/// HAR cookie.
#[derive(Debug, Clone)]
pub struct HarCookie {
    pub name: String,
    pub value: String,
}

/// HAR query parameter.
#[derive(Debug, Clone)]
pub struct HarQueryParam {
    pub name: String,
    pub value: String,
}

/// HAR POST data.
#[derive(Debug, Clone)]
pub struct HarPostData {
    pub mime_type: String,
    pub text: String,
    pub params: Vec<HarQueryParam>,
}

/// HAR content.
#[derive(Debug, Clone)]
pub struct HarContent {
    pub size: i64,
    pub compression: Option<i64>,
    pub mime_type: String,
    pub text: Option<String>,
    pub encoding: Option<String>,
}

/// HAR cache.
#[derive(Debug, Clone)]
pub struct HarCache {}

/// HAR timings.
#[derive(Debug, Clone)]
pub struct HarTimings {
    pub blocked: f64,
    pub dns: f64,
    pub connect: f64,
    pub send: f64,
    pub wait: f64,
    pub receive: f64,
    pub ssl: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_network_panel() {
        let mut panel = NetworkPanel::new();
        let request_id = panel.new_request_id();
        let loader_id = panel.new_loader_id();
        
        let request = Request::new("https://example.com/", HttpMethod::Get);
        let entry = NetworkEntry::new(
            request_id.clone(),
            loader_id,
            request,
            ResourceType::Document,
            0.0,
            1234567890.0,
        );
        
        panel.record_request(entry);
        assert!(panel.get_entry(&request_id).is_some());
    }
    
    #[test]
    fn test_url_blocking() {
        let mut panel = NetworkPanel::new();
        panel.block_url("*.example.com/*");
        
        assert!(panel.is_url_blocked("https://www.example.com/path"));
        assert!(!panel.is_url_blocked("https://other.com/path"));
    }
    
    #[test]
    fn test_throttling() {
        let slow_3g = NetworkThrottling::slow_3g();
        assert_eq!(slow_3g.latency, 400.0);
        
        let offline = NetworkThrottling::offline();
        assert!(offline.offline);
    }
}
