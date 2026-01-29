//! Network Protocol Fuzzing
//!
//! Fuzzing harness for network protocol parsing.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use crate::{FuzzTarget, FuzzResult};

/// HTTP Parser fuzzer
pub struct HttpFuzzer {
    /// Maximum header size
    max_header_size: usize,
    /// Maximum body size
    max_body_size: usize,
    /// Parse requests or responses
    mode: HttpMode,
}

/// HTTP parsing mode
#[derive(Debug, Clone, Copy)]
pub enum HttpMode {
    /// Parse requests
    Request,
    /// Parse responses
    Response,
    /// Parse both
    Both,
}

impl HttpFuzzer {
    /// Create new HTTP fuzzer
    pub fn new(mode: HttpMode) -> Self {
        Self {
            max_header_size: 64 * 1024,
            max_body_size: 1024 * 1024,
            mode,
        }
    }

    /// Parse HTTP request
    fn parse_request(&self, input: &[u8]) -> Result<(), HttpParseError> {
        // Find end of headers
        let header_end = self.find_header_end(input)
            .ok_or(HttpParseError::IncompleteHeaders)?;

        if header_end > self.max_header_size {
            return Err(HttpParseError::HeadersTooLarge);
        }

        let headers = &input[..header_end];
        
        // Parse request line
        let line_end = headers.iter().position(|&b| b == b'\n')
            .ok_or(HttpParseError::InvalidRequestLine)?;
        
        let request_line = &headers[..line_end];
        
        // Should have: METHOD SP PATH SP VERSION CRLF
        let parts: Vec<_> = request_line.split(|&b| b == b' ').collect();
        if parts.len() < 3 {
            return Err(HttpParseError::InvalidRequestLine);
        }

        // Validate method
        let method = parts[0];
        if !self.is_valid_method(method) {
            return Err(HttpParseError::InvalidMethod);
        }

        // Validate version
        let version = parts[parts.len() - 1].trim_ascii();
        if !version.starts_with(b"HTTP/") {
            return Err(HttpParseError::InvalidVersion);
        }

        // Parse headers
        self.parse_headers(&headers[line_end + 1..])?;

        Ok(())
    }

    /// Parse HTTP response
    fn parse_response(&self, input: &[u8]) -> Result<(), HttpParseError> {
        // Find end of headers
        let header_end = self.find_header_end(input)
            .ok_or(HttpParseError::IncompleteHeaders)?;

        if header_end > self.max_header_size {
            return Err(HttpParseError::HeadersTooLarge);
        }

        let headers = &input[..header_end];
        
        // Parse status line
        let line_end = headers.iter().position(|&b| b == b'\n')
            .ok_or(HttpParseError::InvalidStatusLine)?;
        
        let status_line = &headers[..line_end];
        
        // Should have: VERSION SP STATUS SP REASON CRLF
        if !status_line.starts_with(b"HTTP/") {
            return Err(HttpParseError::InvalidVersion);
        }

        // Parse headers
        self.parse_headers(&headers[line_end + 1..])?;

        Ok(())
    }

    fn find_header_end(&self, input: &[u8]) -> Option<usize> {
        for i in 0..input.len().saturating_sub(3) {
            if &input[i..i+4] == b"\r\n\r\n" {
                return Some(i + 4);
            }
        }
        // Also check for just \n\n
        for i in 0..input.len().saturating_sub(1) {
            if &input[i..i+2] == b"\n\n" {
                return Some(i + 2);
            }
        }
        None
    }

    fn is_valid_method(&self, method: &[u8]) -> bool {
        matches!(method, 
            b"GET" | b"POST" | b"PUT" | b"DELETE" | b"HEAD" | 
            b"OPTIONS" | b"PATCH" | b"CONNECT" | b"TRACE")
    }

    fn parse_headers(&self, _headers: &[u8]) -> Result<(), HttpParseError> {
        // Header parsing logic would go here
        Ok(())
    }
}

