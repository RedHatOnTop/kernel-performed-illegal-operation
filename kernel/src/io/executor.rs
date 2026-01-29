//! I/O Executor
//!
//! Processes submitted I/O operations and generates completions.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use spin::Mutex;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};

use super::ring::{SqEntry, CqEntry};
use super::operations::{OpCode, IoResult, IoOpError};

/// I/O executor state.
pub struct IoExecutor {
    /// Pending operations queue.
    pending: VecDeque<PendingOp>,
    /// Operations in flight.
    in_flight: Vec<InFlightOp>,
    /// Registered file descriptors.
    registered_files: Vec<Option<i32>>,
    /// Registered buffers.
    registered_buffers: Vec<RegisteredBuffer>,
    /// Executor statistics.
    stats: ExecutorStats,
    /// Whether executor is initialized.
    initialized: AtomicBool,
}

/// Pending operation waiting to be processed.
struct PendingOp {
    /// Submission entry.
    entry: SqEntry,
    /// Timestamp when submitted.
    submit_time: u64,
}

/// In-flight operation being processed.
struct InFlightOp {
    /// User data.
    user_data: u64,
    /// Operation code.
    opcode: OpCode,
    /// Operation state.
    state: OpState,
    /// Start time.
    start_time: u64,
}

/// Operation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpState {
    /// Queued for execution.
    Queued,
    /// Currently executing.
    Executing,
    /// Completed successfully.
    Completed,
    /// Failed.
    Failed,
    /// Cancelled.
    Cancelled,
}

/// Registered buffer.
struct RegisteredBuffer {
    /// Buffer address.
    addr: u64,
    /// Buffer length.
    len: usize,
}

/// Executor statistics.
#[derive(Debug, Default)]
pub struct ExecutorStats {
    /// Total operations processed.
    pub ops_processed: AtomicU64,
    /// Total operations completed.
    pub ops_completed: AtomicU64,
    /// Total operations failed.
    pub ops_failed: AtomicU64,
    /// Total operations cancelled.
    pub ops_cancelled: AtomicU64,
    /// Total bytes read.
    pub bytes_read: AtomicU64,
    /// Total bytes written.
    pub bytes_written: AtomicU64,
}

impl ExecutorStats {
    pub const fn new() -> Self {
        Self {
            ops_processed: AtomicU64::new(0),
            ops_completed: AtomicU64::new(0),
            ops_failed: AtomicU64::new(0),
            ops_cancelled: AtomicU64::new(0),
            bytes_read: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
        }
    }
}

