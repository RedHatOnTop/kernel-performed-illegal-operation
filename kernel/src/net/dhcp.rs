//! DHCP Client — RFC 2131
//!
//! Automatic IP configuration via DHCP DISCOVER → OFFER → REQUEST → ACK.
//! Works with QEMU's built-in DHCP server (slirp user-mode networking).

#![allow(dead_code)]

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use super::ethernet;
use super::ipv4;
use super::Ipv4Addr;
use crate::drivers::net::{MacAddress, NETWORK_MANAGER};

// ── DHCP constants ──────────────────────────────────────────

const DHCP_SERVER_PORT: u16 = 67;
const DHCP_CLIENT_PORT: u16 = 68;

/// DHCP message types
const DHCP_DISCOVER: u8 = 1;
const DHCP_OFFER: u8 = 2;
const DHCP_REQUEST: u8 = 3;
const DHCP_ACK: u8 = 5;
const DHCP_NAK: u8 = 6;

/// BOOTP op codes
const BOOTREQUEST: u8 = 1;
const BOOTREPLY: u8 = 2;

/// Hardware type: Ethernet
const HTYPE_ETHERNET: u8 = 1;
/// Hardware address length
const HLEN_ETHERNET: u8 = 6;

/// DHCP magic cookie
const MAGIC_COOKIE: [u8; 4] = [99, 130, 83, 99];

/// DHCP option codes
const OPT_SUBNET_MASK: u8 = 1;
const OPT_ROUTER: u8 = 3;
const OPT_DNS: u8 = 6;
const OPT_HOSTNAME: u8 = 12;
const OPT_REQUESTED_IP: u8 = 50;
const OPT_LEASE_TIME: u8 = 51;
const OPT_MSG_TYPE: u8 = 53;
const OPT_SERVER_ID: u8 = 54;
const OPT_PARAM_LIST: u8 = 55;
const OPT_END: u8 = 255;

// ── DHCP lease result ───────────────────────────────────────

/// Result of a successful DHCP handshake.
#[derive(Debug, Clone)]
pub struct DhcpLease {
    pub ip: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub dns: Ipv4Addr,
    pub server_id: Ipv4Addr,
    pub lease_time: u32,
}

// ── Public API ──────────────────────────────────────────────

/// Perform DHCP discovery and apply the lease to the IP config.
///
/// Returns `Ok(lease)` on success, `Err(msg)` on failure.
pub fn discover_and_apply() -> Result<DhcpLease, String> {
    let mac = get_nic_mac();

    // Generate a transaction ID
    let xid: u32 = 0x4B50_1001; // "KP" + unique

    // Step 1: DHCP DISCOVER
    crate::serial_println!("[DHCP] Sending DISCOVER...");
    let discover = build_dhcp_packet(DHCP_DISCOVER, xid, mac, None, None);
    let frame = build_dhcp_frame(mac, &discover);
    transmit_raw(&frame);

    // Step 2: Wait for OFFER
    let offer = wait_for_dhcp_reply(xid, DHCP_OFFER, 300)?;
    crate::serial_println!(
        "[DHCP] Got OFFER: ip={}, gw={}, dns={}, mask={}",
        offer.ip,
        offer.gateway,
        offer.dns,
        offer.netmask
    );

    // Step 3: DHCP REQUEST
    crate::serial_println!("[DHCP] Sending REQUEST for {}...", offer.ip);
    let request = build_dhcp_packet(
        DHCP_REQUEST,
        xid,
        mac,
        Some(offer.ip),
        Some(offer.server_id),
    );
    let frame = build_dhcp_frame(mac, &request);
    transmit_raw(&frame);

    // Step 4: Wait for ACK
    let lease = wait_for_dhcp_reply(xid, DHCP_ACK, 300)?;
    crate::serial_println!(
        "[DHCP] Got ACK: ip={}, lease={}s",
        lease.ip,
        lease.lease_time
    );

    // Apply to IP config
    ipv4::set_config(ipv4::IpConfig {
        ip: lease.ip,
        netmask: lease.netmask,
        gateway: lease.gateway,
        dns: lease.dns,
        mac,
    });

    crate::serial_println!("[DHCP] IP configuration applied");
    Ok(lease)
}

// ── Packet construction ─────────────────────────────────────

