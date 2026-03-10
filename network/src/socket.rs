//! Socket API implementation.
//!
//! Provides a BSD-like socket API backed by kernel-internal per-socket
//! buffers. For connected (TCP) sockets, data written via `send()` on
//! one end of a connected pair is available via `recv()` on the other
//! end.  This enables in-guest TCP echo testing without requiring
//! real loopback routing (QEMU SLIRP does not route 127.0.0.1).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use spin::Mutex;

use crate::{IpAddr, Ipv4Addr, NetworkError, SocketAddr};

/// Socket handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SocketHandle(pub u32);

/// Socket type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    /// TCP stream socket.
    Stream,
    /// UDP datagram socket.
    Datagram,
}

/// Socket state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Closed,
    Bound,
    Listening,
    Connecting,
    Connected,
}

/// Poll readiness flags.
pub struct PollFlags(u32);

impl PollFlags {
    pub const READABLE: PollFlags = PollFlags(0x1);
    pub const WRITABLE: PollFlags = PollFlags(0x2);
    pub const ERROR: PollFlags = PollFlags(0x4);
    pub const HANGUP: PollFlags = PollFlags(0x8);

    /// Empty flags.
    pub const fn empty() -> Self {
        PollFlags(0)
    }

    /// Raw bits value.
    pub fn bits(&self) -> u32 {
        self.0
    }

    /// Check if a flag is set.
    pub fn contains(&self, other: &PollFlags) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Combine flags.
    pub fn union(self, other: PollFlags) -> PollFlags {
        PollFlags(self.0 | other.0)
    }
}

/// Per-socket receive buffer capacity.
const SOCKET_BUF_CAP: usize = 8192;

/// Global socket table.
static SOCKETS: Mutex<BTreeMap<SocketHandle, Socket>> = Mutex::new(BTreeMap::new());
static NEXT_HANDLE: AtomicU32 = AtomicU32::new(1);

/// Accept queue: maps listening socket handle → vec of ready connected handles.
static ACCEPT_QUEUE: Mutex<BTreeMap<u32, Vec<SocketHandle>>> = Mutex::new(BTreeMap::new());

/// A socket.
pub struct Socket {
    handle: SocketHandle,
    socket_type: SocketType,
    state: SocketState,
    local_addr: Option<SocketAddr>,
    remote_addr: Option<SocketAddr>,
    /// Peer handle for connected pairs (populated by accept/connect pairing).
    peer_handle: Option<SocketHandle>,
    /// Receive buffer: data sent by the remote peer lands here.
    recv_buf: Vec<u8>,
    /// Socket options.
    opts: SocketOptions,
    /// Whether the read side is shut down.
    shut_rd: bool,
    /// Whether the write side is shut down.
    shut_wr: bool,
}

/// Minimal socket option storage.
#[derive(Clone, Copy)]
struct SocketOptions {
    reuse_addr: bool,
    keep_alive: bool,
}

impl Default for SocketOptions {
    fn default() -> Self {
        Self {
            reuse_addr: false,
            keep_alive: false,
        }
    }
}

/// Create a new socket.
pub fn create(socket_type: SocketType) -> Result<SocketHandle, NetworkError> {
    let handle = SocketHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
    let socket = Socket {
        handle,
        socket_type,
        state: SocketState::Closed,
        local_addr: None,
        remote_addr: None,
        peer_handle: None,
        recv_buf: Vec::new(),
        opts: SocketOptions::default(),
        shut_rd: false,
        shut_wr: false,
    };
    SOCKETS.lock().insert(handle, socket);
    Ok(handle)
}

/// Bind a socket to an address.
pub fn bind(handle: SocketHandle, addr: SocketAddr) -> Result<(), NetworkError> {
    let mut sockets = SOCKETS.lock();
    let socket = sockets.get_mut(&handle).ok_or(NetworkError::NotConnected)?;
    socket.local_addr = Some(addr);
    socket.state = SocketState::Bound;
    Ok(())
}

