//! TCP Connection State Machine
//!
//! Manages TCP connections with a simplified state machine.
//! Currently only supports loopback connections to the built-in
//! HTTP server; outbound connections report "network unreachable".

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

use super::{Ipv4Addr, SocketAddr, NetError};

// ── TCP state ───────────────────────────────────────────────

/// TCP connection state (RFC 793 simplified).
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
            TcpState::Closed      => "CLOSED",
            TcpState::Listen      => "LISTEN",
            TcpState::SynSent     => "SYN_SENT",
            TcpState::SynReceived => "SYN_RECV",
            TcpState::Established => "ESTABLISHED",
            TcpState::FinWait1    => "FIN_WAIT1",
            TcpState::FinWait2    => "FIN_WAIT2",
            TcpState::CloseWait   => "CLOSE_WAIT",
            TcpState::Closing     => "CLOSING",
            TcpState::LastAck     => "LAST_ACK",
            TcpState::TimeWait    => "TIME_WAIT",
        }
    }
}

// ── TCP connection ──────────────────────────────────────────

/// Unique connection identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConnId(pub u64);

/// A single TCP connection.
#[derive(Debug)]
pub struct TcpConnection {
    pub id: ConnId,
    pub state: TcpState,
    pub local: SocketAddr,
    pub remote: SocketAddr,
    /// Outgoing data (to remote).
    pub send_buf: Vec<u8>,
    /// Incoming data (from remote).
    pub recv_buf: Vec<u8>,
    /// Sequence numbers (simplified).
    pub seq: u32,
    pub ack: u32,
}

// ── Connection table ────────────────────────────────────────

static NEXT_CONN: AtomicU64 = AtomicU64::new(1);

struct TcpTable {
    conns: BTreeMap<ConnId, TcpConnection>,
    /// Listening sockets: port → ConnId
    listeners: BTreeMap<u16, ConnId>,
}

static TABLE: Mutex<Option<TcpTable>> = Mutex::new(None);

fn with_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut TcpTable) -> R,
{
    let mut guard = TABLE.lock();
    let table = guard.as_mut().expect("TCP table not initialised");
    f(table)
}

/// Initialise the TCP subsystem.
pub fn init() {
    *TABLE.lock() = Some(TcpTable {
        conns: BTreeMap::new(),
        listeners: BTreeMap::new(),
    });
}

// ── Public API ──────────────────────────────────────────────

/// Create a new TCP connection in CLOSED state. Returns ConnId.
pub fn create() -> ConnId {
    let id = ConnId(NEXT_CONN.fetch_add(1, Ordering::Relaxed));
    let conn = TcpConnection {
        id,
        state: TcpState::Closed,
        local: SocketAddr::new(Ipv4Addr::ANY, 0),
        remote: SocketAddr::new(Ipv4Addr::ANY, 0),
        send_buf: Vec::new(),
        recv_buf: Vec::new(),
        seq: 1000,
        ack: 0,
    };
    with_table(|t| t.conns.insert(id, conn));
    id
}

/// Bind a connection to a local port and start listening.
pub fn listen(id: ConnId, port: u16) -> Result<(), NetError> {
    with_table(|t| {
        if t.listeners.contains_key(&port) {
            return Err(NetError::AddressInUse);
        }
        let conn = t.conns.get_mut(&id).ok_or(NetError::InvalidArgument)?;
        conn.state = TcpState::Listen;
        conn.local = SocketAddr::new(Ipv4Addr::ANY, port);
        t.listeners.insert(port, id);
        Ok(())
    })
}

/// Connect to a remote address (loopback only currently).
pub fn connect(id: ConnId, remote: SocketAddr) -> Result<(), NetError> {
    // Only loopback connections are supported
    if !remote.ip.is_loopback() && !remote.ip.is_unspecified() {
        return Err(NetError::NetworkUnreachable);
    }

    with_table(|t| {
        let conn = t.conns.get_mut(&id).ok_or(NetError::InvalidArgument)?;
        if conn.state != TcpState::Closed {
            return Err(NetError::AlreadyConnected);
        }

        // Check if there's a listener on the remote port
        if !t.listeners.contains_key(&remote.port) {
            // For loopback to the built-in HTTP server,
            // simulate instant connection even without a listener
            if remote.port == 80 || remote.port == 443 || remote.port == 8080 {
                conn.state = TcpState::Established;
                conn.local = SocketAddr::new(Ipv4Addr::LOCALHOST, ephemeral_port());
                conn.remote = remote;
                return Ok(());
            }
            return Err(NetError::ConnectionRefused);
        }

        // Instant loopback connection (SYN → SYN-ACK → ACK in one step)
        conn.state = TcpState::Established;
        conn.local = SocketAddr::new(Ipv4Addr::LOCALHOST, ephemeral_port());
        conn.remote = remote;
        Ok(())
    })
}

