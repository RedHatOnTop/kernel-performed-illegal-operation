//! Ring Buffer Implementation
//!
//! Implements submission and completion rings for async I/O operations,
//! inspired by Linux's io_uring design.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

use super::operations::OpCode;

/// Submission queue entry.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SqEntry {
    /// Operation code.
    pub opcode: OpCode,
    /// Flags.
    pub flags: SqFlags,
    /// I/O priority.
    pub ioprio: u16,
    /// File descriptor.
    pub fd: i32,
    /// Offset in file.
    pub offset: u64,
    /// Buffer address.
    pub addr: u64,
    /// Buffer length.
    pub len: u32,
    /// Operation-specific flags.
    pub op_flags: u32,
    /// User data (passed through to completion).
    pub user_data: u64,
    /// Buffer index for fixed buffers.
    pub buf_index: u16,
    /// Personality (credentials).
    pub personality: u16,
    /// Splice file descriptor in.
    pub splice_fd_in: i32,
    /// Reserved.
    _reserved: [u64; 2],
}

impl SqEntry {
    /// Create a new submission entry.
    pub const fn new(opcode: OpCode, fd: i32, user_data: u64) -> Self {
        Self {
            opcode,
            flags: SqFlags::empty(),
            ioprio: 0,
            fd,
            offset: 0,
            addr: 0,
            len: 0,
            op_flags: 0,
            user_data,
            buf_index: 0,
            personality: 0,
            splice_fd_in: -1,
            _reserved: [0; 2],
        }
    }
    
    /// Create a read operation.
    pub fn read(fd: i32, buf: u64, len: u32, offset: u64, user_data: u64) -> Self {
        Self {
            opcode: OpCode::Read,
            flags: SqFlags::empty(),
            ioprio: 0,
            fd,
            offset,
            addr: buf,
            len,
            op_flags: 0,
            user_data,
            buf_index: 0,
            personality: 0,
            splice_fd_in: -1,
            _reserved: [0; 2],
        }
    }
    
    /// Create a write operation.
    pub fn write(fd: i32, buf: u64, len: u32, offset: u64, user_data: u64) -> Self {
        Self {
            opcode: OpCode::Write,
            flags: SqFlags::empty(),
            ioprio: 0,
            fd,
            offset,
            addr: buf,
            len,
            op_flags: 0,
            user_data,
            buf_index: 0,
            personality: 0,
            splice_fd_in: -1,
            _reserved: [0; 2],
        }
    }
    
    /// Create a readv (scatter) operation.
    pub fn readv(fd: i32, iov: u64, iovlen: u32, offset: u64, user_data: u64) -> Self {
        Self {
            opcode: OpCode::Readv,
            flags: SqFlags::empty(),
            ioprio: 0,
            fd,
            offset,
            addr: iov,
            len: iovlen,
            op_flags: 0,
            user_data,
            buf_index: 0,
            personality: 0,
            splice_fd_in: -1,
            _reserved: [0; 2],
        }
    }
    
    /// Create a writev (gather) operation.
    pub fn writev(fd: i32, iov: u64, iovlen: u32, offset: u64, user_data: u64) -> Self {
        Self {
            opcode: OpCode::Writev,
            flags: SqFlags::empty(),
            ioprio: 0,
            fd,
            offset,
            addr: iov,
            len: iovlen,
            op_flags: 0,
            user_data,
            buf_index: 0,
            personality: 0,
            splice_fd_in: -1,
            _reserved: [0; 2],
        }
    }
    
    /// Create a fsync operation.
    pub fn fsync(fd: i32, datasync: bool, user_data: u64) -> Self {
        Self {
            opcode: OpCode::Fsync,
            flags: SqFlags::empty(),
            ioprio: 0,
            fd,
            offset: 0,
            addr: 0,
            len: 0,
            op_flags: if datasync { 1 } else { 0 },
            user_data,
            buf_index: 0,
            personality: 0,
            splice_fd_in: -1,
            _reserved: [0; 2],
        }
    }
    
