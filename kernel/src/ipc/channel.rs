//! IPC Channel implementation.
//!
//! This module defines the channel structure for point-to-point
//! communication between processes.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use super::message::Message;

/// Unique channel identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelId(pub u64);

/// Channel state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelState {
    /// Channel is open and can send/receive.
    Open,
    /// Channel is closed (no more messages).
    Closed,
}

/// A communication channel.
pub struct Channel {
    /// This channel's ID.
    id: ChannelId,

    /// Peer channel's ID.
    peer_id: ChannelId,

    /// Channel state.
    state: ChannelState,

    /// Message queue.
    queue: VecDeque<Message>,

    /// Waiting tasks (blocked on receive).
    waiters: Vec<u64>,

    /// Flow control: max outstanding messages.
    flow_control_limit: usize,
}

impl Channel {
    /// Create a new channel.
    pub fn new(id: ChannelId, peer_id: ChannelId) -> Self {
        Channel {
            id,
            peer_id,
            state: ChannelState::Open,
            queue: VecDeque::new(),
            waiters: Vec::new(),
            flow_control_limit: super::MAX_QUEUE_DEPTH,
        }
    }

    /// Get this channel's ID.
    pub fn id(&self) -> ChannelId {
        self.id
    }

    /// Get the peer channel's ID.
    pub fn peer_id(&self) -> ChannelId {
        self.peer_id
    }

    /// Check if the channel is open.
    pub fn is_open(&self) -> bool {
        self.state == ChannelState::Open
    }

    /// Check if the channel is closed.
    pub fn is_closed(&self) -> bool {
        self.state == ChannelState::Closed
    }

    /// Close the channel.
    pub fn close(&mut self) {
        self.state = ChannelState::Closed;
    }

    /// Enqueue a message.
    pub fn enqueue(&mut self, message: Message) {
        self.queue.push_back(message);

        // Wake up any waiting tasks
        // (would integrate with scheduler here)
    }

    /// Dequeue a message.
    pub fn dequeue(&mut self) -> Option<Message> {
        self.queue.pop_front()
    }

    /// Peek at the next message without removing it.
    pub fn peek(&self) -> Option<&Message> {
        self.queue.front()
    }

    /// Get the number of messages in the queue.
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Check if the queue is full.
    pub fn is_full(&self) -> bool {
        self.queue.len() >= self.flow_control_limit
    }

    /// Add a waiter task.
    pub fn add_waiter(&mut self, task_id: u64) {
        self.waiters.push(task_id);
    }

    /// Remove a waiter task.
    pub fn remove_waiter(&mut self, task_id: u64) {
        self.waiters.retain(|&id| id != task_id);
    }

    /// Get all waiting tasks.
    pub fn waiters(&self) -> &[u64] {
        &self.waiters
    }

    /// Set the flow control limit.
    pub fn set_flow_control_limit(&mut self, limit: usize) {
        self.flow_control_limit = limit;
    }
}
