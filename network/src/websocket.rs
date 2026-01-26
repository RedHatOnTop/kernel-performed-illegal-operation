//! WebSocket Protocol Implementation (RFC 6455)
//!
//! This module provides a WebSocket client implementation for real-time
//! bidirectional communication over TCP connections.

use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use core::fmt;

/// WebSocket protocol version.
pub const WEBSOCKET_VERSION: &str = "13";

/// WebSocket GUID for handshake (RFC 6455).
const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// WebSocket error types.
#[derive(Debug, Clone)]
pub enum WebSocketError {
    /// Connection failed.
    ConnectionFailed(String),
    /// Handshake failed.
    HandshakeFailed(String),
    /// Invalid URL.
    InvalidUrl(String),
    /// Invalid frame.
    InvalidFrame(String),
    /// Protocol error.
    ProtocolError(String),
    /// Connection closed.
    ConnectionClosed(CloseCode),
    /// Message too large.
    MessageTooLarge(usize),
    /// I/O error.
    IoError(String),
    /// Timeout.
    Timeout,
}

impl fmt::Display for WebSocketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WebSocketError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            WebSocketError::HandshakeFailed(msg) => write!(f, "Handshake failed: {}", msg),
            WebSocketError::InvalidUrl(url) => write!(f, "Invalid URL: {}", url),
            WebSocketError::InvalidFrame(msg) => write!(f, "Invalid frame: {}", msg),
            WebSocketError::ProtocolError(msg) => write!(f, "Protocol error: {}", msg),
            WebSocketError::ConnectionClosed(code) => write!(f, "Connection closed: {:?}", code),
            WebSocketError::MessageTooLarge(size) => write!(f, "Message too large: {} bytes", size),
            WebSocketError::IoError(msg) => write!(f, "I/O error: {}", msg),
            WebSocketError::Timeout => write!(f, "Operation timed out"),
        }
    }
}

/// WebSocket close status codes (RFC 6455 Section 7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum CloseCode {
    /// Normal closure.
    Normal = 1000,
    /// Endpoint going away.
    GoingAway = 1001,
    /// Protocol error.
    ProtocolError = 1002,
    /// Unsupported data type.
    UnsupportedData = 1003,
    /// Reserved (no status code was present).
    NoStatusReceived = 1005,
    /// Abnormal closure.
    Abnormal = 1006,
    /// Invalid frame payload data.
    InvalidPayload = 1007,
    /// Policy violation.
    PolicyViolation = 1008,
    /// Message too big.
    MessageTooBig = 1009,
    /// Mandatory extension missing.
    MandatoryExtension = 1010,
    /// Internal server error.
    InternalError = 1011,
    /// TLS handshake failure.
    TlsHandshake = 1015,
}

impl CloseCode {
    /// Parse close code from u16.
    pub fn from_u16(code: u16) -> Self {
        match code {
            1000 => CloseCode::Normal,
            1001 => CloseCode::GoingAway,
            1002 => CloseCode::ProtocolError,
            1003 => CloseCode::UnsupportedData,
            1005 => CloseCode::NoStatusReceived,
            1006 => CloseCode::Abnormal,
            1007 => CloseCode::InvalidPayload,
            1008 => CloseCode::PolicyViolation,
            1009 => CloseCode::MessageTooBig,
            1010 => CloseCode::MandatoryExtension,
            1011 => CloseCode::InternalError,
            1015 => CloseCode::TlsHandshake,
            _ => CloseCode::Normal, // Default for unknown codes
        }
    }
    
    /// Convert to u16.
    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

/// WebSocket opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    /// Continuation frame.
    Continuation = 0x0,
    /// Text frame.
    Text = 0x1,
    /// Binary frame.
    Binary = 0x2,
    /// Connection close.
    Close = 0x8,
    /// Ping.
    Ping = 0x9,
    /// Pong.
    Pong = 0xA,
}

