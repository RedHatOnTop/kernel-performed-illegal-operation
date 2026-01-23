//! IPC Message format and handling.
//!
//! This module defines the message structure used for IPC communication.

use alloc::vec::Vec;

/// Message type identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MessageType {
    /// Normal data message.
    Data = 0,
    /// Request message (expects reply).
    Request = 1,
    /// Reply message.
    Reply = 2,
    /// Error message.
    Error = 3,
    /// Capability transfer.
    CapabilityTransfer = 4,
    /// Close notification.
    Close = 5,
    /// Ping (for keepalive).
    Ping = 6,
    /// Pong (reply to ping).
    Pong = 7,
}

/// Message header.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MessageHeader {
    /// Message type.
    pub msg_type: MessageType,
    /// Message flags.
    pub flags: u32,
    /// Sequence number (for request/reply matching).
    pub sequence: u64,
    /// Sender process ID.
    pub sender_pid: u64,
    /// Data length in bytes.
    pub data_len: u32,
    /// Number of capabilities attached.
    pub cap_count: u32,
}

impl MessageHeader {
    /// Create a new data message header.
    pub fn new_data(data_len: usize) -> Self {
        MessageHeader {
            msg_type: MessageType::Data,
            flags: 0,
            sequence: 0,
            sender_pid: 0,
            data_len: data_len as u32,
            cap_count: 0,
        }
    }
    
    /// Create a new request message header.
    pub fn new_request(sequence: u64, data_len: usize) -> Self {
        MessageHeader {
            msg_type: MessageType::Request,
            flags: 0,
            sequence,
            sender_pid: 0,
            data_len: data_len as u32,
            cap_count: 0,
        }
    }
    
    /// Create a new reply message header.
    pub fn new_reply(sequence: u64, data_len: usize) -> Self {
        MessageHeader {
            msg_type: MessageType::Reply,
            flags: 0,
            sequence,
            sender_pid: 0,
            data_len: data_len as u32,
            cap_count: 0,
        }
    }
    
    /// Create a new error message header.
    pub fn new_error(error_code: u32) -> Self {
        MessageHeader {
            msg_type: MessageType::Error,
            flags: error_code,
            sequence: 0,
            sender_pid: 0,
            data_len: 0,
            cap_count: 0,
        }
    }
    
    /// Get the total message size.
    pub fn total_size(&self) -> usize {
        core::mem::size_of::<MessageHeader>() + self.data_len as usize
    }
}

/// Message flags.
pub mod flags {
    /// Message requires acknowledgment.
    pub const ACK_REQUIRED: u32 = 1 << 0;
    /// Message is urgent (high priority).
    pub const URGENT: u32 = 1 << 1;
    /// Message contains inline capabilities.
    pub const HAS_CAPS: u32 = 1 << 2;
    /// Message is part of a larger transfer.
    pub const FRAGMENTED: u32 = 1 << 3;
    /// This is the last fragment.
    pub const LAST_FRAGMENT: u32 = 1 << 4;
}

/// An IPC message.
#[derive(Debug, Clone)]
pub struct Message {
    /// Message header.
    header: MessageHeader,
    /// Message data.
    data: Vec<u8>,
    /// Attached capability IDs.
    capabilities: Vec<u64>,
}

impl Message {
    /// Create a new empty message.
    pub fn new(msg_type: MessageType) -> Self {
        Message {
            header: MessageHeader {
                msg_type,
                flags: 0,
                sequence: 0,
                sender_pid: 0,
                data_len: 0,
                cap_count: 0,
            },
            data: Vec::new(),
            capabilities: Vec::new(),
        }
    }
    
    /// Create a data message.
    pub fn with_data(data: Vec<u8>) -> Self {
        let mut msg = Self::new(MessageType::Data);
        msg.header.data_len = data.len() as u32;
        msg.data = data;
        msg
    }
    
    /// Create a request message.
    pub fn request(sequence: u64, data: Vec<u8>) -> Self {
        let mut msg = Self::new(MessageType::Request);
        msg.header.sequence = sequence;
        msg.header.data_len = data.len() as u32;
        msg.data = data;
        msg
    }
    
    /// Create a reply message.
    pub fn reply(sequence: u64, data: Vec<u8>) -> Self {
        let mut msg = Self::new(MessageType::Reply);
        msg.header.sequence = sequence;
        msg.header.data_len = data.len() as u32;
        msg.data = data;
        msg
    }
    
    /// Create an error message.
    pub fn error(error_code: u32) -> Self {
        let mut msg = Self::new(MessageType::Error);
        msg.header.flags = error_code;
        msg
    }
    
    /// Get the message header.
    pub fn header(&self) -> &MessageHeader {
        &self.header
    }
    
    /// Get mutable access to the header.
    pub fn header_mut(&mut self) -> &mut MessageHeader {
        &mut self.header
    }
    
    /// Get the message data.
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    
    /// Get mutable access to the data.
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }
    
    /// Set the message data.
    pub fn set_data(&mut self, data: Vec<u8>) {
        self.header.data_len = data.len() as u32;
        self.data = data;
    }
    
    /// Get attached capabilities.
    pub fn capabilities(&self) -> &[u64] {
        &self.capabilities
    }
    
    /// Add a capability to the message.
    pub fn add_capability(&mut self, cap_id: u64) {
        self.capabilities.push(cap_id);
        self.header.cap_count = self.capabilities.len() as u32;
    }
    
    /// Get the message type.
    pub fn msg_type(&self) -> MessageType {
        self.header.msg_type
    }
    
    /// Get the sequence number.
    pub fn sequence(&self) -> u64 {
        self.header.sequence
    }
    
    /// Set the sender PID.
    pub fn set_sender(&mut self, pid: u64) {
        self.header.sender_pid = pid;
    }
    
    /// Get the sender PID.
    pub fn sender(&self) -> u64 {
        self.header.sender_pid
    }
    
    /// Get total message size in bytes.
    pub fn total_size(&self) -> usize {
        core::mem::size_of::<MessageHeader>() 
            + self.data.len() 
            + self.capabilities.len() * 8
    }
}
