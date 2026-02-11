//! HTTP/1.1 Server & Client
//!
//! - **Server**: Serves built-in `kpio://` pages at `http://localhost/`
//! - **Client**: Real HTTP client over TCP for external URLs

#![allow(dead_code)]

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use super::dns;
use super::ipv4;
use super::tcp;
use super::tls;
use super::Ipv4Addr;
use super::SocketAddr;

// ── HTTP types ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: Method,
    pub path: String,
    pub host: String,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn ok(content_type: &str, body: &[u8]) -> Self {
        HttpResponse {
            status: 200,
            status_text: String::from("OK"),
            content_type: String::from(content_type),
            body: body.to_vec(),
        }
    }
    pub fn not_found() -> Self {
        let body = b"<html><body><h1>404 Not Found</h1></body></html>";
        HttpResponse {
            status: 404,
            status_text: String::from("Not Found"),
            content_type: String::from("text/html"),
            body: body.to_vec(),
        }
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        let header = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nServer: KPIO/1.0\r\n\r\n",
            self.status, self.status_text, self.content_type, self.body.len()
        );
        let mut out = header.into_bytes();
        out.extend_from_slice(&self.body);
        out
    }
}

// ── URL parser ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ParsedUrl {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
}

pub fn parse_url(url: &str) -> Option<ParsedUrl> {
    let (scheme, rest) = if url.starts_with("https://") {
        ("https", &url[8..])
    } else if url.starts_with("http://") {
        ("http", &url[7..])
    } else {
        return None;
    };

    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };

    let (host, port) = match host_port.rfind(':') {
        Some(i) => {
            let p = host_port[i + 1..].parse::<u16>().ok()?;
            (&host_port[..i], p)
        }
        None => (host_port, if scheme == "https" { 443 } else { 80 }),
    };

    Some(ParsedUrl {
        scheme: String::from(scheme),
        host: String::from(host),
        port,
        path: String::from(path),
    })
}

// ── HTTP client ─────────────────────────────────────────────

/// Fetch a URL from the real network.
///
/// Supports `http://` and `https://` URLs. Returns the response.
pub fn get(url: &str) -> HttpResponse {
    let parsed = match parse_url(url) {
        Some(p) => p,
        None => return error_response("Invalid URL", url),
    };

    // DNS resolve
    let ip = match dns::resolve(&parsed.host) {
        Ok(entry) if !entry.addresses.is_empty() => entry.addresses[0],
        _ => return error_response("DNS resolution failed", &parsed.host),
    };

    // TCP connect
    let conn = tcp::create();
    let remote = SocketAddr {
        ip,
        port: parsed.port,
    };
    if tcp::connect(conn, remote).is_err() {
        tcp::destroy(conn);
        return error_response("TCP connection failed", url);
    }

    if parsed.scheme == "https" {
        // TLS handshake
        let mut tls_conn = match tls::TlsConnection::handshake(conn) {
            Ok(t) => t,
            Err(_) => {
                tcp::close(conn).ok();
                tcp::destroy(conn);
                return error_response("TLS handshake failed", url);
            }
        };

        // Send HTTP request over TLS
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: KPIO/1.0\r\nAccept: */*\r\nConnection: close\r\n\r\n",
            parsed.path, parsed.host
        );
        if tls_conn.send(request.as_bytes()).is_err() {
            tls_conn.close().ok();
            tcp::destroy(conn);
            return error_response("Failed to send request", url);
        }

        // Read response over TLS
        let mut response_data = Vec::new();
        let mut buf = [0u8; 4096];

        for _ in 0..500 {
            match tls_conn.recv(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    response_data.extend_from_slice(&buf[..n]);
                    if has_complete_response(&response_data) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        tls_conn.close().ok();
        tcp::destroy(conn);

        if response_data.is_empty() {
            return error_response("No response received", url);
        }

        parse_http_response(&response_data)
    } else {
        // Plain HTTP
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: KPIO/1.0\r\nAccept: */*\r\nConnection: close\r\n\r\n",
            parsed.path, parsed.host
        );
        if tcp::send(conn, request.as_bytes()).is_err() {
            tcp::close(conn).ok();
            tcp::destroy(conn);
            return error_response("Failed to send request", url);
        }

        // Read response
        let mut response_data = Vec::new();
        let mut buf = [0u8; 2048];

        for _ in 0..500 {
            super::poll_rx();
            match tcp::recv(conn, &mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => response_data.extend_from_slice(&buf[..n]),
                Err(super::NetError::WouldBlock) => {
                    if !response_data.is_empty() {
                        if has_complete_response(&response_data) {
                            break;
                        }
                    }
                    for _ in 0..50_000 {
                        core::hint::spin_loop();
                    }
                }
                Err(_) => break,
            }
        }

        tcp::close(conn).ok();
        tcp::destroy(conn);

        if response_data.is_empty() {
            return error_response("No response received", url);
        }

        parse_http_response(&response_data)
    }
}