impl IoExecutor {
    /// Create a new executor.
    pub const fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            in_flight: Vec::new(),
            registered_files: Vec::new(),
            registered_buffers: Vec::new(),
            stats: ExecutorStats::new(),
            initialized: AtomicBool::new(false),
        }
    }
    
    /// Initialize the executor.
    pub fn init(&mut self) {
        self.initialized.store(true, Ordering::Release);
    }
    
    /// Check if initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }
    
    /// Process a submission entry and return completion.
    pub fn process(&mut self, sq: SqEntry) -> Option<CqEntry> {
        self.stats.ops_processed.fetch_add(1, Ordering::Relaxed);
        
        let result = match sq.opcode {
            OpCode::Nop => self.handle_nop(&sq),
            OpCode::Read => self.handle_read(&sq),
            OpCode::Write => self.handle_write(&sq),
            OpCode::Readv => self.handle_readv(&sq),
            OpCode::Writev => self.handle_writev(&sq),
            OpCode::Fsync => self.handle_fsync(&sq),
            OpCode::PollAdd => self.handle_poll_add(&sq),
            OpCode::Timeout => self.handle_timeout(&sq),
            OpCode::Close => self.handle_close(&sq),
            OpCode::Accept => self.handle_accept(&sq),
            OpCode::Connect => self.handle_connect(&sq),
            OpCode::SendMsg => self.handle_sendmsg(&sq),
            OpCode::RecvMsg => self.handle_recvmsg(&sq),
            _ => IoResult::Error(IoOpError::NotSupported),
        };
        
        // Generate completion
        let cq = match result {
            IoResult::Pending => return None,
            _ => {
                if result.is_ok() {
                    self.stats.ops_completed.fetch_add(1, Ordering::Relaxed);
                } else {
                    self.stats.ops_failed.fetch_add(1, Ordering::Relaxed);
                }
                CqEntry::new(sq.user_data, result.to_result_code())
            }
        };
        
        Some(cq)
    }
    
    /// Handle NOP operation.
    fn handle_nop(&self, _sq: &SqEntry) -> IoResult {
        IoResult::Success
    }
    
    /// Handle read operation.
    fn handle_read(&mut self, sq: &SqEntry) -> IoResult {
        // Validate file descriptor
        if sq.fd < 0 {
            return IoResult::Error(IoOpError::BadFd);
        }
        
        // Validate buffer
        if sq.addr == 0 || sq.len == 0 {
            return IoResult::Error(IoOpError::InvalidArgument);
        }
        
        // In a real implementation, this would:
        // 1. Look up the file descriptor
        // 2. Read data from the underlying device/file
        // 3. Copy data to the user buffer
        
        // For now, simulate a read
        let bytes_read = sq.len as usize;
        self.stats.bytes_read.fetch_add(bytes_read as u64, Ordering::Relaxed);
        
        IoResult::Bytes(bytes_read)
    }
    
    /// Handle write operation.
    fn handle_write(&mut self, sq: &SqEntry) -> IoResult {
        // Validate file descriptor
        if sq.fd < 0 {
            return IoResult::Error(IoOpError::BadFd);
        }
        
        // Validate buffer
        if sq.addr == 0 || sq.len == 0 {
            return IoResult::Error(IoOpError::InvalidArgument);
        }
        
        // In a real implementation, this would:
        // 1. Look up the file descriptor
        // 2. Write data to the underlying device/file
        
        // For now, simulate a write
        let bytes_written = sq.len as usize;
        self.stats.bytes_written.fetch_add(bytes_written as u64, Ordering::Relaxed);
        
        IoResult::Bytes(bytes_written)
    }
    
    /// Handle vectored read.
    fn handle_readv(&mut self, sq: &SqEntry) -> IoResult {
        if sq.fd < 0 {
            return IoResult::Error(IoOpError::BadFd);
        }
        
        if sq.addr == 0 || sq.len == 0 {
            return IoResult::Error(IoOpError::InvalidArgument);
        }
        
        // sq.addr points to array of IoVec
        // sq.len is number of vectors
        // For now, simulate reading total bytes
        let total_bytes = sq.len as usize * 4096; // Assume 4K per vector
        self.stats.bytes_read.fetch_add(total_bytes as u64, Ordering::Relaxed);
        
        IoResult::Bytes(total_bytes)
    }
    
    /// Handle vectored write.
    fn handle_writev(&mut self, sq: &SqEntry) -> IoResult {
        if sq.fd < 0 {
            return IoResult::Error(IoOpError::BadFd);
        }
        
        if sq.addr == 0 || sq.len == 0 {
            return IoResult::Error(IoOpError::InvalidArgument);
        }
        
        let total_bytes = sq.len as usize * 4096;
        self.stats.bytes_written.fetch_add(total_bytes as u64, Ordering::Relaxed);
        
        IoResult::Bytes(total_bytes)
    }
    
    /// Handle fsync operation.
    fn handle_fsync(&self, sq: &SqEntry) -> IoResult {
        if sq.fd < 0 {
            return IoResult::Error(IoOpError::BadFd);
        }
        
        // In a real implementation, this would flush buffers to disk
        IoResult::Success
    }
    
    /// Handle poll add operation.
    fn handle_poll_add(&self, sq: &SqEntry) -> IoResult {
        if sq.fd < 0 {
            return IoResult::Error(IoOpError::BadFd);
        }
        
        // In a real implementation, this would register poll interest
        // Return pending to indicate async completion
        IoResult::Pending
    }
    
    /// Handle timeout operation.
    fn handle_timeout(&self, _sq: &SqEntry) -> IoResult {
        // Timeouts are handled asynchronously
        IoResult::Pending
    }
    
    /// Handle close operation.
    fn handle_close(&self, sq: &SqEntry) -> IoResult {
        if sq.fd < 0 {
            return IoResult::Error(IoOpError::BadFd);
        }
        
        // In a real implementation, close the file descriptor
        IoResult::Success
    }
    
    /// Handle accept operation.
    fn handle_accept(&self, sq: &SqEntry) -> IoResult {
        if sq.fd < 0 {
            return IoResult::Error(IoOpError::BadFd);
        }
        
        // Accept is async - would block until connection
        IoResult::Pending
    }
    
    /// Handle connect operation.
    fn handle_connect(&self, sq: &SqEntry) -> IoResult {
        if sq.fd < 0 {
            return IoResult::Error(IoOpError::BadFd);
        }
        
        // Connect is async
        IoResult::Pending
    }
    
    /// Handle sendmsg operation.
    fn handle_sendmsg(&mut self, sq: &SqEntry) -> IoResult {
        if sq.fd < 0 {
            return IoResult::Error(IoOpError::BadFd);
        }
        
        let bytes = sq.len as usize;
        self.stats.bytes_written.fetch_add(bytes as u64, Ordering::Relaxed);
        
        IoResult::Bytes(bytes)
    }
    
    /// Handle recvmsg operation.
    fn handle_recvmsg(&mut self, sq: &SqEntry) -> IoResult {
        if sq.fd < 0 {
            return IoResult::Error(IoOpError::BadFd);
        }
        
        // Receive is typically async
        IoResult::Pending
    }
    
    /// Register file descriptors for fixed file operations.
    pub fn register_files(&mut self, fds: &[i32]) -> Result<(), IoOpError> {
        self.registered_files.clear();
        for &fd in fds {
            self.registered_files.push(if fd >= 0 { Some(fd) } else { None });
        }
        Ok(())
    }
    
    /// Unregister all files.
    pub fn unregister_files(&mut self) {
        self.registered_files.clear();
    }
    
    /// Register buffers for fixed buffer operations.
    pub fn register_buffers(&mut self, buffers: &[(u64, usize)]) -> Result<(), IoOpError> {
        self.registered_buffers.clear();
        for &(addr, len) in buffers {
            self.registered_buffers.push(RegisteredBuffer { addr, len });
        }
        Ok(())
    }
    
    /// Unregister all buffers.
    pub fn unregister_buffers(&mut self) {
        self.registered_buffers.clear();
    }
    
    /// Get a registered file descriptor.
    fn get_registered_file(&self, index: usize) -> Option<i32> {
        self.registered_files.get(index).copied().flatten()
    }
    
    /// Get a registered buffer.
    fn get_registered_buffer(&self, index: usize) -> Option<(u64, usize)> {
        self.registered_buffers.get(index).map(|b| (b.addr, b.len))
    }
    
    /// Get executor statistics.
    pub fn stats(&self) -> &ExecutorStats {
        &self.stats
    }
    
    /// Cancel an operation by user_data.
    pub fn cancel(&mut self, user_data: u64) -> bool {
        // Find and cancel the operation
        for op in &mut self.in_flight {
            if op.user_data == user_data && op.state == OpState::Queued {
                op.state = OpState::Cancelled;
                self.stats.ops_cancelled.fetch_add(1, Ordering::Relaxed);
                return true;
            }
        }
        false
    }
    
    /// Process pending operations.
    pub fn tick(&mut self) -> Vec<CqEntry> {
        let mut completions = Vec::new();
        
        // Process pending operations
        while let Some(pending) = self.pending.pop_front() {
            if let Some(cq) = self.process(pending.entry) {
                completions.push(cq);
            }
        }
        
        // Check for completed in-flight operations
        self.in_flight.retain(|op| {
            if op.state == OpState::Completed || 
               op.state == OpState::Failed || 
               op.state == OpState::Cancelled {
                false
            } else {
                true
            }
        });
        
        completions
    }
}

