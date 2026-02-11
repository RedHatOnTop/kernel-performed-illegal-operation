//! DNS Resolver — RFC 1035 Wire Protocol
//!
//! Provides hostname → IPv4 address resolution with wire-format DNS
//! queries sent over UDP. Falls back to a built-in host table for
//! known names.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU16, Ordering};
use spin::Mutex;

use super::ipv4;
use super::udp;
use super::Ipv4Addr;

// ── DNS constants ───────────────────────────────────────────

const DNS_PORT: u16 = 53;
const DNS_HEADER_SIZE: usize = 12;

/// DNS record types.
const TYPE_A: u16 = 1; // IPv4 address
const TYPE_CNAME: u16 = 5; // Canonical name
const CLASS_IN: u16 = 1; // Internet

/// Transaction ID counter.
static NEXT_TXID: AtomicU16 = AtomicU16::new(1);

// ── DNS entry ───────────────────────────────────────────────

/// Cached DNS record.
#[derive(Debug, Clone)]
pub struct DnsEntry {
    pub hostname: String,
    pub addresses: Vec<Ipv4Addr>,
    pub ttl: u32,
}

// ── Resolver ────────────────────────────────────────────────

struct DnsResolver {
    /// Built-in host table (always authoritative).
    hosts: BTreeMap<String, Vec<Ipv4Addr>>,
    /// Dynamic cache (from queries).
    cache: BTreeMap<String, DnsEntry>,
    /// UDP port bound for DNS queries.
    local_port: u16,
}

static RESOLVER: Mutex<Option<DnsResolver>> = Mutex::new(None);

fn with_resolver<F, R>(f: F) -> R
where
    F: FnOnce(&mut DnsResolver) -> R,
{
    let mut guard = RESOLVER.lock();
    let res = guard.as_mut().expect("DNS resolver not initialised");
    f(res)
}

/// Initialise the DNS resolver with built-in host entries.
pub fn init() {
    let mut hosts = BTreeMap::new();

    // Standard entries
    hosts.insert(String::from("localhost"), alloc::vec![Ipv4Addr::LOCALHOST]);
    hosts.insert(
        String::from("localhost.localdomain"),
        alloc::vec![Ipv4Addr::LOCALHOST],
    );
    hosts.insert(String::from("kpio.local"), alloc::vec![Ipv4Addr::LOCALHOST]);
    hosts.insert(String::from("kpio.os"), alloc::vec![Ipv4Addr::LOCALHOST]);

    // Bind a UDP port for outgoing DNS queries
    let local_port = udp::bind(0); // ephemeral port

    *RESOLVER.lock() = Some(DnsResolver {
        hosts,
        cache: BTreeMap::new(),
        local_port,
    });
}

// ── Public API ──────────────────────────────────────────────

/// Resolve a hostname to IPv4 addresses.
///
/// Checks host table, then cache, then sends a wire DNS query.
pub fn resolve(hostname: &str) -> Result<DnsEntry, super::NetError> {
    // 1. Built-in hosts
    let host_result = with_resolver(|r| r.hosts.get(hostname).cloned());
    if let Some(addrs) = host_result {
        return Ok(DnsEntry {
            hostname: String::from(hostname),
            addresses: addrs,
            ttl: 86400,
        });
    }

    // 2. Cache
    let cache_result = with_resolver(|r| r.cache.get(hostname).cloned());
    if let Some(entry) = cache_result {
        return Ok(entry);
    }

    // 3. Wire query
    wire_resolve(hostname)
}

/// Send a DNS query over the wire and parse the response.
fn wire_resolve(hostname: &str) -> Result<DnsEntry, super::NetError> {
    let cfg = ipv4::config();
    let dns_server = cfg.dns;

    let txid = NEXT_TXID.fetch_add(1, Ordering::Relaxed);
    let query = build_query(txid, hostname);

    let local_port = with_resolver(|r| r.local_port);

    // Send the query as a UDP datagram
    // We need to send the raw frame out via the NIC
    if let Some(frame) = udp::send(local_port, dns_server, DNS_PORT, &query) {
        // Transmit the frame
        super::transmit_frame(&frame);
    }

    // Wait for response (poll-based, up to ~2 seconds)
    for _ in 0..200 {
        // Process any incoming packets
        super::poll_rx();

        if let Some(dgram) = udp::recv(local_port) {
            if let Some(entry) = parse_response(&dgram.data, hostname) {
                // Cache the result
                with_resolver(|r| {
                    r.cache.insert(String::from(hostname), entry.clone());
                });
                return Ok(entry);
            }
        }

        // Small delay (~10ms equivalent)
        for _ in 0..100_000 {
            core::hint::spin_loop();
        }
    }

    Err(super::NetError::DnsNotFound)
}

/// Add / update a cache entry.
pub fn cache_insert(hostname: &str, addrs: Vec<Ipv4Addr>, ttl: u32) {
    with_resolver(|r| {
        r.cache.insert(
            String::from(hostname),
            DnsEntry {
                hostname: String::from(hostname),
                addresses: addrs,
                ttl,
            },
        );
    });
}

