//! Network abstraction layer for KPIO
//!
//! This module provides TCP, UDP, and DNS functionality by communicating
//! with the KPIO kernel's network service via IPC.

use alloc::string::String;
use alloc::vec::Vec;
use core::time::Duration;

use crate::error::{NetError, PlatformError, Result};
use crate::ipc::ServiceChannel;

/// Network service channel
static mut NET_SERVICE: Option<ServiceChannel> = None;

/// Initialize network subsystem
pub fn init() {
    // Connect to kernel network service
    // In actual implementation, this would establish IPC channel
    log::debug!("[KPIO Net] Initializing network subsystem");
}

/// IPv4 address
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Addr(pub [u8; 4]);

impl Ipv4Addr {
    pub const LOCALHOST: Ipv4Addr = Ipv4Addr([127, 0, 0, 1]);
    pub const UNSPECIFIED: Ipv4Addr = Ipv4Addr([0, 0, 0, 0]);

    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Ipv4Addr([a, b, c, d])
    }

    pub fn octets(&self) -> [u8; 4] {
        self.0
    }
}

/// IPv6 address
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv6Addr(pub [u8; 16]);

impl Ipv6Addr {
    pub const LOCALHOST: Ipv6Addr = Ipv6Addr([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    pub const UNSPECIFIED: Ipv6Addr = Ipv6Addr([0; 16]);
}

/// IP address (v4 or v6)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpAddr {
    V4(Ipv4Addr),
    V6(Ipv6Addr),
}

/// Socket address
#[derive(Debug, Clone, Copy)]
pub struct SocketAddr {
    pub ip: IpAddr,
    pub port: u16,
}

impl SocketAddr {
    pub fn new(ip: IpAddr, port: u16) -> Self {
        SocketAddr { ip, port }
    }

    pub fn new_v4(a: u8, b: u8, c: u8, d: u8, port: u16) -> Self {
        SocketAddr {
            ip: IpAddr::V4(Ipv4Addr::new(a, b, c, d)),
            port,
        }
    }
}

/// TCP stream
pub struct TcpStream {
    socket_id: u64,
    peer_addr: SocketAddr,
    local_addr: SocketAddr,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
}

impl TcpStream {
    /// Connect to a remote address
    pub fn connect(addr: SocketAddr) -> Result<TcpStream> {
        Self::connect_timeout(addr, Duration::from_secs(30))
    }

    /// Connect with timeout
    pub fn connect_timeout(addr: SocketAddr, timeout: Duration) -> Result<TcpStream> {
        // Send connect request to kernel network service
        let request = NetRequest::TcpConnect {
            addr,
            timeout_ms: timeout.as_millis() as u64,
        };

        let response = send_net_request(&request)?;

        match response {
            NetResponse::TcpConnected {
                socket_id,
                local_addr,
            } => Ok(TcpStream {
                socket_id,
                peer_addr: addr,
                local_addr,
                read_timeout: None,
                write_timeout: None,
            }),
            NetResponse::Error(e) => Err(PlatformError::Network(e)),
            _ => Err(PlatformError::Network(NetError::Other)),
        }
    }

    /// Read data from stream
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let request = NetRequest::TcpRecv {
            socket_id: self.socket_id,
            max_len: buf.len(),
            timeout_ms: self.read_timeout.map(|d| d.as_millis() as u64),
        };

        let response = send_net_request(&request)?;

        match response {
            NetResponse::Data(data) => {
                let len = data.len().min(buf.len());
                buf[..len].copy_from_slice(&data[..len]);
                Ok(len)
            }
            NetResponse::Error(e) => Err(PlatformError::Network(e)),
            _ => Err(PlatformError::Network(NetError::Other)),
        }
    }

    /// Write data to stream
    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let request = NetRequest::TcpSend {
            socket_id: self.socket_id,
            data: buf.to_vec(),
            timeout_ms: self.write_timeout.map(|d| d.as_millis() as u64),
        };

        let response = send_net_request(&request)?;

