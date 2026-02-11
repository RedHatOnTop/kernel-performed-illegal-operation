//! WebSocket protocol implementation (RFC 6455)
//!
//! Supports:
//!   - ws:// (plain TCP) and wss:// (TLS-encrypted)
//!   - Text and Binary frames
//!   - Ping/Pong keep-alive
//!   - Close handshake
//!   - Client-side masking

#![allow(dead_code)]

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use super::dns;
use super::tcp::{self, ConnId};
use super::tls;
use super::tls13;
use super::http::parse_url;
use super::{Ipv4Addr, SocketAddr, NetError};
use super::crypto::random::csprng_fill;

// ── WebSocket opcodes ───────────────────────────────────────

const OP_CONTINUATION: u8 = 0x00;
const OP_TEXT: u8 = 0x01;
const OP_BINARY: u8 = 0x02;
const OP_CLOSE: u8 = 0x08;
const OP_PING: u8 = 0x09;
const OP_PONG: u8 = 0x0A;

// ── WebSocket connection ────────────────────────────────────

/// Transport abstraction for ws/wss.
enum Transport {
    Plain(ConnId),
    Tls13(tls13::Tls13Connection),
    Tls12(tls::TlsConnection),
}

impl Transport {
    fn send(&mut self, data: &[u8]) -> Result<(), NetError> {
        match self {
            Transport::Plain(id) => { tcp::send(*id, data)?; Ok(()) }
            Transport::Tls13(conn) => conn.send(data),
            Transport::Tls12(conn) => conn.send(data),
        }
    }

    fn recv(&mut self, buf: &mut [u8]) -> Result<usize, NetError> {
        match self {
            Transport::Plain(id) => {
                super::poll_rx();
                tcp::recv(*id, buf)
            }
            Transport::Tls13(conn) => conn.recv(buf),
            Transport::Tls12(conn) => conn.recv(buf),
        }
    }

    fn close(&mut self) -> Result<(), NetError> {
        match self {
            Transport::Plain(id) => tcp::close(*id),
            Transport::Tls13(conn) => conn.close(),
            Transport::Tls12(conn) => conn.close(),
        }
    }

    fn tcp_id(&self) -> Option<ConnId> {
        match self {
            Transport::Plain(id) => Some(*id),
            _ => None,
        }
    }
}

/// A WebSocket connection.
pub struct WebSocketConnection {
    transport: Transport,
    connected: bool,
    recv_buf: Vec<u8>,
}

/// WebSocket message types.
#[derive(Debug, Clone)]
pub enum WsMessage {
    Text(String),
    Binary(Vec<u8>),
    Close(u16, String),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
}

