//! Threading for userspace.
//!
//! This module provides thread creation and synchronization primitives.

use crate::syscall::{syscall1, syscall2, syscall3, syscall4, SyscallNumber, SyscallError};
use core::sync::atomic::{AtomicU32, Ordering};

/// Thread ID type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadId(pub u64);

impl ThreadId {
    /// Create from raw ID.
    pub const fn from_raw(id: u64) -> Self {
        Self(id)
    }
    
    /// Get raw ID value.
    pub const fn raw(&self) -> u64 {
        self.0
    }
}

/// Thread creation flags.
pub mod flags {
    /// Clone parent's address space.
    pub const CLONE_VM: u32 = 0x100;
    /// Clone parent's file descriptors.
    pub const CLONE_FILES: u32 = 0x400;
    /// Clone parent's signal handlers.
    pub const CLONE_SIGHAND: u32 = 0x800;
    /// Set thread local storage.
    pub const CLONE_SETTLS: u32 = 0x80000;
}

/// Thread entry function type.
pub type ThreadFn = extern "C" fn(arg: u64) -> u64;

/// Create a new thread.
///
/// # Arguments
///
/// * `entry` - Thread entry function
/// * `stack` - Top of the thread's stack
/// * `arg` - Argument to pass to the thread function
/// * `flags` - Thread creation flags
///
/// # Returns
///
/// Thread ID of the new thread.
pub fn spawn(entry: ThreadFn, stack: *mut u8, arg: u64, creation_flags: u32) -> Result<ThreadId, SyscallError> {
    let result = unsafe {
        syscall4(
            SyscallNumber::ThreadCreate,
            entry as u64,
            stack as u64,
            arg,
            creation_flags as u64,
        )?
    };
    
    Ok(ThreadId::from_raw(result))
}

/// Exit the current thread.
pub fn exit(code: u64) -> ! {
    unsafe {
        let _ = syscall1(SyscallNumber::ThreadExit, code);
    }
    loop {}
}

/// Join a thread (wait for it to finish).
///
/// Returns the thread's return value.
pub fn join(tid: ThreadId) -> Result<u64, SyscallError> {
    let mut retval: u64 = 0;
    unsafe {
        syscall2(
            SyscallNumber::ThreadJoin,
            tid.0,
            &mut retval as *mut u64 as u64,
        )?;
    }
    Ok(retval)
}

// ==========================================
// Futex-based synchronization primitives
// ==========================================

/// Futex wait: block until the value at `addr` changes from `expected`.
pub fn futex_wait(addr: &AtomicU32, expected: u32, timeout_ns: Option<u64>) -> Result<(), SyscallError> {
    let timeout = timeout_ns.unwrap_or(u64::MAX);
    
    unsafe {
        syscall3(
            SyscallNumber::FutexWait,
            addr as *const _ as u64,
            expected as u64,
            timeout,
        )?;
    }
    
    Ok(())
}

/// Futex wake: wake up to `count` threads waiting on `addr`.
///
/// Returns the number of threads woken.
pub fn futex_wake(addr: &AtomicU32, count: u32) -> Result<u32, SyscallError> {
    let result = unsafe {
        syscall2(
            SyscallNumber::FutexWake,
            addr as *const _ as u64,
            count as u64,
        )?
    };
    
    Ok(result as u32)
}

/// Simple mutex using futex.
pub struct Mutex {
    /// 0 = unlocked, 1 = locked no waiters, 2 = locked with waiters
    state: AtomicU32,
}

impl Mutex {
    /// Create a new unlocked mutex.
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(0),
        }
    }
    
    /// Acquire the mutex.
    pub fn lock(&self) {
        // Fast path: try to acquire immediately
        if self.state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            return;
        }
        
        // Slow path: wait on futex
        loop {
            // Set to "locked with waiters"
            let old = self.state.swap(2, Ordering::Acquire);
            if old == 0 {
                // We got it!
                return;
            }
            
            // Wait for the lock to be released
            let _ = futex_wait(&self.state, 2, None);
        }
    }
    
    /// Try to acquire the mutex without blocking.
    pub fn try_lock(&self) -> bool {
        self.state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok()
    }
    
    /// Release the mutex.
    pub fn unlock(&self) {
        let old = self.state.swap(0, Ordering::Release);
        
        // If there were waiters, wake one up
        if old == 2 {
            let _ = futex_wake(&self.state, 1);
        }
    }
}

/// Condition variable using futex.
pub struct Condvar {
    seq: AtomicU32,
}

impl Condvar {
    /// Create a new condition variable.
    pub const fn new() -> Self {
        Self {
            seq: AtomicU32::new(0),
        }
    }
    
    /// Wait on the condition variable.
    ///
    /// The mutex must be locked before calling this.
    /// It will be unlocked while waiting and re-locked before returning.
    pub fn wait(&self, mutex: &Mutex) {
        let seq = self.seq.load(Ordering::Relaxed);
        
        // Release the mutex
        mutex.unlock();
        
        // Wait for signal
        let _ = futex_wait(&self.seq, seq, None);
        
        // Re-acquire mutex
        mutex.lock();
    }
    
    /// Wake one waiting thread.
    pub fn notify_one(&self) {
        self.seq.fetch_add(1, Ordering::Release);
        let _ = futex_wake(&self.seq, 1);
    }
    
    /// Wake all waiting threads.
    pub fn notify_all(&self) {
        self.seq.fetch_add(1, Ordering::Release);
        let _ = futex_wake(&self.seq, u32::MAX);
    }
}

/// Read-write lock using futex.
pub struct RwLock {
    /// Bit 31: writer present, Bits 0-30: reader count
    state: AtomicU32,
}

impl RwLock {
    const WRITER_BIT: u32 = 1 << 31;
    
    /// Create a new unlocked RwLock.
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(0),
        }
    }
    
    /// Acquire read access.
    pub fn read_lock(&self) {
        loop {
            let state = self.state.load(Ordering::Relaxed);
            
            // Can't acquire if writer present
            if state & Self::WRITER_BIT != 0 {
                let _ = futex_wait(&self.state, state, None);
                continue;
            }
            
            // Try to increment reader count
            if self.state.compare_exchange(
                state,
                state + 1,
                Ordering::Acquire,
                Ordering::Relaxed,
            ).is_ok() {
                return;
            }
        }
    }
    
    /// Release read access.
    pub fn read_unlock(&self) {
        let old = self.state.fetch_sub(1, Ordering::Release);
        
        // If we were the last reader and a writer is waiting
        if old == 1 {
            let _ = futex_wake(&self.state, 1);
        }
    }
    
    /// Acquire write access.
    pub fn write_lock(&self) {
        loop {
            let state = self.state.load(Ordering::Relaxed);
            
            // Can't acquire if any readers or writer present
            if state != 0 {
                let _ = futex_wait(&self.state, state, None);
                continue;
            }
            
            // Try to set writer bit
            if self.state.compare_exchange(
                0,
                Self::WRITER_BIT,
                Ordering::Acquire,
                Ordering::Relaxed,
            ).is_ok() {
                return;
            }
        }
    }
    
    /// Release write access.
    pub fn write_unlock(&self) {
        self.state.store(0, Ordering::Release);
        // Wake all waiting threads (both readers and writers can try)
        let _ = futex_wake(&self.state, u32::MAX);
    }
}