        match response {
            NetResponse::BytesSent(n) => Ok(n),
            NetResponse::Error(e) => Err(PlatformError::Network(e)),
            _ => Err(PlatformError::Network(NetError::Other)),
        }
    }

    /// Flush the stream
    pub fn flush(&mut self) -> Result<()> {
        // TCP is typically unbuffered at this level
        Ok(())
    }

    /// Set read timeout
    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) {
        self.read_timeout = timeout;
    }

    /// Set write timeout
    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) {
        self.write_timeout = timeout;
    }

    /// Get peer address
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    /// Get local address
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Shutdown the connection
    pub fn shutdown(&mut self, how: Shutdown) -> Result<()> {
        let request = NetRequest::TcpShutdown {
            socket_id: self.socket_id,
            how,
        };

        let _response = send_net_request(&request)?;
        Ok(())
    }

    /// Try to clone the stream (creates new handle to same connection)
    pub fn try_clone(&self) -> Result<TcpStream> {
        let request = NetRequest::TcpClone {
            socket_id: self.socket_id,
        };

        let response = send_net_request(&request)?;

        match response {
            NetResponse::TcpCloned { socket_id } => Ok(TcpStream {
                socket_id,
                peer_addr: self.peer_addr,
                local_addr: self.local_addr,
                read_timeout: self.read_timeout,
                write_timeout: self.write_timeout,
            }),
            NetResponse::Error(e) => Err(PlatformError::Network(e)),
            _ => Err(PlatformError::Network(NetError::Other)),
        }
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let _ = self.shutdown(Shutdown::Both);
    }
}

/// TCP listener
pub struct TcpListener {
    socket_id: u64,
    local_addr: SocketAddr,
}

impl TcpListener {
    /// Bind to an address
    pub fn bind(addr: SocketAddr) -> Result<TcpListener> {
        let request = NetRequest::TcpBind { addr };

        let response = send_net_request(&request)?;

        match response {
            NetResponse::TcpBound { socket_id } => Ok(TcpListener {
                socket_id,
                local_addr: addr,
            }),
            NetResponse::Error(e) => Err(PlatformError::Network(e)),
            _ => Err(PlatformError::Network(NetError::Other)),
        }
    }

    /// Accept a connection
    pub fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        let request = NetRequest::TcpAccept {
            socket_id: self.socket_id,
        };

        let response = send_net_request(&request)?;

        match response {
            NetResponse::TcpAccepted {
                socket_id,
                peer_addr,
            } => {
                let stream = TcpStream {
                    socket_id,
                    peer_addr,
                    local_addr: self.local_addr,
                    read_timeout: None,
                    write_timeout: None,
                };
                Ok((stream, peer_addr))
            }
            NetResponse::Error(e) => Err(PlatformError::Network(e)),
            _ => Err(PlatformError::Network(NetError::Other)),
        }
    }

    /// Get local address
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

/// Shutdown mode
#[derive(Debug, Clone, Copy)]
pub enum Shutdown {
    Read,
    Write,
    Both,
}

/// DNS lookup
pub fn lookup_host(host: &str) -> Result<Vec<IpAddr>> {
    let request = NetRequest::DnsLookup {
        hostname: String::from(host),
    };

    let response = send_net_request(&request)?;

    match response {
        NetResponse::DnsResolved { addresses } => Ok(addresses),
        NetResponse::Error(e) => Err(PlatformError::Network(e)),
        _ => Err(PlatformError::Network(NetError::DnsLookupFailed)),
    }
}

// ============================================
// Internal protocol types
// ============================================

#[derive(Debug)]
enum NetRequest {
    TcpConnect {
        addr: SocketAddr,
        timeout_ms: u64,
    },
    TcpBind {
        addr: SocketAddr,
    },
    TcpAccept {
        socket_id: u64,
    },
    TcpSend {
        socket_id: u64,
        data: Vec<u8>,
        timeout_ms: Option<u64>,
    },
    TcpRecv {
        socket_id: u64,
        max_len: usize,
        timeout_ms: Option<u64>,
    },
    TcpShutdown {
        socket_id: u64,
        how: Shutdown,
    },
    TcpClone {
        socket_id: u64,
    },
    DnsLookup {
        hostname: String,
    },
}

#[derive(Debug)]
enum NetResponse {
    TcpConnected {
        socket_id: u64,
        local_addr: SocketAddr,
    },
    TcpBound {
        socket_id: u64,
    },
    TcpAccepted {
        socket_id: u64,
        peer_addr: SocketAddr,
    },
    TcpCloned {
        socket_id: u64,
    },
    BytesSent(usize),
    Data(Vec<u8>),
    DnsResolved {
        addresses: Vec<IpAddr>,
    },
    Error(NetError),
    Ok,
}

fn send_net_request(_request: &NetRequest) -> Result<NetResponse> {
    // TODO: Serialize request and send via IPC to kernel network service
    // For now, return error as network service not yet connected
    Err(PlatformError::Network(NetError::NetworkDown))
}