impl FuzzTarget for HttpFuzzer {
    fn name(&self) -> &str {
        "http_parser"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        let result = match self.mode {
            HttpMode::Request => self.parse_request(input),
            HttpMode::Response => self.parse_response(input),
            HttpMode::Both => {
                self.parse_request(input).or_else(|_| self.parse_response(input))
            }
        };

        match result {
            Ok(()) => FuzzResult::Ok,
            Err(e) => FuzzResult::ParseError(e.message()),
        }
    }

    fn reset(&mut self) {}
}

/// HTTP parsing error
#[derive(Debug)]
enum HttpParseError {
    /// Headers incomplete
    IncompleteHeaders,
    /// Headers too large
    HeadersTooLarge,
    /// Invalid request line
    InvalidRequestLine,
    /// Invalid status line
    InvalidStatusLine,
    /// Invalid method
    InvalidMethod,
    /// Invalid version
    InvalidVersion,
    /// Invalid header
    InvalidHeader,
    /// Invalid content length
    InvalidContentLength,
}

impl HttpParseError {
    fn message(&self) -> String {
        match self {
            Self::IncompleteHeaders => String::from("Incomplete headers"),
            Self::HeadersTooLarge => String::from("Headers too large"),
            Self::InvalidRequestLine => String::from("Invalid request line"),
            Self::InvalidStatusLine => String::from("Invalid status line"),
            Self::InvalidMethod => String::from("Invalid method"),
            Self::InvalidVersion => String::from("Invalid version"),
            Self::InvalidHeader => String::from("Invalid header"),
            Self::InvalidContentLength => String::from("Invalid content length"),
        }
    }
}

/// Create HTTP dictionary for fuzzing
pub fn http_dictionary() -> Vec<Vec<u8>> {
    let entries: &[&[u8]] = &[
        // Methods
        b"GET",
        b"POST",
        b"PUT",
        b"DELETE",
        b"HEAD",
        b"OPTIONS",
        b"PATCH",
        b"CONNECT",
        b"TRACE",
        // Versions
        b"HTTP/1.0",
        b"HTTP/1.1",
        b"HTTP/2",
        b"HTTP/3",
        // Common headers
        b"Host:",
        b"Content-Type:",
        b"Content-Length:",
        b"Transfer-Encoding:",
        b"Connection:",
        b"Accept:",
        b"User-Agent:",
        b"Cookie:",
        b"Set-Cookie:",
        b"Authorization:",
        b"Cache-Control:",
        b"Accept-Encoding:",
        b"Content-Encoding:",
        // Values
        b"chunked",
        b"gzip",
        b"deflate",
        b"br",
        b"identity",
        b"close",
        b"keep-alive",
        b"application/json",
        b"text/html",
        b"text/plain",
        b"multipart/form-data",
        // Line endings
        b"\r\n",
        b"\n",
        // Edge cases
        b"Transfer-Encoding: chunked",
        b"Content-Length: 0",
        b"Content-Length: -1",
        b"Content-Length: 999999999999",
        b"0\r\n\r\n",
    ];

    entries.iter().map(|e| e.to_vec()).collect()
}

