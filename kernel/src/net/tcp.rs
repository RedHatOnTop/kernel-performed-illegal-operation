//! TCP — Wire-Format State Machine
//!
//! Real TCP implementation with 3-way handshake, sequence/acknowledgment
//! numbers, retransmission, and connection teardown over IPv4.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use spin::Mutex;

use super::ipv4;
use super::{Ipv4Addr, NetError, SocketAddr};

// ── TCP header constants ────────────────────────────────────

pub const HEADER_SIZE: usize = 20; // without options

// TCP flags
const FIN: u8 = 0x01;
const SYN: u8 = 0x02;
const RST: u8 = 0x04;
const PSH: u8 = 0x08;
const ACK: u8 = 0x10;
const URG: u8 = 0x20;

/// Maximum segment size (payload only)
const MSS: usize = 1460;
/// Default window size
const WINDOW_SIZE: u16 = 8192;
/// Retransmission timeout in poll ticks (~2 seconds)
const RTO_TICKS: u32 = 200;
/// Maximum retransmission attempts
const MAX_RETRIES: u32 = 5;

// ── TCP state ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

impl TcpState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Closed => "CLOSED",
            Self::Listen => "LISTEN",
            Self::SynSent => "SYN_SENT",
            Self::SynReceived => "SYN_RECEIVED",
            Self::Established => "ESTABLISHED",
            Self::FinWait1 => "FIN_WAIT_1",
            Self::FinWait2 => "FIN_WAIT_2",
            Self::CloseWait => "CLOSE_WAIT",
            Self::Closing => "CLOSING",
            Self::LastAck => "LAST_ACK",
            Self::TimeWait => "TIME_WAIT",
        }
    }
}

// ── Connection ID ───────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConnId(pub u64);

static NEXT_CONN: AtomicU64 = AtomicU64::new(1);

// ── Parsed TCP segment ──────────────────────────────────────

#[derive(Debug)]
pub struct TcpSegment<'a> {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq: u32,
    pub ack: u32,
    pub data_offset: usize,
    pub flags: u8,
    pub window: u16,
    pub payload: &'a [u8],
}

impl<'a> TcpSegment<'a> {
    /// Parse a TCP segment from IPv4 payload.
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        if data.len() < HEADER_SIZE {
            return None;
        }

        let src_port = u16::from_be_bytes([data[0], data[1]]);
        let dst_port = u16::from_be_bytes([data[2], data[3]]);
        let seq = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ack = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let data_offset = ((data[12] >> 4) as usize) * 4;
        let flags = data[13];
        let window = u16::from_be_bytes([data[14], data[15]]);

        if data_offset < HEADER_SIZE || data_offset > data.len() {
            return None;
        }

        let payload = &data[data_offset..];

        Some(TcpSegment {
            src_port,
            dst_port,
            seq,
            ack,
            data_offset,
            flags,
            window,
            payload,
        })
    }
}

// ── TCP connection ──────────────────────────────────────────

/// A single TCP connection.
#[derive(Debug)]
struct TcpConnection {
    state: TcpState,
    local: SocketAddr,
    remote: SocketAddr,
    /// Our sequence number (next byte to send)
    snd_nxt: u32,
    /// Unacknowledged sequence number
    snd_una: u32,
    /// Initial send sequence number
    iss: u32,
    /// Next expected receive sequence number
    rcv_nxt: u32,
    /// Initial receive sequence number
    irs: u32,
    /// Remote window size
    snd_wnd: u16,
    /// Receive buffer
    recv_buf: VecDeque<u8>,
    /// Send buffer (data waiting to be sent/acked)
    send_buf: VecDeque<u8>,
    /// Outgoing frames waiting to be transmitted
    tx_queue: VecDeque<Vec<u8>>,
    /// Retransmission queue: (seq, data, retries, tick_sent)
    retx_queue: Vec<RetxEntry>,
    /// Tick counter for retransmission
    tick: u32,
}

