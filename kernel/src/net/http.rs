//! Built-in HTTP/1.1 Server & Client
//!
//! Serves local pages at `kpio://` and `http://localhost/`.
//! The server runs synchronously inside the kernel — when a TCP
//! connection to port 80 sends an HTTP request, `dispatch()` is
//! called to generate a response.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

// ── HTTP types ──────────────────────────────────────────────

/// HTTP method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
}

/// Parsed HTTP request (simplified).
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: Method,
    pub path: String,
    pub host: String,
    pub body: Vec<u8>,
}

/// HTTP response.
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

    /// Serialise to HTTP/1.1 wire format.
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

// ── Request parser ──────────────────────────────────────────

/// Parse a raw HTTP request (first request line + Host header).
pub fn parse_request(raw: &[u8]) -> Option<HttpRequest> {
    let text = core::str::from_utf8(raw).ok()?;
    let mut lines = text.lines();
    let first = lines.next()?;
    let mut parts = first.split_whitespace();
    let method = match parts.next()? {
        "GET"     => Method::Get,
        "POST"    => Method::Post,
        "PUT"     => Method::Put,
        "DELETE"  => Method::Delete,
        "HEAD"    => Method::Head,
        "OPTIONS" => Method::Options,
        _         => return None,
    };
    let path = String::from(parts.next()?);
    // _version is parts.next() — we don't use it

    let mut host = String::from("localhost");
    let mut header_done = false;
    let mut body_start = 0;
    for line in lines {
        if line.is_empty() {
            header_done = true;
            break;
        }
        if let Some(h) = line.strip_prefix("Host: ") {
            host = String::from(h.trim());
        }
    }

    // Very rough body extraction: everything after \r\n\r\n
    let body = if header_done {
        if let Some(pos) = find_header_end(raw) {
            raw[pos..].to_vec()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    Some(HttpRequest { method, path, host, body })
}

fn find_header_end(data: &[u8]) -> Option<usize> {
    for i in 0..data.len().saturating_sub(3) {
        if &data[i..i + 4] == b"\r\n\r\n" {
            return Some(i + 4);
        }
    }
    None
}

// ── Local page router ───────────────────────────────────────

/// Dispatch a request to the built-in local server.
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
        if stats.total > 0 { stats.used * 100 / stats.total } else { 0 }
    );
    HttpResponse::ok("application/json", json.as_bytes())
}

// ── HTTP client (fetch from local server) ───────────────────

/// Fetch a page from the built-in local server.
/// Works like `fetch("http://localhost/path")`.
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

/// Initialise the HTTP server (currently a no-op placeholder).
pub fn init() {
    *INITIALIZED.lock() = true;
}
