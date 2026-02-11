//! UDP Layer
//!
//! Simple datagram protocol over IPv4. Used primarily for DNS queries.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU16, Ordering};
use spin::Mutex;

use super::ipv4;
use super::Ipv4Addr;

// ── UDP header ──────────────────────────────────────────────

/// UDP header size
pub const HEADER_SIZE: usize = 8;

/// A parsed UDP datagram.
#[derive(Debug)]
pub struct UdpPacket<'a> {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub payload: &'a [u8],
}

impl<'a> UdpPacket<'a> {
    /// Parse a UDP datagram from IPv4 payload.
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        if data.len() < HEADER_SIZE {
            return None;
        }

        let src_port = u16::from_be_bytes([data[0], data[1]]);
        let dst_port = u16::from_be_bytes([data[2], data[3]]);
        let length = u16::from_be_bytes([data[4], data[5]]);
        // checksum at [6..8] — skip verification for now

        let payload_len = (length as usize).saturating_sub(HEADER_SIZE);
        if data.len() < HEADER_SIZE + payload_len {
            return None;
        }

        let payload = &data[HEADER_SIZE..HEADER_SIZE + payload_len];

        Some(UdpPacket {
            src_port,
            dst_port,
            length,
            payload,
        })
    }
}

// ── Socket receive queue ────────────────────────────────────

/// Received datagram with source address.
#[derive(Debug, Clone)]
pub struct ReceivedDatagram {
    pub src_ip: Ipv4Addr,
    pub src_port: u16,
    pub data: Vec<u8>,
}

struct UdpSocketTable {
    /// port -> receive queue
    sockets: BTreeMap<u16, VecDeque<ReceivedDatagram>>,
}

static SOCKETS: Mutex<Option<UdpSocketTable>> = Mutex::new(None);

/// Ephemeral port counter.
static NEXT_EPHEMERAL: AtomicU16 = AtomicU16::new(49152);

fn with_sockets<F, R>(f: F) -> R
where
    F: FnOnce(&mut UdpSocketTable) -> R,
{
    let mut guard = SOCKETS.lock();
    let table = guard.as_mut().expect("UDP not initialised");
    f(table)
}

/// Initialise the UDP subsystem.
pub fn init() {
    *SOCKETS.lock() = Some(UdpSocketTable {
        sockets: BTreeMap::new(),
    });
}

// ── Public API ──────────────────────────────────────────────

/// Bind a socket to a local port. Returns the port number.
pub fn bind(port: u16) -> u16 {
    let port = if port == 0 {
        NEXT_EPHEMERAL.fetch_add(1, Ordering::Relaxed)
    } else {
        port
    };
    with_sockets(|t| {
        t.sockets.entry(port).or_default();
    });
    port
}

/// Unbind a socket.
pub fn unbind(port: u16) {
    with_sockets(|t| {
        t.sockets.remove(&port);
    });
}

/// Build a UDP datagram wrapped in an IPv4 packet (no Ethernet yet).
///
/// Returns the raw IPv4 packet bytes.
pub fn build_datagram(src_port: u16, dst_ip: Ipv4Addr, dst_port: u16, payload: &[u8]) -> Vec<u8> {
    let udp_len = (HEADER_SIZE + payload.len()) as u16;
    let cfg = ipv4::config();

    // Build UDP header
    let mut udp = Vec::with_capacity(udp_len as usize);
    udp.push((src_port >> 8) as u8);
    udp.push(src_port as u8);
    udp.push((dst_port >> 8) as u8);
    udp.push(dst_port as u8);
    udp.push((udp_len >> 8) as u8);
    udp.push(udp_len as u8);
    // Checksum placeholder
    udp.push(0x00);
    udp.push(0x00);
    // Payload
    udp.extend_from_slice(payload);

    // Compute UDP checksum with pseudo-header
    let phdr = ipv4::pseudo_header_checksum(cfg.ip, dst_ip, ipv4::PROTO_UDP, udp_len);
    let cksum = checksum_with_pseudo(&udp, phdr);
    udp[6] = (cksum >> 8) as u8;
    udp[7] = cksum as u8;

    // Wrap in IPv4
    ipv4::build_packet(dst_ip, ipv4::PROTO_UDP, &udp)
}

/// Send a UDP datagram out as a full Ethernet frame.
///
/// Returns `None` if ARP resolution is pending.
pub fn send(src_port: u16, dst_ip: Ipv4Addr, dst_port: u16, payload: &[u8]) -> Option<Vec<u8>> {
    ipv4::send_packet(
        dst_ip,
        ipv4::PROTO_UDP,
        &build_udp_segment(src_port, dst_ip, dst_port, payload),
    )
}

/// Process an incoming UDP datagram (called from IPv4 dispatch).
pub fn process_incoming(src_ip: Ipv4Addr, data: &[u8]) {
    if let Some(pkt) = UdpPacket::parse(data) {
        with_sockets(|t| {
            if let Some(queue) = t.sockets.get_mut(&pkt.dst_port) {
                queue.push_back(ReceivedDatagram {
                    src_ip,
                    src_port: pkt.src_port,
                    data: Vec::from(pkt.payload),
                });
                // Keep queue bounded
                while queue.len() > 64 {
                    queue.pop_front();
                }
            }
        });
    }
}

/// Receive a datagram from a bound port. Returns None if nothing available.
pub fn recv(port: u16) -> Option<ReceivedDatagram> {
    with_sockets(|t| t.sockets.get_mut(&port).and_then(|q| q.pop_front()))
}

/// Check if there's data available on a port.
pub fn has_data(port: u16) -> bool {
    with_sockets(|t| t.sockets.get(&port).map_or(false, |q| !q.is_empty()))
}

// ── Internal helpers ────────────────────────────────────────

/// Build a raw UDP segment (header + payload) without IPv4 wrapping.
fn build_udp_segment(src_port: u16, dst_ip: Ipv4Addr, dst_port: u16, payload: &[u8]) -> Vec<u8> {
    let udp_len = (HEADER_SIZE + payload.len()) as u16;
    let cfg = ipv4::config();

    let mut udp = Vec::with_capacity(udp_len as usize);
    udp.push((src_port >> 8) as u8);
    udp.push(src_port as u8);
    udp.push((dst_port >> 8) as u8);
    udp.push(dst_port as u8);
    udp.push((udp_len >> 8) as u8);
    udp.push(udp_len as u8);
    udp.push(0x00);
    udp.push(0x00);
    udp.extend_from_slice(payload);

    let phdr = ipv4::pseudo_header_checksum(cfg.ip, dst_ip, ipv4::PROTO_UDP, udp_len);
    let cksum = checksum_with_pseudo(&udp, phdr);
    udp[6] = (cksum >> 8) as u8;
    udp[7] = cksum as u8;

    udp
}

/// Compute checksum over data, adding a pseudo-header sum.
fn checksum_with_pseudo(data: &[u8], pseudo: u32) -> u16 {
    let mut sum = pseudo;
    let mut i = 0;

    while i + 1 < data.len() {
        sum += u16::from_be_bytes([data[i], data[i + 1]]) as u32;
        i += 2;
    }
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }

    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    let result = !(sum as u16);
    // UDP allows 0x0000 checksum to mean "no checksum"; if computed
    // checksum is 0, use 0xFFFF instead.
    if result == 0 {
        0xFFFF
    } else {
        result
    }
}