impl Opcode {
    /// Parse opcode from u8.
    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte & 0x0F {
            0x0 => Some(Opcode::Continuation),
            0x1 => Some(Opcode::Text),
            0x2 => Some(Opcode::Binary),
            0x8 => Some(Opcode::Close),
            0x9 => Some(Opcode::Ping),
            0xA => Some(Opcode::Pong),
            _ => None,
        }
    }
    
    /// Check if this is a control opcode.
    pub fn is_control(self) -> bool {
        matches!(self, Opcode::Close | Opcode::Ping | Opcode::Pong)
    }
    
    /// Check if this is a data opcode.
    pub fn is_data(self) -> bool {
        matches!(self, Opcode::Text | Opcode::Binary | Opcode::Continuation)
    }
}

/// WebSocket frame.
#[derive(Debug, Clone)]
pub struct Frame {
    /// FIN bit - is this the final fragment?
    pub fin: bool,
    /// RSV1-3 bits (reserved for extensions).
    pub rsv: u8,
    /// Opcode.
    pub opcode: Opcode,
    /// Payload data.
    pub payload: Vec<u8>,
}

impl Frame {
    /// Create a new frame.
    pub fn new(opcode: Opcode, payload: Vec<u8>) -> Self {
        Self {
            fin: true,
            rsv: 0,
            opcode,
            payload,
        }
    }
    
    /// Create a text frame.
    pub fn text(data: &str) -> Self {
        Self::new(Opcode::Text, data.as_bytes().to_vec())
    }
    
    /// Create a binary frame.
    pub fn binary(data: Vec<u8>) -> Self {
        Self::new(Opcode::Binary, data)
    }
    
    /// Create a ping frame.
    pub fn ping(data: Vec<u8>) -> Self {
        Self::new(Opcode::Ping, data)
    }
    
    /// Create a pong frame.
    pub fn pong(data: Vec<u8>) -> Self {
        Self::new(Opcode::Pong, data)
    }
    
    /// Create a close frame.
    pub fn close(code: CloseCode, reason: &str) -> Self {
        let mut payload = Vec::with_capacity(2 + reason.len());
        let code_bytes = code.as_u16().to_be_bytes();
        payload.extend_from_slice(&code_bytes);
        payload.extend_from_slice(reason.as_bytes());
        Self::new(Opcode::Close, payload)
    }
    
    /// Encode frame to bytes (client-side, masked).
    pub fn encode(&self, mask_key: [u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        // First byte: FIN + RSV + Opcode
        let first_byte = (if self.fin { 0x80 } else { 0 })
            | ((self.rsv & 0x07) << 4)
            | (self.opcode as u8);
        bytes.push(first_byte);
        
        // Second byte: MASK + Payload length
        let len = self.payload.len();
        if len < 126 {
            bytes.push(0x80 | (len as u8)); // Mask bit set
        } else if len < 65536 {
            bytes.push(0x80 | 126);
            bytes.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            bytes.push(0x80 | 127);
            bytes.extend_from_slice(&(len as u64).to_be_bytes());
        }
        
        // Masking key
        bytes.extend_from_slice(&mask_key);
        
        // Masked payload
        for (i, byte) in self.payload.iter().enumerate() {
            bytes.push(byte ^ mask_key[i % 4]);
        }
        
        bytes
    }
    
    /// Decode frame from bytes (server response, unmasked).
    pub fn decode(data: &[u8]) -> Result<(Frame, usize), WebSocketError> {
        if data.len() < 2 {
            return Err(WebSocketError::InvalidFrame("Frame too short".to_string()));
        }
        
        let first_byte = data[0];
        let second_byte = data[1];
        
        let fin = (first_byte & 0x80) != 0;
        let rsv = (first_byte >> 4) & 0x07;
        let opcode = Opcode::from_u8(first_byte)
            .ok_or_else(|| WebSocketError::InvalidFrame("Invalid opcode".to_string()))?;
        
        let masked = (second_byte & 0x80) != 0;
        let mut payload_len = (second_byte & 0x7F) as u64;
        let mut offset = 2;
        
        // Extended payload length
        if payload_len == 126 {
            if data.len() < offset + 2 {
                return Err(WebSocketError::InvalidFrame("Frame too short".to_string()));
            }
            payload_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as u64;
            offset += 2;
        } else if payload_len == 127 {
            if data.len() < offset + 8 {
                return Err(WebSocketError::InvalidFrame("Frame too short".to_string()));
            }
            payload_len = u64::from_be_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
            ]);
            offset += 8;
        }
        