    /// Create a poll operation.
    pub fn poll_add(fd: i32, events: u32, user_data: u64) -> Self {
        Self {
            opcode: OpCode::PollAdd,
            flags: SqFlags::empty(),
            ioprio: 0,
            fd,
            offset: 0,
            addr: 0,
            len: 0,
            op_flags: events,
            user_data,
            buf_index: 0,
            personality: 0,
            splice_fd_in: -1,
            _reserved: [0; 2],
        }
    }
    
    /// Create a timeout operation.
    pub fn timeout(ts: u64, count: u32, user_data: u64) -> Self {
        Self {
            opcode: OpCode::Timeout,
            flags: SqFlags::empty(),
            ioprio: 0,
            fd: -1,
            offset: count as u64,
            addr: ts,
            len: 1,
            op_flags: 0,
            user_data,
            buf_index: 0,
            personality: 0,
            splice_fd_in: -1,
            _reserved: [0; 2],
        }
    }
    
    /// Create a nop (no operation) entry.
    pub fn nop(user_data: u64) -> Self {
        Self::new(OpCode::Nop, -1, user_data)
    }
    
    /// Set link flag (link to next operation).
    pub fn set_link(mut self) -> Self {
        self.flags = self.flags.union(SqFlags::LINK);
        self
    }
    
    /// Set drain flag (wait for previous ops).
    pub fn set_drain(mut self) -> Self {
        self.flags = self.flags.union(SqFlags::DRAIN);
        self
    }
}

/// Submission queue flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqFlags(u8);

impl SqFlags {
    /// Empty flags.
    pub const fn empty() -> Self {
        Self(0)
    }
    
    /// Fixed file (use registered file index).
    pub const FIXED_FILE: Self = Self(1 << 0);
    /// Drain previous ops before this one.
    pub const DRAIN: Self = Self(1 << 1);
    /// Link with next op.
    pub const LINK: Self = Self(1 << 2);
    /// Hard link (fail chain on error).
    pub const HARDLINK: Self = Self(1 << 3);
    /// Async operation.
    pub const ASYNC: Self = Self(1 << 4);
    /// Use registered buffer.
    pub const BUFFER_SELECT: Self = Self(1 << 5);
    
    /// Union of two flags.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    
    /// Check if flag is set.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// Completion queue entry.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct CqEntry {
    /// User data from submission.
    pub user_data: u64,
    /// Result value.
    pub result: i64,
    /// Flags.
    pub flags: CqFlags,
}

impl CqEntry {
    /// Create a completion entry.
    pub const fn new(user_data: u64, result: i64) -> Self {
        Self {
            user_data,
            result,
            flags: CqFlags::empty(),
        }
    }
    
    /// Create a success completion.
    pub const fn success(user_data: u64, value: i64) -> Self {
        Self::new(user_data, value)
    }
    
    /// Create an error completion.
    pub const fn error(user_data: u64, errno: i32) -> Self {
        Self::new(user_data, -(errno as i64))
    }
    
    /// Check if result is an error.
    pub const fn is_error(&self) -> bool {
        self.result < 0
    }
    
    /// Get error code if error.
    pub const fn error_code(&self) -> Option<i32> {
        if self.result < 0 {
            Some((-self.result) as i32)
        } else {
            None
        }
    }
}

/// Completion queue flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CqFlags(u32);

impl CqFlags {
    /// Empty flags.
    pub const fn empty() -> Self {
        Self(0)
    }
    
    /// More completions pending.
    pub const MORE: Self = Self(1 << 0);
    /// Socket notification.
    pub const SOCK_NONEMPTY: Self = Self(1 << 1);
    /// Notification.
    pub const NOTIF: Self = Self(1 << 2);
    
    /// Union of two flags.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    
    /// Check if flag is set.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// Submission ring buffer.
pub struct SubmissionRing {
    /// Ring entries.
    entries: Vec<SqEntry>,
    /// Head index (consumer).
    head: AtomicU32,
    /// Tail index (producer).
    tail: AtomicU32,
    /// Ring mask.
    mask: u32,
}

impl SubmissionRing {
    /// Create a new submission ring.
    pub fn new(size: usize) -> Self {
        // Round up to power of 2
        let size = size.next_power_of_two();
        
        Self {
            entries: alloc::vec![SqEntry::nop(0); size],
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            mask: (size - 1) as u32,
        }
    }
    
