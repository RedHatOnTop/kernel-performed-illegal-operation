//! DNS resolver implementation.
//!
//! This module provides DNS resolution capabilities for the network stack.

use crate::{IpAddress, NetworkError};

/// DNS record types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum RecordType {
    /// IPv4 address record.
    A = 1,
    /// IPv6 address record.
    AAAA = 28,
    /// Canonical name record.
    CNAME = 5,
    /// Mail exchange record.
    MX = 15,
    /// Name server record.
    NS = 2,
    /// Pointer record.
    PTR = 12,
    /// Start of authority record.
    SOA = 6,
    /// Service record.
    SRV = 33,
    /// Text record.
    TXT = 16,
}

/// DNS query class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum QueryClass {
    /// Internet class.
    IN = 1,
    /// Chaos class.
    CH = 3,
    /// Hesiod class.
    HS = 4,
    /// Any class.
    ANY = 255,
}

/// DNS response codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResponseCode {
    /// No error.
    NoError = 0,
    /// Format error.
    FormErr = 1,
    /// Server failure.
    ServFail = 2,
    /// Name error (NXDOMAIN).
    NXDomain = 3,
    /// Not implemented.
    NotImp = 4,
    /// Refused.
    Refused = 5,
}

/// DNS header structure.
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct DnsHeader {
    /// Transaction ID.
    pub id: u16,
    /// Flags.
    pub flags: u16,
    /// Number of questions.
    pub qdcount: u16,
    /// Number of answers.
    pub ancount: u16,
    /// Number of authority records.
    pub nscount: u16,
    /// Number of additional records.
    pub arcount: u16,
}

impl DnsHeader {
    /// DNS header size in bytes.
    pub const SIZE: usize = 12;

    /// Create a new query header.
    pub fn new_query(id: u16) -> Self {
        DnsHeader {
            id,
            flags: 0x0100, // Standard query with recursion desired
            qdcount: 1,
            ancount: 0,
            nscount: 0,
            arcount: 0,
        }
    }

    /// Check if this is a response.
    pub fn is_response(&self) -> bool {
        (self.flags & 0x8000) != 0
    }

    /// Get the response code.
    pub fn response_code(&self) -> ResponseCode {
        match self.flags & 0x000F {
            0 => ResponseCode::NoError,
            1 => ResponseCode::FormErr,
            2 => ResponseCode::ServFail,
            3 => ResponseCode::NXDomain,
            4 => ResponseCode::NotImp,
            _ => ResponseCode::Refused,
        }
    }
}

/// DNS question structure.
#[derive(Debug, Clone)]
pub struct DnsQuestion {
    /// Query name.
    pub name: [u8; 256],
    /// Name length.
    pub name_len: usize,
    /// Record type.
    pub qtype: RecordType,
    /// Query class.
    pub qclass: QueryClass,
}

impl DnsQuestion {
    /// Create a new DNS question.
    pub fn new(hostname: &str, qtype: RecordType) -> Self {
        let mut name = [0u8; 256];
        let name_len = Self::encode_name(hostname, &mut name);

        DnsQuestion {
            name,
            name_len,
            qtype,
            qclass: QueryClass::IN,
        }
    }

    /// Encode a hostname into DNS name format.
    fn encode_name(hostname: &str, buffer: &mut [u8]) -> usize {
        let mut offset = 0;

        for label in hostname.split('.') {
            let label_bytes = label.as_bytes();
            if label_bytes.is_empty() || label_bytes.len() > 63 {
                continue;
            }

            buffer[offset] = label_bytes.len() as u8;
            offset += 1;

            for &b in label_bytes {
                buffer[offset] = b;
                offset += 1;
            }
        }

        // Null terminator
        buffer[offset] = 0;
        offset += 1;

        offset
    }

    /// Get the wire format size.
    pub fn wire_size(&self) -> usize {
        self.name_len + 4 // name + qtype (2) + qclass (2)
    }
}