/// Check if we have a complete HTTP response.
fn has_complete_response(data: &[u8]) -> bool {
    // Find header/body separator
    let header_end = find_header_end(data);
    if header_end.is_none() {
        return false;
    }
    let header_end = header_end.unwrap();

    let header = core::str::from_utf8(&data[..header_end]).unwrap_or("");

    // Check Content-Length
    for line in header.lines() {
        if let Some(cl) = line.strip_prefix("Content-Length: ") {
            if let Ok(len) = cl.trim().parse::<usize>() {
                return data.len() >= header_end + len;
            }
        }
    }

    // Check chunked: look for terminal 0\r\n\r\n
    if header.contains("chunked") {
        return data.ends_with(b"0\r\n\r\n") || data.ends_with(b"0\r\n");
    }

    // No content-length, no chunked — assume complete after some data
    data.len() > header_end + 1
}

/// Parse raw HTTP response bytes.
fn parse_http_response(data: &[u8]) -> HttpResponse {
    let header_end = match find_header_end(data) {
        Some(e) => e,
        None => {
            return HttpResponse {
                status: 0,
                status_text: String::from("Malformed"),
                content_type: String::from("text/plain"),
                body: Vec::from(data),
            };
        }
    };

    let header = core::str::from_utf8(&data[..header_end]).unwrap_or("");
    let mut lines = header.lines();

    // Status line
    let status_line = lines.next().unwrap_or("");
    let mut parts = status_line.splitn(3, ' ');
    let _version = parts.next().unwrap_or("");
    let status: u16 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let status_text = String::from(parts.next().unwrap_or(""));

    // Headers
    let mut content_type = String::from("text/html");
    let mut content_length: Option<usize> = None;
    let mut chunked = false;

    for line in lines {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("content-type:") {
            content_type = String::from(line[13..].trim());
        } else if lower.starts_with("content-length:") {
            content_length = line[15..].trim().parse().ok();
        } else if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
            chunked = true;
        }
    }

    // Body
    let body_data = &data[header_end..];
    let body = if chunked {
        decode_chunked(body_data)
    } else if let Some(len) = content_length {
        body_data[..len.min(body_data.len())].to_vec()
    } else {
        body_data.to_vec()
    };

    HttpResponse {
        status,
        status_text,
        content_type,
        body,
    }
}

/// Decode chunked transfer encoding.
fn decode_chunked(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut offset = 0;

    loop {
        // Find chunk size line
        let line_end = match data[offset..].windows(2).position(|w| w == b"\r\n") {
            Some(p) => offset + p,
            None => break,
        };
        let size_str = core::str::from_utf8(&data[offset..line_end]).unwrap_or("0");
        let chunk_size = usize::from_str_radix(size_str.trim(), 16).unwrap_or(0);
        if chunk_size == 0 {
            break;
        }

        offset = line_end + 2;
        let end = (offset + chunk_size).min(data.len());
        out.extend_from_slice(&data[offset..end]);
        offset = end + 2; // skip trailing \r\n
        if offset >= data.len() {
            break;
        }
    }
    out
}

fn to_ascii_lowercase(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        out.push(if c.is_ascii_uppercase() {
            (c as u8 + 32) as char
        } else {
            c
        });
    }
    out
}

fn error_response(msg: &str, detail: &str) -> HttpResponse {
    let html = format!(
        "<html><body><h1>{}</h1><p>{}</p><p><a href=\"kpio://home\">Go Home</a></p></body></html>",
        msg, detail
    );
    HttpResponse {
        status: 0,
        status_text: String::from("Error"),
        content_type: String::from("text/html"),
        body: html.into_bytes(),
    }
}

// ── Request parser (for built-in server) ────────────────────

pub fn parse_request(raw: &[u8]) -> Option<HttpRequest> {
    let text = core::str::from_utf8(raw).ok()?;
    let mut lines = text.lines();
    let first = lines.next()?;
    let mut parts = first.split_whitespace();
    let method = match parts.next()? {
        "GET" => Method::Get,
        "POST" => Method::Post,
        "PUT" => Method::Put,
        "DELETE" => Method::Delete,
        "HEAD" => Method::Head,
        "OPTIONS" => Method::Options,
        _ => return None,
    };
    let path = String::from(parts.next()?);
    let mut host = String::from("localhost");
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some(h) = line.strip_prefix("Host: ") {
            host = String::from(h.trim());
        }
    }
    let body = if let Some(pos) = find_header_end(raw) {
        raw[pos..].to_vec()
    } else {
        Vec::new()
    };
    Some(HttpRequest {
        method,
        path,
        host,
        body,
    })
}

