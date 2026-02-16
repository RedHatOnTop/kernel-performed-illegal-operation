//! HTTP Client Implementation
//!
//! This module provides HTTP/1.1 client functionality for the network stack.
//! It supports basic GET, POST, HEAD requests and handles chunked transfer encoding.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::NetworkError;

/// HTTP method types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
    Patch,
}

impl HttpMethod {
    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
            HttpMethod::Patch => "PATCH",
        }
    }
}

/// HTTP version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpVersion {
    Http10,
    Http11,
}

impl HttpVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpVersion::Http10 => "HTTP/1.0",
            HttpVersion::Http11 => "HTTP/1.1",
        }
    }
}

/// HTTP request.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// HTTP method.
    pub method: HttpMethod,
    /// Request path (e.g., "/index.html").
    pub path: String,
    /// HTTP version.
    pub version: HttpVersion,
    /// Request headers.
    pub headers: BTreeMap<String, String>,
    /// Request body.
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// Create a new GET request.
    pub fn get(path: &str) -> Self {
        Self {
            method: HttpMethod::Get,
            path: path.to_string(),
            version: HttpVersion::Http11,
            headers: BTreeMap::new(),
            body: Vec::new(),
        }
    }

    /// Create a new POST request.
    pub fn post(path: &str, body: Vec<u8>) -> Self {
        let mut headers = BTreeMap::new();
        headers.insert("Content-Length".to_string(), body.len().to_string());

        Self {
            method: HttpMethod::Post,
            path: path.to_string(),
            version: HttpVersion::Http11,
            headers,
            body,
        }
    }

    /// Create a new HEAD request.
    pub fn head(path: &str) -> Self {
        Self {
            method: HttpMethod::Head,
            path: path.to_string(),
            version: HttpVersion::Http11,
            headers: BTreeMap::new(),
            body: Vec::new(),
        }
    }

    /// Set a header.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.insert(name.to_string(), value.to_string());
        self
    }

    /// Set the Host header.
    pub fn host(self, host: &str) -> Self {
        self.header("Host", host)
    }

    /// Set the User-Agent header.
    pub fn user_agent(self, agent: &str) -> Self {
        self.header("User-Agent", agent)
    }

    /// Set the Accept header.
    pub fn accept(self, accept: &str) -> Self {
        self.header("Accept", accept)
    }

    /// Set the Content-Type header.
    pub fn content_type(self, content_type: &str) -> Self {
        self.header("Content-Type", content_type)
    }

    /// Serialize the request to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut request = format!(
            "{} {} {}\r\n",
            self.method.as_str(),
            self.path,
            self.version.as_str()
        );

        for (name, value) in &self.headers {
            request.push_str(&format!("{}: {}\r\n", name, value));
        }

        request.push_str("\r\n");

        let mut bytes = request.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

/// HTTP response status code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusCode(pub u16);

impl StatusCode {
    pub const OK: StatusCode = StatusCode(200);
    pub const CREATED: StatusCode = StatusCode(201);
    pub const NO_CONTENT: StatusCode = StatusCode(204);
    pub const MOVED_PERMANENTLY: StatusCode = StatusCode(301);
    pub const FOUND: StatusCode = StatusCode(302);
    pub const NOT_MODIFIED: StatusCode = StatusCode(304);
    pub const BAD_REQUEST: StatusCode = StatusCode(400);
    pub const UNAUTHORIZED: StatusCode = StatusCode(401);
    pub const FORBIDDEN: StatusCode = StatusCode(403);
    pub const NOT_FOUND: StatusCode = StatusCode(404);
    pub const INTERNAL_SERVER_ERROR: StatusCode = StatusCode(500);
    pub const BAD_GATEWAY: StatusCode = StatusCode(502);
    pub const SERVICE_UNAVAILABLE: StatusCode = StatusCode(503);

    /// Check if this is a success status (2xx).
    pub fn is_success(&self) -> bool {
        self.0 >= 200 && self.0 < 300
    }

    /// Check if this is a redirect status (3xx).
    pub fn is_redirect(&self) -> bool {
        self.0 >= 300 && self.0 < 400
    }

    /// Check if this is a client error status (4xx).
    pub fn is_client_error(&self) -> bool {
        self.0 >= 400 && self.0 < 500
    }

    /// Check if this is a server error status (5xx).
    pub fn is_server_error(&self) -> bool {
        self.0 >= 500 && self.0 < 600
    }
}