/// DNS resource record.
#[derive(Debug, Clone)]
pub struct DnsRecord {
    /// Record name.
    pub name: [u8; 256],
    /// Name length.
    pub name_len: usize,
    /// Record type.
    pub rtype: RecordType,
    /// Record class.
    pub rclass: u16,
    /// Time to live.
    pub ttl: u32,
    /// Record data.
    pub rdata: [u8; 256],
    /// Record data length.
    pub rdlen: usize,
}

impl DnsRecord {
    /// Parse an A record to get the IPv4 address.
    pub fn as_ipv4(&self) -> Option<[u8; 4]> {
        if self.rtype != RecordType::A || self.rdlen != 4 {
            return None;
        }

        let mut addr = [0u8; 4];
        addr.copy_from_slice(&self.rdata[..4]);
        Some(addr)
    }

    /// Parse an AAAA record to get the IPv6 address.
    pub fn as_ipv6(&self) -> Option<[u8; 16]> {
        if self.rtype != RecordType::AAAA || self.rdlen != 16 {
            return None;
        }

        let mut addr = [0u8; 16];
        addr.copy_from_slice(&self.rdata[..16]);
        Some(addr)
    }
}

/// DNS resolver configuration.
#[derive(Debug, Clone)]
pub struct ResolverConfig {
    /// Primary DNS server.
    pub primary_server: IpAddress,
    /// Secondary DNS server.
    pub secondary_server: Option<IpAddress>,
    /// Query timeout in milliseconds.
    pub timeout_ms: u32,
    /// Number of retries.
    pub retries: u8,
    /// Use TCP for queries larger than 512 bytes.
    pub use_tcp: bool,
}

impl Default for ResolverConfig {
    fn default() -> Self {
        ResolverConfig {
            primary_server: IpAddress::V4(crate::Ipv4Addr([8, 8, 8, 8])), // Google DNS
            secondary_server: Some(IpAddress::V4(crate::Ipv4Addr([8, 8, 4, 4]))),
            timeout_ms: 5000,
            retries: 3,
            use_tcp: false,
        }
    }
}

/// DNS resolver.
pub struct DnsResolver {
    config: ResolverConfig,
    next_id: u16,
}

impl DnsResolver {
    /// Create a new DNS resolver with the given configuration.
    pub fn new(config: ResolverConfig) -> Self {
        DnsResolver { config, next_id: 1 }
    }

    /// Create a resolver with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ResolverConfig::default())
    }

    /// Set the primary DNS server.
    pub fn set_primary_server(&mut self, server: IpAddress) {
        self.config.primary_server = server;
    }

    /// Set the secondary DNS server.
    pub fn set_secondary_server(&mut self, server: Option<IpAddress>) {
        self.config.secondary_server = server;
    }

    /// Get the next transaction ID.
    fn next_transaction_id(&mut self) -> u16 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        if self.next_id == 0 {
            self.next_id = 1;
        }
        id
    }

    /// Build a DNS query packet.
    pub fn build_query(&mut self, hostname: &str, record_type: RecordType) -> ([u8; 512], usize) {
        let mut buffer = [0u8; 512];
        let id = self.next_transaction_id();

        // Build header
        let header = DnsHeader::new_query(id);
        buffer[0..2].copy_from_slice(&header.id.to_be_bytes());
        buffer[2..4].copy_from_slice(&header.flags.to_be_bytes());
        buffer[4..6].copy_from_slice(&header.qdcount.to_be_bytes());
        buffer[6..8].copy_from_slice(&header.ancount.to_be_bytes());
        buffer[8..10].copy_from_slice(&header.nscount.to_be_bytes());
        buffer[10..12].copy_from_slice(&header.arcount.to_be_bytes());

        // Build question
        let question = DnsQuestion::new(hostname, record_type);
        let mut offset = DnsHeader::SIZE;

        // Copy name
        buffer[offset..offset + question.name_len]
            .copy_from_slice(&question.name[..question.name_len]);
        offset += question.name_len;

        // Copy type and class
        buffer[offset..offset + 2].copy_from_slice(&(question.qtype as u16).to_be_bytes());
        offset += 2;
        buffer[offset..offset + 2].copy_from_slice(&(question.qclass as u16).to_be_bytes());
        offset += 2;

        (buffer, offset)
    }

    /// Parse a DNS response.
    pub fn parse_response(&self, data: &[u8]) -> Result<DnsResponse, NetworkError> {
        if data.len() < DnsHeader::SIZE {
            return Err(NetworkError::InvalidPacket);
        }

        // Parse header
        let id = u16::from_be_bytes([data[0], data[1]]);
        let flags = u16::from_be_bytes([data[2], data[3]]);
        let qdcount = u16::from_be_bytes([data[4], data[5]]);
        let ancount = u16::from_be_bytes([data[6], data[7]]);
        let _nscount = u16::from_be_bytes([data[8], data[9]]);
        let _arcount = u16::from_be_bytes([data[10], data[11]]);

        let header = DnsHeader {
            id,
            flags,
            qdcount,
            ancount,
            nscount: _nscount,
            arcount: _arcount,
        };

        if !header.is_response() {
            return Err(NetworkError::InvalidPacket);
        }

        // Check response code
        let rcode = header.response_code();
        if rcode != ResponseCode::NoError {
            return Err(NetworkError::DnsError(alloc::string::String::from(
                "Bad response code",
            )));
        }

        Ok(DnsResponse {
            id: header.id,
            answer_count: ancount,
            answers: [None, None, None, None, None, None, None, None],
        })
    }

    /// Resolve a hostname to an IPv4 address.
    pub fn resolve_v4(&mut self, _hostname: &str) -> Result<[u8; 4], NetworkError> {
        // TODO: Send query and wait for response
        Err(NetworkError::NotImplemented)
    }

    /// Resolve a hostname to an IPv6 address.
    pub fn resolve_v6(&mut self, _hostname: &str) -> Result<[u8; 16], NetworkError> {
        // TODO: Send query and wait for response
        Err(NetworkError::NotImplemented)
    }
}