/// Listen on a socket.
pub fn listen(handle: SocketHandle, _backlog: u32) -> Result<(), NetworkError> {
    let mut sockets = SOCKETS.lock();
    let socket = sockets.get_mut(&handle).ok_or(NetworkError::NotConnected)?;
    socket.state = SocketState::Listening;
    drop(sockets);
    // Initialize an empty accept queue for this listener.
    ACCEPT_QUEUE.lock().entry(handle.0).or_insert_with(Vec::new);
    Ok(())
}

/// Connect a socket to a remote address.
///
/// For kernel-internal testing, if the target address matches a
/// listening socket's local address, we create a connected pair
/// (the "accepted" socket is pushed onto the listener's accept queue).
pub fn connect(handle: SocketHandle, addr: SocketAddr) -> Result<(), NetworkError> {
    let mut sockets = SOCKETS.lock();
    let socket = sockets.get_mut(&handle).ok_or(NetworkError::NotConnected)?;
    socket.remote_addr = Some(addr);
    socket.state = SocketState::Connected;

    // Assign a local ephemeral address if not already bound.
    if socket.local_addr.is_none() {
        socket.local_addr = Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 2, 15)),
            49152 + (handle.0 as u16 % 16384),
        ));
    }

    let client_local = socket.local_addr;
    let client_type = socket.socket_type;

    // Find a listening socket that matches the target address.
    let listener_handle = sockets.iter().find_map(|(&h, s)| {
        if s.state == SocketState::Listening && s.socket_type == client_type {
            if let Some(la) = &s.local_addr {
                // Match port; 0.0.0.0 binds accept any IP.
                if la.port == addr.port {
                    return Some(h);
                }
            }
        }
        None
    });

    if let Some(lh) = listener_handle {
        // Create a new socket for the accepted side of the connection.
        let peer = SocketHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
        let listener_local = sockets.get(&lh).and_then(|s| s.local_addr);
        let peer_sock = Socket {
            handle: peer,
            socket_type: client_type,
            state: SocketState::Connected,
            local_addr: listener_local,
            remote_addr: client_local,
            peer_handle: Some(handle),
            recv_buf: Vec::new(),
            opts: SocketOptions::default(),
            shut_rd: false,
            shut_wr: false,
        };
        sockets.insert(peer, peer_sock);

        // Link the client to the peer.
        if let Some(cli) = sockets.get_mut(&handle) {
            cli.peer_handle = Some(peer);
        }

        drop(sockets);

        // Push accepted socket onto the listener's accept queue.
        ACCEPT_QUEUE.lock().entry(lh.0).or_insert_with(Vec::new).push(peer);
    } else {
        drop(sockets);
    }

    Ok(())
}

/// Accept a connection on a listening socket.
///
/// Returns a new `SocketHandle` for the accepted connection, or
/// `Err(WouldBlock)` if no pending connections.
pub fn accept(handle: SocketHandle) -> Result<SocketHandle, NetworkError> {
    let mut queue = ACCEPT_QUEUE.lock();
    if let Some(q) = queue.get_mut(&handle.0) {
        if let Some(peer) = q.pop() {
            return Ok(peer);
        }
    }
    Err(NetworkError::WouldBlock)
}

/// Send data on a connected socket.
///
/// Writes data into the peer socket's receive buffer so that the
/// peer can `recv()` it.
pub fn send(handle: SocketHandle, data: &[u8]) -> Result<usize, NetworkError> {
    let sockets = SOCKETS.lock();
    let socket = sockets.get(&handle).ok_or(NetworkError::NotConnected)?;
    if socket.state != SocketState::Connected {
        return Err(NetworkError::NotConnected);
    }
    if socket.shut_wr {
        return Err(NetworkError::ConnectionReset);
    }
    let peer = socket.peer_handle.ok_or(NetworkError::NotConnected)?;
    drop(sockets);

    // Write into the peer's recv buffer.
    let mut sockets = SOCKETS.lock();
    let peer_sock = sockets.get_mut(&peer).ok_or(NetworkError::NotConnected)?;
    let space = SOCKET_BUF_CAP.saturating_sub(peer_sock.recv_buf.len());
    let to_write = data.len().min(space);
    if to_write == 0 {
        return Err(NetworkError::WouldBlock);
    }
    peer_sock.recv_buf.extend_from_slice(&data[..to_write]);
    Ok(to_write)
}