/// HTTP response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP version.
    pub version: HttpVersion,
    /// Status code.
    pub status: StatusCode,
    /// Status reason phrase.
    pub reason: String,
    /// Response headers.
    pub headers: BTreeMap<String, String>,
    /// Response body.
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Create a new empty response.
    pub fn new() -> Self {
        Self {
            version: HttpVersion::Http11,
            status: StatusCode::OK,
            reason: String::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        }
    }

    /// Get a header value.
    pub fn header(&self, name: &str) -> Option<&String> {
        // Case-insensitive header lookup
        let name_lower = name.to_ascii_lowercase();
        for (key, value) in &self.headers {
            if key.to_ascii_lowercase() == name_lower {
                return Some(value);
            }
        }
        None
    }

    /// Get Content-Length header.
    pub fn content_length(&self) -> Option<usize> {
        self.header("Content-Length").and_then(|v| v.parse().ok())
    }

    /// Get Content-Type header.
    pub fn content_type(&self) -> Option<&String> {
        self.header("Content-Type")
    }

    /// Check if response uses chunked transfer encoding.
    pub fn is_chunked(&self) -> bool {
        self.header("Transfer-Encoding")
            .map(|v| v.to_ascii_lowercase().contains("chunked"))
            .unwrap_or(false)
    }

    /// Get body as string (UTF-8).
    pub fn text(&self) -> Option<String> {
        String::from_utf8(self.body.clone()).ok()
    }

    /// Get Location header for redirects.
    pub fn location(&self) -> Option<&String> {
        self.header("Location")
    }
}

impl Default for HttpResponse {
    fn default() -> Self {
        Self::new()
    }
}

/// HTTP client error.
#[derive(Debug, Clone)]
pub enum HttpError {
    /// Network error.
    Network(String),
    /// Invalid URL.
    InvalidUrl(String),
    /// Invalid response.
    InvalidResponse(String),
    /// Timeout.
    Timeout,
    /// Too many redirects.
    TooManyRedirects,
    /// Connection closed.
    ConnectionClosed,
    /// DNS resolution failed.
    DnsError(String),
}

impl From<NetworkError> for HttpError {
    fn from(err: NetworkError) -> Self {
        HttpError::Network(format!("{:?}", err))
    }
}

/// HTTP response parser.
pub struct HttpParser {
    state: ParserState,
    response: HttpResponse,
    buffer: Vec<u8>,
    content_length: Option<usize>,
    chunked: bool,
    body_received: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ParserState {
    StatusLine,
    Headers,
    Body,
    ChunkedSize,
    ChunkedData,
    Complete,
    Error,
}

impl HttpParser {
    /// Create a new HTTP parser.
    pub fn new() -> Self {
        Self {
            state: ParserState::StatusLine,
            response: HttpResponse::new(),
            buffer: Vec::new(),
            content_length: None,
            chunked: false,
            body_received: 0,
        }
    }

    /// Feed data to the parser.
    pub fn feed(&mut self, data: &[u8]) -> Result<bool, HttpError> {
        self.buffer.extend_from_slice(data);

        loop {
            match self.state {
                ParserState::StatusLine => {
                    if !self.parse_status_line()? {
                        return Ok(false);
                    }
                }
                ParserState::Headers => {
                    if !self.parse_headers()? {
                        return Ok(false);
                    }
                }
                ParserState::Body => {
                    if !self.parse_body()? {
                        return Ok(false);
                    }
                }
                ParserState::ChunkedSize => {
                    if !self.parse_chunk_size()? {
                        return Ok(false);
                    }
                }
                ParserState::ChunkedData => {
                    if !self.parse_chunk_data()? {
                        return Ok(false);
                    }
                }
                ParserState::Complete => {
                    return Ok(true);
                }
                ParserState::Error => {
                    return Err(HttpError::InvalidResponse("Parser error".to_string()));
                }
            }
        }
    }

    /// Get the parsed response.
    pub fn response(self) -> HttpResponse {
        self.response
    }

    /// Check if parsing is complete.
    pub fn is_complete(&self) -> bool {
        self.state == ParserState::Complete
    }

