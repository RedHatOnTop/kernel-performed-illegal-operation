//! DHCP client implementation.
//!
//! This module provides Dynamic Host Configuration Protocol (DHCP) client
//! functionality for automatic network configuration.

use crate::{IpAddress, MacAddress, NetworkError};

/// DHCP message types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DhcpMessageType {
    /// DHCP Discover.
    Discover = 1,
    /// DHCP Offer.
    Offer = 2,
    /// DHCP Request.
    Request = 3,
    /// DHCP Decline.
    Decline = 4,
    /// DHCP Acknowledgment.
    Ack = 5,
    /// DHCP Negative Acknowledgment.
    Nak = 6,
    /// DHCP Release.
    Release = 7,
    /// DHCP Inform.
    Inform = 8,
}

/// DHCP option codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DhcpOption {
    /// Padding.
    Pad = 0,
    /// Subnet mask.
    SubnetMask = 1,
    /// Router (default gateway).
    Router = 3,
    /// DNS server.
    DnsServer = 6,
    /// Domain name.
    DomainName = 15,
    /// Broadcast address.
    BroadcastAddress = 28,
    /// Requested IP address.
    RequestedIp = 50,
    /// IP address lease time.
    LeaseTime = 51,
    /// Message type.
    MessageType = 53,
    /// Server identifier.
    ServerIdentifier = 54,
    /// Parameter request list.
    ParameterRequestList = 55,
    /// Renewal time (T1).
    RenewalTime = 58,
    /// Rebinding time (T2).
    RebindingTime = 59,
    /// Client identifier.
    ClientIdentifier = 61,
    /// End of options.
    End = 255,
}

/// DHCP operation codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DhcpOp {
    /// Boot request.
    Request = 1,
    /// Boot reply.
    Reply = 2,
}

/// DHCP hardware types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HardwareType {
    /// Ethernet.
    Ethernet = 1,
}

/// DHCP packet structure.
#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct DhcpPacket {
    /// Operation code.
    pub op: u8,
    /// Hardware type.
    pub htype: u8,
    /// Hardware address length.
    pub hlen: u8,
    /// Hops.
    pub hops: u8,
    /// Transaction ID.
    pub xid: u32,
    /// Seconds elapsed.
    pub secs: u16,
    /// Flags.
    pub flags: u16,
    /// Client IP address.
    pub ciaddr: [u8; 4],
    /// Your (client) IP address.
    pub yiaddr: [u8; 4],
    /// Server IP address.
    pub siaddr: [u8; 4],
    /// Gateway IP address.
    pub giaddr: [u8; 4],
    /// Client hardware address.
    pub chaddr: [u8; 16],
    /// Server hostname.
    pub sname: [u8; 64],
    /// Boot filename.
    pub file: [u8; 128],
    /// Magic cookie.
    pub magic: [u8; 4],
}

impl DhcpPacket {
    /// DHCP packet header size (without options).
    pub const HEADER_SIZE: usize = 240;

    /// DHCP magic cookie.
    pub const MAGIC_COOKIE: [u8; 4] = [99, 130, 83, 99];

    /// Create a new DHCP discover packet.
    pub fn new_discover(xid: u32, mac: MacAddress) -> Self {
        let mut chaddr = [0u8; 16];
        chaddr[..6].copy_from_slice(&mac.0);

        DhcpPacket {
            op: DhcpOp::Request as u8,
            htype: HardwareType::Ethernet as u8,
            hlen: 6,
            hops: 0,
            xid,
            secs: 0,
            flags: 0x8000, // Broadcast flag
            ciaddr: [0; 4],
            yiaddr: [0; 4],
            siaddr: [0; 4],
            giaddr: [0; 4],
            chaddr,
            sname: [0; 64],
            file: [0; 128],
            magic: Self::MAGIC_COOKIE,
        }
    }

    /// Create a new DHCP request packet.
    pub fn new_request(
        xid: u32,
        mac: MacAddress,
        requested_ip: [u8; 4],
        server_ip: [u8; 4],
    ) -> Self {
        let mut packet = Self::new_discover(xid, mac);
        packet.siaddr = server_ip;
        // Note: requested_ip goes in options, not in packet fields
        let _ = requested_ip; // Used in options
        packet
    }

