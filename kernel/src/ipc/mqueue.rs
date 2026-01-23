//! Message Queue IPC
//!
//! This module implements POSIX-like message queues for
//! asynchronous communication between processes.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::{Mutex, RwLock};

/// Message queue ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MqId(pub u64);

impl MqId {
    /// Invalid message queue ID.
    pub const INVALID: MqId = MqId(0);
}

/// Message priority (0 = lowest, 31 = highest).
pub type Priority = u8;

/// Maximum priority level.
pub const MAX_PRIORITY: Priority = 31;

/// Default maximum messages in a queue.
pub const DEFAULT_MAX_MESSAGES: usize = 64;

/// Default maximum message size.
pub const DEFAULT_MAX_MSG_SIZE: usize = 8192;

/// Message queue attributes.
#[derive(Debug, Clone)]
pub struct MqAttr {
    /// Maximum number of messages.
    pub max_messages: usize,
    /// Maximum message size.
    pub max_msg_size: usize,
    /// Current number of messages.
    pub cur_messages: usize,
    /// Flags (non-blocking, etc.).
    pub flags: u32,
}

impl Default for MqAttr {
    fn default() -> Self {
        MqAttr {
            max_messages: DEFAULT_MAX_MESSAGES,
            max_msg_size: DEFAULT_MAX_MSG_SIZE,
            cur_messages: 0,
            flags: 0,
        }
    }
}

/// A message in the queue.
#[derive(Debug, Clone)]
pub struct QueueMessage {
    /// Message priority.
    pub priority: Priority,
    /// Message data.
    pub data: Vec<u8>,
    /// Sender process ID.
    pub sender_pid: u64,
    /// Timestamp (ticks since boot).
    pub timestamp: u64,
}

impl QueueMessage {
    /// Create a new message.
    pub fn new(priority: Priority, data: Vec<u8>, sender_pid: u64) -> Self {
        QueueMessage {
            priority,
            data,
            sender_pid,
            timestamp: 0, // TODO: Get actual timestamp
        }
    }
}

/// Message queue state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MqState {
    /// Queue is open.
    Open,
    /// Queue is being destroyed.
    Closing,
    /// Queue is closed.
    Closed,
}

/// A message queue.
pub struct MessageQueue {
    /// Unique ID.
    id: MqId,
    
    /// Queue name.
    name: String,
    
    /// Queue attributes.
    attr: MqAttr,
    
    /// Current state.
    state: MqState,
    
    /// Messages organized by priority.
    /// Higher index = higher priority.
    priority_queues: [VecDeque<QueueMessage>; 32],
    
    /// Total message count.
    message_count: usize,
    
    /// Creator process ID.
    creator: u64,
    
    /// Processes waiting to send (blocked on full queue).
    send_waiters: Vec<u64>,
    
    /// Processes waiting to receive (blocked on empty queue).
    recv_waiters: Vec<u64>,
}

impl MessageQueue {
    /// Create a new message queue.
    pub fn new(id: MqId, name: &str, creator: u64, attr: MqAttr) -> Self {
        const EMPTY_QUEUE: VecDeque<QueueMessage> = VecDeque::new();
        
        MessageQueue {
            id,
            name: String::from(name),
            attr,
            state: MqState::Open,
            priority_queues: [EMPTY_QUEUE; 32],
            message_count: 0,
            creator,
            send_waiters: Vec::new(),
            recv_waiters: Vec::new(),
        }
    }
    
    /// Get queue ID.
    pub fn id(&self) -> MqId {
        self.id
    }
    
    /// Get queue name.
    pub fn name(&self) -> &str {
        &self.name
    }
    
    /// Get queue attributes.
    pub fn attr(&self) -> MqAttr {
        let mut attr = self.attr.clone();
        attr.cur_messages = self.message_count;
        attr
    }
    
    /// Check if queue is full.
    pub fn is_full(&self) -> bool {
        self.message_count >= self.attr.max_messages
    }
    
    /// Check if queue is empty.
    pub fn is_empty(&self) -> bool {
        self.message_count == 0
    }
    
    /// Send a message.
    pub fn send(&mut self, msg: QueueMessage) -> Result<(), MqError> {
        if self.state != MqState::Open {
            return Err(MqError::QueueClosed);
        }
        
        if msg.data.len() > self.attr.max_msg_size {
            return Err(MqError::MessageTooLarge);
        }
        
        if self.is_full() {
            return Err(MqError::QueueFull);
        }
        
        let priority = (msg.priority as usize).min(MAX_PRIORITY as usize);
        self.priority_queues[priority].push_back(msg);
        self.message_count += 1;
        
        // TODO: Wake up waiting receivers
        
        Ok(())
    }
    
    /// Receive a message (highest priority first).
    pub fn receive(&mut self) -> Result<QueueMessage, MqError> {
        if self.state != MqState::Open {
            return Err(MqError::QueueClosed);
        }
        
        if self.is_empty() {
            return Err(MqError::QueueEmpty);
        }
        
        // Find highest priority non-empty queue
        for priority in (0..=MAX_PRIORITY as usize).rev() {
            if let Some(msg) = self.priority_queues[priority].pop_front() {
                self.message_count -= 1;
                // TODO: Wake up waiting senders
                return Ok(msg);
            }
        }
        
        Err(MqError::QueueEmpty)
    }
    
    /// Peek at the next message without removing it.
    pub fn peek(&self) -> Option<&QueueMessage> {
        for priority in (0..=MAX_PRIORITY as usize).rev() {
            if let Some(msg) = self.priority_queues[priority].front() {
                return Some(msg);
            }
        }
        None
    }
    
