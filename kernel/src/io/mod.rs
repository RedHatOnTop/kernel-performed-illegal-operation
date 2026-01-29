//! Async I/O Subsystem
//!
//! This module implements an io_uring-style asynchronous I/O system for
//! high-performance, non-blocking I/O operations in the kernel.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────┐
//! │                    Async I/O Subsystem                           │
//! ├──────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │  ┌─────────────────┐         ┌─────────────────┐                │
//! │  │ Submission Ring │ ──────> │ Completion Ring │                │
//! │  │  (SQ Entries)   │         │  (CQ Entries)   │                │
//! │  └─────────────────┘         └─────────────────┘                │
//! │           │                          ▲                          │
//! │           ▼                          │                          │
//! │  ┌─────────────────────────────────────────────┐                │
//! │  │              I/O Executor                   │                │
//! │  │  (Processes SQ, generates CQ entries)       │                │
//! │  └─────────────────────────────────────────────┘                │
//! │           │                                                     │
//! │           ▼                                                     │
//! │  ┌──────────┬──────────┬──────────┬──────────┐                 │
//! │  │   Read   │  Write   │   Sync   │  Poll    │                 │
//! │  │ Handler  │ Handler  │ Handler  │ Handler  │                 │
//! │  └──────────┴──────────┴──────────┴──────────┘                 │
//! │                                                                  │
//! └──────────────────────────────────────────────────────────────────┘
//! ```

pub mod ring;
pub mod executor;
pub mod operations;

use alloc::vec::Vec;
use alloc::sync::Arc;
use spin::Mutex;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub use ring::{SubmissionRing, CompletionRing, SqEntry, CqEntry};
pub use executor::IoExecutor;
pub use operations::{IoOp, IoResult};

/// Maximum number of entries in a ring.
pub const MAX_RING_SIZE: usize = 4096;

/// Default ring size.
pub const DEFAULT_RING_SIZE: usize = 256;

/// I/O context for a process.
pub struct IoContext {
    /// Submission ring.
    submission: SubmissionRing,
    /// Completion ring.
    completion: CompletionRing,
    /// Context ID.
    id: u64,
    /// Statistics.
    stats: IoStats,
}

/// I/O operation result.
#[derive(Debug, Clone, Copy)]
pub struct IoCompletionResult {
    /// User data from submission.
    pub user_data: u64,
    /// Result value (positive = success, negative = error).
    pub result: i64,
    /// Flags.
    pub flags: u32,
}

/// I/O statistics.
#[derive(Debug, Default)]
pub struct IoStats {
    /// Total submissions.
    pub submissions: AtomicU64,
    /// Total completions.
    pub completions: AtomicU64,
    /// Operations in progress.
    pub in_flight: AtomicU32,
    /// Total bytes read.
    pub bytes_read: AtomicU64,
    /// Total bytes written.
    pub bytes_written: AtomicU64,
    /// Submission ring overflows.
    pub sq_overflows: AtomicU64,
    /// Completion ring overflows.
    pub cq_overflows: AtomicU64,
}

impl IoStats {
    pub const fn new() -> Self {
        Self {
            submissions: AtomicU64::new(0),
            completions: AtomicU64::new(0),
            in_flight: AtomicU32::new(0),
            bytes_read: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
            sq_overflows: AtomicU64::new(0),
            cq_overflows: AtomicU64::new(0),
        }
    }
}

