//! Kernel Network Stack
//!
//! Full TCP/IP stack for online browsing over VirtIO-net.
//!
//! Layer overview (bottom → top):
//!   VirtIO-net → Ethernet → ARP → IPv4 → UDP/TCP → DNS → HTTP/TLS

#![allow(dead_code)]

pub mod ethernet;
pub mod arp;
pub mod ipv4;
pub mod udp;
pub mod tcp;
pub mod dns;
pub mod dhcp;
pub mod crypto;
pub mod x509;
pub mod tls;
pub mod tls13;
pub mod http;
pub mod websocket;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::drivers::net::NETWORK_MANAGER;

// ── IPv4 address ────────────────────────────────────────────

/// IPv4 address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    ConnectionNotFound,
    Timeout,
    TimedOut,
    DnsNotFound,
    WouldBlock,
    AddressInUse,
    InvalidArgument,
    NotConnected,
    AlreadyConnected,
    NetworkUnreachable,
    TlsHandshakeFailed,
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetError::ConnectionRefused => write!(f, "Connection refused"),
            NetError::ConnectionReset => write!(f, "Connection reset"),
            NetError::ConnectionNotFound => write!(f, "Connection not found"),
            NetError::Timeout => write!(f, "Timeout"),
            NetError::TimedOut => write!(f, "Timed out"),
            NetError::DnsNotFound => write!(f, "DNS not found"),
            NetError::WouldBlock => write!(f, "Would block"),
            NetError::AddressInUse => write!(f, "Address in use"),
            NetError::InvalidArgument => write!(f, "Invalid argument"),
            NetError::NotConnected => write!(f, "Not connected"),
            NetError::AlreadyConnected => write!(f, "Already connected"),
            NetError::NetworkUnreachable => write!(f, "Network unreachable"),
            NetError::TlsHandshakeFailed => write!(f, "TLS handshake failed"),
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

/// Get the list of network interfaces (loopback + physical NICs).
pub fn interfaces() -> Vec<InterfaceInfo> {
    let mut ifaces = Vec::new();

    // Physical NICs from NETWORK_MANAGER
    let cfg = ipv4::config();
    let mgr = NETWORK_MANAGER.lock();
    for name in mgr.device_names() {
        if let Some(dev) = mgr.device(&name) {
            let stats = dev.stats();
            let link = dev.link_status();
            let mac_addr = dev.mac_address();
            let caps = dev.capabilities();
            ifaces.push(InterfaceInfo {
                name: name.clone(),
                ip: cfg.ip,
                netmask: cfg.netmask,
                mac: *mac_addr.as_bytes(),
                mtu: if caps.mtu > 0 { caps.mtu } else { 1500 },
                up: link.up,
                rx_packets: stats.rx_packets,
                tx_packets: stats.tx_packets,
                rx_bytes: stats.rx_bytes,
                tx_bytes: stats.tx_bytes,
            });
        }
    }
    drop(mgr);

    // Loopback
    let lo = LOOPBACK.lock();
    ifaces.push(InterfaceInfo {
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
    });

    ifaces
}

/// Initialise the network stack.
pub fn init() {
    arp::init();
    udp::init();
    dns::init();
    tcp::init();
    http::init();

    // Pre-populate ARP with the gateway MAC so that the first packet
    // doesn't have to wait for a reply.  QEMU user-mode networking
    // always responds to any MAC so a well-known placeholder works.
    let cfg = ipv4::config();
    arp::insert(cfg.gateway, cfg.mac); // seed gateway (self-MAC trick for QEMU slirp)

    // Try DHCP to get a real IP configuration.
    // Falls back to the hardcoded QEMU defaults if DHCP fails.
    match dhcp::discover_and_apply() {
        Ok(lease) => {
            crate::serial_println!(
                "[Net] DHCP lease acquired: {} (gw {}, dns {})",
                lease.ip, lease.gateway, lease.dns
            );
            // Update ARP with new gateway
            arp::insert(lease.gateway, cfg.mac);
        }
        Err(e) => {
            crate::serial_println!(
                "[Net] DHCP failed ({}), using static config ({})",
                e, cfg.ip
            );
        }
    }

    crate::serial_println!("[Net] Network stack initialized (virtio-net ready)");
}

// ── NIC ↔ Protocol glue ────────────────────────────────────

/// Transmit a raw Ethernet frame via the first available NIC.
pub fn transmit_frame(frame: &[u8]) {
    let mut mgr = NETWORK_MANAGER.lock();
    // Try every registered device; stop after the first success.
    for name in mgr.device_names() {
        if let Some(dev) = mgr.device_mut(&name) {
            if dev.transmit(frame).is_ok() {
                return;
            }
        }
    }
    // No device available — silently drop (loopback-only mode).
}

/// Poll all NICs for received frames and feed them into `process_rx`.
pub fn poll_rx() {
    let mut mgr = NETWORK_MANAGER.lock();
    let names: Vec<String> = mgr.device_names();
    for name in &names {
        if let Some(dev) = mgr.device_mut(name) {
            let mut buf = [0u8; 2048];
            while dev.rx_available() {
                match dev.receive(&mut buf) {
                    Ok(n) if n > 0 => {
                        // Must drop the manager lock before calling process_rx
                        // because the handler may want to transmit.
                        let pkt = buf[..n].to_vec();
                        drop(mgr);
                        process_rx(&pkt);
                        mgr = NETWORK_MANAGER.lock();
                        // Re-lookup the device because we dropped & re-acquired.
                        break; // will loop back via outer while
                    }
                    _ => break,
                }
            }
        }
    }
}

/// Dispatch a received Ethernet frame through the protocol stack.
pub fn process_rx(frame: &[u8]) {
    let eth = match ethernet::EthernetFrame::parse(frame) {
        Some(f) => f,
        None => return,
    };

    let cfg = ipv4::config();

    // Only accept frames addressed to us or broadcast
    if !eth.is_for_us(&cfg.mac) {
        return;
    }

    match eth.ethertype {
        ethernet::ETHERTYPE_ARP => {
            if let Some(reply) = arp::process_incoming(eth.payload, cfg.mac, cfg.ip) {
                transmit_frame(&reply);
            }
        }
        ethernet::ETHERTYPE_IPV4 => {
            if let Some(ip_pkt) = ipv4::Ipv4Packet::parse(eth.payload) {
                match ip_pkt.protocol {
                    ipv4::PROTO_ICMP => {
                        if let Some(reply) = ipv4::process_icmp(&ip_pkt) {
                            transmit_frame(&reply);
                        }
                    }
                    ipv4::PROTO_UDP => {
                        udp::process_incoming(ip_pkt.src, ip_pkt.payload);
                    }
                    ipv4::PROTO_TCP => {
                        tcp::process_incoming(ip_pkt.src, ip_pkt.payload);
                    }
                    _ => {} // drop unknown protocol
                }
            }
        }
        _ => {} // drop unknown ethertype
    }
}