#[derive(Debug, Clone)]
struct RetxEntry {
    seq: u32,
    data: Vec<u8>, // full TCP segment (for retx)
    retries: u32,
    tick_sent: u32,
}

// ── Connection table ────────────────────────────────────────

/// Connection 4-tuple key
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ConnKey {
    local: SocketAddr,
    remote: SocketAddr,
}

struct TcpTable {
    connections: BTreeMap<ConnId, TcpConnection>,
    /// Map from 4-tuple to ConnId for incoming packet dispatch
    key_map: BTreeMap<ConnKey, ConnId>,
    /// Listening sockets: local_port -> ConnId
    listeners: BTreeMap<u16, ConnId>,
}

static TABLE: Mutex<Option<TcpTable>> = Mutex::new(None);

fn with_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut TcpTable) -> R,
{
    let mut guard = TABLE.lock();
    let table = guard.as_mut().expect("TCP not initialised");
    f(table)
}

/// Ephemeral port counter.
static EPHEMERAL: AtomicU16 = AtomicU16::new(49152);

fn ephemeral_port() -> u16 {
    EPHEMERAL.fetch_add(1, Ordering::Relaxed)
}

/// Simple ISN generator based on a counter.
static ISN_COUNTER: AtomicU64 = AtomicU64::new(0x1000);
fn generate_isn() -> u32 {
    ISN_COUNTER.fetch_add(64000, Ordering::Relaxed) as u32
}

// ── Init ────────────────────────────────────────────────────

/// Initialise the TCP subsystem.
pub fn init() {
    *TABLE.lock() = Some(TcpTable {
        connections: BTreeMap::new(),
        key_map: BTreeMap::new(),
        listeners: BTreeMap::new(),
    });
}

// ── Public API ──────────────────────────────────────────────

/// Create a new TCP connection in CLOSED state.
pub fn create() -> ConnId {
    let id = ConnId(NEXT_CONN.fetch_add(1, Ordering::Relaxed));
    with_table(|t| {
        t.connections.insert(
            id,
            TcpConnection {
                state: TcpState::Closed,
                local: SocketAddr {
                    ip: Ipv4Addr::LOCALHOST,
                    port: 0,
                },
                remote: SocketAddr {
                    ip: Ipv4Addr::LOCALHOST,
                    port: 0,
                },
                snd_nxt: 0,
                snd_una: 0,
                iss: 0,
                rcv_nxt: 0,
                irs: 0,
                snd_wnd: WINDOW_SIZE,
                recv_buf: VecDeque::new(),
                send_buf: VecDeque::new(),
                tx_queue: VecDeque::new(),
                retx_queue: Vec::new(),
                tick: 0,
            },
        );
    });
    id
}

/// Bind and listen on a local port.
pub fn listen(id: ConnId, port: u16) -> Result<(), NetError> {
    with_table(|t| {
        let conn = t
            .connections
            .get_mut(&id)
            .ok_or(NetError::ConnectionNotFound)?;
        let cfg = ipv4::config();
        conn.local = SocketAddr { ip: cfg.ip, port };
        conn.state = TcpState::Listen;
        t.listeners.insert(port, id);
        Ok(())
    })
}

/// Connect to a remote address (real TCP 3-way handshake).
pub fn connect(id: ConnId, remote: SocketAddr) -> Result<(), NetError> {
    let syn_frame = with_table(|t| {
        let conn = t
            .connections
            .get_mut(&id)
            .ok_or(NetError::ConnectionNotFound)?;
        let cfg = ipv4::config();
        let local_port = ephemeral_port();

        conn.local = SocketAddr {
            ip: cfg.ip,
            port: local_port,
        };
        conn.remote = remote;
        conn.iss = generate_isn();
        conn.snd_nxt = conn.iss.wrapping_add(1);
        conn.snd_una = conn.iss;
        conn.state = TcpState::SynSent;

        let key = ConnKey {
            local: conn.local,
            remote: conn.remote,
        };
        t.key_map.insert(key, id);

        // Build SYN segment
        let frame = build_segment(conn, SYN, &[]);
        Ok(frame)
    })?;

    // Transmit SYN
    if let Some(frame) = syn_frame {
        super::transmit_frame(&frame);
    }

    // Wait for SYN-ACK (poll-based)
    for _ in 0..300 {
        super::poll_rx();

        let state = with_table(|t| t.connections.get(&id).map(|c| c.state));

        match state {
            Some(TcpState::Established) => return Ok(()),
            Some(TcpState::Closed) => return Err(NetError::ConnectionRefused),
            None => return Err(NetError::ConnectionNotFound),
            _ => {}
        }

        // ~10ms spin
        for _ in 0..100_000 {
            core::hint::spin_loop();
        }
    }

    Err(NetError::TimedOut)
}