    /// Serialize the packet to bytes.
    pub fn to_bytes(&self, options: &[u8], buffer: &mut [u8]) -> usize {
        let mut offset = 0;

        buffer[offset] = self.op;
        offset += 1;
        buffer[offset] = self.htype;
        offset += 1;
        buffer[offset] = self.hlen;
        offset += 1;
        buffer[offset] = self.hops;
        offset += 1;
        buffer[offset..offset + 4].copy_from_slice(&self.xid.to_be_bytes());
        offset += 4;
        buffer[offset..offset + 2].copy_from_slice(&self.secs.to_be_bytes());
        offset += 2;
        buffer[offset..offset + 2].copy_from_slice(&self.flags.to_be_bytes());
        offset += 2;
        buffer[offset..offset + 4].copy_from_slice(&self.ciaddr);
        offset += 4;
        buffer[offset..offset + 4].copy_from_slice(&self.yiaddr);
        offset += 4;
        buffer[offset..offset + 4].copy_from_slice(&self.siaddr);
        offset += 4;
        buffer[offset..offset + 4].copy_from_slice(&self.giaddr);
        offset += 4;
        buffer[offset..offset + 16].copy_from_slice(&self.chaddr);
        offset += 16;
        buffer[offset..offset + 64].copy_from_slice(&self.sname);
        offset += 64;
        buffer[offset..offset + 128].copy_from_slice(&self.file);
        offset += 128;
        buffer[offset..offset + 4].copy_from_slice(&self.magic);
        offset += 4;

        // Copy options
        buffer[offset..offset + options.len()].copy_from_slice(options);
        offset += options.len();

        offset
    }

    /// Parse a packet from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, NetworkError> {
        if data.len() < Self::HEADER_SIZE {
            return Err(NetworkError::InvalidPacket);
        }

        let mut chaddr = [0u8; 16];
        chaddr.copy_from_slice(&data[28..44]);

        let mut sname = [0u8; 64];
        sname.copy_from_slice(&data[44..108]);

        let mut file = [0u8; 128];
        file.copy_from_slice(&data[108..236]);

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&data[236..240]);

        if magic != Self::MAGIC_COOKIE {
            return Err(NetworkError::InvalidPacket);
        }

        Ok(DhcpPacket {
            op: data[0],
            htype: data[1],
            hlen: data[2],
            hops: data[3],
            xid: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
            secs: u16::from_be_bytes([data[8], data[9]]),
            flags: u16::from_be_bytes([data[10], data[11]]),
            ciaddr: [data[12], data[13], data[14], data[15]],
            yiaddr: [data[16], data[17], data[18], data[19]],
            siaddr: [data[20], data[21], data[22], data[23]],
            giaddr: [data[24], data[25], data[26], data[27]],
            chaddr,
            sname,
            file,
            magic,
        })
    }
}

/// DHCP lease information.
#[derive(Debug, Clone)]
pub struct DhcpLease {
    /// Assigned IP address.
    pub ip_address: [u8; 4],
    /// Subnet mask.
    pub subnet_mask: [u8; 4],
    /// Default gateway.
    pub gateway: Option<[u8; 4]>,
    /// DNS servers.
    pub dns_servers: [[u8; 4]; 4],
    /// Number of DNS servers.
    pub dns_count: usize,
    /// Domain name.
    pub domain_name: [u8; 64],
    /// Domain name length.
    pub domain_len: usize,
    /// Lease time in seconds.
    pub lease_time: u32,
    /// Renewal time (T1) in seconds.
    pub renewal_time: u32,
    /// Rebinding time (T2) in seconds.
    pub rebind_time: u32,
    /// DHCP server IP.
    pub server_ip: [u8; 4],
    /// Timestamp when lease was acquired.
    pub acquired_at: u64,
}

impl DhcpLease {
    /// Check if the lease is expired.
    pub fn is_expired(&self, current_time: u64) -> bool {
        let elapsed = current_time.saturating_sub(self.acquired_at);
        elapsed >= self.lease_time as u64
    }

    /// Check if renewal is needed.
    pub fn needs_renewal(&self, current_time: u64) -> bool {
        let elapsed = current_time.saturating_sub(self.acquired_at);
        elapsed >= self.renewal_time as u64
    }

    /// Check if rebinding is needed.
    pub fn needs_rebind(&self, current_time: u64) -> bool {
        let elapsed = current_time.saturating_sub(self.acquired_at);
        elapsed >= self.rebind_time as u64
    }
}