/// Receive data from a socket.
///
/// Drains data from the socket's own receive buffer.
pub fn recv(handle: SocketHandle, buffer: &mut [u8]) -> Result<usize, NetworkError> {
    let mut sockets = SOCKETS.lock();
    let socket = sockets.get_mut(&handle).ok_or(NetworkError::NotConnected)?;
    if socket.recv_buf.is_empty() {
        if socket.shut_rd || socket.state == SocketState::Closed {
            return Ok(0); // EOF
        }
        return Err(NetworkError::WouldBlock);
    }
    let to_read = buffer.len().min(socket.recv_buf.len());
    buffer[..to_read].copy_from_slice(&socket.recv_buf[..to_read]);
    // Drain consumed bytes.
    socket.recv_buf.drain(..to_read);
    Ok(to_read)
}

/// Shutdown parts of a full-duplex connection.
///
/// `how`: 0 = SHUT_RD, 1 = SHUT_WR, 2 = SHUT_RDWR.
pub fn shutdown(handle: SocketHandle, how: u32) -> Result<(), NetworkError> {
    let mut sockets = SOCKETS.lock();
    let socket = sockets.get_mut(&handle).ok_or(NetworkError::NotConnected)?;
    match how {
        0 => socket.shut_rd = true,
        1 => socket.shut_wr = true,
        2 => {
            socket.shut_rd = true;
            socket.shut_wr = true;
        }
        _ => return Err(NetworkError::InvalidAddress),
    }
    Ok(())
}

/// Get the local address of a socket.
pub fn getsockname(handle: SocketHandle) -> Result<SocketAddr, NetworkError> {
    let sockets = SOCKETS.lock();
    let socket = sockets.get(&handle).ok_or(NetworkError::NotConnected)?;
    socket.local_addr.ok_or(NetworkError::InvalidAddress)
}

/// Get the remote address of a socket.
pub fn getpeername(handle: SocketHandle) -> Result<SocketAddr, NetworkError> {
    let sockets = SOCKETS.lock();
    let socket = sockets.get(&handle).ok_or(NetworkError::NotConnected)?;
    if socket.state != SocketState::Connected {
        return Err(NetworkError::NotConnected);
    }
    socket.remote_addr.ok_or(NetworkError::NotConnected)
}

/// Set a socket option.
///
/// Supported: `SO_REUSEADDR` (level=1, optname=2),
/// `SO_KEEPALIVE` (level=1, optname=9).
pub fn setsockopt(
    handle: SocketHandle,
    level: u32,
    optname: u32,
    value: u32,
) -> Result<(), NetworkError> {
    let mut sockets = SOCKETS.lock();
    let socket = sockets.get_mut(&handle).ok_or(NetworkError::NotConnected)?;
    if level == 1 {
        // SOL_SOCKET
        match optname {
            2 => socket.opts.reuse_addr = value != 0, // SO_REUSEADDR
            9 => socket.opts.keep_alive = value != 0, // SO_KEEPALIVE
            20 | 21 => {}                              // SO_RCVTIMEO / SO_SNDTIMEO (no-op for now)
            _ => return Err(NetworkError::NotImplemented), // ENOPROTOOPT
        }
        Ok(())
    } else {
        Err(NetworkError::NotImplemented)
    }
}

