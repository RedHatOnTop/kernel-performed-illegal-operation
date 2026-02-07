//! Kernel Network Stack
//!
//! Provides TCP/IP, DNS, and HTTP services.  The stack operates in
//! software—it does not currently drive real hardware NICs but
//! presents a loopback / local-server interface that the browser
//! engine and syscall layer can use.

#![allow(dead_code)]

pub mod tcp;
pub mod dns;
pub mod http;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

// ── IPv4 address ────────────────────────────────────────────

/// IPv4 address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv4Addr(pub [u8; 4]);

impl Ipv4Addr {
    pub const LOCALHOST: Ipv4Addr = Ipv4Addr([127, 0, 0, 1]);
    pub const ANY: Ipv4Addr = Ipv4Addr([0, 0, 0, 0]);
    pub const BROADCAST: Ipv4Addr = Ipv4Addr([255, 255, 255, 255]);

    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Ipv4Addr([a, b, c, d])
    }

    pub fn is_loopback(&self) -> bool {
        self.0[0] == 127
    }

    pub fn is_unspecified(&self) -> bool {
        self.0 == [0, 0, 0, 0]
    }

    pub fn octets(&self) -> [u8; 4] {
        self.0
    }
}

impl fmt::Display for Ipv4Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

// ── Socket address ──────────────────────────────────────────

/// Socket address (IPv4 + port).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketAddr {
    pub ip: Ipv4Addr,
    pub port: u16,
}

impl SocketAddr {
    pub fn new(ip: Ipv4Addr, port: u16) -> Self {
        SocketAddr { ip, port }
    }
}

impl fmt::Display for SocketAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.ip, self.port)
    }
}

// ── Network error ───────────────────────────────────────────

/// Network stack error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetError {
    ConnectionRefused,
    ConnectionReset,
    Timeout,
    DnsNotFound,
    WouldBlock,
    AddressInUse,
    InvalidArgument,
    NotConnected,
    AlreadyConnected,
    NetworkUnreachable,
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetError::ConnectionRefused => write!(f, "Connection refused"),
            NetError::ConnectionReset => write!(f, "Connection reset"),
            NetError::Timeout => write!(f, "Timeout"),
            NetError::DnsNotFound => write!(f, "DNS not found"),
            NetError::WouldBlock => write!(f, "Would block"),
            NetError::AddressInUse => write!(f, "Address in use"),
            NetError::InvalidArgument => write!(f, "Invalid argument"),
            NetError::NotConnected => write!(f, "Not connected"),
            NetError::AlreadyConnected => write!(f, "Already connected"),
            NetError::NetworkUnreachable => write!(f, "Network unreachable"),
        }
    }
}

// ── Network interface info ──────────────────────────────────

/// Information about a network interface (for ifconfig / netstat).
#[derive(Debug, Clone)]
pub struct InterfaceInfo {
    pub name: String,
    pub ip: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub mac: [u8; 6],
    pub mtu: u16,
    pub up: bool,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

use spin::Mutex;

/// Loopback interface statistics.
struct LoopbackStats {
    rx_packets: u64,
    tx_packets: u64,
    rx_bytes: u64,
    tx_bytes: u64,
}

static LOOPBACK: Mutex<LoopbackStats> = Mutex::new(LoopbackStats {
    rx_packets: 0,
    tx_packets: 0,
    rx_bytes: 0,
    tx_bytes: 0,
});

/// Record a loopback send/recv pair.
pub fn loopback_transfer(bytes: u64) {
    let mut lo = LOOPBACK.lock();
    lo.rx_packets += 1;
    lo.tx_packets += 1;
    lo.rx_bytes += bytes;
    lo.tx_bytes += bytes;
}

/// Get the list of network interfaces (currently just loopback).
pub fn interfaces() -> Vec<InterfaceInfo> {
    let lo = LOOPBACK.lock();
    alloc::vec![InterfaceInfo {
        name: String::from("lo"),
        ip: Ipv4Addr::LOCALHOST,
        netmask: Ipv4Addr::new(255, 0, 0, 0),
        mac: [0; 6],
        mtu: 65535,
        up: true,
        rx_packets: lo.rx_packets,
        tx_packets: lo.tx_packets,
        rx_bytes: lo.rx_bytes,
        tx_bytes: lo.tx_bytes,
    }]
}

/// Initialise the network stack.
pub fn init() {
    dns::init();
    tcp::init();
    http::init();
    crate::serial_println!("[Net] Network stack initialized (loopback only)");
}
