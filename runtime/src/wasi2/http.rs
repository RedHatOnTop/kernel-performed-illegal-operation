//! WASI Preview 2 — `wasi:http` interfaces.
//!
//! Provides outgoing HTTP request handling and related types.
//!
//! When compiled with the `kernel` feature, HTTP requests are forwarded
//! to the kernel's real TCP/IP stack (VirtIO NIC → Ethernet → IP → TCP
//! → DNS → HTTP/TLS).  Without the feature, a mock response is returned
//! so that unit tests and host-side tooling remain functional.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// HTTP Types
// ---------------------------------------------------------------------------

/// HTTP method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
    Patch,
    Other(String),
}

impl Method {
    pub fn as_str(&self) -> &str {
        match self {
            Method::Get => "GET",
            Method::Head => "HEAD",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Connect => "CONNECT",
            Method::Options => "OPTIONS",
            Method::Trace => "TRACE",
            Method::Patch => "PATCH",
            Method::Other(s) => s.as_str(),
        }
    }
}

/// HTTP scheme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scheme {
    Http,
    Https,
    Other(String),
}

/// HTTP status code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusCode(pub u16);

impl StatusCode {
    pub fn ok() -> Self {
        StatusCode(200)
    }
    pub fn not_found() -> Self {
        StatusCode(404)
    }
    pub fn internal_error() -> Self {
        StatusCode(500)
    }
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.0)
    }
    pub fn is_redirect(&self) -> bool {
        (300..400).contains(&self.0)
    }
    pub fn is_client_error(&self) -> bool {
        (400..500).contains(&self.0)
    }
    pub fn is_server_error(&self) -> bool {
        (500..600).contains(&self.0)
    }
}

/// HTTP header fields (case-insensitive keys).
#[derive(Debug, Clone)]
pub struct Fields {
    entries: BTreeMap<String, Vec<Vec<u8>>>,
}

impl Fields {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Get all values for a header name.
    pub fn get(&self, name: &str) -> Vec<Vec<u8>> {
        let key = name.to_ascii_lowercase();
        self.entries.get(&key).cloned().unwrap_or_default()
    }

    /// Set a header (replaces all existing values).
    pub fn set(&mut self, name: &str, values: Vec<Vec<u8>>) {
        let key = name.to_ascii_lowercase();
        self.entries.insert(key, values);
    }

    /// Append a value to a header.
    pub fn append(&mut self, name: &str, value: Vec<u8>) {
        let key = name.to_ascii_lowercase();
        self.entries.entry(key).or_default().push(value);
    }

    /// Delete a header.
    pub fn delete(&mut self, name: &str) {
        let key = name.to_ascii_lowercase();
        self.entries.remove(&key);
    }

    /// Check if a header exists.
    pub fn has(&self, name: &str) -> bool {
        let key = name.to_ascii_lowercase();
        self.entries.contains_key(&key)
    }

