# Networking Subsystem Design Document

**Document Version:** 2.1.0  
**Last Updated:** 2026-02-24  
**Status:** Implemented (VirtIO PIO + DHCP verified on QEMU)

---

## Table of Contents

1. [Overview](#1-overview)
2. [Design Principles](#2-design-principles)
3. [Architecture](#3-architecture)
4. [TCP/IP Stack (smoltcp)](#4-tcpip-stack-smoltcp)
5. [Network Drivers](#5-network-drivers)
6. [Socket Interface](#6-socket-interface)
7. [Service Architecture](#7-service-architecture)
8. [Performance Optimizations](#8-performance-optimizations)
9. [Security Considerations](#9-security-considerations)
10. [HTTP Server Capability](#10-http-server-capability)

---

## 1. Overview

### 1.1 Purpose

This document specifies the design of the KPIO networking subsystem, which provides a user-space TCP/IP stack capable of running robust web servers and network applications.

### 1.2 Scope

This document covers:
- TCP/IP stack implementation using smoltcp
- Network driver architecture
- Socket API for WASM applications
- Network service isolation
- HTTP server implementation

This document does NOT cover:
- Wireless networking (future extension)
- VPN/tunneling protocols
- Network configuration UI

### 1.3 Source Location

```
network/
    Cargo.toml
    src/
        lib.rs
        stack/
            mod.rs
            tcp.rs          # TCP socket implementation
            udp.rs          # UDP socket implementation
            icmp.rs         # ICMP handling
            dns.rs          # DNS resolver
            dhcp.rs         # DHCP client
        drivers/
            mod.rs
            virtio.rs       # VirtIO-Net driver
            e1000.rs        # Intel E1000 driver
            loopback.rs     # Loopback interface
        service/
            mod.rs
            ipc.rs          # IPC protocol for network requests
            dispatcher.rs   # Request routing
        http/
            mod.rs
            server.rs       # HTTP/1.1 server
            request.rs      # Request parsing
            response.rs     # Response building
```

---

## 2. Design Principles

### 2.1 User-Space Implementation

The entire network stack runs in user space for:

| Benefit | Description |
|---------|-------------|
| Isolation | Network bugs cannot crash the kernel |
| Restartability | Stack can be restarted without reboot |
| Debuggability | Standard debugging tools apply |
| Flexibility | Easy to modify and extend |

### 2.2 Rust-Based Stack

Using **smoltcp** (Rust TCP/IP stack):

- **Memory Safety:** No buffer overflows possible
- **No Heap Allocation:** Suitable for embedded/constrained environments
- **Event-Driven:** Fits async programming model
- **Portable:** No OS-specific dependencies

### 2.3 Minimalist Driver Support

Initial driver support targets:

| Driver | Target Use Case |
|--------|-----------------|
| VirtIO-Net | QEMU/KVM virtualization |
| E1000 | VMware, older hardware |
| Loopback | Local testing |

---

## 3. Architecture

### 3.1 Stack Overview

```
+=========================================================================+
|                        APPLICATION LAYER                                 |
+=========================================================================+
|                                                                          |
|  +------------------+  +------------------+  +------------------------+  |
|  |  WASM App        |  |  HTTP Server     |  |  DNS Resolver          |  |
|  |  (Socket API)    |  |  (WASM Service)  |  |  (WASM Service)        |  |
|  +--------+---------+  +--------+---------+  +-----------+------------+  |
|           |                     |                        |               |
|           +---------------------+------------------------+               |
|                                 |                                        |
+=========================================================================+
|                        SOCKET ABSTRACTION                                |
+=========================================================================+
|                                                                          |
|  +--------------------------------------------------------------------+  |
|  |                    WASI Socket Extensions                           |  |
|  |  - tcp_connect, tcp_listen, tcp_accept                              |  |
|  |  - udp_bind, udp_send, udp_recv                                     |  |
|  +-------------------------------+------------------------------------+  |
|                                  |                                       |
+=========================================================================+
|                        NETWORK SERVICE                                   |
+=========================================================================+
|                                                                          |
|  +--------------------------------------------------------------------+  |
|  |                    Network Stack Service                            |  |
|  |  (Runs as isolated WASM process)                                    |  |
|  +--------------------------------------------------------------------+  |
|                                                                          |
|  +---------------------------+  +------------------------------------+   |
|  |   smoltcp TCP/IP Stack    |  |    Interface Management            |   |
|  |  - TCP state machine      |  |  - IP addressing                   |   |
|  |  - UDP handling           |  |  - Routing table                   |   |
|  |  - ICMP                   |  |  - ARP cache                       |   |
|  +---------------------------+  +------------------------------------+   |
|                                                                          |
+=========================================================================+
|                        DRIVER LAYER                                      |
+=========================================================================+
|                                                                          |
|  +------------------+  +------------------+  +------------------------+  |
|  |  VirtIO-Net      |  |  E1000           |  |  Loopback              |  |
|  +------------------+  +------------------+  +------------------------+  |
|                                                                          |
+=========================================================================+
|                        KERNEL INTERFACE                                  |
+=========================================================================+
|  - MMIO access (via capability)                                         |
|  - Interrupt handling (via capability)                                  |
|  - DMA buffer allocation                                                |
+=========================================================================+
```

### 3.2 Component Interaction

```
+------------------+
|  WASM Application|
+--------+---------+
         |
         | WASI socket calls
         v
+--------+---------+     IPC     +------------------+
|  Runtime (WASI)  | <---------> |  Network Service |
+------------------+             +--------+---------+
                                          |
                                          | smoltcp
                                          v
                                 +--------+---------+
                                 |  Network Driver  |
                                 +--------+---------+
                                          |
                                          | Packets
                                          v
                                 +--------+---------+
                                 |    Hardware      |
                                 +------------------+
```

---

## 4. TCP/IP Stack (smoltcp)

### 4.1 Stack Configuration

```rust
// network/src/stack/mod.rs

use smoltcp::iface::{Config, Interface, SocketSet};
use smoltcp::phy::Device;
use smoltcp::socket::tcp;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, IpCidr, Ipv4Address};

pub struct NetworkStack<D: Device> {
    /// Network interface
    iface: Interface,
    
    /// Device driver
    device: D,
    
    /// Socket set
    sockets: SocketSet<'static>,
    
    /// ARP cache
    neighbor_cache: NeighborCache,
    
    /// Routing table
    routes: Routes,
    
    /// Configuration
    config: StackConfig,
}

#[derive(Debug, Clone)]
pub struct StackConfig {
    /// Maximum number of sockets
    pub max_sockets: usize,
    
    /// TCP receive buffer size per socket
    pub tcp_rx_buffer_size: usize,
    
    /// TCP send buffer size per socket
    pub tcp_tx_buffer_size: usize,
    
    /// UDP receive buffer size
    pub udp_rx_buffer_size: usize,
    
    /// UDP send buffer size
    pub udp_tx_buffer_size: usize,
    
    /// Maximum TCP connections
    pub max_tcp_connections: usize,
    
    /// Enable IP forwarding
    pub ip_forwarding: bool,
}

impl Default for StackConfig {
    fn default() -> Self {
        Self {
            max_sockets: 1024,
            tcp_rx_buffer_size: 65536,
            tcp_tx_buffer_size: 65536,
            udp_rx_buffer_size: 65536,
            udp_tx_buffer_size: 65536,
            max_tcp_connections: 1024,
            ip_forwarding: false,
        }
    }
}

impl<D: Device> NetworkStack<D> {
    pub fn new(device: D, config: StackConfig) -> Self {
        let mac = device.mac_address();
        
        let iface_config = Config::new(EthernetAddress(mac).into());
        let iface = Interface::new(iface_config, &mut device, Instant::now());
        
        Self {
            iface,
            device,
            sockets: SocketSet::new(vec![]),
            neighbor_cache: NeighborCache::new(),
            routes: Routes::new(),
            config,
        }
    }
    
    /// Configure IP address
    pub fn set_ip_address(&mut self, cidr: IpCidr) {
        self.iface.update_ip_addrs(|addrs| {
            addrs.clear();
            addrs.push(cidr).unwrap();
        });
    }
    
    /// Set default gateway
    pub fn set_gateway(&mut self, gateway: Ipv4Address) {
        self.routes.add_default_ipv4_route(gateway).unwrap();
        self.iface.routes_mut().update(|routes| {
            routes.clear();
            routes.push(self.routes.default_v4_route().unwrap()).unwrap();
        });
    }
    
    /// Poll the network stack (must be called regularly)
    pub fn poll(&mut self, timestamp: Instant) -> bool {
        self.iface.poll(timestamp, &mut self.device, &mut self.sockets)
    }
    
    /// Get next poll time
    pub fn poll_delay(&mut self, timestamp: Instant) -> Option<Duration> {
        self.iface.poll_delay(timestamp, &self.sockets)
    }
}
```

### 4.2 TCP Socket Implementation

```rust
// network/src/stack/tcp.rs

use smoltcp::socket::tcp::{Socket as TcpSocket, SocketBuffer, State};

pub struct TcpConnection {
    /// Socket handle in the socket set
    handle: SocketHandle,
    
    /// Local address
    local_addr: SocketAddr,
    
    /// Remote address (for connected sockets)
    remote_addr: Option<SocketAddr>,
    
    /// Connection state
    state: ConnectionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

impl TcpConnection {
    /// Create a new TCP socket
    pub fn new(stack: &mut NetworkStack<impl Device>, config: &StackConfig) -> Self {
        let rx_buffer = TcpSocketBuffer::new(vec![0; config.tcp_rx_buffer_size]);
        let tx_buffer = TcpSocketBuffer::new(vec![0; config.tcp_tx_buffer_size]);
        let socket = TcpSocket::new(rx_buffer, tx_buffer);
        
        let handle = stack.sockets.add(socket);
        
        Self {
            handle,
            local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            remote_addr: None,
            state: ConnectionState::Closed,
        }
    }
    
    /// Bind to local address
    pub fn bind(&mut self, addr: SocketAddr) -> Result<(), TcpError> {
        self.local_addr = addr;
        Ok(())
    }
    
    /// Listen for incoming connections
    pub fn listen(&mut self, stack: &mut NetworkStack<impl Device>) -> Result<(), TcpError> {
        let socket = stack.sockets.get_mut::<TcpSocket>(self.handle);
        socket.listen(self.local_addr.port())
            .map_err(|_| TcpError::AddressInUse)?;
        self.state = ConnectionState::Listen;
        Ok(())
    }
    
    /// Connect to remote address
    pub fn connect(
        &mut self,
        stack: &mut NetworkStack<impl Device>,
        remote: SocketAddr,
    ) -> Result<(), TcpError> {
        let socket = stack.sockets.get_mut::<TcpSocket>(self.handle);
        
        // Find a local port if not bound
        if self.local_addr.port() == 0 {
            self.local_addr.set_port(stack.allocate_ephemeral_port()?);
        }
        
        socket.connect(
            stack.iface.context(),
            remote,
            self.local_addr,
        ).map_err(|e| TcpError::ConnectFailed(e))?;
        
        self.remote_addr = Some(remote);
        self.state = ConnectionState::SynSent;
        Ok(())
    }
    
    /// Accept incoming connection (for listening sockets)
    pub fn accept(&self, stack: &mut NetworkStack<impl Device>) -> Option<TcpConnection> {
        let socket = stack.sockets.get::<TcpSocket>(self.handle);
        
        if socket.is_active() && socket.state() == State::Established {
            // Clone the connection for the client
            let remote = socket.remote_endpoint()?;
            
            // Create new listening socket for next connection
            let new_listener = TcpConnection::new(stack, &stack.config);
            // ...
            
            Some(TcpConnection {
                handle: self.handle, // Transfer the established socket
                local_addr: self.local_addr,
                remote_addr: Some(remote.into()),
                state: ConnectionState::Established,
            })
        } else {
            None
        }
    }
    
    /// Send data
    pub fn send(&self, stack: &mut NetworkStack<impl Device>, data: &[u8]) -> Result<usize, TcpError> {
        let socket = stack.sockets.get_mut::<TcpSocket>(self.handle);
        
        if !socket.may_send() {
            return Err(TcpError::NotConnected);
        }
        
        socket.send_slice(data)
            .map_err(|_| TcpError::SendFailed)
    }
    
    /// Receive data
    pub fn recv(&self, stack: &mut NetworkStack<impl Device>, buf: &mut [u8]) -> Result<usize, TcpError> {
        let socket = stack.sockets.get_mut::<TcpSocket>(self.handle);
        
        if !socket.may_recv() {
            if socket.state() == State::CloseWait {
                return Err(TcpError::ConnectionClosed);
            }
            return Ok(0); // No data available
        }
        
        socket.recv_slice(buf)
            .map_err(|_| TcpError::RecvFailed)
    }
    
    /// Close the connection
    pub fn close(&mut self, stack: &mut NetworkStack<impl Device>) {
        let socket = stack.sockets.get_mut::<TcpSocket>(self.handle);
        socket.close();
        self.state = ConnectionState::FinWait1;
    }
    
    /// Update connection state from socket
    pub fn update_state(&mut self, stack: &NetworkStack<impl Device>) {
        let socket = stack.sockets.get::<TcpSocket>(self.handle);
        self.state = match socket.state() {
            State::Closed => ConnectionState::Closed,
            State::Listen => ConnectionState::Listen,
            State::SynSent => ConnectionState::SynSent,
            State::SynReceived => ConnectionState::SynReceived,
            State::Established => ConnectionState::Established,
            State::FinWait1 => ConnectionState::FinWait1,
            State::FinWait2 => ConnectionState::FinWait2,
            State::CloseWait => ConnectionState::CloseWait,
            State::Closing => ConnectionState::Closing,
            State::LastAck => ConnectionState::LastAck,
            State::TimeWait => ConnectionState::TimeWait,
        };
    }
}
```

### 4.3 UDP Socket Implementation

```rust
// network/src/stack/udp.rs

use smoltcp::socket::udp::{Socket as UdpSocket, PacketBuffer, PacketMetadata};

pub struct UdpBinding {
    /// Socket handle
    handle: SocketHandle,
    
    /// Bound address
    local_addr: SocketAddr,
}

impl UdpBinding {
    pub fn new(stack: &mut NetworkStack<impl Device>, config: &StackConfig) -> Self {
        let rx_buffer = PacketBuffer::new(
            vec![PacketMetadata::EMPTY; 64],
            vec![0; config.udp_rx_buffer_size],
        );
        let tx_buffer = PacketBuffer::new(
            vec![PacketMetadata::EMPTY; 64],
            vec![0; config.udp_tx_buffer_size],
        );
        let socket = UdpSocket::new(rx_buffer, tx_buffer);
        
        let handle = stack.sockets.add(socket);
        
        Self {
            handle,
            local_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        }
    }
    
    /// Bind to local address
    pub fn bind(
        &mut self,
        stack: &mut NetworkStack<impl Device>,
        addr: SocketAddr,
    ) -> Result<(), UdpError> {
        let socket = stack.sockets.get_mut::<UdpSocket>(self.handle);
        socket.bind(addr.port())
            .map_err(|_| UdpError::AddressInUse)?;
        self.local_addr = addr;
        Ok(())
    }
    
    /// Send datagram to remote address
    pub fn send_to(
        &self,
        stack: &mut NetworkStack<impl Device>,
        data: &[u8],
        remote: SocketAddr,
    ) -> Result<(), UdpError> {
        let socket = stack.sockets.get_mut::<UdpSocket>(self.handle);
        socket.send_slice(data, remote.into())
            .map_err(|_| UdpError::SendFailed)
    }
    
    /// Receive datagram
    pub fn recv_from(
        &self,
        stack: &mut NetworkStack<impl Device>,
        buf: &mut [u8],
    ) -> Result<(usize, SocketAddr), UdpError> {
        let socket = stack.sockets.get_mut::<UdpSocket>(self.handle);
        
        match socket.recv_slice(buf) {
            Ok((len, endpoint)) => Ok((len, endpoint.into())),
            Err(_) => Err(UdpError::RecvFailed),
        }
    }
}
```

### 4.4 DNS Resolver

```rust
// network/src/stack/dns.rs

use smoltcp::wire::{DnsPacket, DnsQuery, DnsRecord, DnsRepr};

pub struct DnsResolver {
    /// DNS server addresses
    servers: Vec<IpAddr>,
    
    /// DNS cache
    cache: DnsCache,
    
    /// Pending queries
    pending: HashMap<u16, PendingQuery>,
    
    /// Next query ID
    next_id: u16,
}

struct DnsCache {
    entries: HashMap<String, CacheEntry>,
    max_entries: usize,
}

struct CacheEntry {
    addresses: Vec<IpAddr>,
    expires_at: Instant,
}

struct PendingQuery {
    name: String,
    started_at: Instant,
    waker: Option<Waker>,
}

impl DnsResolver {
    pub fn new(servers: Vec<IpAddr>) -> Self {
        Self {
            servers,
            cache: DnsCache::new(1000),
            pending: HashMap::new(),
            next_id: 1,
        }
    }
    
    /// Resolve hostname to IP address
    pub async fn resolve(&mut self, name: &str) -> Result<Vec<IpAddr>, DnsError> {
        // Check cache first
        if let Some(addrs) = self.cache.lookup(name) {
            return Ok(addrs);
        }
        
        // Build DNS query
        let query_id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        
        let query = DnsQuery {
            name: name.as_bytes(),
            qtype: DnsQueryType::A,
        };
        
        let packet = self.build_query_packet(query_id, &query);
        
        // Send to first DNS server
        let server = self.servers.first()
            .ok_or(DnsError::NoServers)?;
        
        self.pending.insert(query_id, PendingQuery {
            name: name.to_string(),
            started_at: Instant::now(),
            waker: None,
        });
        
        // Send UDP packet (via network stack)
        self.send_query(&packet, *server).await?;
        
        // Wait for response
        self.wait_for_response(query_id).await
    }
    
    /// Handle incoming DNS response
    pub fn handle_response(&mut self, packet: &[u8]) -> Result<(), DnsError> {
        let dns = DnsPacket::new_checked(packet)?;
        let repr = DnsRepr::parse(&dns)?;
        
        let query_id = repr.transaction_id;
        
        if let Some(pending) = self.pending.remove(&query_id) {
            // Extract A records
            let addresses: Vec<IpAddr> = repr.answers
                .iter()
                .filter_map(|record| {
                    if let DnsRecord::A { address, .. } = record {
                        Some(IpAddr::V4((*address).into()))
                    } else {
                        None
                    }
                })
                .collect();
            
            // Cache result
            let ttl = repr.answers.iter()
                .map(|r| r.ttl())
                .min()
                .unwrap_or(300);
            
            self.cache.insert(&pending.name, addresses.clone(), ttl);
            
            // Wake the waiting task
            if let Some(waker) = pending.waker {
                waker.wake();
            }
        }
        
        Ok(())
    }
}
```

### 4.5 DHCP Client

```rust
// network/src/stack/dhcp.rs

use smoltcp::socket::dhcpv4::{Socket as Dhcpv4Socket, Event};

pub struct DhcpClient {
    /// DHCP socket handle
    handle: SocketHandle,
    
    /// Current lease
    lease: Option<DhcpLease>,
    
    /// Client state
    state: DhcpState,
}

#[derive(Debug, Clone)]
pub struct DhcpLease {
    pub ip_address: Ipv4Cidr,
    pub gateway: Option<Ipv4Address>,
    pub dns_servers: Vec<Ipv4Address>,
    pub lease_duration: Duration,
    pub obtained_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DhcpState {
    Discovering,
    Requesting,
    Bound,
    Renewing,
    Rebinding,
}

impl DhcpClient {
    pub fn new(stack: &mut NetworkStack<impl Device>) -> Self {
        let socket = Dhcpv4Socket::new();
        let handle = stack.sockets.add(socket);
        
        Self {
            handle,
            lease: None,
            state: DhcpState::Discovering,
        }
    }
    
    /// Poll DHCP client
    pub fn poll(&mut self, stack: &mut NetworkStack<impl Device>) -> Option<DhcpEvent> {
        let socket = stack.sockets.get_mut::<Dhcpv4Socket>(self.handle);
        
        match socket.poll() {
            Some(Event::Configured(config)) => {
                let lease = DhcpLease {
                    ip_address: config.address,
                    gateway: config.router,
                    dns_servers: config.dns_servers.to_vec(),
                    lease_duration: Duration::from_secs(config.lease_duration.unwrap_or(86400) as u64),
                    obtained_at: Instant::now(),
                };
                
                // Configure interface
                stack.set_ip_address(lease.ip_address.into());
                if let Some(gw) = lease.gateway {
                    stack.set_gateway(gw);
                }
                
                self.lease = Some(lease.clone());
                self.state = DhcpState::Bound;
                
                Some(DhcpEvent::Configured(lease))
            }
            Some(Event::Deconfigured) => {
                self.lease = None;
                self.state = DhcpState::Discovering;
                Some(DhcpEvent::Deconfigured)
            }
            None => None,
        }
    }
}

pub enum DhcpEvent {
    Configured(DhcpLease),
    Deconfigured,
}
```

---

## 5. Network Drivers

### 5.1 Driver Trait

```rust
// network/src/drivers/mod.rs

use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};

pub trait NetworkDriver: Device + Send {
    /// Get MAC address
    fn mac_address(&self) -> [u8; 6];
    
    /// Enable the device
    fn enable(&mut self) -> Result<(), DriverError>;
    
    /// Disable the device
    fn disable(&mut self) -> Result<(), DriverError>;
    
    /// Get link status
    fn link_status(&self) -> LinkStatus;
    
    /// Get statistics
    fn statistics(&self) -> DriverStats;
    
    /// Handle interrupt
    fn handle_interrupt(&mut self) -> InterruptResult;
}

#[derive(Debug, Clone, Copy)]
pub struct LinkStatus {
    pub up: bool,
    pub speed: LinkSpeed,
    pub duplex: Duplex,
}

#[derive(Debug, Clone, Copy)]
pub enum LinkSpeed {
    Speed10Mbps,
    Speed100Mbps,
    Speed1Gbps,
    Speed10Gbps,
}

#[derive(Debug, Clone, Copy)]
pub enum Duplex {
    Half,
    Full,
}

#[derive(Debug, Default)]
pub struct DriverStats {
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
}
```

### 5.2 VirtIO-Net Driver

The VirtIO-Net driver supports two transport modes:

#### 5.2.1 PIO Mode (Legacy PCI Transport) — Implemented in Phase 9-1

The PIO driver uses `x86_64::instructions::port::Port` for register access, following
the VirtIO 1.0 legacy PCI interface (§4.1.4.8). This is the primary driver for QEMU
`-device virtio-net-pci`.

```rust
// kernel/src/drivers/net/virtio_net.rs — PIO register offsets
mod pio_reg {
    pub const DEVICE_FEATURES: u16 = 0x00;  // 4 bytes
    pub const DRIVER_FEATURES: u16 = 0x04;  // 4 bytes
    pub const QUEUE_ADDRESS: u16   = 0x08;  // 4 bytes (PFN)
    pub const QUEUE_SIZE: u16      = 0x0C;  // 2 bytes
    pub const QUEUE_SELECT: u16    = 0x0E;  // 2 bytes
    pub const QUEUE_NOTIFY: u16    = 0x10;  // 2 bytes
    pub const DEVICE_STATUS: u16   = 0x12;  // 1 byte
    pub const ISR_STATUS: u16      = 0x13;  // 1 byte
    pub const MAC0: u16            = 0x14;  // 6 bytes
    pub const NET_STATUS: u16      = 0x1A;  // 2 bytes
}
```

Init sequence: reset → ACKNOWLEDGE → DRIVER → read features → write features →
FEATURES_OK → read MAC → allocate virtqueues (desc + avail + used rings) →
DRIVER_OK. PCI bus mastering and I/O space access are enabled before init.

#### 5.2.2 MMIO Mode (VirtIO MMIO Transport)

The MMIO driver uses memory-mapped registers for platforms that expose VirtIO via MMIO.

```rust
// network/src/drivers/virtio.rs

use virtio_drivers::{VirtIONet, VirtIOHeader};

pub struct VirtioNetDriver {
    /// VirtIO device
    inner: VirtIONet<'static, HalImpl>,
    
    /// MAC address
    mac: [u8; 6],
    
    /// Receive buffer pool
    rx_buffers: Vec<Box<[u8; 1514]>>,
    
    /// Transmit buffer pool
    tx_buffers: Vec<Box<[u8; 1514]>>,
    
    /// Statistics
    stats: DriverStats,
}

impl VirtioNetDriver {
    pub fn new(header: &'static mut VirtIOHeader) -> Result<Self, DriverError> {
        let inner = VirtIONet::new(header)
            .map_err(|e| DriverError::InitFailed(format!("{:?}", e)))?;
        
        let mac = inner.mac_address();
        
        // Allocate buffer pools
        let rx_buffers: Vec<_> = (0..256)
            .map(|_| Box::new([0u8; 1514]))
            .collect();
        let tx_buffers: Vec<_> = (0..256)
            .map(|_| Box::new([0u8; 1514]))
            .collect();
        
        Ok(Self {
            inner,
            mac,
            rx_buffers,
            tx_buffers,
            stats: DriverStats::default(),
        })
    }
    
    fn receive_packet(&mut self) -> Option<(Box<[u8; 1514]>, usize)> {
        if let Ok(buf) = self.inner.recv() {
            self.stats.rx_packets += 1;
            self.stats.rx_bytes += buf.len() as u64;
            
            let mut packet = self.rx_buffers.pop()?;
            let len = buf.len().min(1514);
            packet[..len].copy_from_slice(&buf[..len]);
            
            Some((packet, len))
        } else {
            None
        }
    }
    
    fn transmit_packet(&mut self, data: &[u8]) -> Result<(), DriverError> {
        self.inner.send(data)
            .map_err(|e| DriverError::TxFailed(format!("{:?}", e)))?;
        
        self.stats.tx_packets += 1;
        self.stats.tx_bytes += data.len() as u64;
        
        Ok(())
    }
}

impl Device for VirtioNetDriver {
    type RxToken<'a> = VirtioRxToken<'a> where Self: 'a;
    type TxToken<'a> = VirtioTxToken<'a> where Self: 'a;
    
    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        if self.inner.can_recv() && self.inner.can_send() {
            Some((
                VirtioRxToken { driver: self },
                VirtioTxToken { driver: self },
            ))
        } else {
            None
        }
    }
    
    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        if self.inner.can_send() {
            Some(VirtioTxToken { driver: self })
        } else {
            None
        }
    }
    
    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = 1500;
        caps.max_burst_size = Some(1);
        caps
    }
}

struct VirtioRxToken<'a> {
    driver: &'a mut VirtioNetDriver,
}

impl<'a> RxToken for VirtioRxToken<'a> {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        if let Some((mut packet, len)) = self.driver.receive_packet() {
            let result = f(&mut packet[..len]);
            self.driver.rx_buffers.push(packet);
            result
        } else {
            panic!("RxToken consumed but no packet available");
        }
    }
}

struct VirtioTxToken<'a> {
    driver: &'a mut VirtioNetDriver,
}

impl<'a> TxToken for VirtioTxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut packet = self.driver.tx_buffers.pop()
            .expect("TX buffer pool exhausted");
        
        let result = f(&mut packet[..len]);
        
        let _ = self.driver.transmit_packet(&packet[..len]);
        self.driver.tx_buffers.push(packet);
        
        result
    }
}

impl NetworkDriver for VirtioNetDriver {
    fn mac_address(&self) -> [u8; 6] {
        self.mac
    }
    
    fn enable(&mut self) -> Result<(), DriverError> {
        // VirtIO is enabled on creation
        Ok(())
    }
    
    fn disable(&mut self) -> Result<(), DriverError> {
        // TODO: Proper shutdown
        Ok(())
    }
    
    fn link_status(&self) -> LinkStatus {
        LinkStatus {
            up: true,
            speed: LinkSpeed::Speed1Gbps,
            duplex: Duplex::Full,
        }
    }
    
    fn statistics(&self) -> DriverStats {
        self.stats.clone()
    }
    
    fn handle_interrupt(&mut self) -> InterruptResult {
        self.inner.ack_interrupt();
        InterruptResult::Handled
    }
}
```

### 5.3 Intel E1000 Driver

```rust
// network/src/drivers/e1000.rs

pub struct E1000Driver {
    /// MMIO base address
    mmio_base: *mut u8,
    
    /// MAC address
    mac: [u8; 6],
    
    /// Receive descriptors
    rx_ring: RxRing,
    
    /// Transmit descriptors
    tx_ring: TxRing,
    
    /// Statistics
    stats: DriverStats,
}

struct RxRing {
    descriptors: Box<[RxDescriptor; RX_RING_SIZE]>,
    buffers: Vec<Box<[u8; 2048]>>,
    head: usize,
    tail: usize,
}

struct TxRing {
    descriptors: Box<[TxDescriptor; TX_RING_SIZE]>,
    buffers: Vec<Box<[u8; 2048]>>,
    head: usize,
    tail: usize,
}

#[repr(C, align(16))]
struct RxDescriptor {
    buffer_addr: u64,
    length: u16,
    checksum: u16,
    status: u8,
    errors: u8,
    special: u16,
}

#[repr(C, align(16))]
struct TxDescriptor {
    buffer_addr: u64,
    length: u16,
    cso: u8,
    cmd: u8,
    status: u8,
    css: u8,
    special: u16,
}

const RX_RING_SIZE: usize = 256;
const TX_RING_SIZE: usize = 256;

impl E1000Driver {
    pub unsafe fn new(mmio_base: *mut u8) -> Result<Self, DriverError> {
        let mut driver = Self {
            mmio_base,
            mac: [0; 6],
            rx_ring: RxRing::new()?,
            tx_ring: TxRing::new()?,
            stats: DriverStats::default(),
        };
        
        driver.init()?;
        
        Ok(driver)
    }
    
    unsafe fn init(&mut self) -> Result<(), DriverError> {
        // Reset device
        self.write_reg(E1000_CTRL, E1000_CTRL_RST);
        while self.read_reg(E1000_CTRL) & E1000_CTRL_RST != 0 {}
        
        // Read MAC address from EEPROM
        self.read_mac_address();
        
        // Initialize multicast table
        for i in 0..128 {
            self.write_reg(E1000_MTA + i * 4, 0);
        }
        
        // Initialize receive
        self.init_rx()?;
        
        // Initialize transmit
        self.init_tx()?;
        
        // Enable interrupts
        self.write_reg(E1000_IMS, E1000_IMS_RXT0 | E1000_IMS_TXDW);
        
        // Enable receiver and transmitter
        self.write_reg(E1000_RCTL, E1000_RCTL_EN | E1000_RCTL_BAM | E1000_RCTL_BSIZE_2048);
        self.write_reg(E1000_TCTL, E1000_TCTL_EN | E1000_TCTL_PSP);
        
        Ok(())
    }
    
    unsafe fn init_rx(&mut self) -> Result<(), DriverError> {
        // Set receive descriptor base address
        let rdba = self.rx_ring.descriptors.as_ptr() as u64;
        self.write_reg(E1000_RDBAL, rdba as u32);
        self.write_reg(E1000_RDBAH, (rdba >> 32) as u32);
        
        // Set receive descriptor length
        self.write_reg(E1000_RDLEN, (RX_RING_SIZE * 16) as u32);
        
        // Set head and tail
        self.write_reg(E1000_RDH, 0);
        self.write_reg(E1000_RDT, (RX_RING_SIZE - 1) as u32);
        
        Ok(())
    }
    
    unsafe fn init_tx(&mut self) -> Result<(), DriverError> {
        // Set transmit descriptor base address
        let tdba = self.tx_ring.descriptors.as_ptr() as u64;
        self.write_reg(E1000_TDBAL, tdba as u32);
        self.write_reg(E1000_TDBAH, (tdba >> 32) as u32);
        
        // Set transmit descriptor length
        self.write_reg(E1000_TDLEN, (TX_RING_SIZE * 16) as u32);
        
        // Set head and tail
        self.write_reg(E1000_TDH, 0);
        self.write_reg(E1000_TDT, 0);
        
        Ok(())
    }
    
    unsafe fn read_reg(&self, offset: u32) -> u32 {
        core::ptr::read_volatile(self.mmio_base.add(offset as usize) as *const u32)
    }
    
    unsafe fn write_reg(&self, offset: u32, value: u32) {
        core::ptr::write_volatile(self.mmio_base.add(offset as usize) as *mut u32, value)
    }
}

// E1000 register offsets
const E1000_CTRL: u32 = 0x0000;
const E1000_STATUS: u32 = 0x0008;
const E1000_RCTL: u32 = 0x0100;
const E1000_TCTL: u32 = 0x0400;
const E1000_RDBAL: u32 = 0x2800;
const E1000_RDBAH: u32 = 0x2804;
const E1000_RDLEN: u32 = 0x2808;
const E1000_RDH: u32 = 0x2810;
const E1000_RDT: u32 = 0x2818;
const E1000_TDBAL: u32 = 0x3800;
const E1000_TDBAH: u32 = 0x3804;
const E1000_TDLEN: u32 = 0x3808;
const E1000_TDH: u32 = 0x3810;
const E1000_TDT: u32 = 0x3818;
const E1000_MTA: u32 = 0x5200;
const E1000_IMS: u32 = 0x00D0;

// Control register flags
const E1000_CTRL_RST: u32 = 1 << 26;

// Receive control flags
const E1000_RCTL_EN: u32 = 1 << 1;
const E1000_RCTL_BAM: u32 = 1 << 15;
const E1000_RCTL_BSIZE_2048: u32 = 0;

// Transmit control flags
const E1000_TCTL_EN: u32 = 1 << 1;
const E1000_TCTL_PSP: u32 = 1 << 3;

// Interrupt flags
const E1000_IMS_RXT0: u32 = 1 << 7;
const E1000_IMS_TXDW: u32 = 1 << 0;
```

---

## 6. Socket Interface

### 6.1 WASI Socket Extensions

```rust
// runtime/src/wasi/sock.rs

use wasmtime_wasi::preview2::sockets::*;

impl HostTcpSocket for KpioWasiCtx {
    async fn create(&mut self, address_family: AddressFamily) -> Result<Resource<TcpSocket>, SocketError> {
        // Validate network capability
        if !self.capabilities.has_network_access() {
            return Err(SocketError::AccessDenied);
        }
        
        // Create socket via network service IPC
        let response = self.network_service
            .send(NetworkRequest::TcpCreate { address_family })
            .await?;
        
        match response {
            NetworkResponse::SocketCreated { id } => {
                let socket = TcpSocketResource { id };
                let resource = self.table.push(socket)?;
                Ok(resource)
            }
            NetworkResponse::Error(e) => Err(e.into()),
            _ => Err(SocketError::InvalidResponse),
        }
    }
    
    async fn connect(
        &mut self,
        socket: Resource<TcpSocket>,
        remote: IpSocketAddress,
    ) -> Result<(Resource<InputStream>, Resource<OutputStream>), SocketError> {
        let sock = self.table.get(&socket)?;
        
        // Validate connection capability
        if !self.capabilities.can_connect_to(&remote) {
            return Err(SocketError::AccessDenied);
        }
        
        let response = self.network_service
            .send(NetworkRequest::TcpConnect {
                socket_id: sock.id,
                remote,
            })
            .await?;
        
        match response {
            NetworkResponse::Connected { input_id, output_id } => {
                let input = self.table.push(InputStreamResource::Network(input_id))?;
                let output = self.table.push(OutputStreamResource::Network(output_id))?;
                Ok((input, output))
            }
            NetworkResponse::Error(e) => Err(e.into()),
            _ => Err(SocketError::InvalidResponse),
        }
    }
    
    async fn listen(
        &mut self,
        socket: Resource<TcpSocket>,
        local: IpSocketAddress,
    ) -> Result<(), SocketError> {
        let sock = self.table.get(&socket)?;
        
        // Validate listen capability
        if !self.capabilities.can_listen_on(&local) {
            return Err(SocketError::AccessDenied);
        }
        
        let response = self.network_service
            .send(NetworkRequest::TcpListen {
                socket_id: sock.id,
                local,
                backlog: 128,
            })
            .await?;
        
        match response {
            NetworkResponse::Listening => Ok(()),
            NetworkResponse::Error(e) => Err(e.into()),
            _ => Err(SocketError::InvalidResponse),
        }
    }
    
    async fn accept(
        &mut self,
        socket: Resource<TcpSocket>,
    ) -> Result<(Resource<TcpSocket>, Resource<InputStream>, Resource<OutputStream>), SocketError> {
        let sock = self.table.get(&socket)?;
        
        let response = self.network_service
            .send(NetworkRequest::TcpAccept {
                socket_id: sock.id,
            })
            .await?;
        
        match response {
            NetworkResponse::Accepted { new_socket_id, input_id, output_id } => {
                let new_socket = self.table.push(TcpSocketResource { id: new_socket_id })?;
                let input = self.table.push(InputStreamResource::Network(input_id))?;
                let output = self.table.push(OutputStreamResource::Network(output_id))?;
                Ok((new_socket, input, output))
            }
            NetworkResponse::Error(e) => Err(e.into()),
            _ => Err(SocketError::InvalidResponse),
        }
    }
}
```

### 6.2 Network Request/Response Protocol

```rust
// network/src/service/ipc.rs

#[derive(Debug, Serialize, Deserialize)]
pub enum NetworkRequest {
    // TCP operations
    TcpCreate {
        address_family: AddressFamily,
    },
    TcpConnect {
        socket_id: u64,
        remote: IpSocketAddress,
    },
    TcpListen {
        socket_id: u64,
        local: IpSocketAddress,
        backlog: u32,
    },
    TcpAccept {
        socket_id: u64,
    },
    TcpSend {
        socket_id: u64,
        data: Vec<u8>,
    },
    TcpRecv {
        socket_id: u64,
        max_len: usize,
    },
    TcpClose {
        socket_id: u64,
    },
    
    // UDP operations
    UdpCreate {
        address_family: AddressFamily,
    },
    UdpBind {
        socket_id: u64,
        local: IpSocketAddress,
    },
    UdpSendTo {
        socket_id: u64,
        data: Vec<u8>,
        remote: IpSocketAddress,
    },
    UdpRecvFrom {
        socket_id: u64,
        max_len: usize,
    },
    
    // DNS operations
    DnsResolve {
        hostname: String,
    },
    
    // Interface management
    GetInterfaces,
    ConfigureInterface {
        name: String,
        config: InterfaceConfig,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum NetworkResponse {
    // Success responses
    SocketCreated {
        id: u64,
    },
    Connected {
        input_id: u64,
        output_id: u64,
    },
    Listening,
    Accepted {
        new_socket_id: u64,
        input_id: u64,
        output_id: u64,
    },
    DataSent {
        bytes: usize,
    },
    DataReceived {
        data: Vec<u8>,
    },
    DataReceivedFrom {
        data: Vec<u8>,
        remote: IpSocketAddress,
    },
    Closed,
    Resolved {
        addresses: Vec<IpAddr>,
    },
    Interfaces {
        list: Vec<InterfaceInfo>,
    },
    
    // Error responses
    Error(NetworkError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceInfo {
    pub name: String,
    pub mac: [u8; 6],
    pub addresses: Vec<IpCidr>,
    pub status: LinkStatus,
    pub stats: DriverStats,
}
```

---

## 7. Service Architecture

### 7.1 Network Service

```rust
// network/src/service/mod.rs

pub struct NetworkService {
    /// Network stack
    stack: NetworkStack<Box<dyn NetworkDriver>>,
    
    /// Active sockets
    sockets: HashMap<u64, SocketState>,
    
    /// IPC channel for requests
    ipc_channel: IpcChannel,
    
    /// Next socket ID
    next_socket_id: u64,
    
    /// DNS resolver
    dns: DnsResolver,
    
    /// DHCP client (optional)
    dhcp: Option<DhcpClient>,
}

enum SocketState {
    TcpListening(TcpConnection),
    TcpConnected(TcpConnection),
    UdpBound(UdpBinding),
}

impl NetworkService {
    pub async fn run(&mut self) -> ! {
        loop {
            // Calculate poll delay
            let now = Instant::now();
            let poll_delay = self.stack.poll_delay(now);
            
            // Wait for event (IPC message or timeout)
            let event = timeout(poll_delay, self.ipc_channel.recv()).await;
            
            match event {
                Ok(Some(msg)) => {
                    // Handle IPC request
                    let response = self.handle_request(msg.request).await;
                    self.ipc_channel.reply(msg.id, response).await;
                }
                Ok(None) => {
                    // Channel closed
                    break;
                }
                Err(_) => {
                    // Timeout - just poll the stack
                }
            }
            
            // Poll network stack
            let now = Instant::now();
            self.stack.poll(now);
            
            // Process DHCP
            if let Some(ref mut dhcp) = self.dhcp {
                if let Some(event) = dhcp.poll(&mut self.stack) {
                    self.handle_dhcp_event(event);
                }
            }
            
            // Process DNS responses
            self.process_dns_responses();
        }
        
        loop { /* Zombie state */ }
    }
    
    async fn handle_request(&mut self, request: NetworkRequest) -> NetworkResponse {
        match request {
            NetworkRequest::TcpCreate { address_family } => {
                let socket = TcpConnection::new(&mut self.stack, &self.stack.config);
                let id = self.next_socket_id;
                self.next_socket_id += 1;
                self.sockets.insert(id, SocketState::TcpListening(socket));
                NetworkResponse::SocketCreated { id }
            }
            
            NetworkRequest::TcpConnect { socket_id, remote } => {
                if let Some(SocketState::TcpListening(ref mut socket)) = self.sockets.get_mut(&socket_id) {
                    match socket.connect(&mut self.stack, remote.into()) {
                        Ok(()) => {
                            // Wait for connection to establish
                            loop {
                                self.stack.poll(Instant::now());
                                socket.update_state(&self.stack);
                                
                                match socket.state {
                                    ConnectionState::Established => {
                                        return NetworkResponse::Connected {
                                            input_id: socket_id,
                                            output_id: socket_id,
                                        };
                                    }
                                    ConnectionState::Closed => {
                                        return NetworkResponse::Error(NetworkError::ConnectionRefused);
                                    }
                                    _ => {
                                        // Still connecting
                                        tokio::time::sleep(Duration::from_millis(10)).await;
                                    }
                                }
                            }
                        }
                        Err(e) => NetworkResponse::Error(e.into()),
                    }
                } else {
                    NetworkResponse::Error(NetworkError::InvalidSocket)
                }
            }
            
            NetworkRequest::TcpListen { socket_id, local, backlog: _ } => {
                if let Some(SocketState::TcpListening(ref mut socket)) = self.sockets.get_mut(&socket_id) {
                    socket.bind(local.into()).ok();
                    match socket.listen(&mut self.stack) {
                        Ok(()) => NetworkResponse::Listening,
                        Err(e) => NetworkResponse::Error(e.into()),
                    }
                } else {
                    NetworkResponse::Error(NetworkError::InvalidSocket)
                }
            }
            
            NetworkRequest::TcpSend { socket_id, data } => {
                if let Some(SocketState::TcpConnected(ref socket)) = self.sockets.get(&socket_id) {
                    match socket.send(&mut self.stack, &data) {
                        Ok(bytes) => NetworkResponse::DataSent { bytes },
                        Err(e) => NetworkResponse::Error(e.into()),
                    }
                } else {
                    NetworkResponse::Error(NetworkError::InvalidSocket)
                }
            }
            
            NetworkRequest::TcpRecv { socket_id, max_len } => {
                if let Some(SocketState::TcpConnected(ref socket)) = self.sockets.get(&socket_id) {
                    let mut buf = vec![0u8; max_len];
                    match socket.recv(&mut self.stack, &mut buf) {
                        Ok(bytes) => {
                            buf.truncate(bytes);
                            NetworkResponse::DataReceived { data: buf }
                        }
                        Err(e) => NetworkResponse::Error(e.into()),
                    }
                } else {
                    NetworkResponse::Error(NetworkError::InvalidSocket)
                }
            }
            
            NetworkRequest::DnsResolve { hostname } => {
                match self.dns.resolve(&hostname).await {
                    Ok(addresses) => NetworkResponse::Resolved { addresses },
                    Err(e) => NetworkResponse::Error(e.into()),
                }
            }
            
            _ => NetworkResponse::Error(NetworkError::NotImplemented),
        }
    }
}
```

---

## 8. Performance Optimizations

### 8.1 Zero-Copy Receive

```rust
// network/src/stack/zero_copy.rs

pub struct ZeroCopyBuffer {
    /// DMA-capable pages
    pages: Vec<PhysAddr>,
    
    /// Virtual mapping
    virt: VirtAddr,
    
    /// Current write offset
    write_offset: usize,
    
    /// Current read offset
    read_offset: usize,
    
    /// Total capacity
    capacity: usize,
}

impl ZeroCopyBuffer {
    pub fn new(page_count: usize) -> Result<Self, MemoryError> {
        let pages: Vec<_> = (0..page_count)
            .map(|_| kernel_syscall::alloc_dma_page())
            .collect::<Result<_, _>>()?;
        
        let virt = kernel_syscall::map_dma_pages(&pages)?;
        
        Ok(Self {
            pages,
            virt,
            write_offset: 0,
            read_offset: 0,
            capacity: page_count * PAGE_SIZE,
        })
    }
    
    /// Get a slice of received data without copying
    pub fn peek(&self) -> &[u8] {
        let len = self.write_offset - self.read_offset;
        unsafe {
            std::slice::from_raw_parts(
                (self.virt.as_u64() as *const u8).add(self.read_offset),
                len,
            )
        }
    }
    
    /// Consume data from the buffer
    pub fn consume(&mut self, len: usize) {
        self.read_offset += len;
        
        // Reset if empty
        if self.read_offset == self.write_offset {
            self.read_offset = 0;
            self.write_offset = 0;
        }
    }
}
```

### 8.2 Batch Syscalls

```rust
// network/src/service/batch.rs

pub struct BatchedOperations {
    operations: Vec<NetworkRequest>,
    max_batch_size: usize,
}

impl BatchedOperations {
    pub fn add(&mut self, op: NetworkRequest) {
        self.operations.push(op);
    }
    
    pub fn flush(&mut self, service: &mut NetworkService) -> Vec<NetworkResponse> {
        let ops = std::mem::take(&mut self.operations);
        
        // Process all operations
        let responses: Vec<_> = ops.into_iter()
            .map(|op| service.handle_request_sync(op))
            .collect();
        
        // Single poll after batch
        service.stack.poll(Instant::now());
        
        responses
    }
}
```

### 8.3 Connection Pooling

```rust
// network/src/http/pool.rs

pub struct ConnectionPool {
    /// Idle connections by host
    idle: HashMap<String, Vec<PooledConnection>>,
    
    /// Maximum idle connections per host
    max_idle_per_host: usize,
    
    /// Maximum total connections
    max_total: usize,
    
    /// Current connection count
    current_count: usize,
}

struct PooledConnection {
    socket_id: u64,
    created_at: Instant,
    last_used: Instant,
}

impl ConnectionPool {
    pub async fn get_connection(
        &mut self,
        host: &str,
        port: u16,
        service: &mut NetworkService,
    ) -> Result<PooledConnection, PoolError> {
        // Check for idle connection
        if let Some(conns) = self.idle.get_mut(host) {
            while let Some(conn) = conns.pop() {
                // Validate connection is still alive
                if conn.is_valid(service) {
                    return Ok(conn);
                }
            }
        }
        
        // Create new connection
        if self.current_count >= self.max_total {
            return Err(PoolError::PoolExhausted);
        }
        
        let socket_id = service.create_tcp_socket()?;
        let addr = service.dns.resolve(host).await?
            .first()
            .ok_or(PoolError::DnsResolutionFailed)?;
        
        service.connect(socket_id, SocketAddr::new(*addr, port)).await?;
        
        self.current_count += 1;
        
        Ok(PooledConnection {
            socket_id,
            created_at: Instant::now(),
            last_used: Instant::now(),
        })
    }
    
    pub fn return_connection(&mut self, host: &str, conn: PooledConnection) {
        let conns = self.idle.entry(host.to_string()).or_insert_with(Vec::new);
        
        if conns.len() < self.max_idle_per_host {
            conns.push(conn);
        } else {
            // Close excess connection
            self.current_count -= 1;
        }
    }
}
```

---

## 9. Security Considerations

### 9.1 Network Capability Model

```rust
// runtime/src/extensions/network_cap.rs

#[derive(Debug, Clone)]
pub enum NetworkCapability {
    /// Can connect to any address
    ConnectAny,
    
    /// Can connect to specific hosts/ports
    Connect {
        allowed_hosts: Vec<HostPattern>,
        allowed_ports: PortRange,
    },
    
    /// Can listen on any port
    ListenAny,
    
    /// Can listen on specific ports
    Listen {
        allowed_ports: PortRange,
    },
    
    /// Can perform DNS resolution
    DnsResolve,
}

#[derive(Debug, Clone)]
pub enum HostPattern {
    Exact(IpAddr),
    Subnet(IpCidr),
    Domain(String),
    Any,
}

#[derive(Debug, Clone)]
pub struct PortRange {
    pub start: u16,
    pub end: u16,
}

impl NetworkCapability {
    pub fn allows_connect(&self, addr: &IpSocketAddress) -> bool {
        match self {
            Self::ConnectAny => true,
            Self::Connect { allowed_hosts, allowed_ports } => {
                let host_ok = allowed_hosts.iter().any(|h| h.matches(&addr.ip()));
                let port_ok = allowed_ports.contains(addr.port());
                host_ok && port_ok
            }
            _ => false,
        }
    }
    
    pub fn allows_listen(&self, port: u16) -> bool {
        match self {
            Self::ListenAny => true,
            Self::Listen { allowed_ports } => allowed_ports.contains(port),
            _ => false,
        }
    }
}
```

### 9.2 Rate Limiting

```rust
// network/src/service/rate_limit.rs

pub struct RateLimiter {
    /// Connections per second limit
    cps_limit: u32,
    
    /// Bytes per second limit
    bps_limit: u64,
    
    /// Current connection count in window
    current_connections: u32,
    
    /// Current bytes in window
    current_bytes: u64,
    
    /// Window start time
    window_start: Instant,
    
    /// Window duration
    window_duration: Duration,
}

impl RateLimiter {
    pub fn check_connection(&mut self) -> Result<(), RateLimitError> {
        self.maybe_reset_window();
        
        if self.current_connections >= self.cps_limit {
            return Err(RateLimitError::TooManyConnections);
        }
        
        self.current_connections += 1;
        Ok(())
    }
    
    pub fn check_bytes(&mut self, bytes: u64) -> Result<(), RateLimitError> {
        self.maybe_reset_window();
        
        if self.current_bytes + bytes > self.bps_limit {
            return Err(RateLimitError::BandwidthExceeded);
        }
        
        self.current_bytes += bytes;
        Ok(())
    }
    
    fn maybe_reset_window(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.window_start) >= self.window_duration {
            self.current_connections = 0;
            self.current_bytes = 0;
            self.window_start = now;
        }
    }
}
```

---

## 10. HTTP Server Capability

### 10.1 HTTP/1.1 Server

```rust
// network/src/http/server.rs

pub struct HttpServer {
    /// Listening socket
    listener: TcpConnection,
    
    /// Active connections
    connections: Vec<HttpConnection>,
    
    /// Request handler
    handler: Box<dyn HttpHandler>,
    
    /// Configuration
    config: HttpServerConfig,
}

#[derive(Debug, Clone)]
pub struct HttpServerConfig {
    pub max_connections: usize,
    pub max_request_size: usize,
    pub request_timeout: Duration,
    pub keep_alive_timeout: Duration,
}

pub trait HttpHandler: Send {
    fn handle(&self, request: &HttpRequest) -> HttpResponse;
}

impl HttpServer {
    pub fn new(
        addr: SocketAddr,
        handler: impl HttpHandler + 'static,
        config: HttpServerConfig,
    ) -> Result<Self, HttpError> {
        let mut listener = TcpConnection::new(/* ... */);
        listener.bind(addr)?;
        listener.listen(/* ... */)?;
        
        Ok(Self {
            listener,
            connections: Vec::new(),
            handler: Box::new(handler),
            config,
        })
    }
    
    pub async fn run(&mut self, stack: &mut NetworkStack<impl Device>) {
        loop {
            // Accept new connections
            if self.connections.len() < self.config.max_connections {
                if let Some(conn) = self.listener.accept(stack) {
                    self.connections.push(HttpConnection::new(conn));
                }
            }
            
            // Process existing connections
            let mut to_remove = Vec::new();
            
            for (i, conn) in self.connections.iter_mut().enumerate() {
                match conn.process(stack, &*self.handler).await {
                    Ok(ConnectionState::KeepAlive) => {}
                    Ok(ConnectionState::Close) | Err(_) => {
                        to_remove.push(i);
                    }
                }
            }
            
            // Remove closed connections
            for i in to_remove.into_iter().rev() {
                self.connections.remove(i);
            }
            
            // Poll stack
            stack.poll(Instant::now());
        }
    }
}

pub struct HttpConnection {
    socket: TcpConnection,
    state: ParseState,
    buffer: Vec<u8>,
    keep_alive: bool,
}

impl HttpConnection {
    async fn process(
        &mut self,
        stack: &mut NetworkStack<impl Device>,
        handler: &dyn HttpHandler,
    ) -> Result<ConnectionState, HttpError> {
        // Read data into buffer
        let mut chunk = [0u8; 4096];
        let n = self.socket.recv(stack, &mut chunk)?;
        if n == 0 {
            return Ok(ConnectionState::Close);
        }
        self.buffer.extend_from_slice(&chunk[..n]);
        
        // Try to parse request
        if let Some(request) = self.try_parse_request()? {
            // Check keep-alive
            self.keep_alive = request.headers.get("Connection")
                .map(|v| v.eq_ignore_ascii_case("keep-alive"))
                .unwrap_or(false);
            
            // Handle request
            let response = handler.handle(&request);
            
            // Send response
            self.send_response(stack, &response)?;
            
            // Clear buffer
            self.buffer.clear();
            
            if self.keep_alive {
                Ok(ConnectionState::KeepAlive)
            } else {
                Ok(ConnectionState::Close)
            }
        } else {
            // Need more data
            Ok(ConnectionState::KeepAlive)
        }
    }
}
```

### 10.2 HTTP Request/Response

```rust
// network/src/http/request.rs

#[derive(Debug)]
pub struct HttpRequest {
    pub method: Method,
    pub path: String,
    pub version: HttpVersion,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
    Patch,
}

impl HttpRequest {
    pub fn parse(data: &[u8]) -> Result<Option<Self>, ParseError> {
        let mut headers_end = None;
        for i in 0..data.len().saturating_sub(3) {
            if &data[i..i+4] == b"\r\n\r\n" {
                headers_end = Some(i);
                break;
            }
        }
        
        let headers_end = match headers_end {
            Some(i) => i,
            None => return Ok(None), // Need more data
        };
        
        let header_str = std::str::from_utf8(&data[..headers_end])?;
        let mut lines = header_str.lines();
        
        // Parse request line
        let request_line = lines.next().ok_or(ParseError::InvalidRequest)?;
        let mut parts = request_line.split_whitespace();
        
        let method = match parts.next() {
            Some("GET") => Method::Get,
            Some("POST") => Method::Post,
            Some("PUT") => Method::Put,
            Some("DELETE") => Method::Delete,
            Some("HEAD") => Method::Head,
            Some("OPTIONS") => Method::Options,
            Some("PATCH") => Method::Patch,
            _ => return Err(ParseError::InvalidMethod),
        };
        
        let path = parts.next().ok_or(ParseError::InvalidRequest)?.to_string();
        let version = match parts.next() {
            Some("HTTP/1.0") => HttpVersion::Http10,
            Some("HTTP/1.1") => HttpVersion::Http11,
            _ => return Err(ParseError::InvalidVersion),
        };
        
        // Parse headers
        let mut headers = HashMap::new();
        for line in lines {
            if let Some((key, value)) = line.split_once(':') {
                headers.insert(
                    key.trim().to_string(),
                    value.trim().to_string(),
                );
            }
        }
        
        // Get body
        let body_start = headers_end + 4;
        let content_length: usize = headers.get("Content-Length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        
        if data.len() < body_start + content_length {
            return Ok(None); // Need more data
        }
        
        let body = data[body_start..body_start + content_length].to_vec();
        
        Ok(Some(Self {
            method,
            path,
            version,
            headers,
            body,
        }))
    }
}

// network/src/http/response.rs

#[derive(Debug)]
pub struct HttpResponse {
    pub status: StatusCode,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn ok() -> Self {
        Self {
            status: StatusCode::Ok,
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }
    
    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self.headers.insert("Content-Length".to_string(), self.body.len().to_string());
        self
    }
    
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
    
    pub fn serialize(&self) -> Vec<u8> {
        let mut result = format!("HTTP/1.1 {} {}\r\n", 
            self.status.code(), 
            self.status.reason()
        ).into_bytes();
        
        for (key, value) in &self.headers {
            result.extend_from_slice(format!("{}: {}\r\n", key, value).as_bytes());
        }
        
        result.extend_from_slice(b"\r\n");
        result.extend_from_slice(&self.body);
        
        result
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StatusCode {
    Ok = 200,
    Created = 201,
    NoContent = 204,
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    InternalServerError = 500,
}

impl StatusCode {
    pub fn code(&self) -> u16 {
        *self as u16
    }
    
    pub fn reason(&self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Created => "Created",
            Self::NoContent => "No Content",
            Self::BadRequest => "Bad Request",
            Self::Unauthorized => "Unauthorized",
            Self::Forbidden => "Forbidden",
            Self::NotFound => "Not Found",
            Self::InternalServerError => "Internal Server Error",
        }
    }
}
```

---

## Appendix A: Supported Protocols

| Protocol | Status | Notes |
|----------|--------|-------|
| IPv4 | Implemented | Full support |
| IPv6 | Planned | Phase 2 |
| TCP | Implemented | Full state machine |
| UDP | Implemented | Datagram support |
| ICMP | Implemented | Echo request/reply |
| ARP | Implemented | Via smoltcp |
| DHCP | Implemented | Client only |
| DNS | Implemented | A/AAAA queries |
| HTTP/1.1 | Implemented | Basic server |
| HTTP/2 | Planned | Phase 3 |
| TLS | Planned | Via rustls |
| WebSocket | Planned | Phase 3 |

---

## Appendix B: Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| TCP throughput | > 1 Gbps | With VirtIO |
| HTTP requests/sec | > 50,000 | Simple responses |
| Connection latency | < 1ms | Local connections |
| Memory per connection | < 32KB | Including buffers |
| Maximum connections | > 10,000 | Concurrent |