        // Control frames must not be fragmented and payload must be <= 125
        if opcode.is_control() {
            if !fin {
                return Err(WebSocketError::ProtocolError(
                    "Control frame must not be fragmented".to_string()
                ));
            }
            if payload_len > 125 {
                return Err(WebSocketError::ProtocolError(
                    "Control frame payload too large".to_string()
                ));
            }
        }
        
        // Masking key (if present)
        let mask_key = if masked {
            if data.len() < offset + 4 {
                return Err(WebSocketError::InvalidFrame("Frame too short".to_string()));
            }
            let key = [data[offset], data[offset + 1], data[offset + 2], data[offset + 3]];
            offset += 4;
            Some(key)
        } else {
            None
        };
        
        // Payload
        let payload_len = payload_len as usize;
        if data.len() < offset + payload_len {
            return Err(WebSocketError::InvalidFrame("Incomplete payload".to_string()));
        }
        
        let mut payload = data[offset..offset + payload_len].to_vec();
        
        // Unmask if needed
        if let Some(key) = mask_key {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= key[i % 4];
            }
        }
        
        let total_len = offset + payload_len;
        
        Ok((Frame { fin, rsv, opcode, payload }, total_len))
    }
    
    /// Get payload as string (for text frames).
    pub fn payload_as_str(&self) -> Result<&str, WebSocketError> {
        core::str::from_utf8(&self.payload)
            .map_err(|_| WebSocketError::InvalidFrame("Invalid UTF-8".to_string()))
    }
}

/// WebSocket connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected.
    Disconnected,
    /// Connecting (handshake in progress).
    Connecting,
    /// Connected and ready.
    Open,
    /// Closing (close frame sent).
    Closing,
    /// Closed.
    Closed,
}

/// WebSocket message (can span multiple frames).
#[derive(Debug, Clone)]
pub enum Message {
    /// Text message.
    Text(String),
    /// Binary message.
    Binary(Vec<u8>),
    /// Ping message.
    Ping(Vec<u8>),
    /// Pong message.
    Pong(Vec<u8>),
    /// Close message.
    Close(CloseCode, String),
}

impl Message {
    /// Convert to frame(s).
    pub fn to_frame(&self) -> Frame {
        match self {
            Message::Text(text) => Frame::text(text),
            Message::Binary(data) => Frame::binary(data.clone()),
            Message::Ping(data) => Frame::ping(data.clone()),
            Message::Pong(data) => Frame::pong(data.clone()),
            Message::Close(code, reason) => Frame::close(*code, reason),
        }
    }
}

/// WebSocket URL parser.
#[derive(Debug, Clone)]
pub struct WebSocketUrl {
    /// Use secure connection (wss://).
    pub secure: bool,
    /// Host.
    pub host: String,
    /// Port.
    pub port: u16,
    /// Path.
    pub path: String,
}

impl WebSocketUrl {
    /// Parse WebSocket URL.
    pub fn parse(url: &str) -> Result<Self, WebSocketError> {
        let (secure, rest) = if url.starts_with("wss://") {
            (true, &url[6..])
        } else if url.starts_with("ws://") {
            (false, &url[5..])
        } else {
            return Err(WebSocketError::InvalidUrl(
                "URL must start with ws:// or wss://".to_string()
            ));
        };
        
        // Split host:port from path
        let (host_port, path) = match rest.find('/') {
            Some(idx) => (&rest[..idx], &rest[idx..]),
            None => (rest, "/"),
        };
        
        // Parse host and port
        let (host, port) = match host_port.rfind(':') {
            Some(idx) => {
                let host = &host_port[..idx];
                let port_str = &host_port[idx + 1..];
                let port = port_str.parse::<u16>()
                    .map_err(|_| WebSocketError::InvalidUrl(
                        "Invalid port number".to_string()
                    ))?;
                (host.to_string(), port)
            }
            None => {
                let port = if secure { 443 } else { 80 };
                (host_port.to_string(), port)
            }
        };
        
        if host.is_empty() {
            return Err(WebSocketError::InvalidUrl("Empty host".to_string()));
        }
        
        Ok(Self {
            secure,
            host,
            port,
            path: path.to_string(),
        })
    }
}