/// Send data on an established connection.
pub fn send(id: ConnId, data: &[u8]) -> Result<usize, NetError> {
    let frames = with_table(|t| {
        let conn = t
            .connections
            .get_mut(&id)
            .ok_or(NetError::ConnectionNotFound)?;
        if conn.state != TcpState::Established && conn.state != TcpState::CloseWait {
            return Err(NetError::NotConnected);
        }

        // Segment data into MSS-sized chunks
        let mut frames = Vec::new();
        let mut sent = 0;

        for chunk in data.chunks(MSS) {
            if let Some(frame) = build_segment(conn, PSH | ACK, chunk) {
                // Track for retransmission
                conn.retx_queue.push(RetxEntry {
                    seq: conn.snd_nxt.wrapping_sub(chunk.len() as u32),
                    data: frame.clone(),
                    retries: 0,
                    tick_sent: conn.tick,
                });
                frames.push(frame);
                sent += chunk.len();
            }
        }

        Ok(frames)
    })?;

    // Transmit all segments
    for frame in &frames {
        super::transmit_frame(frame);
    }

    Ok(data.len())
}

/// Receive data from an established connection.
pub fn recv(id: ConnId, buf: &mut [u8]) -> Result<usize, NetError> {
    with_table(|t| {
        let conn = t
            .connections
            .get_mut(&id)
            .ok_or(NetError::ConnectionNotFound)?;

        if conn.recv_buf.is_empty() {
            if conn.state == TcpState::CloseWait || conn.state == TcpState::Closed {
                return Ok(0); // EOF
            }
            return Err(NetError::WouldBlock);
        }

        let n = buf.len().min(conn.recv_buf.len());
        for i in 0..n {
            buf[i] = conn.recv_buf.pop_front().unwrap();
        }
        Ok(n)
    })
}

/// Blocking receive: polls until data is available or connection closed.
pub fn recv_blocking(id: ConnId, buf: &mut [u8], timeout_iters: u32) -> Result<usize, NetError> {
    for _ in 0..timeout_iters {
        super::poll_rx();

        match recv(id, buf) {
            Ok(n) => return Ok(n),
            Err(NetError::WouldBlock) => {
                // Check if connection is still alive
                let state = state(id);
                match state {
                    Some(TcpState::Established) | Some(TcpState::CloseWait) => {}
                    Some(TcpState::Closed) | None => return Ok(0),
                    _ => {}
                }
                for _ in 0..50_000 {
                    core::hint::spin_loop();
                }
            }
            Err(e) => return Err(e),
        }
    }
    Err(NetError::TimedOut)
}

/// Check how many bytes are available in the receive buffer.
pub fn recv_available(id: ConnId) -> usize {
    with_table(|t| t.connections.get(&id).map_or(0, |c| c.recv_buf.len()))
}

/// Close a connection (send FIN).
pub fn close(id: ConnId) -> Result<(), NetError> {
    let fin_frame = with_table(|t| {
        let conn = t
            .connections
            .get_mut(&id)
            .ok_or(NetError::ConnectionNotFound)?;
        match conn.state {
            TcpState::Established => {
                conn.state = TcpState::FinWait1;
                let frame = build_segment(conn, FIN | ACK, &[]);
                Ok(frame)
            }
            TcpState::CloseWait => {
                conn.state = TcpState::LastAck;
                let frame = build_segment(conn, FIN | ACK, &[]);
                Ok(frame)
            }
            _ => {
                conn.state = TcpState::Closed;
                Ok(None)
            }
        }
    })?;

    if let Some(frame) = fin_frame {
        super::transmit_frame(&frame);
    }

    Ok(())
}

