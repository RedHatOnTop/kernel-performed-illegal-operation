//! IPv4 Layer
//!
//! Parses and constructs IPv4 packets with internet checksum.
//! Handles routing decisions (local subnet vs gateway).

#![allow(dead_code)]

use alloc::vec::Vec;
use spin::Mutex;

use super::Ipv4Addr;
use super::{arp, ethernet};
use crate::drivers::net::MacAddress;

// ── IPv4 header constants ───────────────────────────────────

/// IPv4 header size (no options)
pub const HEADER_SIZE: usize = 20;

/// Protocol numbers
pub const PROTO_ICMP: u8 = 1;
pub const PROTO_TCP: u8 = 6;
pub const PROTO_UDP: u8 = 17;

/// Default TTL
pub const DEFAULT_TTL: u8 = 64;

// ── Network configuration ───────────────────────────────────

/// IP configuration for the interface.
#[derive(Debug, Clone, Copy)]
pub struct IpConfig {
    pub ip: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub dns: Ipv4Addr,
    pub mac: MacAddress,
}

/// QEMU user-mode networking defaults.
impl Default for IpConfig {
    fn default() -> Self {
        IpConfig {
            ip: Ipv4Addr::new(10, 0, 2, 15),
            netmask: Ipv4Addr::new(255, 255, 255, 0),
            gateway: Ipv4Addr::new(10, 0, 2, 2),
            dns: Ipv4Addr::new(10, 0, 2, 3),
            mac: MacAddress::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]),
        }
    }
}

static IP_CONFIG: Mutex<IpConfig> = Mutex::new(IpConfig {
    ip: Ipv4Addr([10, 0, 2, 15]),
    netmask: Ipv4Addr([255, 255, 255, 0]),
    gateway: Ipv4Addr([10, 0, 2, 2]),
    dns: Ipv4Addr([10, 0, 2, 3]),
    mac: MacAddress::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]),
});

static NEXT_ID: Mutex<u16> = Mutex::new(1);

/// Get current IP config.
pub fn config() -> IpConfig {
    *IP_CONFIG.lock()
}

/// Set IP config (e.g. after DHCP or manual configuration).
pub fn set_config(cfg: IpConfig) {
    *IP_CONFIG.lock() = cfg;
}

/// Update MAC address from the NIC driver.
pub fn set_mac(mac: MacAddress) {
    IP_CONFIG.lock().mac = mac;
}

// ── Parsed IPv4 packet ──────────────────────────────────────

/// A parsed IPv4 packet.
#[derive(Debug)]
pub struct Ipv4Packet<'a> {
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub protocol: u8,
    pub ttl: u8,
    pub id: u16,
    pub total_len: u16,
    pub header_len: usize,
    pub payload: &'a [u8],
}

impl<'a> Ipv4Packet<'a> {
    /// Parse raw bytes (after Ethernet header) into an IPv4 packet.
    pub fn parse(raw: &'a [u8]) -> Option<Self> {
        if raw.len() < HEADER_SIZE {
            return None;
        }

        let version = raw[0] >> 4;
        if version != 4 {
            return None;
        }

        let ihl = (raw[0] & 0x0F) as usize;
        let header_len = ihl * 4;
        if header_len < HEADER_SIZE || raw.len() < header_len {
            return None;
        }

        let total_len = u16::from_be_bytes([raw[2], raw[3]]);
        if (total_len as usize) > raw.len() {
            return None;
        }

        // Verify header checksum
        if checksum(&raw[..header_len]) != 0 {
            return None; // Bad checksum
        }

        let id = u16::from_be_bytes([raw[4], raw[5]]);
        let ttl = raw[8];
        let protocol = raw[9];
        let src = Ipv4Addr([raw[12], raw[13], raw[14], raw[15]]);
        let dst = Ipv4Addr([raw[16], raw[17], raw[18], raw[19]]);
        let payload = &raw[header_len..total_len as usize];

        Some(Ipv4Packet {
            src,
            dst,
            protocol,
            ttl,
            id,
            total_len,
            header_len,
            payload,
        })
    }
}

// ── Packet construction ─────────────────────────────────────