    fn parse_status_line(&mut self) -> Result<bool, HttpError> {
        if let Some(pos) = self.find_crlf() {
            let line: Vec<u8> = self.buffer.drain(..pos + 2).collect();
            let line = String::from_utf8_lossy(&line[..line.len() - 2]);

            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            if parts.len() < 2 {
                return Err(HttpError::InvalidResponse(
                    "Invalid status line".to_string(),
                ));
            }

            // Parse version
            self.response.version = match parts[0] {
                "HTTP/1.0" => HttpVersion::Http10,
                "HTTP/1.1" => HttpVersion::Http11,
                _ => HttpVersion::Http11,
            };

            // Parse status code
            let status: u16 = parts[1]
                .parse()
                .map_err(|_| HttpError::InvalidResponse("Invalid status code".to_string()))?;
            self.response.status = StatusCode(status);

            // Parse reason phrase
            if parts.len() >= 3 {
                self.response.reason = parts[2].to_string();
            }

            self.state = ParserState::Headers;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn parse_headers(&mut self) -> Result<bool, HttpError> {
        loop {
            if let Some(pos) = self.find_crlf() {
                let line: Vec<u8> = self.buffer.drain(..pos + 2).collect();

                // Empty line signals end of headers
                if pos == 0 {
                    // Determine body handling
                    self.content_length = self.response.content_length();
                    self.chunked = self.response.is_chunked();

                    if self.chunked {
                        self.state = ParserState::ChunkedSize;
                    } else if let Some(len) = self.content_length {
                        if len == 0 {
                            self.state = ParserState::Complete;
                        } else {
                            self.state = ParserState::Body;
                        }
                    } else {
                        // No Content-Length and not chunked - read until connection close
                        self.state = ParserState::Body;
                    }
                    return Ok(true);
                }

                let line = String::from_utf8_lossy(&line[..line.len() - 2]);

                // Parse header
                if let Some(colon) = line.find(':') {
                    let name = line[..colon].trim().to_string();
                    let value = line[colon + 1..].trim().to_string();
                    self.response.headers.insert(name, value);
                }
            } else {
                return Ok(false);
            }
        }
    }

    fn parse_body(&mut self) -> Result<bool, HttpError> {
        if let Some(content_length) = self.content_length {
            let remaining = content_length - self.body_received;
            let available = self.buffer.len().min(remaining);

            let data: Vec<u8> = self.buffer.drain(..available).collect();
            self.response.body.extend_from_slice(&data);
            self.body_received += available;

            if self.body_received >= content_length {
                self.state = ParserState::Complete;
                return Ok(true);
            }
            Ok(false)
        } else {
            // Read all available data
            self.response.body.extend_from_slice(&self.buffer);
            self.buffer.clear();
            Ok(false)
        }
    }

    fn parse_chunk_size(&mut self) -> Result<bool, HttpError> {
        if let Some(pos) = self.find_crlf() {
            let line: Vec<u8> = self.buffer.drain(..pos + 2).collect();
            let line = String::from_utf8_lossy(&line[..line.len() - 2]);

            // Parse hex size (ignore extensions after semicolon)
            let size_str = line.split(';').next().unwrap_or("");
            let size = usize::from_str_radix(size_str.trim(), 16)
                .map_err(|_| HttpError::InvalidResponse("Invalid chunk size".to_string()))?;

            if size == 0 {
                // Last chunk
                self.state = ParserState::Complete;
                return Ok(true);
            }

            self.content_length = Some(size);
            self.body_received = 0;
            self.state = ParserState::ChunkedData;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn parse_chunk_data(&mut self) -> Result<bool, HttpError> {
        let chunk_size = self.content_length.unwrap_or(0);
        let remaining = chunk_size - self.body_received;
        let available = self.buffer.len().min(remaining);

        let data: Vec<u8> = self.buffer.drain(..available).collect();
        self.response.body.extend_from_slice(&data);
        self.body_received += available;

        if self.body_received >= chunk_size {
            // Consume trailing CRLF
            if self.buffer.len() >= 2 {
                self.buffer.drain(..2);
            }
            self.state = ParserState::ChunkedSize;
            return Ok(true);
        }
        Ok(false)
    }

    fn find_crlf(&self) -> Option<usize> {
        for i in 0..self.buffer.len().saturating_sub(1) {
            if self.buffer[i] == b'\r' && self.buffer[i + 1] == b'\n' {
                return Some(i);
            }
        }
        None
    }
}

impl Default for HttpParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed URL structure.
#[derive(Debug, Clone)]
pub struct Url {
    /// URL scheme (http or https).
    pub scheme: String,
    /// Host name.
    pub host: String,
    /// Port number.
    pub port: u16,
    /// Path.
    pub path: String,
    /// Query string.
    pub query: Option<String>,
    /// Fragment.
    pub fragment: Option<String>,
}

impl Url {
    /// Parse a URL string.
    pub fn parse(url: &str) -> Result<Self, HttpError> {
        let url = url.trim();

        // Parse scheme
        let (scheme, rest) = if let Some(pos) = url.find("://") {
            (&url[..pos], &url[pos + 3..])
        } else {
            return Err(HttpError::InvalidUrl("Missing scheme".to_string()));
        };

        let scheme = scheme.to_ascii_lowercase();
        let default_port = match scheme.as_str() {
            "http" => 80,
            "https" => 443,
            _ => return Err(HttpError::InvalidUrl("Unsupported scheme".to_string())),
        };

        // Parse host and path
        let (host_port, path_query) = if let Some(pos) = rest.find('/') {
            (&rest[..pos], &rest[pos..])
        } else {
            (rest, "/")
        };

        // Parse host and port
        let (host, port) = if let Some(pos) = host_port.rfind(':') {
            let port_str = &host_port[pos + 1..];
            let port: u16 = port_str
                .parse()
                .map_err(|_| HttpError::InvalidUrl("Invalid port".to_string()))?;
            (&host_port[..pos], port)
        } else {
            (host_port, default_port)
        };

        // Parse path, query, and fragment
        let (path_query, fragment) = if let Some(pos) = path_query.find('#') {
            (&path_query[..pos], Some(path_query[pos + 1..].to_string()))
        } else {
            (path_query, None)
        };

        let (path, query) = if let Some(pos) = path_query.find('?') {
            (&path_query[..pos], Some(path_query[pos + 1..].to_string()))
        } else {
            (path_query, None)
        };

        Ok(Self {
            scheme,
            host: host.to_string(),
            port,
            path: if path.is_empty() {
                "/".to_string()
            } else {
                path.to_string()
            },
            query,
            fragment,
        })
    }

    /// Get the full path with query string.
    pub fn path_and_query(&self) -> String {
        match &self.query {
            Some(q) => format!("{}?{}", self.path, q),
            None => self.path.clone(),
        }
    }

    /// Get host with port.
    pub fn host_port(&self) -> String {
        let default_port = match self.scheme.as_str() {
            "http" => 80,
            "https" => 443,
            _ => 0,
        };

        if self.port == default_port {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    /// Check if HTTPS.
    pub fn is_https(&self) -> bool {
        self.scheme == "https"
    }
}

/// Simple HTTP client (for testing/simulation).
///
/// Note: In the actual kernel, this would use the TCP stack.
/// This implementation provides the interface and parsing logic.
pub struct HttpClient {
    /// User-Agent string.
    user_agent: String,
    /// Maximum redirects to follow.
    max_redirects: u32,
    /// Request timeout in milliseconds.
    timeout_ms: u64,
}

impl HttpClient {
    /// Create a new HTTP client.
    pub fn new() -> Self {
        Self {
            user_agent: "KPIO-Browser/0.1".to_string(),
            max_redirects: 10,
            timeout_ms: 30000,
        }
    }

    /// Set user agent.
    pub fn user_agent(mut self, agent: &str) -> Self {
        self.user_agent = agent.to_string();
        self
    }

    /// Set maximum redirects.
    pub fn max_redirects(mut self, max: u32) -> Self {
        self.max_redirects = max;
        self
    }

    /// Set timeout.
    pub fn timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    /// Build a GET request for a URL.
    pub fn get(&self, url: &str) -> Result<HttpRequest, HttpError> {
        let parsed = Url::parse(url)?;

        Ok(HttpRequest::get(&parsed.path_and_query())
            .host(&parsed.host_port())
            .user_agent(&self.user_agent)
            .header("Accept", "*/*")
            .header("Accept-Encoding", "identity")
            .header("Connection", "close"))
    }

    /// Build a POST request for a URL.
    pub fn post(
        &self,
        url: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<HttpRequest, HttpError> {
        let parsed = Url::parse(url)?;

        Ok(HttpRequest::post(&parsed.path_and_query(), body)
            .host(&parsed.host_port())
            .user_agent(&self.user_agent)
            .content_type(content_type)
            .header("Accept", "*/*")
            .header("Connection", "close"))
    }

    /// Parse a response from raw bytes.
    pub fn parse_response(data: &[u8]) -> Result<HttpResponse, HttpError> {
        let mut parser = HttpParser::new();
        parser.feed(data)?;
        Ok(parser.response())
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_parse() {
        let url = Url::parse("http://example.com/path?query=1").unwrap();
        assert_eq!(url.scheme, "http");
        assert_eq!(url.host, "example.com");
        assert_eq!(url.port, 80);
        assert_eq!(url.path, "/path");
        assert_eq!(url.query, Some("query=1".to_string()));
    }

    #[test]
    fn test_url_with_port() {
        let url = Url::parse("https://example.com:8080/api").unwrap();
        assert_eq!(url.scheme, "https");
        assert_eq!(url.port, 8080);
    }

    #[test]
    fn test_request_serialization() {
        let request = HttpRequest::get("/index.html")
            .host("example.com")
            .user_agent("Test/1.0");

        let bytes = request.to_bytes();
        let text = String::from_utf8(bytes).unwrap();

        assert!(text.starts_with("GET /index.html HTTP/1.1\r\n"));
        assert!(text.contains("Host: example.com"));
    }

    #[test]
    fn test_response_parsing() {
        let response_data = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 13\r\n\r\nHello, World!";

        let mut parser = HttpParser::new();
        parser.feed(response_data).unwrap();

        let response = parser.response();
        assert_eq!(response.status.0, 200);
        assert_eq!(response.text(), Some("Hello, World!".to_string()));
    }
}