impl WebSocketConnection {
    /// Connect to a WebSocket server.
    ///
    /// URL format: `ws://host:port/path` or `wss://host:port/path`
    pub fn connect(url: &str) -> Result<Self, NetError> {
        // Parse URL — rewrite scheme for parse_url
        let http_url = if url.starts_with("wss://") {
            format!("https://{}", &url[6..])
        } else if url.starts_with("ws://") {
            format!("http://{}", &url[5..])
        } else {
            return Err(NetError::InvalidArgument);
        };

        let parsed = parse_url(&http_url).ok_or(NetError::InvalidArgument)?;
        let is_secure = url.starts_with("wss://");

        // DNS resolve
        let ip = dns::resolve(&parsed.host)
            .map_err(|_| NetError::DnsNotFound)?
            .addresses.first().copied()
            .ok_or(NetError::DnsNotFound)?;

        // TCP connect
        let conn = tcp::create();
        let remote = SocketAddr { ip, port: parsed.port };
        tcp::connect(conn, remote)?;

        // Establish transport (plain or TLS)
        let mut transport = if is_secure {
            match tls13::Tls13Connection::handshake(conn, &parsed.host) {
                Ok(tls_conn) => Transport::Tls13(tls_conn),
                Err(_) => {
                    // Fallback to TLS 1.2
                    tcp::destroy(conn);
                    let conn2 = tcp::create();
                    tcp::connect(conn2, remote)?;
                    let tls_conn = tls::TlsConnection::handshake(conn2)
                        .map_err(|_| NetError::TlsHandshakeFailed)?;
                    Transport::Tls12(tls_conn)
                }
            }
        } else {
            Transport::Plain(conn)
        };

        // Generate WebSocket key
        let mut key_bytes = [0u8; 16];
        csprng_fill(&mut key_bytes);
        let ws_key = base64_encode(&key_bytes);

        // Send WebSocket upgrade request
        let request = format!(
            "GET {} HTTP/1.1\r\n\
             Host: {}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: {}\r\n\
             Sec-WebSocket-Version: 13\r\n\
             \r\n",
            parsed.path, parsed.host, ws_key
        );
        transport.send(request.as_bytes())?;

        // Read handshake response
        let mut response = Vec::new();
        let mut buf = [0u8; 2048];
        for _ in 0..100 {
            match transport.recv(&mut buf) {
                Ok(n) if n > 0 => {
                    response.extend_from_slice(&buf[..n]);
                    if response.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                _ => {
                    for _ in 0..50_000 { core::hint::spin_loop(); }
                }
            }
        }

        // Verify 101 Switching Protocols
        let resp_str = core::str::from_utf8(&response).unwrap_or("");
        let has_101 = resp_str.contains("101");
        let has_upgrade = resp_str.as_bytes().windows(7).any(|w| {
            w.eq_ignore_ascii_case(b"upgrade")
        });
        if !has_101 || !has_upgrade {
            transport.close().ok();
            return Err(NetError::ConnectionRefused);
        }

        Ok(WebSocketConnection {
            transport,
            connected: true,
            recv_buf: Vec::new(),
        })
    }

    /// Send a text message.
    pub fn send_text(&mut self, text: &str) -> Result<(), NetError> {
        self.send_frame(OP_TEXT, text.as_bytes())
    }

    /// Send a binary message.
    pub fn send_binary(&mut self, data: &[u8]) -> Result<(), NetError> {
        self.send_frame(OP_BINARY, data)
    }

    /// Send a ping frame.
    pub fn send_ping(&mut self, data: &[u8]) -> Result<(), NetError> {
        self.send_frame(OP_PING, data)
    }

    /// Receive a WebSocket message.
    pub fn recv_message(&mut self) -> Result<WsMessage, NetError> {
        // Read data into buffer
        let mut buf = [0u8; 4096];
        match self.transport.recv(&mut buf) {
            Ok(n) if n > 0 => self.recv_buf.extend_from_slice(&buf[..n]),
            Err(e) if e != NetError::WouldBlock => return Err(e),
            _ => {}
        }

        // Parse frame from buffer
        if self.recv_buf.len() < 2 {
            return Err(NetError::WouldBlock);
        }

        let fin = (self.recv_buf[0] & 0x80) != 0;
        let opcode = self.recv_buf[0] & 0x0F;
        let masked = (self.recv_buf[1] & 0x80) != 0;
        let mut payload_len = (self.recv_buf[1] & 0x7F) as u64;
        let mut offset = 2usize;

        if payload_len == 126 {
            if self.recv_buf.len() < 4 { return Err(NetError::WouldBlock); }
            payload_len = u16::from_be_bytes([self.recv_buf[2], self.recv_buf[3]]) as u64;
            offset = 4;
        } else if payload_len == 127 {
            if self.recv_buf.len() < 10 { return Err(NetError::WouldBlock); }
            payload_len = u64::from_be_bytes([
                self.recv_buf[2], self.recv_buf[3], self.recv_buf[4], self.recv_buf[5],
                self.recv_buf[6], self.recv_buf[7], self.recv_buf[8], self.recv_buf[9],
            ]);
            offset = 10;
        }

        let mask_key = if masked {
            if self.recv_buf.len() < offset + 4 { return Err(NetError::WouldBlock); }
            let key = [
                self.recv_buf[offset], self.recv_buf[offset + 1],
                self.recv_buf[offset + 2], self.recv_buf[offset + 3],
            ];
            offset += 4;
            Some(key)
        } else {
            None
        };

        let plen = payload_len as usize;
        if self.recv_buf.len() < offset + plen {
            return Err(NetError::WouldBlock);
        }

        let mut payload = self.recv_buf[offset..offset + plen].to_vec();
        self.recv_buf.drain(..offset + plen);

        // Unmask if needed
        if let Some(key) = mask_key {
            for i in 0..payload.len() {
                payload[i] ^= key[i % 4];
            }
        }

        match opcode {
            OP_TEXT => {
                let text = String::from_utf8(payload).unwrap_or_default();
                Ok(WsMessage::Text(text))
            }
            OP_BINARY => Ok(WsMessage::Binary(payload)),
            OP_CLOSE => {
                let (code, reason) = if payload.len() >= 2 {
                    let code = u16::from_be_bytes([payload[0], payload[1]]);
                    let reason = String::from_utf8(payload[2..].to_vec()).unwrap_or_default();
                    (code, reason)
                } else {
                    (1000, String::new())
                };
                // Send close frame back
                let _ = self.send_frame(OP_CLOSE, &code.to_be_bytes());
                self.connected = false;
                Ok(WsMessage::Close(code, reason))
            }
            OP_PING => {
                // Auto-respond with pong
                let _ = self.send_frame(OP_PONG, &payload);
                Ok(WsMessage::Ping(payload))
            }
            OP_PONG => Ok(WsMessage::Pong(payload)),
            _ => Err(NetError::InvalidArgument),
        }
    }

    /// Close the WebSocket connection.
    pub fn close(&mut self) -> Result<(), NetError> {
        if self.connected {
            let _ = self.send_frame(OP_CLOSE, &1000u16.to_be_bytes());
            self.connected = false;
        }
        self.transport.close()
    }

    /// Check if the connection is open.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    // ── Frame I/O ───────────────────────────────────────────

    fn send_frame(&mut self, opcode: u8, data: &[u8]) -> Result<(), NetError> {
        let mut frame = Vec::new();
        // FIN + opcode
        frame.push(0x80 | opcode);

        // Client frames MUST be masked (RFC 6455 §5.1)
        let mask_bit = 0x80u8;
        if data.len() < 126 {
            frame.push(mask_bit | data.len() as u8);
        } else if data.len() < 65536 {
            frame.push(mask_bit | 126);
            frame.push((data.len() >> 8) as u8);
            frame.push(data.len() as u8);
        } else {
            frame.push(mask_bit | 127);
            let len = data.len() as u64;
            frame.extend_from_slice(&len.to_be_bytes());
        }

        // Masking key (random)
        let mut mask_key = [0u8; 4];
        csprng_fill(&mut mask_key);
        frame.extend_from_slice(&mask_key);

        // Masked payload
        for (i, &byte) in data.iter().enumerate() {
            frame.push(byte ^ mask_key[i % 4]);
        }

        self.transport.send(&frame)
    }
}

// ── Utilities ───────────────────────────────────────────────

/// Base64 encode (for Sec-WebSocket-Key).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() { data[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if i + 1 < data.len() {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if i + 2 < data.len() {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        i += 3;
    }
    result
}
