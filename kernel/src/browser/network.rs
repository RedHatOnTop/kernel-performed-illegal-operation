//! Network Service Interface
//!
//! This module provides the interface for browser processes to access
//! network services through the kernel.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::{Mutex, RwLock};

use super::coordinator::TabId;
use crate::ipc::ChannelId;

/// Socket identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SocketId(pub u64);

/// Socket type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    /// TCP stream socket.
    TcpStream,
    /// UDP datagram socket.
    UdpDatagram,
    /// TLS-wrapped TCP.
    TlsStream,
}

/// Socket state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    /// Socket created but not connected.
    Created,
    /// DNS lookup in progress.
    Resolving,
    /// TCP connection in progress.
    Connecting,
    /// TLS handshake in progress.
    Handshaking,
    /// Socket connected and ready.
    Connected,
    /// Socket is closing.
    Closing,
    /// Socket is closed.
    Closed,
    /// Socket encountered an error.
    Error,
}

/// IPv4 address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Addr(pub [u8; 4]);

impl Ipv4Addr {
    /// Localhost.
    pub const LOCALHOST: Ipv4Addr = Ipv4Addr([127, 0, 0, 1]);
    
    /// Any address.
    pub const ANY: Ipv4Addr = Ipv4Addr([0, 0, 0, 0]);
}

/// Socket address.
#[derive(Debug, Clone, Copy)]
pub struct SocketAddr {
    /// IP address.
    pub addr: Ipv4Addr,
    /// Port number.
    pub port: u16,
}

impl SocketAddr {
    /// Create new socket address.
    pub fn new(addr: Ipv4Addr, port: u16) -> Self {
        SocketAddr { addr, port }
    }
}

/// Socket information.
#[derive(Debug)]
pub struct SocketInfo {
    /// Socket ID.
    pub id: SocketId,
    /// Owning tab.
    pub owner: TabId,
    /// Socket type.
    pub socket_type: SocketType,
    /// Current state.
    pub state: SocketState,
    /// Local address (if bound).
    pub local_addr: Option<SocketAddr>,
    /// Remote address (if connected).
    pub remote_addr: Option<SocketAddr>,
    /// Receive buffer.
    pub recv_buffer: Vec<u8>,
    /// Send buffer.
    pub send_buffer: Vec<u8>,
    /// Bytes received.
    pub bytes_recv: u64,
    /// Bytes sent.
    pub bytes_sent: u64,
}

/// DNS query result.
#[derive(Debug, Clone)]
pub struct DnsResult {
    /// Hostname queried.
    pub hostname: String,
    /// Resolved addresses.
    pub addresses: Vec<Ipv4Addr>,
    /// Time-to-live in seconds.
    pub ttl: u32,
}

/// Network request from browser.
#[derive(Debug, Clone)]
pub enum NetRequest {
    /// DNS lookup.
    DnsLookup {
        hostname: String,
    },
    /// Create socket.
    CreateSocket {
        socket_type: SocketType,
    },
    /// Connect socket.
    Connect {
        socket_id: SocketId,
        addr: SocketAddr,
    },
    /// Send data.
    Send {
        socket_id: SocketId,
        data: Vec<u8>,
    },
    /// Receive data.
    Receive {
        socket_id: SocketId,
        max_len: usize,
    },
    /// Close socket.
    Close {
        socket_id: SocketId,
    },
    /// Start TLS handshake.
    StartTls {
        socket_id: SocketId,
        hostname: String,
    },
}

/// Network response to browser.
#[derive(Debug, Clone)]
pub enum NetResponse {
    /// DNS lookup result.
    DnsResult(DnsResult),
    /// Socket created.
    SocketCreated {
        socket_id: SocketId,
    },
    /// Socket connected.
    Connected {
        socket_id: SocketId,
    },
    /// Data sent.
    Sent {
        socket_id: SocketId,
        len: usize,
    },
    /// Data received.
    Received {
        socket_id: SocketId,
        data: Vec<u8>,
    },
    /// Socket closed.
    Closed {
        socket_id: SocketId,
    },
    /// TLS handshake complete.
    TlsReady {
        socket_id: SocketId,
    },
    /// Error occurred.
    Error {
        socket_id: Option<SocketId>,
        error: NetError,
    },
}

