//! DNS Resolver
//!
//! Provides hostname → IPv4 address resolution with a built-in
//! host table and LRU-style cache.  Currently no external DNS
//! queries are made — only pre-configured entries are returned.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use super::Ipv4Addr;

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
    hosts.insert(String::from("localhost.localdomain"), alloc::vec![Ipv4Addr::LOCALHOST]);
    hosts.insert(String::from("kpio.local"), alloc::vec![Ipv4Addr::LOCALHOST]);
    hosts.insert(String::from("kpio.os"), alloc::vec![Ipv4Addr::LOCALHOST]);

    // Common well-known hosts (resolve to loopback in this sandbox)
    hosts.insert(String::from("example.com"), alloc::vec![Ipv4Addr::new(93, 184, 216, 34)]);
    hosts.insert(String::from("www.example.com"), alloc::vec![Ipv4Addr::new(93, 184, 216, 34)]);

    *RESOLVER.lock() = Some(DnsResolver {
        hosts,
        cache: BTreeMap::new(),
    });
}

// ── Public API ──────────────────────────────────────────────

/// Resolve a hostname to IPv4 addresses.
pub fn resolve(hostname: &str) -> Result<DnsEntry, super::NetError> {
    with_resolver(|r| {
        // 1. Built-in hosts
        if let Some(addrs) = r.hosts.get(hostname) {
            return Ok(DnsEntry {
                hostname: String::from(hostname),
                addresses: addrs.clone(),
                ttl: 86400, // 24 h
            });
        }

        // 2. Cache
        if let Some(entry) = r.cache.get(hostname) {
            return Ok(entry.clone());
        }

        // 3. No external DNS — fail
        Err(super::NetError::DnsNotFound)
    })
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
        r.hosts.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    })
}

/// Cache size.
pub fn cache_size() -> usize {
    with_resolver(|r| r.cache.len())
}