    /// Push an entry to the ring.
    pub fn push(&mut self, entry: SqEntry) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        
        // Check if full
        if tail.wrapping_sub(head) > self.mask {
            return false;
        }
        
        let idx = (tail & self.mask) as usize;
        self.entries[idx] = entry;
        
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        true
    }
    
    /// Pop an entry from the ring.
    pub fn pop(&mut self) -> Option<SqEntry> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        
        // Check if empty
        if head == tail {
            return None;
        }
        
        let idx = (head & self.mask) as usize;
        let entry = self.entries[idx];
        
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Some(entry)
    }
    
    /// Check if ring is empty.
    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        head == tail
    }
    
    /// Check if ring is full.
    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        tail.wrapping_sub(head) > self.mask
    }
    
    /// Get number of entries in ring.
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        tail.wrapping_sub(head) as usize
    }
    
    /// Get ring capacity.
    pub fn capacity(&self) -> usize {
        (self.mask + 1) as usize
    }
}

/// Completion ring buffer.
pub struct CompletionRing {
    /// Ring entries.
    entries: Vec<CqEntry>,
    /// Head index (consumer).
    head: AtomicU32,
    /// Tail index (producer).
    tail: AtomicU32,
    /// Ring mask.
    mask: u32,
}

impl CompletionRing {
    /// Create a new completion ring.
    pub fn new(size: usize) -> Self {
        // Round up to power of 2
        let size = size.next_power_of_two();
        
        Self {
            entries: alloc::vec![CqEntry::new(0, 0); size],
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            mask: (size - 1) as u32,
        }
    }
    
    /// Push a completion entry.
    pub fn push(&mut self, entry: CqEntry) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        
        // Check if full
        if tail.wrapping_sub(head) > self.mask {
            return false;
        }
        
        let idx = (tail & self.mask) as usize;
        self.entries[idx] = entry;
        
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        true
    }
    
    /// Pop a completion entry.
    pub fn pop(&mut self) -> Option<CqEntry> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        
        // Check if empty
        if head == tail {
            return None;
        }
        
        let idx = (head & self.mask) as usize;
        let entry = self.entries[idx];
        
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Some(entry)
    }
    
    /// Check if ring is empty.
    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        head == tail
    }
    
    /// Check if ring is full.
    pub fn is_full(&self) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        tail.wrapping_sub(head) > self.mask
    }
    
    /// Get number of entries.
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        tail.wrapping_sub(head) as usize
    }
    
    /// Get ring capacity.
    pub fn capacity(&self) -> usize {
        (self.mask + 1) as usize
    }
    
    /// Peek at next entry without consuming.
    pub fn peek(&self) -> Option<&CqEntry> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        
        if head == tail {
            return None;
        }
        
        let idx = (head & self.mask) as usize;
        Some(&self.entries[idx])
    }
}

/// I/O vector for scatter/gather I/O.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IoVec {
    /// Buffer base address.
    pub base: u64,
    /// Buffer length.
    pub len: usize,
}

impl IoVec {
    /// Create a new I/O vector.
    pub const fn new(base: u64, len: usize) -> Self {
        Self { base, len }
    }
}

/// Timespec for timeout operations.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TimeSpec {
    /// Seconds.
    pub tv_sec: i64,
    /// Nanoseconds.
    pub tv_nsec: i64,
}

impl TimeSpec {
    /// Create a timespec from milliseconds.
    pub const fn from_millis(ms: u64) -> Self {
        Self {
            tv_sec: (ms / 1000) as i64,
            tv_nsec: ((ms % 1000) * 1_000_000) as i64,
        }
    }
    
    /// Create a timespec from seconds.
    pub const fn from_secs(secs: u64) -> Self {
        Self {
            tv_sec: secs as i64,
            tv_nsec: 0,
        }
    }
}