/// Generate interesting HTTP inputs
pub fn generate_http_corpus() -> Vec<Vec<u8>> {
    let mut corpus = Vec::new();

    // Simple GET request
    corpus.push(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec());

    // POST with body
    corpus.push(b"POST /api HTTP/1.1\r\nHost: localhost\r\nContent-Length: 13\r\n\r\n{\"key\":\"val\"}".to_vec());

    // Chunked encoding
    corpus.push(b"POST /api HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n".to_vec());

    // Response
    corpus.push(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello".to_vec());

    // Many headers
    let mut many = b"GET / HTTP/1.1\r\n".to_vec();
    for i in 0..100 {
        many.extend_from_slice(format!("X-Header-{}: value{}\r\n", i, i).as_bytes());
    }
    many.extend_from_slice(b"\r\n");
    corpus.push(many);

    // Long header value
    let mut long = b"GET / HTTP/1.1\r\nHost: ".to_vec();
    for _ in 0..1000 {
        long.extend_from_slice(b"a");
    }
    long.extend_from_slice(b"\r\n\r\n");
    corpus.push(long);

    // Malformed
    corpus.push(b"GET\r\n\r\n".to_vec());
    corpus.push(b"HTTP/1.1 200\r\n".to_vec());
    corpus.push(b"\r\n\r\n".to_vec());

    // Request smuggling patterns
    corpus.push(b"POST / HTTP/1.1\r\nContent-Length: 5\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n".to_vec());

    // Null bytes
    corpus.push(b"GET /\x00path HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec());

    corpus
}

/// URL Parser fuzzer
pub struct UrlFuzzer {
    /// Maximum URL length
    max_length: usize,
}

impl UrlFuzzer {
    /// Create new URL fuzzer
    pub fn new() -> Self {
        Self { max_length: 8192 }
    }

    fn parse(&self, input: &[u8]) -> Result<(), UrlParseError> {
        if input.len() > self.max_length {
            return Err(UrlParseError::TooLong);
        }

        let text = core::str::from_utf8(input)
            .map_err(|_| UrlParseError::InvalidEncoding)?;

        // Basic URL validation
        if text.is_empty() {
            return Err(UrlParseError::Empty);
        }

        // Check for scheme
        if let Some(colon) = text.find(':') {
            let scheme = &text[..colon];
            if !scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
                return Err(UrlParseError::InvalidScheme);
            }
        }

        Ok(())
    }
}

impl Default for UrlFuzzer {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzTarget for UrlFuzzer {
    fn name(&self) -> &str {
        "url_parser"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        match self.parse(input) {
            Ok(()) => FuzzResult::Ok,
            Err(e) => FuzzResult::ParseError(e.message()),
        }
    }

    fn reset(&mut self) {}
}

/// URL parsing error
#[derive(Debug)]
enum UrlParseError {
    /// Too long
    TooLong,
    /// Empty
    Empty,
    /// Invalid encoding
    InvalidEncoding,
    /// Invalid scheme
    InvalidScheme,
    /// Invalid host
    InvalidHost,
    /// Invalid port
    InvalidPort,
    /// Invalid path
    InvalidPath,
}

impl UrlParseError {
    fn message(&self) -> String {
        match self {
            Self::TooLong => String::from("URL too long"),
            Self::Empty => String::from("Empty URL"),
            Self::InvalidEncoding => String::from("Invalid encoding"),
            Self::InvalidScheme => String::from("Invalid scheme"),
            Self::InvalidHost => String::from("Invalid host"),
            Self::InvalidPort => String::from("Invalid port"),
            Self::InvalidPath => String::from("Invalid path"),
        }
    }
}

/// WebSocket frame fuzzer
pub struct WebSocketFuzzer {
    /// Maximum frame size
    max_frame_size: usize,
}

impl WebSocketFuzzer {
    /// Create new WebSocket fuzzer
    pub fn new() -> Self {
        Self {
            max_frame_size: 16 * 1024 * 1024,
        }
    }
}

impl Default for WebSocketFuzzer {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzTarget for WebSocketFuzzer {
    fn name(&self) -> &str {
        "websocket_frame"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        if input.len() < 2 {
            return FuzzResult::ParseError(String::from("Frame too short"));
        }

        // Parse frame header
        let _fin = (input[0] & 0x80) != 0;
        let _rsv = (input[0] >> 4) & 0x07;
        let opcode = input[0] & 0x0F;
        let masked = (input[1] & 0x80) != 0;
        let payload_len = (input[1] & 0x7F) as usize;

        // Validate opcode
        if opcode > 0x0A || (opcode > 0x02 && opcode < 0x08) {
            return FuzzResult::Interesting(String::from("invalid opcode"));
        }

        // Extended payload length
        let (payload_len, header_len) = if payload_len == 126 {
            if input.len() < 4 {
                return FuzzResult::ParseError(String::from("Incomplete header"));
            }
            let len = u16::from_be_bytes([input[2], input[3]]) as usize;
            (len, 4)
        } else if payload_len == 127 {
            if input.len() < 10 {
                return FuzzResult::ParseError(String::from("Incomplete header"));
            }
            let len = u64::from_be_bytes([
                input[2], input[3], input[4], input[5],
                input[6], input[7], input[8], input[9],
            ]) as usize;
            (len, 10)
        } else {
            (payload_len, 2)
        };

        if payload_len > self.max_frame_size {
            return FuzzResult::Interesting(String::from("very large frame"));
        }

        let _mask_offset = if masked { header_len + 4 } else { header_len };

        FuzzResult::Ok
    }

    fn reset(&mut self) {}
}

/// TLS record fuzzer
pub struct TlsFuzzer {
    /// Maximum record size
    max_record_size: usize,
}

impl TlsFuzzer {
    /// Create new TLS fuzzer
    pub fn new() -> Self {
        Self {
            max_record_size: 16384 + 256, // TLS max record + overhead
        }
    }
}

impl Default for TlsFuzzer {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzTarget for TlsFuzzer {
    fn name(&self) -> &str {
        "tls_record"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        if input.len() < 5 {
            return FuzzResult::ParseError(String::from("Record too short"));
        }

        // TLS record header
        let content_type = input[0];
        let version_major = input[1];
        let version_minor = input[2];
        let length = u16::from_be_bytes([input[3], input[4]]) as usize;

        // Validate content type
        if !matches!(content_type, 20 | 21 | 22 | 23 | 24) {
            return FuzzResult::Interesting(String::from("unknown content type"));
        }

        // Validate version
        if version_major != 3 || version_minor > 4 {
            return FuzzResult::Interesting(String::from("unusual version"));
        }

        // Check length
        if length > self.max_record_size {
            return FuzzResult::Interesting(String::from("record too large"));
        }

        FuzzResult::Ok
    }

    fn reset(&mut self) {}
}

/// DNS message fuzzer
pub struct DnsFuzzer;

impl Default for DnsFuzzer {
    fn default() -> Self {
        Self
    }
}

impl FuzzTarget for DnsFuzzer {
    fn name(&self) -> &str {
        "dns_message"
    }

    fn fuzz(&mut self, input: &[u8]) -> FuzzResult {
        if input.len() < 12 {
            return FuzzResult::ParseError(String::from("Message too short"));
        }

        // DNS header
        let _id = u16::from_be_bytes([input[0], input[1]]);
        let _flags = u16::from_be_bytes([input[2], input[3]]);
        let qdcount = u16::from_be_bytes([input[4], input[5]]) as usize;
        let ancount = u16::from_be_bytes([input[6], input[7]]) as usize;
        let nscount = u16::from_be_bytes([input[8], input[9]]) as usize;
        let arcount = u16::from_be_bytes([input[10], input[11]]) as usize;

        // Check for unusually large counts
        let total = qdcount + ancount + nscount + arcount;
        if total > 1000 {
            return FuzzResult::Interesting(String::from("many records"));
        }

        // Check for compression loops (label pointer to self)
        for i in 12..input.len().saturating_sub(1) {
            if (input[i] & 0xC0) == 0xC0 {
                let offset = u16::from_be_bytes([input[i] & 0x3F, input[i + 1]]) as usize;
                if offset >= i {
                    return FuzzResult::Interesting(String::from("forward compression pointer"));
                }
            }
        }

        FuzzResult::Ok
    }

    fn reset(&mut self) {}
}

trait TrimAscii {
    fn trim_ascii(&self) -> &Self;
}

impl TrimAscii for [u8] {
    fn trim_ascii(&self) -> &[u8] {
        let start = self.iter().position(|&b| !b.is_ascii_whitespace()).unwrap_or(self.len());
        let end = self.iter().rposition(|&b| !b.is_ascii_whitespace()).map_or(start, |i| i + 1);
        &self[start..end]
    }
}