/// DHCP client state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DhcpState {
    /// Initial state.
    Init,
    /// Selecting - waiting for offers.
    Selecting,
    /// Requesting - sent request, waiting for ack.
    Requesting,
    /// Bound - have a valid lease.
    Bound,
    /// Renewing - attempting to renew lease.
    Renewing,
    /// Rebinding - attempting to rebind lease.
    Rebinding,
}

/// DHCP client.
pub struct DhcpClient {
    /// Client MAC address.
    mac: MacAddress,
    /// Current state.
    state: DhcpState,
    /// Transaction ID.
    xid: u32,
    /// Current lease.
    lease: Option<DhcpLease>,
    /// Retry count.
    retries: u8,
    /// Maximum retries.
    max_retries: u8,
    /// Timeout in milliseconds.
    timeout_ms: u32,
}

impl DhcpClient {
    /// DHCP client port.
    pub const CLIENT_PORT: u16 = 68;
    /// DHCP server port.
    pub const SERVER_PORT: u16 = 67;

    /// Create a new DHCP client.
    pub fn new(mac: MacAddress) -> Self {
        DhcpClient {
            mac,
            state: DhcpState::Init,
            xid: 0,
            lease: None,
            retries: 0,
            max_retries: 4,
            timeout_ms: 4000,
        }
    }

    /// Get the current state.
    pub fn state(&self) -> DhcpState {
        self.state
    }

    /// Get the current lease.
    pub fn lease(&self) -> Option<&DhcpLease> {
        self.lease.as_ref()
    }

    /// Generate a new transaction ID.
    fn new_xid(&mut self) -> u32 {
        // Simple PRNG based on MAC address and previous XID
        self.xid = self.xid.wrapping_mul(1103515245).wrapping_add(12345);
        self.xid ^=
            u32::from_be_bytes([self.mac.0[2], self.mac.0[3], self.mac.0[4], self.mac.0[5]]);
        self.xid
    }

    /// Build DHCP options.
    fn build_options(
        &self,
        msg_type: DhcpMessageType,
        requested_ip: Option<[u8; 4]>,
    ) -> ([u8; 64], usize) {
        let mut options = [0u8; 64];
        let mut offset = 0;

        // Message type option
        options[offset] = DhcpOption::MessageType as u8;
        offset += 1;
        options[offset] = 1; // Length
        offset += 1;
        options[offset] = msg_type as u8;
        offset += 1;

        // Client identifier
        options[offset] = DhcpOption::ClientIdentifier as u8;
        offset += 1;
        options[offset] = 7; // Length: 1 (type) + 6 (MAC)
        offset += 1;
        options[offset] = HardwareType::Ethernet as u8;
        offset += 1;
        options[offset..offset + 6].copy_from_slice(&self.mac.0);
        offset += 6;

        // Requested IP (for REQUEST)
        if let Some(ip) = requested_ip {
            options[offset] = DhcpOption::RequestedIp as u8;
            offset += 1;
            options[offset] = 4; // Length
            offset += 1;
            options[offset..offset + 4].copy_from_slice(&ip);
            offset += 4;
        }

        // Parameter request list
        options[offset] = DhcpOption::ParameterRequestList as u8;
        offset += 1;
        options[offset] = 4; // Length
        offset += 1;
        options[offset] = DhcpOption::SubnetMask as u8;
        offset += 1;
        options[offset] = DhcpOption::Router as u8;
        offset += 1;
        options[offset] = DhcpOption::DnsServer as u8;
        offset += 1;
        options[offset] = DhcpOption::DomainName as u8;
        offset += 1;

        // End option
        options[offset] = DhcpOption::End as u8;
        offset += 1;

        (options, offset)
    }

    /// Start the DHCP discovery process.
    pub fn discover(&mut self) -> ([u8; 576], usize) {
        self.state = DhcpState::Selecting;
        self.retries = 0;
        let xid = self.new_xid();

        let packet = DhcpPacket::new_discover(xid, self.mac);
        let (options, opt_len) = self.build_options(DhcpMessageType::Discover, None);

        let mut buffer = [0u8; 576];
        let len = packet.to_bytes(&options[..opt_len], &mut buffer);

        (buffer, len)
    }