    /// Get all header names.
    pub fn names(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    /// Total number of header entries.
    pub fn len(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for Fields {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Outgoing Request
// ---------------------------------------------------------------------------

/// An outgoing HTTP request.
#[derive(Debug, Clone)]
pub struct OutgoingRequest {
    pub method: Method,
    pub scheme: Option<Scheme>,
    pub authority: Option<String>,
    pub path_with_query: Option<String>,
    pub headers: Fields,
    pub body: Vec<u8>,
}

impl OutgoingRequest {
    pub fn new(method: Method) -> Self {
        Self {
            method,
            scheme: None,
            authority: None,
            path_with_query: None,
            headers: Fields::new(),
            body: Vec::new(),
        }
    }

    pub fn get(path: &str) -> Self {
        let mut req = Self::new(Method::Get);
        req.path_with_query = Some(String::from(path));
        req
    }

    pub fn post(path: &str, body: Vec<u8>) -> Self {
        let mut req = Self::new(Method::Post);
        req.path_with_query = Some(String::from(path));
        req.body = body;
        req
    }
}

// ---------------------------------------------------------------------------
// Incoming Response
// ---------------------------------------------------------------------------

/// An incoming HTTP response.
#[derive(Debug, Clone)]
pub struct IncomingResponse {
    pub status: StatusCode,
    pub headers: Fields,
    pub body: Vec<u8>,
}

impl IncomingResponse {
    pub fn new(status: StatusCode) -> Self {
        Self {
            status,
            headers: Fields::new(),
            body: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Request Options
// ---------------------------------------------------------------------------

/// Options for outgoing HTTP requests.
#[derive(Debug, Clone)]
pub struct RequestOptions {
    /// Connect timeout in milliseconds (0 = no timeout).
    pub connect_timeout_ms: u64,
    /// First-byte timeout in milliseconds.
    pub first_byte_timeout_ms: u64,
    /// Between-bytes timeout in milliseconds.
    pub between_bytes_timeout_ms: u64,
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            connect_timeout_ms: 30_000,
            first_byte_timeout_ms: 30_000,
            between_bytes_timeout_ms: 30_000,
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP Error
// ---------------------------------------------------------------------------

/// HTTP-specific errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// DNS lookup failed.
    DnsError(String),
    /// Connection timeout.
    ConnectionTimeout,
    /// TLS error.
    TlsError(String),
    /// Protocol error (HTTP parse failure).
    ProtocolError(String),
    /// Request body too large.
    BodyTooLarge,
    /// Internal error.
    InternalError(String),
}

// ---------------------------------------------------------------------------
// Outgoing Handler
// ---------------------------------------------------------------------------

/// The outgoing-handler interface.
///
/// When the `kernel` feature is enabled, the request is forwarded to
/// the kernel's real HTTP client (DNS → TCP → TLS → HTTP).  Otherwise
/// a deterministic mock response is returned for testing.
pub fn handle(
    request: &OutgoingRequest,
    _options: Option<&RequestOptions>,
) -> Result<IncomingResponse, HttpError> {
    #[cfg(feature = "kernel")]
    {
        handle_kernel(request)
    }
    #[cfg(not(feature = "kernel"))]
    {
        handle_mock(request)
    }
}

/// Real implementation: forwards to the kernel network stack.
#[cfg(feature = "kernel")]
fn handle_kernel(request: &OutgoingRequest) -> Result<IncomingResponse, HttpError> {
    // Reconstruct the full URL from WASI fields.
    let scheme = match &request.scheme {
        Some(Scheme::Https) => "https",
        Some(Scheme::Http) | None => "http",
        Some(Scheme::Other(s)) => s.as_str(),
    };
    let authority = request.authority.as_deref().unwrap_or("localhost");
    let path = request.path_with_query.as_deref().unwrap_or("/");
    let url = alloc::format!("{}://{}{}", scheme, authority, path);

    // Collect headers into (String, String) pairs.
    let headers: alloc::vec::Vec<(alloc::string::String, alloc::string::String)> = request
        .headers
        .names()
        .iter()
        .flat_map(|name| {
            request
                .headers
                .get(name)
                .into_iter()
                .filter_map(|val| {
                    core::str::from_utf8(&val)
                        .ok()
                        .map(|v| (name.clone(), alloc::string::String::from(v)))
                })
                .collect::<alloc::vec::Vec<_>>()
        })
        .collect();

    match kpio_kernel::net::wasi_bridge::http_request(
        request.method.as_str(),
        &url,
        &headers,
        &request.body,
    ) {
        Ok(resp) => {
            let mut response = IncomingResponse::new(StatusCode(resp.status));
            response.body = resp.body;
            // Forward headers from the kernel response.
            for (k, v) in &resp.headers {
                response.headers.append(k, v.as_bytes().to_vec());
            }
            Ok(response)
        }
        Err(_e) => Err(HttpError::InternalError(alloc::format!(
            "kernel network error for {}",
            url
        ))),
    }
}

/// Mock implementation: returns a deterministic fake response.
#[cfg(not(feature = "kernel"))]
fn handle_mock(request: &OutgoingRequest) -> Result<IncomingResponse, HttpError> {
    let status = match request.method {
        Method::Get | Method::Head => StatusCode::ok(),
        Method::Post | Method::Put | Method::Patch => StatusCode::ok(),
        Method::Delete => StatusCode::ok(),
        Method::Options => StatusCode::ok(),
        _ => StatusCode::ok(),
    };

    let mut response = IncomingResponse::new(status);

    // Set standard headers
    response
        .headers
        .set("content-type", alloc::vec![b"text/plain".to_vec()]);
    response
        .headers
        .set("server", alloc::vec![b"kpio/0.1".to_vec()]);

    // Mock body
    let path = request
        .path_with_query
        .as_deref()
        .unwrap_or("/");
    let body_text = alloc::format!("KPIO Mock Response for {} {}", request.method.as_str(), path);
    response.body = body_text.into_bytes();
    response.headers.set(
        "content-length",
        alloc::vec![alloc::format!("{}", response.body.len()).into_bytes()],
    );

    Ok(response)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Method --

    #[test]
    fn method_as_str() {
        assert_eq!(Method::Get.as_str(), "GET");
        assert_eq!(Method::Post.as_str(), "POST");
        assert_eq!(Method::Other(String::from("CUSTOM")).as_str(), "CUSTOM");
    }

    // -- StatusCode --

    #[test]
    fn status_code_categories() {
        assert!(StatusCode::ok().is_success());
        assert!(!StatusCode::ok().is_redirect());
        assert!(StatusCode::not_found().is_client_error());
        assert!(StatusCode::internal_error().is_server_error());
        assert!(StatusCode(301).is_redirect());
    }

    // -- Fields --

    #[test]
    fn fields_get_set_append_delete() {
        let mut fields = Fields::new();
        assert!(fields.is_empty());

        fields.set("Content-Type", alloc::vec![b"text/html".to_vec()]);
        assert!(fields.has("content-type")); // case-insensitive
        assert_eq!(fields.get("Content-Type").len(), 1);

        fields.append("Accept", b"text/plain".to_vec());
        fields.append("Accept", b"application/json".to_vec());
        assert_eq!(fields.get("accept").len(), 2);

        fields.delete("accept");
        assert!(!fields.has("accept"));

        assert_eq!(fields.len(), 1);
    }

    #[test]
    fn fields_names() {
        let mut fields = Fields::new();
        fields.set("x-custom", alloc::vec![b"val".to_vec()]);
        fields.set("content-type", alloc::vec![b"text/plain".to_vec()]);
        let names = fields.names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&String::from("x-custom")));
    }

    // -- OutgoingRequest --

    #[test]
    fn outgoing_request_get() {
        let req = OutgoingRequest::get("/api/data");
        assert_eq!(req.method, Method::Get);
        assert_eq!(req.path_with_query.as_deref(), Some("/api/data"));
        assert!(req.body.is_empty());
    }

    #[test]
    fn outgoing_request_post() {
        let req = OutgoingRequest::post("/api/submit", b"payload".to_vec());
        assert_eq!(req.method, Method::Post);
        assert_eq!(req.body, b"payload");
    }

    #[test]
    fn outgoing_request_with_headers() {
        let mut req = OutgoingRequest::get("/");
        req.headers.set("Authorization", alloc::vec![b"Bearer token".to_vec()]);
        req.scheme = Some(Scheme::Https);
        req.authority = Some(String::from("api.example.com"));
        assert!(req.headers.has("authorization"));
        assert_eq!(req.scheme, Some(Scheme::Https));
    }

    // -- Handler --

    #[test]
    fn handle_get_returns_ok() {
        // Tests always use the mock path (no kernel feature in test builds).
        let req = OutgoingRequest::get("/index.html");
        let resp = handle(&req, None).unwrap();
        assert_eq!(resp.status, StatusCode::ok());
        assert!(resp.headers.has("content-type"));
        assert!(!resp.body.is_empty());
    }

    #[test]
    fn handle_post_returns_ok() {
        let req = OutgoingRequest::post("/submit", b"data".to_vec());
        let resp = handle(&req, None).unwrap();
        assert!(resp.status.is_success());
    }

    #[test]
    fn handle_with_options() {
        let req = OutgoingRequest::get("/");
        let opts = RequestOptions {
            connect_timeout_ms: 5000,
            first_byte_timeout_ms: 5000,
            between_bytes_timeout_ms: 1000,
        };
        let resp = handle(&req, Some(&opts)).unwrap();
        assert_eq!(resp.status.0, 200);
    }

    #[test]
    fn handle_response_body_contains_path() {
        let req = OutgoingRequest::get("/api/v2/users");
        let resp = handle(&req, None).unwrap();
        let body_str = core::str::from_utf8(&resp.body).unwrap();
        assert!(body_str.contains("/api/v2/users"));
        assert!(body_str.contains("GET"));
    }

    #[test]
    fn handle_response_content_length_matches() {
        let req = OutgoingRequest::get("/");
        let resp = handle(&req, None).unwrap();
        let cl = resp.headers.get("content-length");
        assert_eq!(cl.len(), 1);
        let len_str = core::str::from_utf8(&cl[0]).unwrap();
        let len: usize = len_str.parse().unwrap();
        assert_eq!(len, resp.body.len());
    }

    // -- IncomingResponse --

    #[test]
    fn incoming_response_default() {
        let resp = IncomingResponse::new(StatusCode(204));
        assert_eq!(resp.status.0, 204);
        assert!(resp.headers.is_empty());
        assert!(resp.body.is_empty());
    }

    // -- RequestOptions --

    #[test]
    fn request_options_default() {
        let opts = RequestOptions::default();
        assert_eq!(opts.connect_timeout_ms, 30_000);
        assert_eq!(opts.first_byte_timeout_ms, 30_000);
    }

    // -- HttpError --

    #[test]
    fn http_error_variants() {
        let e1 = HttpError::ConnectionTimeout;
        let e2 = HttpError::DnsError(String::from("not found"));
        assert_eq!(e1, HttpError::ConnectionTimeout);
        assert_ne!(e1, e2);
    }
}