/// Build a DHCP message (BOOTP + options).
fn build_dhcp_packet(
    msg_type: u8,
    xid: u32,
    mac: MacAddress,
    requested_ip: Option<Ipv4Addr>,
    server_id: Option<Ipv4Addr>,
) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(548);

    // BOOTP header (236 bytes)
    pkt.push(BOOTREQUEST); // op
    pkt.push(HTYPE_ETHERNET); // htype
    pkt.push(HLEN_ETHERNET); // hlen
    pkt.push(0); // hops

    // Transaction ID
    pkt.extend_from_slice(&xid.to_be_bytes());

    // secs, flags (broadcast)
    pkt.extend_from_slice(&[0, 0]); // secs
    pkt.extend_from_slice(&[0x80, 0x00]); // flags: broadcast

    // ciaddr (client IP - 0 for DISCOVER/REQUEST)
    pkt.extend_from_slice(&[0, 0, 0, 0]);
    // yiaddr (your IP - filled by server)
    pkt.extend_from_slice(&[0, 0, 0, 0]);
    // siaddr (server IP)
    pkt.extend_from_slice(&[0, 0, 0, 0]);
    // giaddr (gateway IP)
    pkt.extend_from_slice(&[0, 0, 0, 0]);

    // chaddr (client hardware address, 16 bytes)
    let mac_bytes = mac.as_bytes();
    pkt.extend_from_slice(mac_bytes);
    pkt.extend_from_slice(&[0; 10]); // padding to 16 bytes

    // sname (64 bytes)
    pkt.extend_from_slice(&[0; 64]);
    // file (128 bytes)
    pkt.extend_from_slice(&[0; 128]);

    // Magic cookie
    pkt.extend_from_slice(&MAGIC_COOKIE);

    // Options
    // Message Type
    pkt.push(OPT_MSG_TYPE);
    pkt.push(1);
    pkt.push(msg_type);

    // Hostname
    let hostname = b"kpio";
    pkt.push(OPT_HOSTNAME);
    pkt.push(hostname.len() as u8);
    pkt.extend_from_slice(hostname);

    // Parameter Request List
    pkt.push(OPT_PARAM_LIST);
    pkt.push(3);
    pkt.push(OPT_SUBNET_MASK);
    pkt.push(OPT_ROUTER);
    pkt.push(OPT_DNS);

    // Requested IP (for REQUEST)
    if let Some(ip) = requested_ip {
        pkt.push(OPT_REQUESTED_IP);
        pkt.push(4);
        pkt.extend_from_slice(&ip.0);
    }

    // Server Identifier (for REQUEST)
    if let Some(sid) = server_id {
        pkt.push(OPT_SERVER_ID);
        pkt.push(4);
        pkt.extend_from_slice(&sid.0);
    }

    // End
    pkt.push(OPT_END);

    // Pad to minimum 300 bytes (BOOTP minimum)
    while pkt.len() < 300 {
        pkt.push(0);
    }

    pkt
}

/// Wrap a DHCP payload in UDP + IPv4 + Ethernet with broadcast addresses.
fn build_dhcp_frame(src_mac: MacAddress, dhcp_payload: &[u8]) -> Vec<u8> {
    // UDP header
    let udp_len = (8 + dhcp_payload.len()) as u16;
    let mut udp = Vec::with_capacity(udp_len as usize);
    // Source port: 68 (client)
    udp.push(0);
    udp.push(DHCP_CLIENT_PORT as u8);
    // Dest port: 67 (server)
    udp.push(0);
    udp.push(DHCP_SERVER_PORT as u8);
    // Length
    udp.push((udp_len >> 8) as u8);
    udp.push(udp_len as u8);
    // Checksum (0 = disabled for DHCP)
    udp.push(0);
    udp.push(0);
    // Payload
    udp.extend_from_slice(dhcp_payload);

    // IPv4 header
    let src_ip = Ipv4Addr::ANY; // 0.0.0.0
    let dst_ip = Ipv4Addr::BROADCAST; // 255.255.255.255
    let total_len = (20 + udp.len()) as u16;

    let mut ipv4_pkt = Vec::with_capacity(total_len as usize);
    ipv4_pkt.push(0x45); // version + IHL
    ipv4_pkt.push(0x00); // DSCP
    ipv4_pkt.push((total_len >> 8) as u8);
    ipv4_pkt.push(total_len as u8);
    ipv4_pkt.extend_from_slice(&[0x00, 0x00]); // ID
    ipv4_pkt.extend_from_slice(&[0x00, 0x00]); // flags + frag
    ipv4_pkt.push(128); // TTL
    ipv4_pkt.push(17); // protocol = UDP
    ipv4_pkt.extend_from_slice(&[0x00, 0x00]); // checksum placeholder
    ipv4_pkt.extend_from_slice(&src_ip.0);
    ipv4_pkt.extend_from_slice(&dst_ip.0);

    // Compute IPv4 checksum
    let cksum = ipv4::checksum(&ipv4_pkt[..20]);
    ipv4_pkt[10] = (cksum >> 8) as u8;
    ipv4_pkt[11] = cksum as u8;

    // Append UDP
    ipv4_pkt.extend_from_slice(&udp);

    // Ethernet frame
    ethernet::build_frame(
        MacAddress::BROADCAST,
        src_mac,
        ethernet::ETHERTYPE_IPV4,
        &ipv4_pkt,
    )
}