/// Get a socket option.
pub fn getsockopt(handle: SocketHandle, level: u32, optname: u32) -> Result<u32, NetworkError> {
    let sockets = SOCKETS.lock();
    let socket = sockets.get(&handle).ok_or(NetworkError::NotConnected)?;
    if level == 1 {
        match optname {
            2 => Ok(socket.opts.reuse_addr as u32),
            9 => Ok(socket.opts.keep_alive as u32),
            20 | 21 => Ok(0),
            _ => Err(NetworkError::NotImplemented),
        }
    } else {
        Err(NetworkError::NotImplemented)
    }
}

/// Query readiness flags for a socket.
pub fn poll(handle: SocketHandle) -> PollFlags {
    let sockets = SOCKETS.lock();
    let socket = match sockets.get(&handle) {
        Some(s) => s,
        None => return PollFlags::ERROR,
    };
    let mut flags = PollFlags::empty();

    // Readable if recv buffer is non-empty or shut_rd (EOF ready).
    if !socket.recv_buf.is_empty() || socket.shut_rd {
        flags = flags.union(PollFlags::READABLE);
    }
    // Writable if connected and peer buffer has space.
    if socket.state == SocketState::Connected && !socket.shut_wr {
        if let Some(peer) = socket.peer_handle {
            if let Some(ps) = sockets.get(&peer) {
                if ps.recv_buf.len() < SOCKET_BUF_CAP {
                    flags = flags.union(PollFlags::WRITABLE);
                }
            }
        } else {
            // No peer yet — consider writable to avoid blocking.
            flags = flags.union(PollFlags::WRITABLE);
        }
    }
    // Listening sockets: readable if accept queue non-empty.
    if socket.state == SocketState::Listening {
        drop(sockets);
        let queue = ACCEPT_QUEUE.lock();
        if let Some(q) = queue.get(&handle.0) {
            if !q.is_empty() {
                flags = flags.union(PollFlags::READABLE);
            }
        }
    }
    flags
}

/// Close a socket.
pub fn close(handle: SocketHandle) -> Result<(), NetworkError> {
    let mut sockets = SOCKETS.lock();
    if let Some(socket) = sockets.remove(&handle) {
        // Clean up accept queue if this was a listener.
        if socket.state == SocketState::Listening {
            drop(sockets);
            ACCEPT_QUEUE.lock().remove(&handle.0);
        }
    }
    Ok(())
}

/// Get the socket type for a handle.
pub fn get_type(handle: SocketHandle) -> Result<SocketType, NetworkError> {
    let sockets = SOCKETS.lock();
    let socket = sockets.get(&handle).ok_or(NetworkError::NotConnected)?;
    Ok(socket.socket_type)
}

/// Get the socket state for a handle.
pub fn get_state(handle: SocketHandle) -> Result<SocketState, NetworkError> {
    let sockets = SOCKETS.lock();
    let socket = sockets.get(&handle).ok_or(NetworkError::NotConnected)?;
    Ok(socket.state)
}

/// Send a datagram to a specific address (UDP sendto).
///
/// Looks up a Datagram socket bound to `dest.port` and deposits `data`
/// directly into its receive buffer.  Returns bytes sent.
pub fn sendto_dgram(
    _handle: SocketHandle,
    data: &[u8],
    dest: SocketAddr,
) -> Result<usize, NetworkError> {
    let mut sockets = SOCKETS.lock();

    // Find a bound datagram socket listening on the target port.
    let target = sockets.iter_mut().find_map(|(h, s)| {
        if s.socket_type == SocketType::Datagram {
            if let Some(la) = &s.local_addr {
                if la.port == dest.port {
                    return Some(*h);
                }
            }
        }
        None
    });

    if let Some(target_h) = target {
        if let Some(target_sock) = sockets.get_mut(&target_h) {
            let space = SOCKET_BUF_CAP.saturating_sub(target_sock.recv_buf.len());
            let to_write = data.len().min(space);
            if to_write == 0 {
                return Err(NetworkError::WouldBlock);
            }
            target_sock.recv_buf.extend_from_slice(&data[..to_write]);
            return Ok(to_write);
        }
    }

    // No matching socket found — data is silently dropped (UDP semantics).
    Ok(data.len())
}
