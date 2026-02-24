//! WASI Preview 2 — `wasi:sockets` interfaces.
//!
//! Provides TCP and UDP socket resources and an IP name-lookup
//! service.
//!
//! When compiled with the `kernel` feature, TCP connections and DNS
//! lookups go through the kernel's real network stack (VirtIO NIC).
//! Without the feature, an in-memory loopback simulation is used so
//! that unit tests remain deterministic.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use spin::Mutex;

// ---------------------------------------------------------------------------
// Network types
// ---------------------------------------------------------------------------

/// IP address family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpAddressFamily {
    Ipv4,
    Ipv6,
}

/// An IP address (v4 or v6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpAddress {
    Ipv4(u8, u8, u8, u8),
    Ipv6([u16; 8]),
}

impl IpAddress {
    pub fn localhost_v4() -> Self {
        IpAddress::Ipv4(127, 0, 0, 1)
    }

    pub fn unspecified_v4() -> Self {
        IpAddress::Ipv4(0, 0, 0, 0)
    }

    pub fn family(&self) -> IpAddressFamily {
        match self {
            IpAddress::Ipv4(..) => IpAddressFamily::Ipv4,
            IpAddress::Ipv6(..) => IpAddressFamily::Ipv6,
        }
    }
}

/// A socket address = IP + port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpSocketAddress {
    pub address: IpAddress,
    pub port: u16,
}

/// Shutdown mode for a TCP socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownType {
    Receive,
    Send,
    Both,
}

/// Socket errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketError {
    /// Address already in use.
    AddressInUse,
    /// Address not bindable.
    AddressNotBindable,
    /// Connection refused.
    ConnectionRefused,
    /// Connection reset.
    ConnectionReset,
    /// Connection aborted.
    ConnectionAborted,
    /// Not connected.
    NotConnected,
    /// Invalid argument.
    InvalidArgument,
    /// Would block (non-blocking mode).
    WouldBlock,
    /// Already bound.
    AlreadyBound,
    /// Already connected.
    AlreadyConnected,
    /// Already listening.
    AlreadyListening,
    /// Not bound.
    NotBound,
    /// Not listening.
    NotListening,
    /// Generic / unknown error.
    Unknown(String),
}

// ---------------------------------------------------------------------------
// TCP Socket
// ---------------------------------------------------------------------------

/// State machine for a TCP socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    /// Initial state after creation.
    Unbound,
    /// Bind started (async).
    BindStarted,
    /// Bound to a local address.
    Bound,
    /// Listen started (async).
    ListenStarted,
    /// Listening for incoming connections.
    Listening,
    /// Connect started (async).
    ConnectStarted,
    /// Connected to a remote peer.
    Connected,
    /// Shut down.
    Closed,
}

/// An in-memory TCP socket.
///
/// When the `kernel` feature is enabled and the socket connects to a
/// non-loopback address, `kernel_conn` holds the raw `ConnId` from
/// the kernel TCP stack so that `send()`/`recv()` go through the real
/// VirtIO NIC.
#[derive(Debug, Clone)]
pub struct TcpSocket {
    pub id: u32,
    pub family: IpAddressFamily,
    pub state: TcpState,
    pub local_address: Option<IpSocketAddress>,
    pub remote_address: Option<IpSocketAddress>,
    /// Incoming data (data received from peer).
    pub recv_buffer: Vec<u8>,
    /// Outgoing data (data sent to peer).
    pub send_buffer: Vec<u8>,
    /// Accepted connections waiting in the backlog.
    pub accept_queue: Vec<u32>,
    /// Kernel TCP connection handle (raw `ConnId` value).
    /// `Some(id)` when backed by a real kernel connection.
    pub kernel_conn: Option<u64>,
}

impl TcpSocket {
    pub fn new(id: u32, family: IpAddressFamily) -> Self {
        Self {
            id,
            family,
            state: TcpState::Unbound,
            local_address: None,
            remote_address: None,
            recv_buffer: Vec::new(),
            send_buffer: Vec::new(),
            accept_queue: Vec::new(),
            kernel_conn: None,
        }
    }