    /// Handle a DHCP offer.
    pub fn handle_offer(
        &mut self,
        data: &[u8],
    ) -> Result<Option<([u8; 576], usize)>, NetworkError> {
        if self.state != DhcpState::Selecting {
            return Ok(None);
        }

        let packet = DhcpPacket::from_bytes(data)?;

        if packet.op != DhcpOp::Reply as u8 || packet.xid != self.xid {
            return Ok(None);
        }

        // Parse options to find message type
        let options = &data[DhcpPacket::HEADER_SIZE..];
        let msg_type = self.parse_message_type(options)?;

        if msg_type != DhcpMessageType::Offer {
            return Ok(None);
        }

        // Build request
        self.state = DhcpState::Requesting;
        let request = DhcpPacket::new_request(self.xid, self.mac, packet.yiaddr, packet.siaddr);
        let (opts, opt_len) = self.build_options(DhcpMessageType::Request, Some(packet.yiaddr));

        let mut buffer = [0u8; 576];
        let len = request.to_bytes(&opts[..opt_len], &mut buffer);

        Ok(Some((buffer, len)))
    }

    /// Handle a DHCP acknowledgment.
    pub fn handle_ack(&mut self, data: &[u8], current_time: u64) -> Result<bool, NetworkError> {
        if self.state != DhcpState::Requesting
            && self.state != DhcpState::Renewing
            && self.state != DhcpState::Rebinding
        {
            return Ok(false);
        }

        let packet = DhcpPacket::from_bytes(data)?;

        if packet.op != DhcpOp::Reply as u8 || packet.xid != self.xid {
            return Ok(false);
        }

        // Parse options
        let options = &data[DhcpPacket::HEADER_SIZE..];
        let msg_type = self.parse_message_type(options)?;

        match msg_type {
            DhcpMessageType::Ack => {
                // Parse lease information
                let lease = self.parse_lease(&packet, options, current_time)?;
                self.lease = Some(lease);
                self.state = DhcpState::Bound;
                Ok(true)
            }
            DhcpMessageType::Nak => {
                self.state = DhcpState::Init;
                self.lease = None;
                Err(NetworkError::DhcpNak)
            }
            _ => Ok(false),
        }
    }

    /// Parse message type from options.
    fn parse_message_type(&self, options: &[u8]) -> Result<DhcpMessageType, NetworkError> {
        let mut offset = 0;

        while offset < options.len() {
            let opt = options[offset];
            offset += 1;

            if opt == DhcpOption::Pad as u8 {
                continue;
            }

            if opt == DhcpOption::End as u8 {
                break;
            }

            if offset >= options.len() {
                return Err(NetworkError::InvalidPacket);
            }

            let len = options[offset] as usize;
            offset += 1;

            if opt == DhcpOption::MessageType as u8 && len >= 1 {
                return match options[offset] {
                    1 => Ok(DhcpMessageType::Discover),
                    2 => Ok(DhcpMessageType::Offer),
                    3 => Ok(DhcpMessageType::Request),
                    4 => Ok(DhcpMessageType::Decline),
                    5 => Ok(DhcpMessageType::Ack),
                    6 => Ok(DhcpMessageType::Nak),
                    7 => Ok(DhcpMessageType::Release),
                    8 => Ok(DhcpMessageType::Inform),
                    _ => Err(NetworkError::InvalidPacket),
                };
            }

            offset += len;
        }

        Err(NetworkError::InvalidPacket)
    }