/// Network errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetError {
    /// Socket not found.
    SocketNotFound,
    /// Connection refused.
    ConnectionRefused,
    /// Connection reset.
    ConnectionReset,
    /// Connection timeout.
    Timeout,
    /// DNS lookup failed.
    DnsLookupFailed,
    /// TLS handshake failed.
    TlsError,
    /// Would block.
    WouldBlock,
    /// Address in use.
    AddressInUse,
    /// Permission denied.
    PermissionDenied,
    /// Network unreachable.
    NetworkUnreachable,
    /// Invalid argument.
    InvalidArgument,
}

/// Network service manager.
pub struct NetworkManager {
    /// All sockets.
    sockets: BTreeMap<SocketId, Mutex<SocketInfo>>,
    
    /// Per-tab sockets.
    tab_sockets: BTreeMap<TabId, Vec<SocketId>>,
    
    /// DNS cache.
    dns_cache: BTreeMap<String, DnsResult>,
    
    /// Next socket ID.
    next_socket_id: AtomicU64,
    
    /// Maximum sockets per tab.
    max_sockets_per_tab: usize,
    
    /// Buffer size limit.
    buffer_size_limit: usize,
}

impl NetworkManager {
    /// Create new network manager.
    pub fn new() -> Self {
        NetworkManager {
            sockets: BTreeMap::new(),
            tab_sockets: BTreeMap::new(),
            dns_cache: BTreeMap::new(),
            next_socket_id: AtomicU64::new(1),
            max_sockets_per_tab: 64,
            buffer_size_limit: 64 * 1024, // 64KB per socket buffer
        }
    }
    
    /// Create a new socket.
    pub fn create_socket(&mut self, tab: TabId, socket_type: SocketType) -> Result<SocketId, NetError> {
        // Check per-tab limit
        let tab_count = self.tab_sockets.get(&tab).map(|v| v.len()).unwrap_or(0);
        if tab_count >= self.max_sockets_per_tab {
            return Err(NetError::PermissionDenied);
        }
        
        let socket_id = SocketId(self.next_socket_id.fetch_add(1, Ordering::Relaxed));
        
        let socket = SocketInfo {
            id: socket_id,
            owner: tab,
            socket_type,
            state: SocketState::Created,
            local_addr: None,
            remote_addr: None,
            recv_buffer: Vec::new(),
            send_buffer: Vec::new(),
            bytes_recv: 0,
            bytes_sent: 0,
        };
        
        self.sockets.insert(socket_id, Mutex::new(socket));
        self.tab_sockets.entry(tab).or_default().push(socket_id);
        
        crate::serial_println!("[Net] Created socket {:?} for tab {}", socket_id, tab.0);
        
        Ok(socket_id)
    }
    
    /// Connect a socket.
    pub fn connect(&mut self, socket_id: SocketId, addr: SocketAddr) -> Result<(), NetError> {
        let socket = self.sockets.get(&socket_id).ok_or(NetError::SocketNotFound)?;
        let mut socket = socket.lock();
        
        if socket.state != SocketState::Created {
            return Err(NetError::InvalidArgument);
        }
        
        socket.state = SocketState::Connecting;
        socket.remote_addr = Some(addr);
        
        // TODO: Actual network stack integration
        // For now, simulate connection
        socket.state = SocketState::Connected;
        
        crate::serial_println!(
            "[Net] Socket {:?} connected to {:?}:{}",
            socket_id, addr.addr.0, addr.port
        );
        
        Ok(())
    }
    
    /// Send data on a socket.
    pub fn send(&self, socket_id: SocketId, data: &[u8]) -> Result<usize, NetError> {
        let socket = self.sockets.get(&socket_id).ok_or(NetError::SocketNotFound)?;
        let mut socket = socket.lock();
        
        if socket.state != SocketState::Connected {
            return Err(NetError::InvalidArgument);
        }
        
        // TODO: Actual send
        socket.bytes_sent += data.len() as u64;
        
        Ok(data.len())
    }
    