/// Build a raw IPv4 packet (header + payload).
pub fn build_packet(dst: Ipv4Addr, protocol: u8, payload: &[u8]) -> Vec<u8> {
    let total_len = (HEADER_SIZE + payload.len()) as u16;
    let id = {
        let mut g = NEXT_ID.lock();
        let id = *g;
        *g = g.wrapping_add(1);
        id
    };

    let cfg = config();
    let mut pkt = Vec::with_capacity(total_len as usize);

    // Version (4) + IHL (5 = 20 bytes)
    pkt.push(0x45);
    // DSCP / ECN
    pkt.push(0x00);
    // Total length
    pkt.push((total_len >> 8) as u8);
    pkt.push(total_len as u8);
    // Identification
    pkt.push((id >> 8) as u8);
    pkt.push(id as u8);
    // Flags (Don't Fragment) + Fragment offset
    pkt.push(0x40); // DF set
    pkt.push(0x00);
    // TTL
    pkt.push(DEFAULT_TTL);
    // Protocol
    pkt.push(protocol);
    // Header checksum (placeholder, filled below)
    pkt.push(0x00);
    pkt.push(0x00);
    // Source IP
    pkt.extend_from_slice(&cfg.ip.0);
    // Destination IP
    pkt.extend_from_slice(&dst.0);

    // Compute and fill header checksum
    let cksum = checksum(&pkt[..HEADER_SIZE]);
    pkt[10] = (cksum >> 8) as u8;
    pkt[11] = cksum as u8;

    // Payload
    pkt.extend_from_slice(payload);

    pkt
}

/// Wrap an IPv4 packet into a full Ethernet frame, resolving the
/// next-hop MAC via ARP.
///
/// Returns `None` if the MAC is not yet known (caller should send
/// an ARP request and retry).
pub fn send_packet(dst: Ipv4Addr, protocol: u8, payload: &[u8]) -> Option<Vec<u8>> {
    let cfg = config();
    let ip_pkt = build_packet(dst, protocol, payload);

    // Determine next-hop IP (gateway if not on local subnet)
    let next_hop = if is_local(dst, &cfg) {
        dst
    } else {
        cfg.gateway
    };

    // Resolve MAC via ARP
    let dst_mac = arp::lookup(next_hop)?;

    Some(ethernet::build_frame(
        dst_mac,
        cfg.mac,
        ethernet::ETHERTYPE_IPV4,
        &ip_pkt,
    ))
}

/// Build an ARP request frame for the given destination IP.
pub fn arp_request_for(dst: Ipv4Addr) -> Vec<u8> {
    let cfg = config();
    let next_hop = if is_local(dst, &cfg) {
        dst
    } else {
        cfg.gateway
    };
    arp::build_request(cfg.mac, cfg.ip, next_hop)
}

// ── ICMP ────────────────────────────────────────────────────

/// Process an incoming ICMP packet. Returns an echo reply frame if
/// the packet is an echo request.
pub fn process_icmp(pkt: &Ipv4Packet) -> Option<Vec<u8>> {
    if pkt.payload.len() < 8 {
        return None;
    }

    let icmp_type = pkt.payload[0];
    let _icmp_code = pkt.payload[1];

    // Echo request (type 8) → echo reply (type 0)
    if icmp_type == 8 {
        let mut reply_payload = Vec::from(pkt.payload);
        reply_payload[0] = 0; // type = echo reply
        reply_payload[2] = 0; // clear checksum
        reply_payload[3] = 0;
        let ck = checksum(&reply_payload);
        reply_payload[2] = (ck >> 8) as u8;
        reply_payload[3] = ck as u8;

        send_packet(pkt.src, PROTO_ICMP, &reply_payload)
    } else {
        None
    }
}

// ── Helpers ─────────────────────────────────────────────────

/// Check if an IP is on our local subnet.
fn is_local(ip: Ipv4Addr, cfg: &IpConfig) -> bool {
    for i in 0..4 {
        if (ip.0[i] & cfg.netmask.0[i]) != (cfg.ip.0[i] & cfg.netmask.0[i]) {
            return false;
        }
    }
    true
}

/// Internet checksum (RFC 1071).
///
/// Also used for ICMP, TCP, and UDP pseudo-header checksums.
pub fn checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;

    while i + 1 < data.len() {
        sum += u16::from_be_bytes([data[i], data[i + 1]]) as u32;
        i += 2;
    }

    // Handle odd byte
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }

    // Fold carry
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !(sum as u16)
}

/// Compute TCP/UDP pseudo-header checksum contribution.
pub fn pseudo_header_checksum(src: Ipv4Addr, dst: Ipv4Addr, protocol: u8, length: u16) -> u32 {
    let mut sum: u32 = 0;
    // Source IP
    sum += u16::from_be_bytes([src.0[0], src.0[1]]) as u32;
    sum += u16::from_be_bytes([src.0[2], src.0[3]]) as u32;
    // Destination IP
    sum += u16::from_be_bytes([dst.0[0], dst.0[1]]) as u32;
    sum += u16::from_be_bytes([dst.0[2], dst.0[3]]) as u32;
    // Protocol
    sum += protocol as u32;
    // Length
    sum += length as u32;
    sum
}