impl IoContext {
    /// Create a new I/O context.
    pub fn new(ring_size: usize) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        
        Self {
            submission: SubmissionRing::new(ring_size),
            completion: CompletionRing::new(ring_size),
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            stats: IoStats::new(),
        }
    }
    
    /// Submit an I/O operation.
    pub fn submit(&mut self, entry: SqEntry) -> Result<(), IoError> {
        if self.submission.is_full() {
            self.stats.sq_overflows.fetch_add(1, Ordering::Relaxed);
            return Err(IoError::RingFull);
        }
        
        self.submission.push(entry);
        self.stats.submissions.fetch_add(1, Ordering::Relaxed);
        self.stats.in_flight.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    
    /// Get a completion result if available.
    pub fn poll_completion(&mut self) -> Option<CqEntry> {
        let entry = self.completion.pop()?;
        self.stats.completions.fetch_add(1, Ordering::Relaxed);
        self.stats.in_flight.fetch_sub(1, Ordering::Relaxed);
        Some(entry)
    }
    
    /// Get all available completions.
    pub fn poll_completions(&mut self, max: usize) -> Vec<CqEntry> {
        let mut results = Vec::with_capacity(max);
        for _ in 0..max {
            if let Some(entry) = self.poll_completion() {
                results.push(entry);
            } else {
                break;
            }
        }
        results
    }
    
    /// Wait for at least one completion.
    pub fn wait_completion(&mut self) -> Option<CqEntry> {
        // In a real implementation, this would block until completion
        // For now, just poll
        self.poll_completion()
    }
    
    /// Submit and wait for all completions.
    pub fn submit_and_wait(&mut self) -> Vec<CqEntry> {
        let pending = self.stats.in_flight.load(Ordering::Relaxed);
        self.poll_completions(pending as usize)
    }
    
    /// Get the context ID.
    pub fn id(&self) -> u64 {
        self.id
    }
    
    /// Get statistics.
    pub fn stats(&self) -> &IoStats {
        &self.stats
    }
    
    /// Add a completion (called by executor).
    pub(crate) fn complete(&mut self, entry: CqEntry) -> Result<(), IoError> {
        if self.completion.is_full() {
            self.stats.cq_overflows.fetch_add(1, Ordering::Relaxed);
            return Err(IoError::RingFull);
        }
        self.completion.push(entry);
        Ok(())
    }
    
    /// Get pending submission entries.
    pub(crate) fn drain_submissions(&mut self) -> Vec<SqEntry> {
        let mut entries = Vec::new();
        while let Some(entry) = self.submission.pop() {
            entries.push(entry);
        }
        entries
    }
}

/// I/O error types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoError {
    /// Ring buffer is full.
    RingFull,
    /// Invalid file descriptor.
    InvalidFd,
    /// Invalid buffer.
    InvalidBuffer,
    /// Operation not supported.
    NotSupported,
    /// Operation cancelled.
    Cancelled,
    /// I/O error.
    IoError(i32),
    /// Resource busy.
    Busy,
    /// Timeout.
    Timeout,
}

/// Global I/O subsystem state.
pub struct IoSubsystem {
    /// Active I/O contexts.
    contexts: Vec<Arc<Mutex<IoContext>>>,
    /// Global executor.
    executor: IoExecutor,
    /// Next context ID.
    next_ctx_id: AtomicU64,
}

impl IoSubsystem {
    /// Create a new I/O subsystem.
    pub const fn new() -> Self {
        Self {
            contexts: Vec::new(),
            executor: IoExecutor::new(),
            next_ctx_id: AtomicU64::new(1),
        }
    }
    
    /// Initialize the I/O subsystem.
    pub fn init(&mut self) {
        // Initialize executor
        self.executor.init();
    }
    
    /// Create a new I/O context.
    pub fn create_context(&mut self, ring_size: usize) -> Arc<Mutex<IoContext>> {
        let ctx = Arc::new(Mutex::new(IoContext::new(ring_size)));
        self.contexts.push(ctx.clone());
        ctx
    }
    
    /// Process pending I/O operations.
    pub fn process(&mut self) {
        // Collect all submissions
        let mut all_submissions = Vec::new();
        
        for ctx in &self.contexts {
            let mut ctx = ctx.lock();
            let submissions = ctx.drain_submissions();
            for sq in submissions {
                all_submissions.push((ctx.id(), sq));
            }
        }
        
        // Process submissions
        for (ctx_id, sq) in all_submissions {
            if let Some(cq) = self.executor.process(sq) {
                // Find context and complete
                for ctx in &self.contexts {
                    let mut ctx_guard = ctx.lock();
                    if ctx_guard.id() == ctx_id {
                        let _ = ctx_guard.complete(cq);
                        break;
                    }
                }
            }
        }
    }
}

/// Global I/O subsystem instance.
static IO_SUBSYSTEM: Mutex<Option<IoSubsystem>> = Mutex::new(None);

/// Initialize the I/O subsystem.
pub fn init() {
    let mut subsys = IoSubsystem::new();
    subsys.init();
    *IO_SUBSYSTEM.lock() = Some(subsys);
}

/// Create a new I/O context.
pub fn create_context(ring_size: usize) -> Option<Arc<Mutex<IoContext>>> {
    IO_SUBSYSTEM.lock().as_mut().map(|s| s.create_context(ring_size))
}

/// Process pending I/O.
pub fn process() {
    if let Some(subsys) = IO_SUBSYSTEM.lock().as_mut() {
        subsys.process();
    }
}
