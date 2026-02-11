//! ARP — Address Resolution Protocol (RFC 826)
//!
//! Resolves IPv4 addresses to MAC addresses on the local network.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use spin::Mutex;

use super::ethernet;
use super::Ipv4Addr;
use crate::drivers::net::MacAddress;

// ── ARP packet constants ────────────────────────────────────

const HTYPE_ETHERNET: u16 = 1;
const PTYPE_IPV4: u16 = 0x0800;
const HLEN: u8 = 6; // MAC address length
const PLEN: u8 = 4; // IPv4 address length

const OP_REQUEST: u16 = 1;
const OP_REPLY: u16 = 2;

/// ARP packet size (for Ethernet + IPv4)
pub const ARP_PACKET_SIZE: usize = 28;

// ── ARP table entry ─────────────────────────────────────────

#[derive(Debug, Clone)]
struct ArpEntry {
    mac: MacAddress,
    /// Time-to-live in "ticks" (decremented by the network poll loop)
    ttl: u32,
}

/// Default TTL for ARP entries (~5 minutes at 100 ticks/sec)
const DEFAULT_TTL: u32 = 30000;

// ── ARP table ───────────────────────────────────────────────

struct ArpTable {
    entries: BTreeMap<[u8; 4], ArpEntry>,
    /// Pending resolution requests: IP -> list of waiting packets
    pending: BTreeMap<[u8; 4], Vec<Vec<u8>>>,
}

static ARP_TABLE: Mutex<Option<ArpTable>> = Mutex::new(None);

fn with_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut ArpTable) -> R,
{
    let mut guard = ARP_TABLE.lock();
    let table = guard.as_mut().expect("ARP not initialised");
    f(table)
}

/// Initialise the ARP subsystem.
pub fn init() {
    *ARP_TABLE.lock() = Some(ArpTable {
        entries: BTreeMap::new(),
        pending: BTreeMap::new(),
    });
}

// ── Public API ──────────────────────────────────────────────

/// Look up a MAC address for the given IPv4 address.
///
/// Returns `Some(mac)` if cached, `None` if unknown (caller should
/// call `send_request` and retry later).
pub fn lookup(ip: Ipv4Addr) -> Option<MacAddress> {
    with_table(|t| t.entries.get(&ip.0).map(|e| e.mac))
}

/// Insert or update an ARP entry.
pub fn insert(ip: Ipv4Addr, mac: MacAddress) {
    with_table(|t| {
        t.entries.insert(
            ip.0,
            ArpEntry {
                mac,
                ttl: DEFAULT_TTL,
            },
        );
    });
}

/// Build an ARP request packet (Ethernet frame).
///
/// `sender_mac` / `sender_ip` — our addresses.
/// `target_ip` — the IP we want to resolve.
pub fn build_request(sender_mac: MacAddress, sender_ip: Ipv4Addr, target_ip: Ipv4Addr) -> Vec<u8> {
    let arp_payload = build_arp_packet(
        OP_REQUEST,
        sender_mac,
        sender_ip,
        MacAddress::ZERO, // unknown target MAC
        target_ip,
    );
    ethernet::build_frame(
        MacAddress::BROADCAST,
        sender_mac,
        ethernet::ETHERTYPE_ARP,
        &arp_payload,
    )
}

/// Build an ARP reply packet (Ethernet frame).
pub fn build_reply(
    sender_mac: MacAddress,
    sender_ip: Ipv4Addr,
    target_mac: MacAddress,
    target_ip: Ipv4Addr,
) -> Vec<u8> {
    let arp_payload = build_arp_packet(OP_REPLY, sender_mac, sender_ip, target_mac, target_ip);
    ethernet::build_frame(
        target_mac,
        sender_mac,
        ethernet::ETHERTYPE_ARP,
        &arp_payload,
    )
}

/// Process an incoming ARP packet payload (after Ethernet header is stripped).
///
/// Returns `Some(reply_frame)` if we should send an ARP reply.
pub fn process_incoming(payload: &[u8], our_mac: MacAddress, our_ip: Ipv4Addr) -> Option<Vec<u8>> {
    if payload.len() < ARP_PACKET_SIZE {
        return None;
    }

    let htype = u16::from_be_bytes([payload[0], payload[1]]);
    let ptype = u16::from_be_bytes([payload[2], payload[3]]);
    if htype != HTYPE_ETHERNET || ptype != PTYPE_IPV4 {
        return None;
    }

    let op = u16::from_be_bytes([payload[6], payload[7]]);
    let sender_mac = MacAddress::new([
        payload[8],
        payload[9],
        payload[10],
        payload[11],
        payload[12],
        payload[13],
    ]);
    let sender_ip = Ipv4Addr([payload[14], payload[15], payload[16], payload[17]]);
    let target_ip = Ipv4Addr([payload[22], payload[23], payload[24], payload[25]]);

    // Always learn the sender's MAC
    insert(sender_ip, sender_mac);

    match op {
        OP_REQUEST => {
            // If someone is asking for our IP, send a reply
            if target_ip == our_ip {
                Some(build_reply(our_mac, our_ip, sender_mac, sender_ip))
            } else {
                None
            }
        }
        OP_REPLY => {
            // Already inserted above; nothing more to do
            None
        }
        _ => None,
    }
}

// ── Internal helpers ────────────────────────────────────────

fn build_arp_packet(
    op: u16,
    sender_mac: MacAddress,
    sender_ip: Ipv4Addr,
    target_mac: MacAddress,
    target_ip: Ipv4Addr,
) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(ARP_PACKET_SIZE);

    // Hardware type (Ethernet = 1)
    pkt.push((HTYPE_ETHERNET >> 8) as u8);
    pkt.push(HTYPE_ETHERNET as u8);
    // Protocol type (IPv4 = 0x0800)
    pkt.push((PTYPE_IPV4 >> 8) as u8);
    pkt.push(PTYPE_IPV4 as u8);
    // Hardware address length
    pkt.push(HLEN);
    // Protocol address length
    pkt.push(PLEN);
    // Operation
    pkt.push((op >> 8) as u8);
    pkt.push(op as u8);
    // Sender hardware address
    pkt.extend_from_slice(sender_mac.as_bytes());
    // Sender protocol address
    pkt.extend_from_slice(&sender_ip.0);
    // Target hardware address
    pkt.extend_from_slice(target_mac.as_bytes());
    // Target protocol address
    pkt.extend_from_slice(&target_ip.0);

    pkt
}