/// Simple random number generator for masking keys.
pub struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    /// Create new RNG with seed.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    
    /// Generate next random u32.
    pub fn next_u32(&mut self) -> u32 {
        // xorshift64
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state as u32
    }
    
    /// Generate masking key.
    pub fn mask_key(&mut self) -> [u8; 4] {
        let n = self.next_u32();
        n.to_be_bytes()
    }
}

/// Base64 encoding for handshake.
pub fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    
    let mut output = String::new();
    let mut i = 0;
    
    while i < input.len() {
        let b0 = input[i];
        let b1 = if i + 1 < input.len() { input[i + 1] } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] } else { 0 };
        
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        
        output.push(ALPHABET[(n >> 18) as usize & 0x3F] as char);
        output.push(ALPHABET[(n >> 12) as usize & 0x3F] as char);
        
        if i + 1 < input.len() {
            output.push(ALPHABET[(n >> 6) as usize & 0x3F] as char);
        } else {
            output.push('=');
        }
        
        if i + 2 < input.len() {
            output.push(ALPHABET[n as usize & 0x3F] as char);
        } else {
            output.push('=');
        }
        
        i += 3;
    }
    
    output
}

/// Simple SHA-1 implementation for WebSocket handshake.
pub fn sha1(input: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0
    ];
    
    // Pre-processing: adding padding bits
    let ml = (input.len() as u64) * 8;
    let mut msg = input.to_vec();
    msg.push(0x80);
    
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    
    msg.extend_from_slice(&ml.to_be_bytes());
    
    // Process each 64-byte chunk
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        
        // Break chunk into sixteen 32-bit big-endian words
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        
        // Extend the sixteen 32-bit words into eighty 32-bit words
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };
            
            let temp = a.rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    
    let mut result = [0u8; 20];
    for (i, &val) in h.iter().enumerate() {
        result[i * 4..(i + 1) * 4].copy_from_slice(&val.to_be_bytes());
    }
    
    result
}

/// Generate WebSocket accept key from client key.
pub fn generate_accept_key(client_key: &str) -> String {
    let mut combined = String::with_capacity(client_key.len() + WEBSOCKET_GUID.len());
    combined.push_str(client_key);
    combined.push_str(WEBSOCKET_GUID);
    
    let hash = sha1(combined.as_bytes());
    base64_encode(&hash)
}

/// WebSocket handshake builder.
pub struct HandshakeBuilder {
    url: WebSocketUrl,
    key: String,
    protocols: Vec<String>,
    extensions: Vec<String>,
    headers: Vec<(String, String)>,
}

impl HandshakeBuilder {
    /// Create new handshake builder.
    pub fn new(url: WebSocketUrl, rng: &mut SimpleRng) -> Self {
        // Generate random 16-byte key
        let mut key_bytes = [0u8; 16];
        for chunk in key_bytes.chunks_mut(4) {
            let n = rng.next_u32();
            chunk.copy_from_slice(&n.to_be_bytes());
        }
        let key = base64_encode(&key_bytes);
        
        Self {
            url,
            key,
            protocols: Vec::new(),
            extensions: Vec::new(),
            headers: Vec::new(),
        }
    }
    
    /// Add subprotocol.
    pub fn protocol(mut self, protocol: &str) -> Self {
        self.protocols.push(protocol.to_string());
        self
    }
    
    /// Add extension.
    pub fn extension(mut self, extension: &str) -> Self {
        self.extensions.push(extension.to_string());
        self
    }
    
    /// Add custom header.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }
    