/// Work queue for background I/O processing.
pub struct IoWorkQueue {
    /// Work items.
    items: Mutex<VecDeque<WorkItem>>,
    /// Worker count.
    worker_count: usize,
}

/// Work item.
struct WorkItem {
    /// Context ID.
    ctx_id: u64,
    /// Submission entry.
    entry: SqEntry,
}

impl IoWorkQueue {
    /// Create a new work queue.
    pub fn new(worker_count: usize) -> Self {
        Self {
            items: Mutex::new(VecDeque::new()),
            worker_count,
        }
    }
    
    /// Submit work.
    pub fn submit(&self, ctx_id: u64, entry: SqEntry) {
        self.items.lock().push_back(WorkItem { ctx_id, entry });
    }
    
    /// Get next work item.
    pub fn pop(&self) -> Option<(u64, SqEntry)> {
        self.items.lock().pop_front().map(|w| (w.ctx_id, w.entry))
    }
    
    /// Get queue length.
    pub fn len(&self) -> usize {
        self.items.lock().len()
    }
    
    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.items.lock().is_empty()
    }
}

/// Async I/O worker thread (simulated).
pub struct IoWorker {
    /// Worker ID.
    id: usize,
    /// Work queue reference.
    queue: *const IoWorkQueue,
    /// Running flag.
    running: AtomicBool,
}

impl IoWorker {
    /// Create a new worker.
    pub fn new(id: usize, queue: &IoWorkQueue) -> Self {
        Self {
            id,
            queue: queue as *const _,
            running: AtomicBool::new(false),
        }
    }
    
    /// Start the worker.
    pub fn start(&self) {
        self.running.store(true, Ordering::Release);
    }
    
    /// Stop the worker.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
    }
    
    /// Check if running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
    
    /// Worker ID.
    pub fn id(&self) -> usize {
        self.id
    }
}
