//! IPC abstraction layer for KPIO
//!
//! This module provides inter-process communication primitives for
//! communicating with kernel services.

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;

use crate::error::{IpcError, PlatformError, Result};

/// Initialize IPC subsystem
pub fn init() {
    log::debug!("[KPIO IPC] Initializing IPC subsystem");
}

/// Channel endpoint ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelId(pub u64);

static NEXT_CHANNEL_ID: AtomicU64 = AtomicU64::new(1);

/// Service channel for communicating with kernel services
pub struct ServiceChannel {
    id: ChannelId,
    connected: AtomicBool,
}

impl ServiceChannel {
    /// Connect to a named kernel service
    pub fn connect(service_name: &str) -> Result<Self> {
        let id = ChannelId(NEXT_CHANNEL_ID.fetch_add(1, Ordering::Relaxed));
        
        // In real implementation:
        // 1. Send connect request via syscall
        // 2. Kernel looks up service by name
        // 3. Creates channel pair and returns our endpoint
        
        log::debug!("[KPIO IPC] Connecting to service: {}", service_name);
        
        Ok(ServiceChannel {
            id,
            connected: AtomicBool::new(true),
        })
    }
    
    /// Send a message
    pub fn send(&self, data: &[u8]) -> Result<()> {
        if !self.connected.load(Ordering::Acquire) {
            return Err(PlatformError::Ipc(IpcError::NotConnected));
        }
        
        // In real implementation: syscall to send message
        Ok(())
    }
    
    /// Receive a message (blocking)
    pub fn recv(&self, buf: &mut [u8]) -> Result<usize> {
        if !self.connected.load(Ordering::Acquire) {
            return Err(PlatformError::Ipc(IpcError::NotConnected));
        }
        
        // In real implementation: syscall to receive message
        Ok(0)
    }
    
    /// Try to receive a message (non-blocking)
    pub fn try_recv(&self, buf: &mut [u8]) -> Result<Option<usize>> {
        if !self.connected.load(Ordering::Acquire) {
            return Err(PlatformError::Ipc(IpcError::NotConnected));
        }
        
        // In real implementation: non-blocking syscall
        Ok(None)
    }
    
    /// Close the channel
    pub fn close(&self) {
        self.connected.store(false, Ordering::Release);
        // In real implementation: syscall to close channel
    }
    
    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
    
    /// Get channel ID
    pub fn id(&self) -> ChannelId {
        self.id
    }
}

impl Drop for ServiceChannel {
    fn drop(&mut self) {
        self.close();
    }
}

/// Bidirectional channel pair for local IPC
pub struct Channel<T> {
    sender: ChannelSender<T>,
    receiver: ChannelReceiver<T>,
}

impl<T> Channel<T> {
    /// Create a new channel pair
    pub fn new() -> Self {
        let buffer = Arc::new(Mutex::new(VecDeque::new()));
        let closed = Arc::new(AtomicBool::new(false));
        
        Channel {
            sender: ChannelSender {
                buffer: buffer.clone(),
                closed: closed.clone(),
            },
            receiver: ChannelReceiver {
                buffer,
                closed,
            },
        }
    }
    
    /// Split into sender and receiver
    pub fn split(self) -> (ChannelSender<T>, ChannelReceiver<T>) {
        (self.sender, self.receiver)
    }
}

impl<T> Default for Channel<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Channel sender
pub struct ChannelSender<T> {
    buffer: Arc<Mutex<VecDeque<T>>>,
    closed: Arc<AtomicBool>,
}

impl<T> ChannelSender<T> {
    /// Send a value
    pub fn send(&self, value: T) -> Result<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(PlatformError::Ipc(IpcError::NotConnected));
        }
        
        self.buffer.lock().push_back(value);
        Ok(())
    }
    
    /// Close the sender
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }
}

impl<T> Clone for ChannelSender<T> {
    fn clone(&self) -> Self {
        ChannelSender {
            buffer: self.buffer.clone(),
            closed: self.closed.clone(),
        }
    }
}

/// Channel receiver
pub struct ChannelReceiver<T> {
    buffer: Arc<Mutex<VecDeque<T>>>,
    closed: Arc<AtomicBool>,
}

impl<T> ChannelReceiver<T> {
    /// Receive a value (blocking)
    pub fn recv(&self) -> Result<T> {
        loop {
            if let Some(value) = self.buffer.lock().pop_front() {
                return Ok(value);
            }
            
            if self.closed.load(Ordering::Acquire) {
                return Err(PlatformError::Ipc(IpcError::NotConnected));
            }
            
            crate::thread::yield_now();
        }
    }
    
