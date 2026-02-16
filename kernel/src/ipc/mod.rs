//! Inter-Process Communication (IPC) module.
//!
//! This module implements the capability-based IPC system for
//! communication between WASM processes and kernel services.

pub mod capability;
pub mod channel;
pub mod message;
pub mod mqueue;
pub mod services;
pub mod shm;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::{Mutex, RwLock};

pub use capability::{Capability, CapabilityId, CapabilityRights, CapabilityType};
pub use channel::{Channel, ChannelId};
pub use message::{Message, MessageHeader};
pub use mqueue::{MessageQueue, MqError, MqId};
pub use services::{ServiceConnection, ServiceError, ServiceId, ServiceInfo, ServiceRegistry};
pub use shm::{SharedMemoryRegion, ShmError, ShmId};

/// Maximum message size in bytes.
pub const MAX_MESSAGE_SIZE: usize = 64 * 1024; // 64 KB

/// Maximum messages in a channel queue.
pub const MAX_QUEUE_DEPTH: usize = 256;

/// Global IPC registry.
static IPC_REGISTRY: RwLock<Option<IpcRegistry>> = RwLock::new(None);

/// Initialize the IPC subsystem.
pub fn init() {
    let mut registry = IPC_REGISTRY.write();
    *registry = Some(IpcRegistry::new());
}

/// Create a new channel.
pub fn create_channel() -> Option<(ChannelId, ChannelId)> {
    IPC_REGISTRY.write().as_mut()?.create_channel_pair()
}

/// Send a message through a channel.
pub fn send(channel_id: ChannelId, message: Message) -> Result<(), IpcError> {
    IPC_REGISTRY
        .read()
        .as_ref()
        .ok_or(IpcError::NotInitialized)?
        .send(channel_id, message)
}

/// Receive a message from a channel.
pub fn receive(channel_id: ChannelId) -> Result<Message, IpcError> {
    IPC_REGISTRY
        .read()
        .as_ref()
        .ok_or(IpcError::NotInitialized)?
        .receive(channel_id)
}

/// IPC error types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    /// IPC subsystem not initialized.
    NotInitialized,
    /// Channel not found.
    ChannelNotFound,
    /// Channel is closed.
    ChannelClosed,
    /// Message queue is full.
    QueueFull,
    /// Message queue is empty.
    QueueEmpty,
    /// Message too large.
    MessageTooLarge,
    /// Invalid capability.
    InvalidCapability,
    /// Permission denied.
    PermissionDenied,
    /// Would block (non-blocking mode).
    WouldBlock,
}

/// IPC registry for managing channels and capabilities.
pub struct IpcRegistry {
    /// All channels indexed by ID.
    channels: BTreeMap<ChannelId, Arc<Mutex<Channel>>>,

    /// All capabilities indexed by ID.
    capabilities: BTreeMap<CapabilityId, Capability>,

    /// Next channel ID.
    next_channel_id: AtomicU64,

    /// Next capability ID.
    next_capability_id: AtomicU64,
}

impl IpcRegistry {
    /// Create a new IPC registry.
    pub fn new() -> Self {
        IpcRegistry {
            channels: BTreeMap::new(),
            capabilities: BTreeMap::new(),
            next_channel_id: AtomicU64::new(1),
            next_capability_id: AtomicU64::new(1),
        }
    }

    /// Create a bidirectional channel pair.
    pub fn create_channel_pair(&mut self) -> Option<(ChannelId, ChannelId)> {
        let id_a = ChannelId(self.next_channel_id.fetch_add(1, Ordering::Relaxed));
        let id_b = ChannelId(self.next_channel_id.fetch_add(1, Ordering::Relaxed));

        let channel_a = Channel::new(id_a, id_b);
        let channel_b = Channel::new(id_b, id_a);

        self.channels.insert(id_a, Arc::new(Mutex::new(channel_a)));
        self.channels.insert(id_b, Arc::new(Mutex::new(channel_b)));

        Some((id_a, id_b))
    }

    /// Send a message through a channel.
    pub fn send(&self, channel_id: ChannelId, message: Message) -> Result<(), IpcError> {
        if message.data().len() > MAX_MESSAGE_SIZE {
            return Err(IpcError::MessageTooLarge);
        }

        let channel = self
            .channels
            .get(&channel_id)
            .ok_or(IpcError::ChannelNotFound)?;

        let mut chan = channel.lock();

        if chan.is_closed() {
            return Err(IpcError::ChannelClosed);
        }

        if chan.queue_len() >= MAX_QUEUE_DEPTH {
            return Err(IpcError::QueueFull);
        }

        // Find peer channel and enqueue message
        let peer_id = chan.peer_id();
        drop(chan);

        let peer = self
            .channels
            .get(&peer_id)
            .ok_or(IpcError::ChannelNotFound)?;

        peer.lock().enqueue(message);

        Ok(())
    }

    /// Receive a message from a channel.
    pub fn receive(&self, channel_id: ChannelId) -> Result<Message, IpcError> {
        let channel = self
            .channels
            .get(&channel_id)
            .ok_or(IpcError::ChannelNotFound)?;

        let mut chan = channel.lock();

        chan.dequeue().ok_or(IpcError::QueueEmpty)
    }

    /// Close a channel.
    pub fn close_channel(&mut self, channel_id: ChannelId) -> Result<(), IpcError> {
        let channel = self
            .channels
            .get(&channel_id)
            .ok_or(IpcError::ChannelNotFound)?;

        channel.lock().close();

        Ok(())
    }

    /// Create a new capability.
    pub fn create_capability(&mut self, cap_type: CapabilityType) -> CapabilityId {
        let id = CapabilityId(self.next_capability_id.fetch_add(1, Ordering::Relaxed));
        let cap = Capability::new(id, cap_type);
        self.capabilities.insert(id, cap);
        id
    }

    /// Validate a capability.
    pub fn validate_capability(&self, id: CapabilityId) -> Result<&Capability, IpcError> {
        self.capabilities
            .get(&id)
            .ok_or(IpcError::InvalidCapability)
    }

    /// Revoke a capability.
    pub fn revoke_capability(&mut self, id: CapabilityId) -> Result<(), IpcError> {
        self.capabilities
            .remove(&id)
            .ok_or(IpcError::InvalidCapability)?;
        Ok(())
    }
}