    /// Add a send waiter.
    pub fn add_send_waiter(&mut self, pid: u64) {
        self.send_waiters.push(pid);
    }
    
    /// Add a receive waiter.
    pub fn add_recv_waiter(&mut self, pid: u64) {
        self.recv_waiters.push(pid);
    }
    
    /// Remove a send waiter.
    pub fn remove_send_waiter(&mut self, pid: u64) {
        self.send_waiters.retain(|&p| p != pid);
    }
    
    /// Remove a receive waiter.
    pub fn remove_recv_waiter(&mut self, pid: u64) {
        self.recv_waiters.retain(|&p| p != pid);
    }
    
    /// Close the queue.
    pub fn close(&mut self) {
        self.state = MqState::Closed;
    }
}

/// Message queue error types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MqError {
    /// Queue not found.
    NotFound,
    /// Queue is full.
    QueueFull,
    /// Queue is empty.
    QueueEmpty,
    /// Queue is closed.
    QueueClosed,
    /// Message too large.
    MessageTooLarge,
    /// Name already exists.
    AlreadyExists,
    /// Permission denied.
    PermissionDenied,
    /// Invalid argument.
    InvalidArgument,
    /// Limit reached.
    LimitReached,
}

/// Global message queue manager.
pub struct MessageQueueManager {
    /// Named queues.
    named_queues: BTreeMap<String, MqId>,
    
    /// All queues by ID.
    queues: BTreeMap<MqId, Arc<Mutex<MessageQueue>>>,
    
    /// Next queue ID.
    next_id: AtomicU64,
    
    /// Maximum queues.
    max_queues: usize,
}

impl MessageQueueManager {
    /// Create a new manager.
    pub fn new() -> Self {
        MessageQueueManager {
            named_queues: BTreeMap::new(),
            queues: BTreeMap::new(),
            next_id: AtomicU64::new(1),
            max_queues: 1024,
        }
    }
    
    /// Create or open a message queue.
    pub fn open(
        &mut self,
        name: &str,
        creator: u64,
        create: bool,
        exclusive: bool,
        attr: Option<MqAttr>,
    ) -> Result<MqId, MqError> {
        // Check if queue exists
        if let Some(&id) = self.named_queues.get(name) {
            if exclusive && create {
                return Err(MqError::AlreadyExists);
            }
            return Ok(id);
        }
        
        // Must create if doesn't exist
        if !create {
            return Err(MqError::NotFound);
        }
        
        // Check limits
        if self.queues.len() >= self.max_queues {
            return Err(MqError::LimitReached);
        }
        
        // Create new queue
        let id = MqId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let queue = MessageQueue::new(id, name, creator, attr.unwrap_or_default());
        
        self.named_queues.insert(String::from(name), id);
        self.queues.insert(id, Arc::new(Mutex::new(queue)));
        
        Ok(id)
    }
    
    /// Get a queue by ID.
    pub fn get(&self, id: MqId) -> Option<Arc<Mutex<MessageQueue>>> {
        self.queues.get(&id).cloned()
    }
    
    /// Get a queue by name.
    pub fn get_by_name(&self, name: &str) -> Option<Arc<Mutex<MessageQueue>>> {
        let id = self.named_queues.get(name)?;
        self.queues.get(id).cloned()
    }
    
    /// Close and remove a queue.
    pub fn close(&mut self, name: &str, pid: u64) -> Result<(), MqError> {
        let id = *self.named_queues.get(name).ok_or(MqError::NotFound)?;
        
        {
            let queue = self.queues.get(&id).ok_or(MqError::NotFound)?;
            let mut q = queue.lock();
            
            // Only creator can unlink
            if q.creator != pid {
                return Err(MqError::PermissionDenied);
            }
            
            q.close();
        }
        
        self.named_queues.remove(name);
        self.queues.remove(&id);
        
        Ok(())
    }
    
    /// Send to a queue.
    pub fn send(&self, id: MqId, msg: QueueMessage) -> Result<(), MqError> {
        let queue = self.queues.get(&id).ok_or(MqError::NotFound)?;
        queue.lock().send(msg)
    }
    
    /// Receive from a queue.
    pub fn receive(&self, id: MqId) -> Result<QueueMessage, MqError> {
        let queue = self.queues.get(&id).ok_or(MqError::NotFound)?;
        queue.lock().receive()
    }
}

/// Global message queue manager.
static MQ_MANAGER: RwLock<Option<MessageQueueManager>> = RwLock::new(None);

/// Initialize message queue subsystem.
pub fn init() {
    let mut manager = MQ_MANAGER.write();
    *manager = Some(MessageQueueManager::new());
}

/// Open or create a message queue.
pub fn open(
    name: &str,
    creator: u64,
    create: bool,
    exclusive: bool,
    attr: Option<MqAttr>,
) -> Result<MqId, MqError> {
    MQ_MANAGER.write()
        .as_mut()
        .ok_or(MqError::NotFound)?
        .open(name, creator, create, exclusive, attr)
}

/// Send to a message queue.
pub fn send(id: MqId, msg: QueueMessage) -> Result<(), MqError> {
    MQ_MANAGER.read()
        .as_ref()
        .ok_or(MqError::NotFound)?
        .send(id, msg)
}

/// Receive from a message queue.
pub fn receive(id: MqId) -> Result<QueueMessage, MqError> {
    MQ_MANAGER.read()
        .as_ref()
        .ok_or(MqError::NotFound)?
        .receive(id)
}

/// Close a message queue.
pub fn close(name: &str, pid: u64) -> Result<(), MqError> {
    MQ_MANAGER.write()
        .as_mut()
        .ok_or(MqError::NotFound)?
        .close(name, pid)
}