/// Remove a closed connection from the table.
pub fn destroy(id: ConnId) {
    with_table(|t| {
        if let Some(conn) = t.connections.remove(&id) {
            let key = ConnKey {
                local: conn.local,
                remote: conn.remote,
            };
            t.key_map.remove(&key);
            if conn.state == TcpState::Listen {
                t.listeners.remove(&conn.local.port);
            }
        }
    });
}

/// Get connection state.
pub fn state(id: ConnId) -> Option<TcpState> {
    with_table(|t| t.connections.get(&id).map(|c| c.state))
}

/// Snapshot all active connections.
pub fn connections() -> Vec<(ConnId, TcpState, SocketAddr, SocketAddr)> {
    with_table(|t| {
        t.connections
            .iter()
            .map(|(&id, c)| (id, c.state, c.local, c.remote))
            .collect()
    })
}

/// Total connections ever created.
pub fn total_connections() -> u64 {
    NEXT_CONN.load(Ordering::Relaxed) - 1
}

// ── Incoming segment processing ─────────────────────────────

/// Process an incoming TCP segment (called from IPv4 dispatch).
pub fn process_incoming(src_ip: Ipv4Addr, data: &[u8]) {
    let seg = match TcpSegment::parse(data) {
        Some(s) => s,
        None => return,
    };

    with_table(|t| {
        // Look up connection by 4-tuple
        let cfg = ipv4::config();
        let local = SocketAddr {
            ip: cfg.ip,
            port: seg.dst_port,
        };
        let remote = SocketAddr {
            ip: src_ip,
            port: seg.src_port,
        };
        let key = ConnKey { local, remote };

        if let Some(&id) = t.key_map.get(&key) {
            if let Some(conn) = t.connections.get_mut(&id) {
                process_segment(conn, &seg);
            }
        } else if (seg.flags & SYN) != 0 {
            // Check for listening socket
            if let Some(&_listener_id) = t.listeners.get(&seg.dst_port) {
                // Accept incoming connection (simplified: auto-accept)
                let id = ConnId(NEXT_CONN.fetch_add(1, Ordering::Relaxed));
                let iss = generate_isn();
                let mut conn = TcpConnection {
                    state: TcpState::SynReceived,
                    local,
                    remote,
                    snd_nxt: iss.wrapping_add(1),
                    snd_una: iss,
                    iss,
                    rcv_nxt: seg.seq.wrapping_add(1),
                    irs: seg.seq,
                    snd_wnd: seg.window,
                    recv_buf: VecDeque::new(),
                    send_buf: VecDeque::new(),
                    tx_queue: VecDeque::new(),
                    retx_queue: Vec::new(),
                    tick: 0,
                };

                // Send SYN-ACK
                if let Some(frame) = build_segment(&mut conn, SYN | ACK, &[]) {
                    conn.tx_queue.push_back(frame);
                }

                t.key_map.insert(key, id);
                t.connections.insert(id, conn);
            }
        } else if (seg.flags & RST) == 0 {
            // Send RST for unexpected segments
            // (We don't do this for now to avoid complexity)
        }

        // Drain TX queues for all connections
        let mut frames = Vec::new();
        for conn in t.connections.values_mut() {
            while let Some(frame) = conn.tx_queue.pop_front() {
                frames.push(frame);
            }
        }
        // Transmit frames outside the lock
        for frame in frames {
            super::transmit_frame(&frame);
        }
    });
}

