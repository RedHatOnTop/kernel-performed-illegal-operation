//! Ethernet Frame Layer
//!
//! Parses and constructs IEEE 802.3 Ethernet II frames.

#![allow(dead_code)]

use crate::drivers::net::MacAddress;
use alloc::vec::Vec;

// ── EtherType constants ─────────────────────────────────────

/// EtherType: IPv4
pub const ETHERTYPE_IPV4: u16 = 0x0800;
/// EtherType: ARP
pub const ETHERTYPE_ARP: u16 = 0x0806;
/// EtherType: IPv6
pub const ETHERTYPE_IPV6: u16 = 0x86DD;

/// Minimum Ethernet frame size (without FCS)
pub const MIN_FRAME_SIZE: usize = 60;
/// Maximum Ethernet payload (MTU)
pub const MAX_PAYLOAD: usize = 1500;
/// Ethernet header size
pub const HEADER_SIZE: usize = 14;

// ── Parsed frame ────────────────────────────────────────────

/// A parsed Ethernet frame (header + payload reference).
#[derive(Debug)]
pub struct EthernetFrame<'a> {
    pub dst: MacAddress,
    pub src: MacAddress,
    pub ethertype: u16,
    pub payload: &'a [u8],
}

impl<'a> EthernetFrame<'a> {
    /// Parse raw bytes into an Ethernet frame.
    ///
    /// Returns `None` if the frame is too short.
    pub fn parse(raw: &'a [u8]) -> Option<Self> {
        if raw.len() < HEADER_SIZE {
            return None;
        }

        let dst = MacAddress::new([raw[0], raw[1], raw[2], raw[3], raw[4], raw[5]]);
        let src = MacAddress::new([raw[6], raw[7], raw[8], raw[9], raw[10], raw[11]]);
        let ethertype = u16::from_be_bytes([raw[12], raw[13]]);
        let payload = &raw[HEADER_SIZE..];

        Some(EthernetFrame {
            dst,
            src,
            ethertype,
            payload,
        })
    }

    /// Check if this frame is addressed to us or broadcast.
    pub fn is_for_us(&self, our_mac: &MacAddress) -> bool {
        self.dst == *our_mac || self.dst.is_broadcast()
    }
}

// ── Frame builder ───────────────────────────────────────────

/// Build a raw Ethernet frame.
///
/// The frame will be padded to the minimum size (60 bytes) if necessary.
pub fn build_frame(dst: MacAddress, src: MacAddress, ethertype: u16, payload: &[u8]) -> Vec<u8> {
    let total = HEADER_SIZE + payload.len();
    let padded = if total < MIN_FRAME_SIZE {
        MIN_FRAME_SIZE
    } else {
        total
    };
    let mut frame = Vec::with_capacity(padded);

    // Destination MAC
    frame.extend_from_slice(dst.as_bytes());
    // Source MAC
    frame.extend_from_slice(src.as_bytes());
    // EtherType
    frame.push((ethertype >> 8) as u8);
    frame.push(ethertype as u8);
    // Payload
    frame.extend_from_slice(payload);
    // Pad to minimum
    while frame.len() < MIN_FRAME_SIZE {
        frame.push(0);
    }

    frame
}