    /// Try to receive a value (non-blocking)
    pub fn try_recv(&self) -> Result<Option<T>> {
        if let Some(value) = self.buffer.lock().pop_front() {
            return Ok(Some(value));
        }
        
        if self.closed.load(Ordering::Acquire) {
            return Err(PlatformError::Ipc(IpcError::NotConnected));
        }
        
        Ok(None)
    }
    
    /// Check if channel is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.lock().is_empty()
    }
}

/// Shared memory region
pub struct SharedMemory {
    id: u64,
    ptr: *mut u8,
    size: usize,
}

unsafe impl Send for SharedMemory {}
unsafe impl Sync for SharedMemory {}

static NEXT_SHM_ID: AtomicU64 = AtomicU64::new(1);

impl SharedMemory {
    /// Create a new shared memory region
    pub fn create(size: usize) -> Result<Self> {
        let id = NEXT_SHM_ID.fetch_add(1, Ordering::Relaxed);
        
        // In real implementation: syscall to allocate shared memory
        
        Ok(SharedMemory {
            id,
            ptr: core::ptr::null_mut(),
            size,
        })
    }
    
    /// Open an existing shared memory region by ID
    pub fn open(id: u64) -> Result<Self> {
        // In real implementation: syscall to map shared memory
        
        Ok(SharedMemory {
            id,
            ptr: core::ptr::null_mut(),
            size: 0,
        })
    }
    
    /// Get the shared memory ID
    pub fn id(&self) -> u64 {
        self.id
    }
    
    /// Get the size of the shared memory
    pub fn size(&self) -> usize {
        self.size
    }
    
    /// Get a pointer to the shared memory
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }
    
    /// Get a mutable pointer to the shared memory
    pub fn as_mut_ptr(&self) -> *mut u8 {
        self.ptr
    }
    
    /// Get a slice view of the shared memory
    pub fn as_slice(&self) -> &[u8] {
        if self.ptr.is_null() {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(self.ptr, self.size) }
        }
    }
    
    /// Get a mutable slice view of the shared memory
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        if self.ptr.is_null() {
            &mut []
        } else {
            unsafe { core::slice::from_raw_parts_mut(self.ptr, self.size) }
        }
    }
}

impl Drop for SharedMemory {
    fn drop(&mut self) {
        // In real implementation: syscall to unmap and release
    }
}

/// Message queue for one-to-many communication
pub struct MessageQueue {
    id: u64,
    max_messages: usize,
    max_message_size: usize,
}

static NEXT_MQ_ID: AtomicU64 = AtomicU64::new(1);

impl MessageQueue {
    /// Create a new message queue
    pub fn create(max_messages: usize, max_message_size: usize) -> Result<Self> {
        let id = NEXT_MQ_ID.fetch_add(1, Ordering::Relaxed);
        
        Ok(MessageQueue {
            id,
            max_messages,
            max_message_size,
        })
    }
    
    /// Open an existing message queue
    pub fn open(id: u64) -> Result<Self> {
        Ok(MessageQueue {
            id,
            max_messages: 0,
            max_message_size: 0,
        })
    }
    
    /// Send a message
    pub fn send(&self, data: &[u8], priority: u32) -> Result<()> {
        if data.len() > self.max_message_size {
            return Err(PlatformError::Ipc(IpcError::MessageTooLarge));
        }
        
        // In real implementation: syscall to send message
        Ok(())
    }
    
    /// Receive a message (blocking)
    pub fn recv(&self, buf: &mut [u8]) -> Result<(usize, u32)> {
        // In real implementation: syscall to receive message
        // Returns (message_size, priority)
        Ok((0, 0))
    }
    
    /// Try to receive a message (non-blocking)
    pub fn try_recv(&self, buf: &mut [u8]) -> Result<Option<(usize, u32)>> {
        // In real implementation: non-blocking syscall
        Ok(None)
    }
    
    /// Get queue ID
    pub fn id(&self) -> u64 {
        self.id
    }
}

/// RPC client for remote procedure calls
pub struct RpcClient {
    channel: ServiceChannel,
    request_id: AtomicU64,
}

impl RpcClient {
    /// Create a new RPC client connected to a service
    pub fn new(service_name: &str) -> Result<Self> {
        let channel = ServiceChannel::connect(service_name)?;
        
        Ok(RpcClient {
            channel,
            request_id: AtomicU64::new(1),
        })
    }
    
    /// Make an RPC call
    pub fn call(&self, method: &str, params: &[u8]) -> Result<Vec<u8>> {
        let _request_id = self.request_id.fetch_add(1, Ordering::Relaxed);
        
        // In real implementation:
        // 1. Serialize request with method name, id, and params
        // 2. Send via channel
        // 3. Wait for response with matching id
        // 4. Return result
        
        self.channel.send(params)?;
        
        let mut buf = [0u8; 4096];
        let len = self.channel.recv(&mut buf)?;
        
        Ok(buf[..len].to_vec())
    }
}