/// Add a host-table entry (e.g. from /etc/hosts).
pub fn hosts_insert(hostname: &str, addr: Ipv4Addr) {
    with_resolver(|r| {
        r.hosts
            .entry(String::from(hostname))
            .or_default()
            .push(addr);
    });
}

/// Clear the cache (but not the host table).
pub fn flush_cache() {
    with_resolver(|r| r.cache.clear());
}

/// Get all host entries (for debugging).
pub fn host_entries() -> Vec<(String, Vec<Ipv4Addr>)> {
    with_resolver(|r| {
        r.hosts
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    })
}

/// Cache size.
pub fn cache_size() -> usize {
    with_resolver(|r| r.cache.len())
}

// ── DNS packet construction (RFC 1035 §4) ───────────────────

/// Build a DNS query packet for an A record.
fn build_query(txid: u16, hostname: &str) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(64);

    // Header (12 bytes)
    // Transaction ID
    pkt.push((txid >> 8) as u8);
    pkt.push(txid as u8);
    // Flags: standard query, recursion desired
    pkt.push(0x01); // QR=0, OPCODE=0, AA=0, TC=0, RD=1
    pkt.push(0x00); // RA=0, Z=0, RCODE=0
                    // QDCOUNT = 1
    pkt.push(0x00);
    pkt.push(0x01);
    // ANCOUNT = 0
    pkt.push(0x00);
    pkt.push(0x00);
    // NSCOUNT = 0
    pkt.push(0x00);
    pkt.push(0x00);
    // ARCOUNT = 0
    pkt.push(0x00);
    pkt.push(0x00);

    // Question section: encode hostname as DNS name
    for label in hostname.split('.') {
        let len = label.len().min(63);
        pkt.push(len as u8);
        pkt.extend_from_slice(&label.as_bytes()[..len]);
    }
    pkt.push(0x00); // Root label

    // QTYPE = A (1)
    pkt.push(0x00);
    pkt.push(TYPE_A as u8);
    // QCLASS = IN (1)
    pkt.push(0x00);
    pkt.push(CLASS_IN as u8);

    pkt
}

/// Parse a DNS response, extracting A records.
fn parse_response(data: &[u8], hostname: &str) -> Option<DnsEntry> {
    if data.len() < DNS_HEADER_SIZE {
        return None;
    }

    // Check QR bit (should be 1 for response)
    if (data[2] & 0x80) == 0 {
        return None;
    }

    // Check RCODE (should be 0 for no error)
    let rcode = data[3] & 0x0F;
    if rcode != 0 {
        return None;
    }

    let ancount = u16::from_be_bytes([data[6], data[7]]) as usize;
    if ancount == 0 {
        return None;
    }

    // Skip question section
    let mut offset = DNS_HEADER_SIZE;
    let qdcount = u16::from_be_bytes([data[4], data[5]]) as usize;
    for _ in 0..qdcount {
        offset = skip_dns_name(data, offset)?;
        offset += 4; // QTYPE + QCLASS
        if offset > data.len() {
            return None;
        }
    }

    // Parse answer records
    let mut addresses = Vec::new();
    let mut ttl = 300u32; // default

    for _ in 0..ancount {
        if offset >= data.len() {
            break;
        }

        // Skip name (may be compressed)
        offset = skip_dns_name(data, offset)?;

        if offset + 10 > data.len() {
            break;
        }

        let rtype = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let _rclass = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
        let rttl = u32::from_be_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        let rdlength = u16::from_be_bytes([data[offset + 8], data[offset + 9]]) as usize;
        offset += 10;

        if offset + rdlength > data.len() {
            break;
        }

        if rtype == TYPE_A && rdlength == 4 {
            addresses.push(Ipv4Addr([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            ttl = rttl;
        }

        offset += rdlength;
    }

    if addresses.is_empty() {
        None
    } else {
        Some(DnsEntry {
            hostname: String::from(hostname),
            addresses,
            ttl,
        })
    }
}

/// Skip a DNS name at the given offset, handling compression pointers.
/// Returns the new offset after the name.
fn skip_dns_name(data: &[u8], mut offset: usize) -> Option<usize> {
    let mut jumps = 0;
    let mut final_offset = None;

    loop {
        if offset >= data.len() || jumps > 10 {
            return None;
        }

        let len = data[offset];

        if len == 0 {
            // End of name
            offset += 1;
            break;
        }

        if (len & 0xC0) == 0xC0 {
            // Compression pointer
            if offset + 1 >= data.len() {
                return None;
            }
            if final_offset.is_none() {
                final_offset = Some(offset + 2);
            }
            let ptr = ((len as usize & 0x3F) << 8) | data[offset + 1] as usize;
            offset = ptr;
            jumps += 1;
        } else {
            // Regular label
            offset += 1 + len as usize;
        }
    }

    Some(final_offset.unwrap_or(offset))
}