    /// Build HTTP upgrade request.
    pub fn build_request(&self) -> Vec<u8> {
        let mut request = String::new();
        
        // Request line
        request.push_str("GET ");
        request.push_str(&self.url.path);
        request.push_str(" HTTP/1.1\r\n");
        
        // Required headers
        request.push_str("Host: ");
        request.push_str(&self.url.host);
        if (self.url.secure && self.url.port != 443) || (!self.url.secure && self.url.port != 80) {
            request.push(':');
            // Format port number manually
            let mut port_buf = [0u8; 5];
            let port_str = format_u16(self.url.port, &mut port_buf);
            request.push_str(port_str);
        }
        request.push_str("\r\n");
        
        request.push_str("Upgrade: websocket\r\n");
        request.push_str("Connection: Upgrade\r\n");
        request.push_str("Sec-WebSocket-Key: ");
        request.push_str(&self.key);
        request.push_str("\r\n");
        request.push_str("Sec-WebSocket-Version: ");
        request.push_str(WEBSOCKET_VERSION);
        request.push_str("\r\n");
        
        // Optional subprotocols
        if !self.protocols.is_empty() {
            request.push_str("Sec-WebSocket-Protocol: ");
            request.push_str(&self.protocols.join(", "));
            request.push_str("\r\n");
        }
        
        // Optional extensions
        if !self.extensions.is_empty() {
            request.push_str("Sec-WebSocket-Extensions: ");
            request.push_str(&self.extensions.join(", "));
            request.push_str("\r\n");
        }
        
        // Custom headers
        for (name, value) in &self.headers {
            request.push_str(name);
            request.push_str(": ");
            request.push_str(value);
            request.push_str("\r\n");
        }
        
        request.push_str("\r\n");
        
        request.into_bytes()
    }
    
    /// Get the expected accept key.
    pub fn expected_accept_key(&self) -> String {
        generate_accept_key(&self.key)
    }
    
    /// Get client key.
    pub fn client_key(&self) -> &str {
        &self.key
    }
}

/// Format u16 to string (no_std helper).
fn format_u16(n: u16, buf: &mut [u8; 5]) -> &str {
    let mut n = n;
    let mut i = buf.len();
    
    if n == 0 {
        buf[4] = b'0';
        return core::str::from_utf8(&buf[4..]).unwrap();
    }
    
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    
    core::str::from_utf8(&buf[i..]).unwrap()
}

/// WebSocket client.
pub struct WebSocketClient {
    /// Connection state.
    state: ConnectionState,
    /// Incoming frame buffer.
    recv_buffer: Vec<u8>,
    /// Outgoing frame queue.
    send_queue: VecDeque<Vec<u8>>,
    /// Fragmented message buffer.
    fragment_buffer: Vec<u8>,
    /// Fragment message opcode.
    fragment_opcode: Option<Opcode>,
    /// Random number generator.
    rng: SimpleRng,
    /// Maximum message size.
    max_message_size: usize,
    /// Negotiated protocol.
    protocol: Option<String>,
    /// Negotiated extensions.
    extensions: Vec<String>,
}

impl WebSocketClient {
    /// Create new WebSocket client.
    pub fn new(seed: u64) -> Self {
        Self {
            state: ConnectionState::Disconnected,
            recv_buffer: Vec::new(),
            send_queue: VecDeque::new(),
            fragment_buffer: Vec::new(),
            fragment_opcode: None,
            rng: SimpleRng::new(seed),
            max_message_size: 16 * 1024 * 1024, // 16MB default
            protocol: None,
            extensions: Vec::new(),
        }
    }
    
    /// Set maximum message size.
    pub fn max_message_size(mut self, size: usize) -> Self {
        self.max_message_size = size;
        self
    }
    