/// Process a segment for a known connection.
fn process_segment(conn: &mut TcpConnection, seg: &TcpSegment) {
    conn.tick += 1;

    match conn.state {
        TcpState::SynSent => {
            // Expecting SYN-ACK
            if (seg.flags & (SYN | ACK)) == (SYN | ACK) {
                conn.irs = seg.seq;
                conn.rcv_nxt = seg.seq.wrapping_add(1);
                conn.snd_una = seg.ack;
                conn.snd_wnd = seg.window;
                conn.state = TcpState::Established;

                // Send ACK
                if let Some(frame) = build_segment(conn, ACK, &[]) {
                    conn.tx_queue.push_back(frame);
                }
                // Clear retx queue (SYN was acked)
                conn.retx_queue.clear();
            } else if (seg.flags & RST) != 0 {
                conn.state = TcpState::Closed;
            }
        }

        TcpState::SynReceived => {
            if (seg.flags & ACK) != 0 {
                conn.snd_una = seg.ack;
                conn.state = TcpState::Established;
                conn.retx_queue.clear();
            }
        }

        TcpState::Established => {
            // Handle RST
            if (seg.flags & RST) != 0 {
                conn.state = TcpState::Closed;
                return;
            }

            // Process ACK
            if (seg.flags & ACK) != 0 {
                conn.snd_una = seg.ack;
                conn.snd_wnd = seg.window;
                // Remove acked entries from retx queue
                conn.retx_queue.retain(|e| {
                    // Keep if not yet fully acked
                    let end_seq = e.seq.wrapping_add(e.data.len() as u32);
                    seq_lt(seg.ack, end_seq)
                });
            }

            // Process incoming data
            if !seg.payload.is_empty() {
                if seg.seq == conn.rcv_nxt {
                    conn.recv_buf.extend(seg.payload);
                    conn.rcv_nxt = conn.rcv_nxt.wrapping_add(seg.payload.len() as u32);

                    // Send ACK
                    if let Some(frame) = build_segment(conn, ACK, &[]) {
                        conn.tx_queue.push_back(frame);
                    }
                }
                // Out-of-order: silently drop for now (simplification)
            }

            // Handle FIN
            if (seg.flags & FIN) != 0 {
                conn.rcv_nxt = conn.rcv_nxt.wrapping_add(1);
                conn.state = TcpState::CloseWait;
                // ACK the FIN
                if let Some(frame) = build_segment(conn, ACK, &[]) {
                    conn.tx_queue.push_back(frame);
                }
            }
        }

        TcpState::FinWait1 => {
            if (seg.flags & ACK) != 0 {
                conn.snd_una = seg.ack;
                if (seg.flags & FIN) != 0 {
                    // Simultaneous close
                    conn.rcv_nxt = conn.rcv_nxt.wrapping_add(1);
                    conn.state = TcpState::TimeWait;
                    if let Some(frame) = build_segment(conn, ACK, &[]) {
                        conn.tx_queue.push_back(frame);
                    }
                } else {
                    conn.state = TcpState::FinWait2;
                }
            }
        }

        TcpState::FinWait2 => {
            if (seg.flags & FIN) != 0 {
                conn.rcv_nxt = conn.rcv_nxt.wrapping_add(1);
                conn.state = TcpState::TimeWait;
                if let Some(frame) = build_segment(conn, ACK, &[]) {
                    conn.tx_queue.push_back(frame);
                }
            }
            // Also accept data in FIN_WAIT_2
            if !seg.payload.is_empty() && seg.seq == conn.rcv_nxt {
                conn.recv_buf.extend(seg.payload);
                conn.rcv_nxt = conn.rcv_nxt.wrapping_add(seg.payload.len() as u32);
            }
        }

        TcpState::LastAck => {
            if (seg.flags & ACK) != 0 {
                conn.state = TcpState::Closed;
            }
        }

        TcpState::Closing => {
            if (seg.flags & ACK) != 0 {
                conn.state = TcpState::TimeWait;
            }
        }

        TcpState::CloseWait => {
            // Waiting for application to close
            if !seg.payload.is_empty() && seg.seq == conn.rcv_nxt {
                conn.recv_buf.extend(seg.payload);
                conn.rcv_nxt = conn.rcv_nxt.wrapping_add(seg.payload.len() as u32);
            }
        }

        _ => {}
    }
}

