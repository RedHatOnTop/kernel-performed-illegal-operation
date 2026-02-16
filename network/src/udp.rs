//! UDP protocol handling.

use crate::SocketAddr;

/// UDP socket.
pub struct UdpSocket {
    local_addr: Option<SocketAddr>,
}

impl UdpSocket {
    /// Create a new UDP socket.
    pub fn new() -> Self {
        UdpSocket { local_addr: None }
    }

    /// Bind to an address.
    pub fn bind(&mut self, addr: SocketAddr) {
        self.local_addr = Some(addr);
    }

    /// Get the local address.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }
}

impl Default for UdpSocket {
    fn default() -> Self {
        Self::new()
    }
}