/// Send data on an established connection.
pub fn send(id: ConnId, data: &[u8]) -> Result<usize, NetError> {
    let len = data.len() as u64;
    with_table(|t| {
        let conn = t.conns.get_mut(&id).ok_or(NetError::InvalidArgument)?;
        if conn.state != TcpState::Established {
            return Err(NetError::NotConnected);
        }
        conn.send_buf.extend_from_slice(data);
        conn.seq = conn.seq.wrapping_add(data.len() as u32);
        Ok(data.len())
    })?;
    super::loopback_transfer(len);
    Ok(data.len())
}

/// Receive data from an established connection.
pub fn recv(id: ConnId, buf: &mut [u8]) -> Result<usize, NetError> {
    with_table(|t| {
        let conn = t.conns.get_mut(&id).ok_or(NetError::InvalidArgument)?;
        if conn.state != TcpState::Established && conn.state != TcpState::CloseWait {
            return Err(NetError::NotConnected);
        }
        if conn.recv_buf.is_empty() {
            return Err(NetError::WouldBlock);
        }
        let n = conn.recv_buf.len().min(buf.len());
        buf[..n].copy_from_slice(&conn.recv_buf[..n]);
        conn.recv_buf.drain(..n);
        conn.ack = conn.ack.wrapping_add(n as u32);
        Ok(n)
    })
}

/// Push data into a connection's receive buffer (used by the HTTP server).
pub fn push_recv(id: ConnId, data: &[u8]) -> Result<(), NetError> {
    with_table(|t| {
        let conn = t.conns.get_mut(&id).ok_or(NetError::InvalidArgument)?;
        conn.recv_buf.extend_from_slice(data);
        Ok(())
    })
}

/// Take and clear the send buffer (used by the HTTP server to read requests).
pub fn take_send(id: ConnId) -> Result<Vec<u8>, NetError> {
    with_table(|t| {
        let conn = t.conns.get_mut(&id).ok_or(NetError::InvalidArgument)?;
        let data = core::mem::take(&mut conn.send_buf);
        Ok(data)
    })
}

/// Close a connection.
pub fn close(id: ConnId) -> Result<(), NetError> {
    with_table(|t| {
        let conn = t.conns.get_mut(&id).ok_or(NetError::InvalidArgument)?;
        // Remove listener if this was a listening socket
        if conn.state == TcpState::Listen {
            t.listeners.retain(|_, v| *v != id);
        }
        conn.state = TcpState::Closed;
        Ok(())
    })
}

/// Remove a closed connection entirely.
pub fn destroy(id: ConnId) {
    with_table(|t| {
        t.conns.remove(&id);
    });
}

/// Get the state of a connection.
pub fn state(id: ConnId) -> Option<TcpState> {
    with_table(|t| t.conns.get(&id).map(|c| c.state))
}

/// Snapshot all active connections (for netstat).
pub fn connections() -> Vec<(ConnId, TcpState, SocketAddr, SocketAddr)> {
    with_table(|t| {
        t.conns
            .values()
            .filter(|c| c.state != TcpState::Closed)
            .map(|c| (c.id, c.state, c.local, c.remote))
            .collect()
    })
}

/// Total number of connections ever created.
pub fn total_connections() -> u64 {
    NEXT_CONN.load(Ordering::Relaxed) - 1
}

// ── Helpers ─────────────────────────────────────────────────

static EPHEMERAL: AtomicU64 = AtomicU64::new(49152);

fn ephemeral_port() -> u16 {
    let p = EPHEMERAL.fetch_add(1, Ordering::Relaxed);
    (p % (65535 - 49152) + 49152) as u16
}