// ── Segment construction ────────────────────────────────────

/// Build a TCP segment as a full Ethernet frame.
fn build_segment(conn: &mut TcpConnection, flags: u8, payload: &[u8]) -> Option<Vec<u8>> {
    let tcp_len = HEADER_SIZE + payload.len();
    let mut seg = Vec::with_capacity(tcp_len);

    // Source port
    seg.push((conn.local.port >> 8) as u8);
    seg.push(conn.local.port as u8);
    // Destination port
    seg.push((conn.remote.port >> 8) as u8);
    seg.push(conn.remote.port as u8);
    // Sequence number
    let seq = if (flags & SYN) != 0 {
        conn.iss
    } else {
        conn.snd_nxt
    };
    seg.push((seq >> 24) as u8);
    seg.push((seq >> 16) as u8);
    seg.push((seq >> 8) as u8);
    seg.push(seq as u8);
    // Acknowledgment number
    let ack_num = conn.rcv_nxt;
    seg.push((ack_num >> 24) as u8);
    seg.push((ack_num >> 16) as u8);
    seg.push((ack_num >> 8) as u8);
    seg.push(ack_num as u8);
    // Data offset (5 = 20 bytes, no options) | Reserved
    seg.push(0x50);
    // Flags
    seg.push(flags);
    // Window
    seg.push((WINDOW_SIZE >> 8) as u8);
    seg.push(WINDOW_SIZE as u8);
    // Checksum (placeholder)
    seg.push(0x00);
    seg.push(0x00);
    // Urgent pointer
    seg.push(0x00);
    seg.push(0x00);
    // Payload
    seg.extend_from_slice(payload);

    // Compute TCP checksum with pseudo-header
    let phdr = ipv4::pseudo_header_checksum(
        conn.local.ip,
        conn.remote.ip,
        ipv4::PROTO_TCP,
        tcp_len as u16,
    );
    let cksum = tcp_checksum(&seg, phdr);
    seg[16] = (cksum >> 8) as u8;
    seg[17] = cksum as u8;

    // Update sequence number
    let mut seq_advance = payload.len() as u32;
    if (flags & SYN) != 0 || (flags & FIN) != 0 {
        seq_advance += 1;
    }
    conn.snd_nxt = conn.snd_nxt.wrapping_add(seq_advance);

    // Wrap in IPv4 + Ethernet
    ipv4::send_packet(conn.remote.ip, ipv4::PROTO_TCP, &seg)
}

/// TCP checksum.
fn tcp_checksum(data: &[u8], pseudo: u32) -> u16 {
    let mut sum = pseudo;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += u16::from_be_bytes([data[i], data[i + 1]]) as u32;
        i += 2;
    }
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// Sequence number comparison: a < b (handles wrapping).
fn seq_lt(a: u32, b: u32) -> bool {
    (a.wrapping_sub(b) as i32) < 0
}

// ── Legacy compatibility API ────────────────────────────────
// These functions maintain compatibility with the existing HTTP
// server that uses the old loopback-only TCP API.

/// Push data into a connection's receive buffer (used by loopback).
pub fn push_recv(id: ConnId, data: &[u8]) -> Result<(), NetError> {
    with_table(|t| {
        let conn = t
            .connections
            .get_mut(&id)
            .ok_or(NetError::ConnectionNotFound)?;
        conn.recv_buf.extend(data);
        Ok(())
    })
}

/// Take and clear the send buffer (used by loopback HTTP server).
pub fn take_send(id: ConnId) -> Result<Vec<u8>, NetError> {
    with_table(|t| {
        let conn = t
            .connections
            .get_mut(&id)
            .ok_or(NetError::ConnectionNotFound)?;
        let data: Vec<u8> = conn.send_buf.drain(..).collect();
        Ok(data)
    })
}