    /// Receive data from a socket.
    pub fn receive(&self, socket_id: SocketId, buffer: &mut [u8]) -> Result<usize, NetError> {
        let socket = self.sockets.get(&socket_id).ok_or(NetError::SocketNotFound)?;
        let mut socket = socket.lock();
        
        if socket.state != SocketState::Connected {
            return Err(NetError::InvalidArgument);
        }
        
        let available = socket.recv_buffer.len().min(buffer.len());
        if available == 0 {
            return Err(NetError::WouldBlock);
        }
        
        buffer[..available].copy_from_slice(&socket.recv_buffer[..available]);
        socket.recv_buffer.drain(..available);
        socket.bytes_recv += available as u64;
        
        Ok(available)
    }
    
    /// Close a socket.
    pub fn close(&mut self, socket_id: SocketId) -> Result<(), NetError> {
        let socket = self.sockets.get(&socket_id).ok_or(NetError::SocketNotFound)?;
        let mut socket = socket.lock();
        
        socket.state = SocketState::Closed;
        
        // Remove from tab tracking
        let tab = socket.owner;
        drop(socket);
        
        if let Some(sockets) = self.tab_sockets.get_mut(&tab) {
            sockets.retain(|id| *id != socket_id);
        }
        
        self.sockets.remove(&socket_id);
        
        crate::serial_println!("[Net] Socket {:?} closed", socket_id);
        
        Ok(())
    }
    
    /// Close all sockets for a tab.
    pub fn close_tab_sockets(&mut self, tab: TabId) {
        if let Some(sockets) = self.tab_sockets.remove(&tab) {
            for socket_id in sockets {
                self.sockets.remove(&socket_id);
            }
        }
    }
    
    /// DNS lookup (cached).
    pub fn dns_lookup(&mut self, hostname: &str) -> Result<DnsResult, NetError> {
        // Check cache
        if let Some(result) = self.dns_cache.get(hostname) {
            return Ok(result.clone());
        }
        
        // TODO: Actual DNS resolution
        // For now, return mock result for localhost
        if hostname == "localhost" {
            let result = DnsResult {
                hostname: hostname.into(),
                addresses: alloc::vec![Ipv4Addr::LOCALHOST],
                ttl: 3600,
            };
            self.dns_cache.insert(hostname.into(), result.clone());
            return Ok(result);
        }
        
        Err(NetError::DnsLookupFailed)
    }
    
    /// Get socket info.
    pub fn get_socket(&self, socket_id: SocketId) -> Option<SocketState> {
        self.sockets.get(&socket_id).map(|s| s.lock().state)
    }
}

/// Global network manager.
static NET_MANAGER: RwLock<Option<NetworkManager>> = RwLock::new(None);

/// Initialize network manager.
pub fn init() {
    let mut mgr = NET_MANAGER.write();
    *mgr = Some(NetworkManager::new());
    crate::serial_println!("[Net] Network manager initialized");
}

/// Create socket.
pub fn create_socket(tab: TabId, socket_type: SocketType) -> Result<SocketId, NetError> {
    NET_MANAGER.write()
        .as_mut()
        .ok_or(NetError::PermissionDenied)?
        .create_socket(tab, socket_type)
}

/// Connect socket.
pub fn connect(socket_id: SocketId, addr: SocketAddr) -> Result<(), NetError> {
    NET_MANAGER.write()
        .as_mut()
        .ok_or(NetError::PermissionDenied)?
        .connect(socket_id, addr)
}

/// Send data.
pub fn send(socket_id: SocketId, data: &[u8]) -> Result<usize, NetError> {
    NET_MANAGER.read()
        .as_ref()
        .ok_or(NetError::PermissionDenied)?
        .send(socket_id, data)
}

/// Receive data.
pub fn receive(socket_id: SocketId, buffer: &mut [u8]) -> Result<usize, NetError> {
    NET_MANAGER.read()
        .as_ref()
        .ok_or(NetError::PermissionDenied)?
        .receive(socket_id, buffer)
}

/// Close socket.
pub fn close(socket_id: SocketId) -> Result<(), NetError> {
    NET_MANAGER.write()
        .as_mut()
        .ok_or(NetError::PermissionDenied)?
        .close(socket_id)
}

/// DNS lookup.
pub fn dns_lookup(hostname: &str) -> Result<DnsResult, NetError> {
    NET_MANAGER.write()
        .as_mut()
        .ok_or(NetError::PermissionDenied)?
        .dns_lookup(hostname)
}