    pub fn start_bind(&mut self, address: IpSocketAddress) -> Result<(), SocketError> {
        if self.state != TcpState::Unbound {
            return Err(SocketError::AlreadyBound);
        }
        if address.address.family() != self.family {
            return Err(SocketError::InvalidArgument);
        }
        self.local_address = Some(address);
        self.state = TcpState::BindStarted;
        Ok(())
    }

    pub fn finish_bind(&mut self) -> Result<(), SocketError> {
        if self.state != TcpState::BindStarted {
            return Err(SocketError::NotBound);
        }
        self.state = TcpState::Bound;
        Ok(())
    }

    pub fn start_connect(&mut self, address: IpSocketAddress) -> Result<(), SocketError> {
        match self.state {
            TcpState::Unbound | TcpState::Bound => {}
            TcpState::Connected => return Err(SocketError::AlreadyConnected),
            _ => return Err(SocketError::InvalidArgument),
        }
        if address.address.family() != self.family {
            return Err(SocketError::InvalidArgument);
        }
        self.remote_address = Some(address.clone());
        self.state = TcpState::ConnectStarted;

        // When the kernel feature is enabled and the destination is not
        // loopback, establish a real TCP connection via the kernel stack.
        #[cfg(feature = "kernel")]
        {
            if let IpAddress::Ipv4(a, b, c, d) = &address.address {
                let ip = [*a, *b, *c, *d];
                let is_loopback = ip[0] == 127;
                if !is_loopback {
                    match kpio_kernel::net::wasi_bridge::tcp_connect(ip, address.port) {
                        Ok(handle) => {
                            self.kernel_conn = Some(handle.conn_id.0);
                            self.state = TcpState::Connected;
                            // Intentionally skip destroy — handle is consumed
                            // by storing the conn_id; the kernel owns the
                            // connection until we explicitly close it.
                            core::mem::forget(handle);
                        }
                        Err(_) => {
                            self.state = TcpState::Closed;
                            return Err(SocketError::ConnectionRefused);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn finish_connect(&mut self) -> Result<(), SocketError> {
        // If the kernel already promoted us to Connected in start_connect,
        // just return success.
        if self.state == TcpState::Connected {
            return Ok(());
        }
        if self.state != TcpState::ConnectStarted {
            return Err(SocketError::NotConnected);
        }
        self.state = TcpState::Connected;
        Ok(())
    }

    pub fn start_listen(&mut self) -> Result<(), SocketError> {
        if self.state != TcpState::Bound {
            return Err(SocketError::NotBound);
        }
        self.state = TcpState::ListenStarted;
        Ok(())
    }

    pub fn finish_listen(&mut self) -> Result<(), SocketError> {
        if self.state != TcpState::ListenStarted {
            return Err(SocketError::NotListening);
        }
        self.state = TcpState::Listening;
        Ok(())
    }

    pub fn accept(&mut self) -> Result<u32, SocketError> {
        if self.state != TcpState::Listening {
            return Err(SocketError::NotListening);
        }
        self.accept_queue.pop().ok_or(SocketError::WouldBlock)
    }

    pub fn send(&mut self, data: &[u8]) -> Result<usize, SocketError> {
        if self.state != TcpState::Connected {
            return Err(SocketError::NotConnected);
        }
        // Kernel-backed connection: forward to real TCP stack.
        #[cfg(feature = "kernel")]
        if let Some(conn_id) = self.kernel_conn {
            let handle = kpio_kernel::net::wasi_bridge::TcpHandle {
                conn_id: kpio_kernel::net::tcp::ConnId(conn_id),
            };
            let result = kpio_kernel::net::wasi_bridge::tcp_send(&handle, data);
            core::mem::forget(handle);
            return result.map_err(|_| SocketError::ConnectionReset);
        }
        // Loopback fallback.
        self.send_buffer.extend_from_slice(data);
        Ok(data.len())
    }

    pub fn recv(&mut self, max_len: usize) -> Result<Vec<u8>, SocketError> {
        if self.state != TcpState::Connected {
            return Err(SocketError::NotConnected);
        }
        // Kernel-backed connection: receive from real TCP stack.
        #[cfg(feature = "kernel")]
        if let Some(conn_id) = self.kernel_conn {
            let handle = kpio_kernel::net::wasi_bridge::TcpHandle {
                conn_id: kpio_kernel::net::tcp::ConnId(conn_id),
            };
            let mut buf = alloc::vec![0u8; max_len];
            let result = kpio_kernel::net::wasi_bridge::tcp_recv(&handle, &mut buf);
            core::mem::forget(handle);
            return match result {
                Ok(0) => Err(SocketError::WouldBlock),
                Ok(n) => Ok(buf[..n].to_vec()),
                Err(_) => Err(SocketError::WouldBlock),
            };
        }
        // Loopback fallback.
        if self.recv_buffer.is_empty() {
            return Err(SocketError::WouldBlock);
        }
        let len = max_len.min(self.recv_buffer.len());
        let data = self.recv_buffer[..len].to_vec();
        self.recv_buffer = self.recv_buffer[len..].to_vec();
        Ok(data)
    }

    pub fn shutdown(&mut self, _how: ShutdownType) -> Result<(), SocketError> {
        if self.state != TcpState::Connected {
            return Err(SocketError::NotConnected);
        }
        // Kernel-backed: close the real TCP connection.
        #[cfg(feature = "kernel")]
        if let Some(conn_id) = self.kernel_conn.take() {
            let handle = kpio_kernel::net::wasi_bridge::TcpHandle {
                conn_id: kpio_kernel::net::tcp::ConnId(conn_id),
            };
            let _ = kpio_kernel::net::wasi_bridge::tcp_close(&handle);
            kpio_kernel::net::wasi_bridge::tcp_destroy(handle);
        }
        self.state = TcpState::Closed;
        Ok(())
    }

    pub fn local_address(&self) -> Result<IpSocketAddress, SocketError> {
        self.local_address.clone().ok_or(SocketError::NotBound)
    }

    pub fn remote_address(&self) -> Result<IpSocketAddress, SocketError> {
        self.remote_address.clone().ok_or(SocketError::NotConnected)
    }
}

// ---------------------------------------------------------------------------
// UDP Socket
// ---------------------------------------------------------------------------

/// State machine for a UDP socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdpState {
    Unbound,
    BindStarted,
    Bound,
    Closed,
}

/// A single UDP datagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Datagram {
    pub data: Vec<u8>,
    pub remote_address: IpSocketAddress,
}

/// An in-memory UDP socket.
#[derive(Debug, Clone)]
pub struct UdpSocket {
    pub id: u32,
    pub family: IpAddressFamily,
    pub state: UdpState,
    pub local_address: Option<IpSocketAddress>,
    /// Incoming datagrams.
    pub incoming: Vec<Datagram>,
    /// Outgoing datagrams.
    pub outgoing: Vec<Datagram>,
}

impl UdpSocket {
    pub fn new(id: u32, family: IpAddressFamily) -> Self {
        Self {
            id,
            family,
            state: UdpState::Unbound,
            local_address: None,
            incoming: Vec::new(),
            outgoing: Vec::new(),
        }
    }

    pub fn start_bind(&mut self, address: IpSocketAddress) -> Result<(), SocketError> {
        if self.state != UdpState::Unbound {
            return Err(SocketError::AlreadyBound);
        }
        if address.address.family() != self.family {
            return Err(SocketError::InvalidArgument);
        }
        self.local_address = Some(address);
        self.state = UdpState::BindStarted;
        Ok(())
    }

    pub fn finish_bind(&mut self) -> Result<(), SocketError> {
        if self.state != UdpState::BindStarted {
            return Err(SocketError::NotBound);
        }
        self.state = UdpState::Bound;
        Ok(())
    }

    pub fn send_datagram(&mut self, datagram: Datagram) -> Result<(), SocketError> {
        if self.state != UdpState::Bound {
            return Err(SocketError::NotBound);
        }
        self.outgoing.push(datagram);
        Ok(())
    }

    pub fn recv_datagram(&mut self) -> Result<Datagram, SocketError> {
        if self.state != UdpState::Bound {
            return Err(SocketError::NotBound);
        }
        self.incoming.pop().ok_or(SocketError::WouldBlock)
    }

    pub fn local_address(&self) -> Result<IpSocketAddress, SocketError> {
        self.local_address.clone().ok_or(SocketError::NotBound)
    }
}

// ---------------------------------------------------------------------------
// IP Name Lookup
// ---------------------------------------------------------------------------

/// Result of an IP name lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveAddressStream {
    addresses: Vec<IpAddress>,
    index: usize,
}

impl ResolveAddressStream {
    pub fn next(&mut self) -> Option<IpAddress> {
        if self.index < self.addresses.len() {
            let addr = self.addresses[self.index].clone();
            self.index += 1;
            Some(addr)
        } else {
            None
        }
    }

    pub fn is_done(&self) -> bool {
        self.index >= self.addresses.len()
    }
}

/// Resolve hostnames to IP addresses.
///
/// With the `kernel` feature: performs a real DNS lookup (host table →
/// cache → wire query over UDP through the VirtIO NIC).
/// Without the feature: returns deterministic stubs for testing.
pub fn resolve_addresses(name: &str) -> Result<ResolveAddressStream, SocketError> {
    if name.is_empty() {
        return Err(SocketError::InvalidArgument);
    }

    #[cfg(feature = "kernel")]
    {
        resolve_addresses_kernel(name)
    }
    #[cfg(not(feature = "kernel"))]
    {
        resolve_addresses_mock(name)
    }
}

/// Real DNS resolver backed by the kernel network stack.
#[cfg(feature = "kernel")]
fn resolve_addresses_kernel(name: &str) -> Result<ResolveAddressStream, SocketError> {
    // Fast-path well-known names without hitting the kernel.
    match name {
        "localhost" | "127.0.0.1" => {
            return Ok(ResolveAddressStream {
                addresses: vec![IpAddress::localhost_v4()],
                index: 0,
            });
        }
        "::1" => {
            return Ok(ResolveAddressStream {
                addresses: vec![IpAddress::Ipv6([0, 0, 0, 0, 0, 0, 0, 1])],
                index: 0,
            });
        }
        _ => {}
    }

    match kpio_kernel::net::wasi_bridge::dns_resolve(name) {
        Ok(addrs) => {
            let addresses: Vec<IpAddress> = addrs
                .iter()
                .map(|octets| IpAddress::Ipv4(octets[0], octets[1], octets[2], octets[3]))
                .collect();
            if addresses.is_empty() {
                Err(SocketError::Unknown(alloc::format!("DNS: no results for {}", name)))
            } else {
                Ok(ResolveAddressStream {
                    addresses,
                    index: 0,
                })
            }
        }
        Err(_) => Err(SocketError::Unknown(alloc::format!(
            "DNS resolution failed for {}",
            name
        ))),
    }
}

/// Stub resolver for testing (no kernel).
#[cfg(not(feature = "kernel"))]
fn resolve_addresses_mock(name: &str) -> Result<ResolveAddressStream, SocketError> {
    let addresses = match name {
        "localhost" | "127.0.0.1" => vec![IpAddress::localhost_v4()],
        "::1" => vec![IpAddress::Ipv6([0, 0, 0, 0, 0, 0, 0, 1])],
        _ => {
            // For unknown hosts, return a deterministic fake
            vec![IpAddress::Ipv4(10, 0, 0, 1)]
        }
    };
    Ok(ResolveAddressStream {
        addresses,
        index: 0,
    })
}

// ---------------------------------------------------------------------------
// Socket Loopback Manager (for testing)
// ---------------------------------------------------------------------------

/// Global socket manager for in-memory loopback.
static SOCKET_MANAGER: Mutex<Option<LoopbackManager>> = Mutex::new(None);

struct LoopbackManager {
    tcp_sockets: BTreeMap<u32, TcpSocket>,
    udp_sockets: BTreeMap<u32, UdpSocket>,
    next_id: u32,
    /// Listening sockets by port → socket ID.
    tcp_listeners: BTreeMap<u16, u32>,
}

impl LoopbackManager {
    fn new() -> Self {
        Self {
            tcp_sockets: BTreeMap::new(),
            udp_sockets: BTreeMap::new(),
            next_id: 1,
            tcp_listeners: BTreeMap::new(),
        }
    }
}

fn with_mgr<F, R>(f: F) -> R
where
    F: FnOnce(&mut LoopbackManager) -> R,
{
    let mut guard = SOCKET_MANAGER.lock();
    if guard.is_none() {
        *guard = Some(LoopbackManager::new());
    }
    f(guard.as_mut().unwrap())
}

/// Create a new TCP socket, returns socket ID.
pub fn create_tcp_socket(family: IpAddressFamily) -> u32 {
    with_mgr(|mgr| {
        let id = mgr.next_id;
        mgr.next_id += 1;
        mgr.tcp_sockets.insert(id, TcpSocket::new(id, family));
        id
    })
}

/// Create a new UDP socket, returns socket ID.
pub fn create_udp_socket(family: IpAddressFamily) -> u32 {
    with_mgr(|mgr| {
        let id = mgr.next_id;
        mgr.next_id += 1;
        mgr.udp_sockets.insert(id, UdpSocket::new(id, family));
        id
    })
}

/// Perform TCP loopback: connect a client to a listening server socket
/// on the same loopback manager, creating the accepted socket.
pub fn tcp_loopback_connect(
    client_id: u32,
    server_port: u16,
) -> Result<u32, SocketError> {
    with_mgr(|mgr| {
        // find the listener
        let listener_id = mgr
            .tcp_listeners
            .get(&server_port)
            .copied()
            .ok_or(SocketError::ConnectionRefused)?;

        // create accepted socket
        let accepted_id = mgr.next_id;
        mgr.next_id += 1;
        let fam = mgr
            .tcp_sockets
            .get(&listener_id)
            .map(|s| s.family)
            .unwrap_or(IpAddressFamily::Ipv4);
        let mut accepted = TcpSocket::new(accepted_id, fam);
        accepted.state = TcpState::Connected;
        accepted.local_address = Some(IpSocketAddress {
            address: IpAddress::localhost_v4(),
            port: server_port,
        });
        mgr.tcp_sockets.insert(accepted_id, accepted);

        // enqueue into listener's accept queue
        if let Some(listener) = mgr.tcp_sockets.get_mut(&listener_id) {
            listener.accept_queue.push(accepted_id);
        }

        // mark client as connected
        if let Some(client) = mgr.tcp_sockets.get_mut(&client_id) {
            client.state = TcpState::Connected;
        }

        Ok(accepted_id)
    })
}

/// Helper: register a bound+listening socket's port in the listener map.
pub fn tcp_register_listener(socket_id: u32, port: u16) {
    with_mgr(|mgr| {
        mgr.tcp_listeners.insert(port, socket_id);
    });
}

/// Helper: deliver data from sender to receiver (loopback pipe).
pub fn tcp_loopback_deliver(from_id: u32, to_id: u32) {
    with_mgr(|mgr| {
        let data = mgr
            .tcp_sockets
            .get_mut(&from_id)
            .map(|s| {
                let d = s.send_buffer.clone();
                s.send_buffer.clear();
                d
            })
            .unwrap_or_default();
        if let Some(to) = mgr.tcp_sockets.get_mut(&to_id) {
            to.recv_buffer.extend_from_slice(&data);
        }
    });
}

/// UDP loopback: move outgoing datagrams from sender to receiver.
pub fn udp_loopback_deliver(from_id: u32, to_id: u32) {
    with_mgr(|mgr| {
        let datagrams: Vec<Datagram> = mgr
            .udp_sockets
            .get_mut(&from_id)
            .map(|s| {
                let d = s.outgoing.clone();
                s.outgoing.clear();
                d
            })
            .unwrap_or_default();
        if let Some(to) = mgr.udp_sockets.get_mut(&to_id) {
            for dg in datagrams {
                to.incoming.push(dg);
            }
        }
    });
}

/// Access a TCP socket by ID (mutable).
pub fn with_tcp_socket<F, R>(id: u32, f: F) -> Result<R, SocketError>
where
    F: FnOnce(&mut TcpSocket) -> R,
{
    with_mgr(|mgr| {
        mgr.tcp_sockets
            .get_mut(&id)
            .map(f)
            .ok_or(SocketError::InvalidArgument)
    })
}

/// Access a UDP socket by ID (mutable).
pub fn with_udp_socket<F, R>(id: u32, f: F) -> Result<R, SocketError>
where
    F: FnOnce(&mut UdpSocket) -> R,
{
    with_mgr(|mgr| {
        mgr.udp_sockets
            .get_mut(&id)
            .map(f)
            .ok_or(SocketError::InvalidArgument)
    })
}

/// Reset the global loopback manager (for test isolation).
pub fn reset_loopback() {
    let mut guard = SOCKET_MANAGER.lock();
    *guard = Some(LoopbackManager::new());
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- IP types --

    #[test]
    fn ip_address_family() {
        assert_eq!(IpAddress::localhost_v4().family(), IpAddressFamily::Ipv4);
        let v6 = IpAddress::Ipv6([0; 8]);
        assert_eq!(v6.family(), IpAddressFamily::Ipv6);
    }

    // -- TCP state machine --

    #[test]
    fn tcp_bind_listen_accept_flow() {
        reset_loopback();

        // Server side
        let srv_id = create_tcp_socket(IpAddressFamily::Ipv4);
        with_tcp_socket(srv_id, |s| {
            s.start_bind(IpSocketAddress {
                address: IpAddress::localhost_v4(),
                port: 8080,
            }).unwrap();
            s.finish_bind().unwrap();
            s.start_listen().unwrap();
            s.finish_listen().unwrap();
        }).unwrap();
        tcp_register_listener(srv_id, 8080);

        // Client side
        let cli_id = create_tcp_socket(IpAddressFamily::Ipv4);
        with_tcp_socket(cli_id, |s| {
            s.start_connect(IpSocketAddress {
                address: IpAddress::localhost_v4(),
                port: 8080,
            }).unwrap();
        }).unwrap();

        // Loopback connect
        let accepted_id = tcp_loopback_connect(cli_id, 8080).unwrap();

        // Client send
        with_tcp_socket(cli_id, |s| {
            s.send(b"Hello TCP").unwrap();
        }).unwrap();

        // Deliver client → accepted
        tcp_loopback_deliver(cli_id, accepted_id);

        // Accepted recv
        let data = with_tcp_socket(accepted_id, |s| {
            s.recv(100).unwrap()
        }).unwrap();
        assert_eq!(data, b"Hello TCP");
    }

    #[test]
    fn tcp_connect_no_listener() {
        reset_loopback();
        let cli_id = create_tcp_socket(IpAddressFamily::Ipv4);
        let result = tcp_loopback_connect(cli_id, 9999);
        assert_eq!(result, Err(SocketError::ConnectionRefused));
    }

    #[test]
    fn tcp_send_not_connected() {
        let mut sock = TcpSocket::new(1, IpAddressFamily::Ipv4);
        let result = sock.send(b"data");
        assert_eq!(result, Err(SocketError::NotConnected));
    }

    #[test]
    fn tcp_recv_empty() {
        let mut sock = TcpSocket::new(1, IpAddressFamily::Ipv4);
        sock.state = TcpState::Connected;
        let result = sock.recv(100);
        assert_eq!(result, Err(SocketError::WouldBlock));
    }

    #[test]
    fn tcp_double_bind() {
        let mut sock = TcpSocket::new(1, IpAddressFamily::Ipv4);
        sock.start_bind(IpSocketAddress {
            address: IpAddress::localhost_v4(),
            port: 80,
        }).unwrap();
        sock.finish_bind().unwrap();
        let result = sock.start_bind(IpSocketAddress {
            address: IpAddress::localhost_v4(),
            port: 81,
        });
        assert_eq!(result, Err(SocketError::AlreadyBound));
    }

    #[test]
    fn tcp_shutdown() {
        let mut sock = TcpSocket::new(1, IpAddressFamily::Ipv4);
        sock.state = TcpState::Connected;
        sock.shutdown(ShutdownType::Both).unwrap();
        assert_eq!(sock.state, TcpState::Closed);
    }

    #[test]
    fn tcp_local_remote_address() {
        let mut sock = TcpSocket::new(1, IpAddressFamily::Ipv4);
        assert!(sock.local_address().is_err());
        assert!(sock.remote_address().is_err());
        sock.local_address = Some(IpSocketAddress {
            address: IpAddress::localhost_v4(),
            port: 3000,
        });
        assert_eq!(sock.local_address().unwrap().port, 3000);
    }

    // -- UDP --

    #[test]
    fn udp_bind_send_recv_loopback() {
        reset_loopback();

        let a_id = create_udp_socket(IpAddressFamily::Ipv4);
        let b_id = create_udp_socket(IpAddressFamily::Ipv4);

        let addr_a = IpSocketAddress {
            address: IpAddress::localhost_v4(),
            port: 5000,
        };
        let addr_b = IpSocketAddress {
            address: IpAddress::localhost_v4(),
            port: 5001,
        };

        // Bind both
        with_udp_socket(a_id, |s| {
            s.start_bind(addr_a.clone()).unwrap();
            s.finish_bind().unwrap();
        }).unwrap();
        with_udp_socket(b_id, |s| {
            s.start_bind(addr_b.clone()).unwrap();
            s.finish_bind().unwrap();
        }).unwrap();

        // A sends to B
        with_udp_socket(a_id, |s| {
            s.send_datagram(Datagram {
                data: alloc::vec![1, 2, 3],
                remote_address: addr_b.clone(),
            }).unwrap();
        }).unwrap();

        // Deliver A → B
        udp_loopback_deliver(a_id, b_id);

        // B receives
        let dg = with_udp_socket(b_id, |s| {
            s.recv_datagram().unwrap()
        }).unwrap();
        assert_eq!(dg.data, alloc::vec![1, 2, 3]);
    }

    #[test]
    fn udp_send_not_bound() {
        let mut sock = UdpSocket::new(1, IpAddressFamily::Ipv4);
        let result = sock.send_datagram(Datagram {
            data: alloc::vec![1],
            remote_address: IpSocketAddress {
                address: IpAddress::localhost_v4(),
                port: 80,
            },
        });
        assert_eq!(result, Err(SocketError::NotBound));
    }

    #[test]
    fn udp_recv_empty() {
        let mut sock = UdpSocket::new(1, IpAddressFamily::Ipv4);
        sock.state = UdpState::Bound;
        assert_eq!(sock.recv_datagram(), Err(SocketError::WouldBlock));
    }

    // -- IP Name Lookup --

    #[test]
    fn resolve_localhost() {
        let mut stream = resolve_addresses("localhost").unwrap();
        let addr = stream.next().unwrap();
        assert_eq!(addr, IpAddress::localhost_v4());
        assert!(stream.is_done());
    }

    #[test]
    fn resolve_empty_name() {
        let result = resolve_addresses("");
        assert_eq!(result, Err(SocketError::InvalidArgument));
    }

    #[test]
    fn resolve_unknown_host() {
        let mut stream = resolve_addresses("example.com").unwrap();
        let addr = stream.next().unwrap();
        // Returns deterministic fake
        assert_eq!(addr, IpAddress::Ipv4(10, 0, 0, 1));
    }

    #[test]
    fn resolve_ipv6_loopback() {
        let mut stream = resolve_addresses("::1").unwrap();
        let addr = stream.next().unwrap();
        assert_eq!(addr, IpAddress::Ipv6([0, 0, 0, 0, 0, 0, 0, 1]));
    }

    // -- Family Mismatch --

    #[test]
    fn tcp_bind_family_mismatch() {
        let mut sock = TcpSocket::new(1, IpAddressFamily::Ipv6);
        let result = sock.start_bind(IpSocketAddress {
            address: IpAddress::Ipv4(0, 0, 0, 0),
            port: 80,
        });
        assert_eq!(result, Err(SocketError::InvalidArgument));
    }

    #[test]
    fn tcp_listen_without_bind() {
        let mut sock = TcpSocket::new(1, IpAddressFamily::Ipv4);
        assert_eq!(sock.start_listen(), Err(SocketError::NotBound));
    }

    #[test]
    fn tcp_accept_not_listening() {
        let mut sock = TcpSocket::new(1, IpAddressFamily::Ipv4);
        assert_eq!(sock.accept(), Err(SocketError::NotListening));
    }
}
