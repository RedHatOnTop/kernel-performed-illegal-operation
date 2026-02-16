//! std::net compatibility layer for KPIO
//!
//! Provides TCP, UDP, and DNS functionality via KPIO syscalls.

use alloc::string::String;
use alloc::vec::Vec;

use crate::syscall::{self, SyscallError};

/// IPv4 address
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Addr {
    octets: [u8; 4],
}

impl Ipv4Addr {
    pub const LOCALHOST: Ipv4Addr = Ipv4Addr {
        octets: [127, 0, 0, 1],
    };
    pub const UNSPECIFIED: Ipv4Addr = Ipv4Addr {
        octets: [0, 0, 0, 0],
    };
    pub const BROADCAST: Ipv4Addr = Ipv4Addr {
        octets: [255, 255, 255, 255],
    };

    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Ipv4Addr {
            octets: [a, b, c, d],
        }
    }

    pub const fn octets(&self) -> [u8; 4] {
        self.octets
    }
}

/// IPv6 address
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv6Addr {
    octets: [u8; 16],
}

impl Ipv6Addr {
    pub const LOCALHOST: Ipv6Addr = Ipv6Addr {
        octets: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    };
    pub const UNSPECIFIED: Ipv6Addr = Ipv6Addr { octets: [0; 16] };
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
    ip: IpAddr,
    port: u16,
}

impl SocketAddr {
    pub fn new(ip: IpAddr, port: u16) -> Self {
        SocketAddr { ip, port }
    }

    pub fn ip(&self) -> IpAddr {
        self.ip
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

/// SocketAddrV4
#[derive(Debug, Clone, Copy)]
pub struct SocketAddrV4 {
    ip: Ipv4Addr,
    port: u16,
}

impl SocketAddrV4 {
    pub fn new(ip: Ipv4Addr, port: u16) -> Self {
        SocketAddrV4 { ip, port }
    }

    pub fn ip(&self) -> &Ipv4Addr {
        &self.ip
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl From<SocketAddrV4> for SocketAddr {
    fn from(v4: SocketAddrV4) -> Self {
        SocketAddr::new(IpAddr::V4(v4.ip), v4.port)
    }
}

/// TCP stream
pub struct TcpStream {
    fd: u64,
}

impl TcpStream {
    /// Connect to a remote address
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<TcpStream, IoError> {
        let addrs = addr.to_socket_addrs()?;

        for addr in addrs {
            match Self::connect_addr(&addr) {
                Ok(stream) => return Ok(stream),
                Err(_) => continue,
            }
        }

        Err(IoError::ConnectionRefused)
    }

    fn connect_addr(addr: &SocketAddr) -> Result<TcpStream, IoError> {
        // Pack address for syscall
        let (ip_bytes, port) = match addr.ip {
            IpAddr::V4(v4) => (v4.octets, addr.port),
            IpAddr::V6(_) => return Err(IoError::AddrNotAvailable),
        };

        let fd = syscall::net_connect(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3], port)
            .map_err(|_| IoError::ConnectionRefused)?;

        Ok(TcpStream { fd })
    }

    /// Read from stream
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError> {
        syscall::net_recv(self.fd, buf)
            .map(|n| n as usize)
            .map_err(|_| IoError::Other)
    }

    /// Write to stream
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> {
        syscall::net_send(self.fd, buf)
            .map(|n| n as usize)
            .map_err(|_| IoError::Other)
    }

    /// Flush (no-op for TCP)
    pub fn flush(&mut self) -> Result<(), IoError> {
        Ok(())
    }

    /// Shutdown
    pub fn shutdown(&mut self, how: Shutdown) -> Result<(), IoError> {
        let how_val = match how {
            Shutdown::Read => 0,
            Shutdown::Write => 1,
            Shutdown::Both => 2,
        };
        syscall::net_shutdown(self.fd, how_val)
            .map(|_| ())
            .map_err(|_| IoError::Other)
    }

    /// Try to clone
    pub fn try_clone(&self) -> Result<TcpStream, IoError> {
        let new_fd = syscall::net_dup(self.fd).map_err(|_| IoError::Other)?;
        Ok(TcpStream { fd: new_fd })
    }

    /// Set read timeout
    pub fn set_read_timeout(&self, _dur: Option<core::time::Duration>) -> Result<(), IoError> {
        // TODO: Implement via syscall
        Ok(())
    }

    /// Set write timeout
    pub fn set_write_timeout(&self, _dur: Option<core::time::Duration>) -> Result<(), IoError> {
        // TODO: Implement via syscall
        Ok(())
    }

    /// Set nodelay
    pub fn set_nodelay(&self, _nodelay: bool) -> Result<(), IoError> {
        // TODO: Implement via syscall
        Ok(())
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let _ = syscall::net_close(self.fd);
    }
}

/// TCP listener
pub struct TcpListener {
    fd: u64,
}

impl TcpListener {
    /// Bind to address
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<TcpListener, IoError> {
        let addrs = addr.to_socket_addrs()?;

        for addr in addrs {
            let (ip_bytes, port) = match addr.ip {
                IpAddr::V4(v4) => (v4.octets, addr.port),
                IpAddr::V6(_) => continue,
            };

            let fd = syscall::net_bind(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3], port)
                .map_err(|_| IoError::AddrInUse)?;

            return Ok(TcpListener { fd });
        }

        Err(IoError::AddrNotAvailable)
    }

    /// Accept connection
    pub fn accept(&self) -> Result<(TcpStream, SocketAddr), IoError> {
        let (fd, ip, port) = syscall::net_accept(self.fd).map_err(|_| IoError::Other)?;

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3])), port);

        Ok((TcpStream { fd }, addr))
    }

    /// Get local address
    pub fn local_addr(&self) -> Result<SocketAddr, IoError> {
        let (ip, port) = syscall::net_local_addr(self.fd).map_err(|_| IoError::Other)?;

        Ok(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3])),
            port,
        ))
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        let _ = syscall::net_close(self.fd);
    }
}

/// Shutdown mode
#[derive(Debug, Clone, Copy)]
pub enum Shutdown {
    Read,
    Write,
    Both,
}

/// ToSocketAddrs trait
pub trait ToSocketAddrs {
    type Iter: Iterator<Item = SocketAddr>;
    fn to_socket_addrs(&self) -> Result<Self::Iter, IoError>;
}

impl ToSocketAddrs for SocketAddr {
    type Iter = core::iter::Once<SocketAddr>;
    fn to_socket_addrs(&self) -> Result<Self::Iter, IoError> {
        Ok(core::iter::once(*self))
    }
}

impl ToSocketAddrs for SocketAddrV4 {
    type Iter = core::iter::Once<SocketAddr>;
    fn to_socket_addrs(&self) -> Result<Self::Iter, IoError> {
        Ok(core::iter::once(SocketAddr::from(*self)))
    }
}

impl ToSocketAddrs for (Ipv4Addr, u16) {
    type Iter = core::iter::Once<SocketAddr>;
    fn to_socket_addrs(&self) -> Result<Self::Iter, IoError> {
        Ok(core::iter::once(SocketAddr::new(
            IpAddr::V4(self.0),
            self.1,
        )))
    }
}

/// IO error type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoError {
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    ConnectionAborted,
    NotConnected,
    AddrInUse,
    AddrNotAvailable,
    BrokenPipe,
    AlreadyExists,
    WouldBlock,
    InvalidInput,
    InvalidData,
    TimedOut,
    WriteZero,
    Interrupted,
    UnexpectedEof,
    Other,
}