    /// Parse lease information from packet and options.
    fn parse_lease(
        &self,
        packet: &DhcpPacket,
        options: &[u8],
        current_time: u64,
    ) -> Result<DhcpLease, NetworkError> {
        let mut lease = DhcpLease {
            ip_address: packet.yiaddr,
            subnet_mask: [255, 255, 255, 0],
            gateway: None,
            dns_servers: [[0; 4]; 4],
            dns_count: 0,
            domain_name: [0; 64],
            domain_len: 0,
            lease_time: 86400,   // Default 24 hours
            renewal_time: 43200, // Default T1 = 0.5 * lease
            rebind_time: 75600,  // Default T2 = 0.875 * lease
            server_ip: packet.siaddr,
            acquired_at: current_time,
        };

        let mut offset = 0;

        while offset < options.len() {
            let opt = options[offset];
            offset += 1;

            if opt == DhcpOption::Pad as u8 {
                continue;
            }

            if opt == DhcpOption::End as u8 {
                break;
            }

            if offset >= options.len() {
                break;
            }

            let len = options[offset] as usize;
            offset += 1;

            if offset + len > options.len() {
                break;
            }

            match opt {
                x if x == DhcpOption::SubnetMask as u8 && len >= 4 => {
                    lease
                        .subnet_mask
                        .copy_from_slice(&options[offset..offset + 4]);
                }
                x if x == DhcpOption::Router as u8 && len >= 4 => {
                    let mut gw = [0u8; 4];
                    gw.copy_from_slice(&options[offset..offset + 4]);
                    lease.gateway = Some(gw);
                }
                x if x == DhcpOption::DnsServer as u8 => {
                    let count = len / 4;
                    for i in 0..count.min(4) {
                        lease.dns_servers[i]
                            .copy_from_slice(&options[offset + i * 4..offset + i * 4 + 4]);
                        lease.dns_count += 1;
                    }
                }
                x if x == DhcpOption::DomainName as u8 => {
                    let copy_len = len.min(64);
                    lease.domain_name[..copy_len]
                        .copy_from_slice(&options[offset..offset + copy_len]);
                    lease.domain_len = copy_len;
                }
                x if x == DhcpOption::LeaseTime as u8 && len >= 4 => {
                    lease.lease_time = u32::from_be_bytes([
                        options[offset],
                        options[offset + 1],
                        options[offset + 2],
                        options[offset + 3],
                    ]);
                }
                x if x == DhcpOption::RenewalTime as u8 && len >= 4 => {
                    lease.renewal_time = u32::from_be_bytes([
                        options[offset],
                        options[offset + 1],
                        options[offset + 2],
                        options[offset + 3],
                    ]);
                }
                x if x == DhcpOption::RebindingTime as u8 && len >= 4 => {
                    lease.rebind_time = u32::from_be_bytes([
                        options[offset],
                        options[offset + 1],
                        options[offset + 2],
                        options[offset + 3],
                    ]);
                }
                x if x == DhcpOption::ServerIdentifier as u8 && len >= 4 => {
                    lease
                        .server_ip
                        .copy_from_slice(&options[offset..offset + 4]);
                }
                _ => {}
            }

            offset += len;
        }

        Ok(lease)
    }

    /// Release the current lease.
    pub fn release(&mut self) -> Option<([u8; 576], usize)> {
        if self.state != DhcpState::Bound {
            return None;
        }

        let lease = self.lease.as_ref()?;

        let mut packet = DhcpPacket::new_discover(self.xid, self.mac);
        packet.ciaddr = lease.ip_address;

        let mut options = [0u8; 16];
        let mut offset = 0;

        // Message type
        options[offset] = DhcpOption::MessageType as u8;
        offset += 1;
        options[offset] = 1;
        offset += 1;
        options[offset] = DhcpMessageType::Release as u8;
        offset += 1;

        // Server identifier
        options[offset] = DhcpOption::ServerIdentifier as u8;
        offset += 1;
        options[offset] = 4;
        offset += 1;
        options[offset..offset + 4].copy_from_slice(&lease.server_ip);
        offset += 4;

        // End
        options[offset] = DhcpOption::End as u8;
        offset += 1;

        let mut buffer = [0u8; 576];
        let len = packet.to_bytes(&options[..offset], &mut buffer);

        self.state = DhcpState::Init;
        self.lease = None;

        Some((buffer, len))
    }

    /// Update the DHCP client state (call periodically).
    pub fn update(&mut self, current_time: u64) -> Option<([u8; 576], usize)> {
        let lease = self.lease.as_ref()?;

        if lease.is_expired(current_time) {
            self.state = DhcpState::Init;
            self.lease = None;
            return Some(self.discover());
        }

        if lease.needs_rebind(current_time) && self.state == DhcpState::Renewing {
            self.state = DhcpState::Rebinding;
            // Send broadcast request
            return self.build_renew_request();
        }

        if lease.needs_renewal(current_time) && self.state == DhcpState::Bound {
            self.state = DhcpState::Renewing;
            // Send unicast request to server
            return self.build_renew_request();
        }

        None
    }

    /// Build a renewal request.
    fn build_renew_request(&mut self) -> Option<([u8; 576], usize)> {
        let lease = self.lease.as_ref()?;

        let mut packet = DhcpPacket::new_discover(self.xid, self.mac);
        packet.ciaddr = lease.ip_address;

        let (options, opt_len) = self.build_options(DhcpMessageType::Request, None);

        let mut buffer = [0u8; 576];
        let len = packet.to_bytes(&options[..opt_len], &mut buffer);

        Some((buffer, len))
    }
}