// ── Response parsing ────────────────────────────────────────

/// Wait for a DHCP reply of the given `msg_type` matching `xid`.
fn wait_for_dhcp_reply(xid: u32, expected_type: u8, max_iters: usize) -> Result<DhcpLease, String> {
    // Bind UDP port 68 temporarily for DHCP client
    super::udp::bind(DHCP_CLIENT_PORT);

    for _ in 0..max_iters {
        super::poll_rx();

        if let Some(dgram) = super::udp::recv(DHCP_CLIENT_PORT) {
            if let Some(lease) = parse_dhcp_reply(&dgram.data, xid, expected_type) {
                super::udp::unbind(DHCP_CLIENT_PORT);
                return Ok(lease);
            }
        }

        for _ in 0..100_000 {
            core::hint::spin_loop();
        }
    }

    super::udp::unbind(DHCP_CLIENT_PORT);
    Err(format!(
        "DHCP timeout waiting for message type {}",
        expected_type
    ))
}

/// Parse a DHCP reply (OFFER or ACK).
fn parse_dhcp_reply(data: &[u8], expected_xid: u32, expected_type: u8) -> Option<DhcpLease> {
    if data.len() < 240 {
        return None;
    }

    // Verify op
    if data[0] != BOOTREPLY {
        return None;
    }

    // Verify XID
    let xid = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    if xid != expected_xid {
        return None;
    }

    // yiaddr (your IP)
    let offered_ip = Ipv4Addr([data[16], data[17], data[18], data[19]]);

    // Parse options (after magic cookie at offset 236)
    if data.len() < 240 || data[236..240] != MAGIC_COOKIE {
        return None;
    }

    let mut msg_type: Option<u8> = None;
    let mut netmask = Ipv4Addr::new(255, 255, 255, 0);
    let mut gateway = Ipv4Addr::ANY;
    let mut dns = Ipv4Addr::ANY;
    let mut server_id = Ipv4Addr::ANY;
    let mut lease_time: u32 = 3600;

    let mut pos = 240;
    while pos < data.len() {
        let opt = data[pos];
        if opt == OPT_END {
            break;
        }
        if opt == 0 {
            // Padding
            pos += 1;
            continue;
        }
        if pos + 1 >= data.len() {
            break;
        }
        let len = data[pos + 1] as usize;
        let val_start = pos + 2;
        let val_end = val_start + len;
        if val_end > data.len() {
            break;
        }
        let val = &data[val_start..val_end];

        match opt {
            OPT_MSG_TYPE if len >= 1 => {
                msg_type = Some(val[0]);
            }
            OPT_SUBNET_MASK if len >= 4 => {
                netmask = Ipv4Addr([val[0], val[1], val[2], val[3]]);
            }
            OPT_ROUTER if len >= 4 => {
                gateway = Ipv4Addr([val[0], val[1], val[2], val[3]]);
            }
            OPT_DNS if len >= 4 => {
                dns = Ipv4Addr([val[0], val[1], val[2], val[3]]);
            }
            OPT_SERVER_ID if len >= 4 => {
                server_id = Ipv4Addr([val[0], val[1], val[2], val[3]]);
            }
            OPT_LEASE_TIME if len >= 4 => {
                lease_time = u32::from_be_bytes([val[0], val[1], val[2], val[3]]);
            }
            _ => {}
        }

        pos = val_end;
    }

    // Verify message type
    if msg_type != Some(expected_type) {
        return None;
    }

    Some(DhcpLease {
        ip: offered_ip,
        netmask,
        gateway,
        dns,
        server_id,
        lease_time,
    })
}

// ── Helpers ─────────────────────────────────────────────────

/// Get the MAC address of the first NIC.
fn get_nic_mac() -> MacAddress {
    let mgr = NETWORK_MANAGER.lock();
    let names = mgr.device_names();
    if let Some(name) = names.first() {
        if let Some(dev) = mgr.device(name) {
            return dev.mac_address();
        }
    }
    // Fallback to default config MAC
    ipv4::config().mac
}

/// Transmit a raw frame via the first NIC (bypasses net::transmit_frame
/// which requires the manager lock — we may already hold it).
fn transmit_raw(frame: &[u8]) {
    super::transmit_frame(frame);
}
