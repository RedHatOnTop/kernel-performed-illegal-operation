//! KPIO Network Stack
//!
//! This crate provides the networking subsystem for the KPIO operating system.
//! It uses smoltcp for the TCP/IP stack and supports VirtIO-Net and E1000 drivers.
//!
//! # Architecture
//!
//! The network stack is organized into:
//!
//! - `driver`: Network device drivers (VirtIO-Net, E1000)
//! - `interface`: Network interface management
//! - `socket`: Socket API implementation
//! - `tcp`: TCP protocol handling
//! - `udp`: UDP protocol handling
//! - `dns`: DNS resolver
//! - `dhcp`: DHCP client

#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

pub mod driver;
pub mod interface;
pub mod socket;
pub mod tcp;
pub mod udp;
pub mod dns;
pub mod dhcp;
pub mod http;
pub mod websocket;

use alloc::string::String;
use alloc::vec::Vec;

/// Network error types.
#[derive(Debug, Clone)]
pub enum NetworkError {
    /// No network interface available.
    NoInterface,
    /// Interface not found.
    InterfaceNotFound(String),
    /// Connection refused.
    ConnectionRefused,
    /// Connection reset.
    ConnectionReset,
    /// Connection timed out.
    TimedOut,
    /// Address in use.
    AddressInUse,
    /// Address not available.
    AddressNotAvailable,
    /// Network unreachable.
    NetworkUnreachable,
    /// Host unreachable.
    HostUnreachable,
    /// Invalid address.
    InvalidAddress,
    /// Socket not connected.
    NotConnected,
    /// Socket already connected.
    AlreadyConnected,
    /// Operation would block.
    WouldBlock,
    /// Buffer too small.
    BufferTooSmall,
    /// DNS resolution failed.
    DnsError(String),
    /// DHCP error.
    DhcpError(String),
    /// Driver error.
    DriverError(String),
    /// Invalid packet.
    InvalidPacket,
    /// Not implemented.
    NotImplemented,
    /// DHCP NAK received.
    DhcpNak,
}

/// IPv4 address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Addr(pub [u8; 4]);

impl Ipv4Addr {
    /// Unspecified address (0.0.0.0).
    pub const UNSPECIFIED: Ipv4Addr = Ipv4Addr([0, 0, 0, 0]);
    
    /// Loopback address (127.0.0.1).
    pub const LOCALHOST: Ipv4Addr = Ipv4Addr([127, 0, 0, 1]);
    
    /// Broadcast address (255.255.255.255).
    pub const BROADCAST: Ipv4Addr = Ipv4Addr([255, 255, 255, 255]);
    
    /// Create a new IPv4 address.
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Ipv4Addr([a, b, c, d])
    }
    
    /// Get the octets.
    pub fn octets(&self) -> [u8; 4] {
        self.0
    }
    
    /// Check if this is a loopback address.
    pub fn is_loopback(&self) -> bool {
        self.0[0] == 127
    }
    
    /// Check if this is a private address.
    pub fn is_private(&self) -> bool {
        match self.0 {
            [10, ..] => true,
            [172, b, ..] if (16..=31).contains(&b) => true,
            [192, 168, ..] => true,
            _ => false,
        }
    }
}

/// IPv6 address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv6Addr(pub [u8; 16]);

impl Ipv6Addr {
    /// Unspecified address (::).
    pub const UNSPECIFIED: Ipv6Addr = Ipv6Addr([0; 16]);
    
    /// Loopback address (::1).
    pub const LOCALHOST: Ipv6Addr = Ipv6Addr([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    
    /// Get the octets.
    pub fn octets(&self) -> [u8; 16] {
        self.0
    }
}

/// IP address (v4 or v6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpAddr {
    V4(Ipv4Addr),
    V6(Ipv6Addr),
}

impl IpAddr {
    /// Create an IPv4 address from bytes.
    pub const fn from_v4_bytes(bytes: [u8; 4]) -> Self {
        IpAddr::V4(Ipv4Addr(bytes))
    }
}

/// Socket address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketAddr {
    /// IP address.
    pub ip: IpAddr,
    /// Port number.
    pub port: u16,
}

impl SocketAddr {
    /// Create a new socket address.
    pub fn new(ip: IpAddr, port: u16) -> Self {
        SocketAddr { ip, port }
    }
    
    /// Create an IPv4 socket address.
    pub fn v4(a: u8, b: u8, c: u8, d: u8, port: u16) -> Self {
        SocketAddr {
            ip: IpAddr::V4(Ipv4Addr::new(a, b, c, d)),
            port,
        }
    }
}

/// MAC address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddr(pub [u8; 6]);

/// Type alias for backward compatibility.
pub type IpAddress = IpAddr;

/// Type alias for backward compatibility.
pub type MacAddress = MacAddr;

impl MacAddr {
    /// Broadcast address.
    pub const BROADCAST: MacAddr = MacAddr([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    
    /// Create a new MAC address.
    pub const fn new(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8) -> Self {
        MacAddr([a, b, c, d, e, f])
    }
    
    /// Get the bytes.
    pub fn bytes(&self) -> [u8; 6] {
        self.0
    }
}

/// Network interface configuration.
#[derive(Debug, Clone)]
pub struct InterfaceConfig {
    /// Interface name.
    pub name: String,
    /// MAC address.
    pub mac: MacAddr,
    /// IPv4 address.
    pub ipv4: Option<Ipv4Addr>,
    /// IPv4 subnet mask.
    pub netmask: Option<Ipv4Addr>,
    /// IPv4 gateway.
    pub gateway: Option<Ipv4Addr>,
    /// DNS servers.
    pub dns_servers: Vec<Ipv4Addr>,
    /// MTU.
    pub mtu: u16,
}

/// Initialize the network stack.
pub fn init() -> Result<(), NetworkError> {
    driver::init()?;
    interface::init()?;
    Ok(())
}

/// Get all network interfaces.
pub fn interfaces() -> Vec<InterfaceConfig> {
    interface::list_interfaces()
}

// Re-export HTTP types for convenience
pub use http::{HttpClient, HttpRequest, HttpResponse, HttpMethod, HttpError, HttpParser, Url, StatusCode};
