//! IPC (Inter-Process Communication) for userspace.
//!
//! This module provides IPC channels for communication between processes.

use crate::syscall::{syscall0, syscall1, syscall3, SyscallError, SyscallNumber, SyscallResult};

/// IPC channel handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channel {
    id: u64,
}

impl Channel {
    /// Create a Channel from a raw ID.
    pub const fn from_raw(id: u64) -> Self {
        Self { id }
    }

    /// Get the raw channel ID.
    pub const fn raw(&self) -> u64 {
        self.id
    }

    /// Send data through the channel.
    pub fn send(&self, data: &[u8]) -> SyscallResult {
        unsafe {
            syscall3(
                SyscallNumber::ChannelSend,
                self.id,
                data.as_ptr() as u64,
                data.len() as u64,
            )
        }
    }

    /// Receive data from the channel.
    ///
    /// Returns the number of bytes received.
    pub fn recv(&self, buf: &mut [u8]) -> SyscallResult {
        unsafe {
            syscall3(
                SyscallNumber::ChannelRecv,
                self.id,
                buf.as_mut_ptr() as u64,
                buf.len() as u64,
            )
        }
    }

    /// Close this channel endpoint.
    pub fn close(self) -> SyscallResult {
        unsafe { syscall1(SyscallNumber::ChannelClose, self.id) }
    }
}

/// Create a pair of connected channels.
///
/// Returns `(channel_a, channel_b)` - two connected endpoints.
/// Data sent on one channel can be received on the other.
pub fn channel_pair() -> Result<(Channel, Channel), SyscallError> {
    let result = unsafe { syscall0(SyscallNumber::ChannelCreate)? };

    // IDs are packed into a single u64
    let id_a = (result >> 32) as u64;
    let id_b = (result & 0xFFFF_FFFF) as u64;

    Ok((Channel::from_raw(id_a), Channel::from_raw(id_b)))
}

/// Message types for structured IPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    /// Raw data message.
    Data = 0,
    /// Request message (expects response).
    Request = 1,
    /// Response to a request.
    Response = 2,
    /// Error response.
    Error = 3,
    /// File descriptor transfer.
    FdTransfer = 4,
    /// Shared memory handle.
    ShmHandle = 5,
}

/// Message header for structured IPC.
#[repr(C)]
pub struct MessageHeader {
    /// Message type.
    pub msg_type: MessageType,
    /// Message flags.
    pub flags: u8,
    /// Sequence number for request/response matching.
    pub seq: u16,
    /// Payload length.
    pub length: u32,
}

impl MessageHeader {
    /// Create a new data message header.
    pub const fn data(length: u32) -> Self {
        Self {
            msg_type: MessageType::Data,
            flags: 0,
            seq: 0,
            length,
        }
    }

    /// Create a new request message header.
    pub const fn request(seq: u16, length: u32) -> Self {
        Self {
            msg_type: MessageType::Request,
            flags: 0,
            seq,
            length,
        }
    }

    /// Create a new response message header.
    pub const fn response(seq: u16, length: u32) -> Self {
        Self {
            msg_type: MessageType::Response,
            flags: 0,
            seq,
            length,
        }
    }

    /// Size of the header in bytes.
    pub const fn size() -> usize {
        core::mem::size_of::<Self>()
    }
}

/// High-level RPC-style channel.
pub struct RpcChannel {
    channel: Channel,
    next_seq: u16,
}

impl RpcChannel {
    /// Create from a raw channel.
    pub const fn new(channel: Channel) -> Self {
        Self {
            channel,
            next_seq: 0,
        }
    }

    /// Send a request and wait for response.
    pub fn call(&mut self, request: &[u8], response_buf: &mut [u8]) -> Result<usize, SyscallError> {
        // Build request with header
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);

        let header = MessageHeader::request(seq, request.len() as u32);
        let header_bytes = unsafe {
            core::slice::from_raw_parts(&header as *const _ as *const u8, MessageHeader::size())
        };

        // Send header + data
        // TODO: Use scatter-gather for efficiency
        let mut msg = [0u8; 4096];
        let total_len = MessageHeader::size() + request.len();
        if total_len > msg.len() {
            return Err(SyscallError::InvalidArgument);
        }

        msg[..MessageHeader::size()].copy_from_slice(header_bytes);
        msg[MessageHeader::size()..total_len].copy_from_slice(request);

        self.channel.send(&msg[..total_len])?;

        // Wait for response
        loop {
            match self.channel.recv(response_buf) {
                Ok(len) => {
                    if len >= MessageHeader::size() as u64 {
                        return Ok((len as usize) - MessageHeader::size());
                    }
                }
                Err(SyscallError::WouldBlock) => {
                    // Yield and retry
                    crate::process::yield_now();
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Get the underlying channel.
    pub fn into_channel(self) -> Channel {
        self.channel
    }
}