/// DNS response.
#[derive(Debug)]
pub struct DnsResponse {
    /// Transaction ID.
    pub id: u16,
    /// Number of answers.
    pub answer_count: u16,
    /// Answer records (up to 8).
    pub answers: [Option<IpAddress>; 8],
}

/// DNS cache entry.
#[derive(Debug, Clone, Copy)]
pub struct CacheEntry {
    /// Resolved addresses.
    pub addresses: [Option<IpAddress>; 4],
    /// Number of addresses.
    pub count: usize,
    /// Time to live in seconds.
    pub ttl: u32,
    /// Timestamp when cached.
    pub cached_at: u64,
}

/// DNS cache.
pub struct DnsCache {
    /// Cache entries (hostname hash -> entry).
    entries: [(u64, Option<CacheEntry>); 256],
}

impl DnsCache {
    /// Create a new DNS cache.
    pub fn new() -> Self {
        DnsCache {
            entries: [(0, None); 256],
        }
    }

    /// Simple hash function for hostnames.
    fn hash(hostname: &str) -> u64 {
        let mut hash: u64 = 5381;
        for b in hostname.bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(b as u64);
        }
        hash
    }

    /// Look up an entry in the cache.
    pub fn lookup(&self, hostname: &str, current_time: u64) -> Option<&CacheEntry> {
        let hash = Self::hash(hostname);
        let index = (hash as usize) % self.entries.len();

        if let (h, Some(ref entry)) = &self.entries[index] {
            if *h == hash {
                // Check if entry is still valid
                let age = current_time.saturating_sub(entry.cached_at);
                if age < entry.ttl as u64 {
                    return Some(entry);
                }
            }
        }

        None
    }

    /// Insert an entry into the cache.
    pub fn insert(&mut self, hostname: &str, entry: CacheEntry) {
        let hash = Self::hash(hostname);
        let index = (hash as usize) % self.entries.len();
        self.entries[index] = (hash, Some(entry));
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = (0, None);
        }
    }

    /// Remove expired entries.
    pub fn purge_expired(&mut self, current_time: u64) {
        for entry in &mut self.entries {
            if let (_, Some(ref cache_entry)) = entry {
                let age = current_time.saturating_sub(cache_entry.cached_at);
                if age >= cache_entry.ttl as u64 {
                    *entry = (0, None);
                }
            }
        }
    }
}

impl Default for DnsCache {
    fn default() -> Self {
        Self::new()
    }
}