pub fn find_header_end(data: &[u8]) -> Option<usize> {
    for i in 0..data.len().saturating_sub(3) {
        if &data[i..i + 4] == b"\r\n\r\n" {
            return Some(i + 4);
        }
    }
    None
}

// ── Local page router ───────────────────────────────────────

pub fn dispatch(req: &HttpRequest) -> HttpResponse {
    match req.path.as_str() {
        "/" | "/index.html" => page_index(),
        "/about" | "/about.html" => page_about(),
        "/status" | "/status.json" => page_status(),
        "/api/time" => api_time(),
        "/api/memory" => api_memory(),
        _ => HttpResponse::not_found(),
    }
}

fn page_index() -> HttpResponse {
    let html = r#"<!DOCTYPE html>
<html lang="ko">
<head><meta charset="utf-8"><title>KPIO OS</title>
<style>
body{font-family:sans-serif;background:#0a0e17;color:#e0e0e0;margin:40px}
h1{color:#4fc3f7}a{color:#81d4fa}
</style></head>
<body>
<h1>KPIO OS에 오신 것을 환영합니다</h1>
<p>이것은 커널 내장 웹 서버에서 제공되는 페이지입니다.</p>
<ul>
<li><a href="/about">About KPIO</a></li>
<li><a href="/status">시스템 상태 (JSON)</a></li>
<li><a href="/api/time">현재 시간 API</a></li>
<li><a href="/api/memory">메모리 정보 API</a></li>
</ul>
</body></html>"#;
    HttpResponse::ok("text/html; charset=utf-8", html.as_bytes())
}

fn page_about() -> HttpResponse {
    let html = r#"<!DOCTYPE html>
<html lang="ko">
<head><meta charset="utf-8"><title>About KPIO</title>
<style>body{font-family:sans-serif;background:#0a0e17;color:#e0e0e0;margin:40px}h1{color:#4fc3f7}</style>
</head>
<body>
<h1>About KPIO OS</h1>
<p>KPIO는 Rust로 작성된 x86_64 운영체제 커널입니다.</p>
<p>WebAssembly 런타임, Vulkan 그래픽스, 내장 브라우저를 지원합니다.</p>
<p><a href="/" style="color:#81d4fa">← 홈으로</a></p>
</body></html>"#;
    HttpResponse::ok("text/html; charset=utf-8", html.as_bytes())
}

fn page_status() -> HttpResponse {
    let stats = crate::allocator::heap_stats();
    let uptime = crate::scheduler::boot_ticks() / 100;
    let tasks = crate::scheduler::total_task_count();
    let json = format!(
        r#"{{"uptime_sec":{},"tasks":{},"heap":{{"total":{},"used":{},"free":{}}}}}"#,
        uptime, tasks, stats.total, stats.used, stats.free
    );
    HttpResponse::ok("application/json", json.as_bytes())
}

fn api_time() -> HttpResponse {
    let ticks = crate::scheduler::boot_ticks();
    let ms = ticks * 10;
    let json = format!(r#"{{"ticks":{},"ms":{},"sec":{}}}"#, ticks, ms, ms / 1000);
    HttpResponse::ok("application/json", json.as_bytes())
}

fn api_memory() -> HttpResponse {
    let stats = crate::allocator::heap_stats();
    let json = format!(
        r#"{{"total":{},"used":{},"free":{},"usage_pct":{}}}"#,
        stats.total,
        stats.used,
        stats.free,
        if stats.total > 0 {
            stats.used * 100 / stats.total
        } else {
            0
        }
    );
    HttpResponse::ok("application/json", json.as_bytes())
}

// ── Local fetch (loopback) ──────────────────────────────────

/// Fetch from built-in local server (no network needed).
pub fn fetch(path: &str) -> HttpResponse {
    let req = HttpRequest {
        method: Method::Get,
        path: String::from(path),
        host: String::from("localhost"),
        body: Vec::new(),
    };
    dispatch(&req)
}

// ── Init ────────────────────────────────────────────────────

static INITIALIZED: Mutex<bool> = Mutex::new(false);

pub fn init() {
    *INITIALIZED.lock() = true;
}

// Ascii lowercase helper for str (no_std compatible)
trait AsciiLowercase {
    fn to_ascii_lowercase(&self) -> String;
}
impl AsciiLowercase for str {
    fn to_ascii_lowercase(&self) -> String {
        let mut s = String::with_capacity(self.len());
        for c in self.chars() {
            s.push(if c.is_ascii_uppercase() {
                (c as u8 + 32) as char
            } else {
                c
            });
        }
        s
    }
}