    /// Get connection state.
    pub fn state(&self) -> ConnectionState {
        self.state
    }
    
    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Open
    }
    
    /// Get negotiated protocol.
    pub fn protocol(&self) -> Option<&str> {
        self.protocol.as_deref()
    }
    
    /// Create handshake for URL.
    pub fn create_handshake(&mut self, url: &str) -> Result<HandshakeBuilder, WebSocketError> {
        let ws_url = WebSocketUrl::parse(url)?;
        self.state = ConnectionState::Connecting;
        Ok(HandshakeBuilder::new(ws_url, &mut self.rng))
    }
    
    /// Process handshake response.
    pub fn process_handshake_response(
        &mut self, 
        response: &[u8],
        expected_accept: &str,
    ) -> Result<bool, WebSocketError> {
        // Find end of HTTP headers
        let header_end = find_header_end(response);
        if header_end.is_none() {
            return Ok(false); // Need more data
        }
        
        let header_end = header_end.unwrap();
        let headers = core::str::from_utf8(&response[..header_end])
            .map_err(|_| WebSocketError::HandshakeFailed("Invalid UTF-8 in response".to_string()))?;
        
        // Check status line
        let first_line = headers.lines().next()
            .ok_or_else(|| WebSocketError::HandshakeFailed("Empty response".to_string()))?;
        
        if !first_line.contains("101") {
            return Err(WebSocketError::HandshakeFailed(
                first_line.to_string()
            ));
        }
        
        // Validate required headers
        let mut upgrade_ok = false;
        let mut connection_ok = false;
        let mut accept_ok = false;
        
        for line in headers.lines().skip(1) {
            if let Some((name, value)) = line.split_once(':') {
                let name = name.trim().to_ascii_lowercase();
                let value = value.trim();
                
                match name.as_str() {
                    "upgrade" => {
                        upgrade_ok = value.eq_ignore_ascii_case("websocket");
                    }
                    "connection" => {
                        connection_ok = value.to_ascii_lowercase().contains("upgrade");
                    }
                    "sec-websocket-accept" => {
                        accept_ok = value == expected_accept;
                    }
                    "sec-websocket-protocol" => {
                        self.protocol = Some(value.to_string());
                    }
                    "sec-websocket-extensions" => {
                        self.extensions = value.split(',')
                            .map(|s| s.trim().to_string())
                            .collect();
                    }
                    _ => {}
                }
            }
        }
        
        if !upgrade_ok {
            return Err(WebSocketError::HandshakeFailed(
                "Missing or invalid Upgrade header".to_string()
            ));
        }
        if !connection_ok {
            return Err(WebSocketError::HandshakeFailed(
                "Missing or invalid Connection header".to_string()
            ));
        }
        if !accept_ok {
            return Err(WebSocketError::HandshakeFailed(
                "Invalid Sec-WebSocket-Accept".to_string()
            ));
        }
        
        self.state = ConnectionState::Open;
        
        // Store remaining data in recv buffer
        if response.len() > header_end + 4 {
            self.recv_buffer.extend_from_slice(&response[header_end + 4..]);
        }
        
        Ok(true)
    }
    
    /// Send a message.
    pub fn send(&mut self, message: Message) -> Result<(), WebSocketError> {
        if self.state != ConnectionState::Open {
            return Err(WebSocketError::ProtocolError(
                "Connection not open".to_string()
            ));
        }
        
        let frame = message.to_frame();
        let mask_key = self.rng.mask_key();
        let encoded = frame.encode(mask_key);
        
        self.send_queue.push_back(encoded);
        Ok(())
    }
    
    /// Send text message.
    pub fn send_text(&mut self, text: &str) -> Result<(), WebSocketError> {
        self.send(Message::Text(text.to_string()))
    }
    
    /// Send binary message.
    pub fn send_binary(&mut self, data: Vec<u8>) -> Result<(), WebSocketError> {
        self.send(Message::Binary(data))
    }
    
    /// Send ping.
    pub fn ping(&mut self, data: Vec<u8>) -> Result<(), WebSocketError> {
        self.send(Message::Ping(data))
    }
    
    /// Close connection.
    pub fn close(&mut self, code: CloseCode, reason: &str) -> Result<(), WebSocketError> {
        if self.state != ConnectionState::Open && self.state != ConnectionState::Closing {
            return Ok(());
        }
        
        self.state = ConnectionState::Closing;
        self.send(Message::Close(code, reason.to_string()))
    }
    
    /// Get next frame to send.
    pub fn next_outgoing(&mut self) -> Option<Vec<u8>> {
        self.send_queue.pop_front()
    }
    
    /// Feed received data.
    pub fn feed(&mut self, data: &[u8]) {
        self.recv_buffer.extend_from_slice(data);
    }
    
    /// Try to receive a message.
    pub fn recv(&mut self) -> Result<Option<Message>, WebSocketError> {
        loop {
            if self.recv_buffer.is_empty() {
                return Ok(None);
            }
            
            // Try to decode a frame
            let (frame, consumed) = match Frame::decode(&self.recv_buffer) {
                Ok(result) => result,
                Err(WebSocketError::InvalidFrame(_)) if self.recv_buffer.len() < 2 => {
                    return Ok(None); // Need more data
                }
                Err(e) => return Err(e),
            };
            
            // Remove consumed bytes
            self.recv_buffer.drain(..consumed);
            
            // Handle control frames immediately
            if frame.opcode.is_control() {
                match frame.opcode {
                    Opcode::Ping => {
                        // Auto-respond with pong
                        let pong = Message::Pong(frame.payload.clone());
                        let pong_frame = pong.to_frame();
                        let mask_key = self.rng.mask_key();
                        self.send_queue.push_back(pong_frame.encode(mask_key));
                        return Ok(Some(Message::Ping(frame.payload)));
                    }
                    Opcode::Pong => {
                        return Ok(Some(Message::Pong(frame.payload)));
                    }
                    Opcode::Close => {
                        let (code, reason) = if frame.payload.len() >= 2 {
                            let code = u16::from_be_bytes([frame.payload[0], frame.payload[1]]);
                            let reason = core::str::from_utf8(&frame.payload[2..])
                                .unwrap_or("")
                                .to_string();
                            (CloseCode::from_u16(code), reason)
                        } else {
                            (CloseCode::Normal, String::new())
                        };
                        
                        // Send close response if we haven't already
                        if self.state == ConnectionState::Open {
                            self.state = ConnectionState::Closing;
                            let close_frame = Frame::close(code, &reason);
                            let mask_key = self.rng.mask_key();
                            self.send_queue.push_back(close_frame.encode(mask_key));
                        }
                        
                        self.state = ConnectionState::Closed;
                        return Ok(Some(Message::Close(code, reason)));
                    }
                    _ => unreachable!(),
                }
            }
            
            // Handle data frames
            match frame.opcode {
                Opcode::Text | Opcode::Binary => {
                    if self.fragment_opcode.is_some() {
                        return Err(WebSocketError::ProtocolError(
                            "Expected continuation frame".to_string()
                        ));
                    }
                    
                    if frame.fin {
                        // Complete message
                        return self.complete_message(frame.opcode, frame.payload);
                    } else {
                        // Start fragmented message
                        self.fragment_opcode = Some(frame.opcode);
                        self.fragment_buffer = frame.payload;
                    }
                }
                Opcode::Continuation => {
                    let opcode = self.fragment_opcode
                        .ok_or_else(|| WebSocketError::ProtocolError(
                            "Unexpected continuation frame".to_string()
                        ))?;
                    
                    // Check size limit
                    if self.fragment_buffer.len() + frame.payload.len() > self.max_message_size {
                        return Err(WebSocketError::MessageTooLarge(
                            self.fragment_buffer.len() + frame.payload.len()
                        ));
                    }
                    
                    self.fragment_buffer.extend_from_slice(&frame.payload);
                    
                    if frame.fin {
                        // Complete fragmented message
                        let payload = core::mem::take(&mut self.fragment_buffer);
                        self.fragment_opcode = None;
                        return self.complete_message(opcode, payload);
                    }
                }
                _ => {}
            }
        }
    }
    
    /// Complete a message from payload.
    fn complete_message(&self, opcode: Opcode, payload: Vec<u8>) -> Result<Option<Message>, WebSocketError> {
        match opcode {
            Opcode::Text => {
                let text = core::str::from_utf8(&payload)
                    .map_err(|_| WebSocketError::InvalidFrame("Invalid UTF-8 in text message".to_string()))?
                    .to_string();
                Ok(Some(Message::Text(text)))
            }
            Opcode::Binary => {
                Ok(Some(Message::Binary(payload)))
            }
            _ => Ok(None),
        }
    }
}

impl Default for WebSocketClient {
    fn default() -> Self {
        Self::new(0x12345678_9ABCDEF0)
    }
}

/// Find end of HTTP headers (\r\n\r\n).
fn find_header_end(data: &[u8]) -> Option<usize> {
    for i in 0..data.len().saturating_sub(3) {
        if data[i..i + 4] == *b"\r\n\r\n" {
            return Some(i);
        }
    }
    None
}
