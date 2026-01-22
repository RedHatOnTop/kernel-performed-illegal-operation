//! Network device drivers.
//!
//! This module contains network device drivers for VirtIO-Net and E1000.

pub mod virtio;
pub mod e1000;

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::{MacAddr, NetworkError};

/// Initialize network drivers.
pub fn init() -> Result<(), NetworkError> {
    // Probe for VirtIO-Net devices
    virtio::probe()?;
    
    // Probe for E1000 devices
    e1000::probe()?;
    
    Ok(())
}

/// Network device trait.
pub trait NetworkDevice: Send + Sync {
    /// Get the MAC address.
    fn mac_address(&self) -> MacAddr;
    
    /// Get the MTU.
    fn mtu(&self) -> u16;
    
    /// Check if link is up.
    fn link_up(&self) -> bool;
    
    /// Transmit a packet.
    fn transmit(&mut self, packet: &[u8]) -> Result<(), NetworkError>;
    
    /// Receive a packet.
    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, NetworkError>;
    
    /// Check if there are packets to receive.
    fn can_receive(&self) -> bool;
    
    /// Get link speed in Mbps.
    fn link_speed(&self) -> u32;
}

/// Packet buffer for network I/O.
pub struct PacketBuffer {
    /// Buffer data.
    data: Vec<u8>,
    /// Valid data length.
    len: usize,
}

impl PacketBuffer {
    /// Create a new packet buffer.
    pub fn new(capacity: usize) -> Self {
        let mut data = Vec::with_capacity(capacity);
        data.resize(capacity, 0);
        PacketBuffer { data, len: 0 }
    }
    
    /// Get the buffer as a slice.
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }
    
    /// Get the buffer as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }
    
    /// Set the valid data length.
    pub fn set_len(&mut self, len: usize) {
        self.len = len.min(self.data.len());
    }
    
    /// Get the valid data length.
    pub fn len(&self) -> usize {
        self.len
    }
    
    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    
    /// Get the capacity.
    pub fn capacity(&self) -> usize {
        self.data.len()
    }
}
