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

/// ICMP echo reply storage for ping.
#[derive(Debug, Clone)]
pub struct IcmpEchoReply {
    pub src: Ipv4Addr,
    pub id: u16,
    pub seq: u16,
    pub ttl: u8,
    pub data_len: usize,
    /// Tick count when the reply was received (for RTT calculation).
    pub rx_tick: u64,
}

/// Pending echo replies dequeued by the ping command.
static ICMP_REPLIES: Mutex<Vec<IcmpEchoReply>> = Mutex::new(Vec::new());

/// Monotonic tick counter (incremented via `tick()`).
static ICMP_TICK: Mutex<u64> = Mutex::new(0);

/// Advance the ICMP tick counter (call from timer tick or poll loop).
pub fn icmp_tick() {
    let mut t = ICMP_TICK.lock();
    *t = t.wrapping_add(1);
}

/// Current tick value.
pub fn icmp_now() -> u64 {
    *ICMP_TICK.lock()
}

/// Send an ICMP Echo Request and return the transmit tick.
///
/// `id` — identifier (e.g. PID or session), `seq` — sequence number.
/// Payload is filled with 56 bytes of pattern data (total 64 + 20 = 84
/// bytes on the wire, matching Linux ping default).
pub fn send_echo_request(dst: Ipv4Addr, id: u16, seq: u16) -> Option<u64> {
    // Build ICMP echo request: type 8, code 0
    let mut payload = Vec::with_capacity(64);
    payload.push(8); // type = echo request
    payload.push(0); // code
    payload.push(0); // checksum placeholder
    payload.push(0);
    // identifier
    payload.push((id >> 8) as u8);
    payload.push(id as u8);
    // sequence number
    payload.push((seq >> 8) as u8);
    payload.push(seq as u8);
    // 56 bytes of data payload
    for i in 0u8..56 {
        payload.push(i);
    }
    // Compute ICMP checksum
    let ck = checksum(&payload);
    payload[2] = (ck >> 8) as u8;
    payload[3] = ck as u8;

    let tx_tick = icmp_now();

    // Build full Ethernet frame and transmit
    if let Some(frame) = send_packet(dst, PROTO_ICMP, &payload) {
        super::transmit_frame(&frame);
        Some(tx_tick)
    } else {
        // ARP not resolved yet — send ARP request and retry
        let arp_frame = arp_request_for(dst);
        super::transmit_frame(&arp_frame);
        // Wait briefly for ARP reply, then retry
        for _ in 0..50 {
            super::poll_rx();
            for _ in 0..20_000 {
                core::hint::spin_loop();
            }
        }
        if let Some(frame) = send_packet(dst, PROTO_ICMP, &payload) {
            super::transmit_frame(&frame);
            Some(icmp_now())
        } else {
            None
        }
    }
}

/// Dequeue one ICMP echo reply matching the given `id` and `seq`.
/// Returns `None` if no matching reply is available yet.
pub fn recv_echo_reply(id: u16, seq: u16) -> Option<IcmpEchoReply> {
    let mut replies = ICMP_REPLIES.lock();
    if let Some(pos) = replies.iter().position(|r| r.id == id && r.seq == seq) {
        Some(replies.remove(pos))
    } else {
        None
    }
}

/// Process an incoming ICMP packet. Returns an echo reply frame if
/// the packet is an echo request.  Echo replies are stored for the
/// `ping` command to consume.
pub fn process_icmp(pkt: &Ipv4Packet) -> Option<Vec<u8>> {
    if pkt.payload.len() < 8 {
        return None;
    }

    let icmp_type = pkt.payload[0];
    let _icmp_code = pkt.payload[1];

    match icmp_type {
        // Echo reply (type 0) — store for ping consumers
        0 => {
            let id = u16::from_be_bytes([pkt.payload[4], pkt.payload[5]]);
            let seq = u16::from_be_bytes([pkt.payload[6], pkt.payload[7]]);
            let reply = IcmpEchoReply {
                src: pkt.src,
                id,
                seq,
                ttl: pkt.ttl,
                data_len: pkt.payload.len().saturating_sub(8),
                rx_tick: icmp_now(),
            };
            ICMP_REPLIES.lock().push(reply);
            None
        }
        // Echo request (type 8) → echo reply (type 0)
        8 => {
            let mut reply_payload = Vec::from(pkt.payload);
            reply_payload[0] = 0; // type = echo reply
            reply_payload[2] = 0; // clear checksum
            reply_payload[3] = 0;
            let ck = checksum(&reply_payload);
            reply_payload[2] = (ck >> 8) as u8;
            reply_payload[3] = ck as u8;

            send_packet(pkt.src, PROTO_ICMP, &reply_payload)
        }
        _ => None,
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
