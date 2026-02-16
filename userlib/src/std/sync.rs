//! std::sync compatibility layer for KPIO
//!
//! Provides synchronization primitives via KPIO syscalls.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::syscall;

/// Mutex (mutual exclusion lock)
pub struct Mutex<T: ?Sized> {
    futex: AtomicU32,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    /// Creates a new mutex.
    pub const fn new(value: T) -> Self {
        Mutex {
            futex: AtomicU32::new(0),
            data: UnsafeCell::new(value),
        }
    }

    /// Acquires the mutex, blocking until available.
    pub fn lock(&self) -> MutexGuard<'_, T> {
        loop {
            // Try to acquire lock
            if self
                .futex
                .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return MutexGuard { mutex: self };
            }

            // Mark as contended and wait
            if self.futex.swap(2, Ordering::Acquire) != 0 {
                let _ = syscall::futex_wait(&self.futex as *const _ as usize, 2);
            }
        }
    }

    /// Tries to acquire the mutex without blocking.
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        if self
            .futex
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(MutexGuard { mutex: self })
        } else {
            None
        }
    }

    /// Returns a mutable reference to the underlying data.
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Consumes the mutex, returning the underlying data.
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

/// RAII guard for Mutex
pub struct MutexGuard<'a, T: ?Sized> {
    mutex: &'a Mutex<T>,
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        if self.mutex.futex.swap(0, Ordering::Release) == 2 {
            // There were waiters, wake one
            let _ = syscall::futex_wake(&self.mutex.futex as *const _ as usize, 1);
        }
    }
}

/// Read-write lock
pub struct RwLock<T: ?Sized> {
    state: AtomicU32, // 0 = unlocked, u32::MAX = write locked, 1..MAX = reader count
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

impl<T> RwLock<T> {
    /// Creates a new read-write lock.
    pub const fn new(value: T) -> Self {
        RwLock {
            state: AtomicU32::new(0),
            data: UnsafeCell::new(value),
        }
    }

    /// Acquires a read lock.
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        loop {
            let state = self.state.load(Ordering::Relaxed);
            if state != u32::MAX {
                if self
                    .state
                    .compare_exchange_weak(state, state + 1, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    return RwLockReadGuard { lock: self };
                }
            } else {
                core::hint::spin_loop();
            }
        }
    }

    /// Acquires a write lock.
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        while self
            .state
            .compare_exchange_weak(0, u32::MAX, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }

        RwLockWriteGuard { lock: self }
    }

    /// Tries to acquire a read lock.
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
        let state = self.state.load(Ordering::Relaxed);
        if state != u32::MAX {
            if self
                .state
                .compare_exchange(state, state + 1, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return Some(RwLockReadGuard { lock: self });
            }
        }
        None
    }

    /// Tries to acquire a write lock.
    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
        if self
            .state
            .compare_exchange(0, u32::MAX, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(RwLockWriteGuard { lock: self })
        } else {
            None
        }
    }

    /// Returns a mutable reference to the underlying data.
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Consumes the lock, returning the underlying data.
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

pub struct RwLockReadGuard<'a, T: ?Sized> {
    lock: &'a RwLock<T>,
}

impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.fetch_sub(1, Ordering::Release);
    }
}

pub struct RwLockWriteGuard<'a, T: ?Sized> {
    lock: &'a RwLock<T>,
}

impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.store(0, Ordering::Release);
    }
}

/// Condition variable
pub struct Condvar {
    futex: AtomicU32,
}

impl Condvar {
    /// Creates a new condition variable.
    pub const fn new() -> Self {
        Condvar {
            futex: AtomicU32::new(0),
        }
    }

    /// Waits on the condition variable.
    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        let mutex = guard.mutex;
        let key = self.futex.load(Ordering::Relaxed);

        // Release the mutex
        drop(guard);

        // Wait for notification
        let _ = syscall::futex_wait(&self.futex as *const _ as usize, key);

        // Re-acquire the mutex
        mutex.lock()
    }

    /// Wakes one waiting thread.
    pub fn notify_one(&self) {
        self.futex.fetch_add(1, Ordering::SeqCst);
        let _ = syscall::futex_wake(&self.futex as *const _ as usize, 1);
    }

    /// Wakes all waiting threads.
    pub fn notify_all(&self) {
        self.futex.fetch_add(1, Ordering::SeqCst);
        let _ = syscall::futex_wake(&self.futex as *const _ as usize, u32::MAX);
    }
}

impl Default for Condvar {
    fn default() -> Self {
        Self::new()
    }
}

/// Once cell for one-time initialization.
pub struct Once {
    state: AtomicU32, // 0 = incomplete, 1 = running, 2 = complete
}

impl Once {
    /// Creates a new Once cell.
    pub const fn new() -> Self {
        Once {
            state: AtomicU32::new(0),
        }
    }

    /// Calls the function only once.
    pub fn call_once<F: FnOnce()>(&self, f: F) {
        if self.state.load(Ordering::Acquire) == 2 {
            return;
        }

        if self
            .state
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            f();
            self.state.store(2, Ordering::Release);
        } else {
            while self.state.load(Ordering::Acquire) != 2 {
                core::hint::spin_loop();
            }
        }
    }

    /// Returns true if call_once has completed.
    pub fn is_completed(&self) -> bool {
        self.state.load(Ordering::Acquire) == 2
    }
}

impl Default for Once {
    fn default() -> Self {
        Self::new()
    }
}

/// Barrier for synchronizing multiple threads.
pub struct Barrier {
    count: u32,
    state: AtomicU32,
    generation: AtomicU32,
}

impl Barrier {
    /// Creates a new barrier for `n` threads.
    pub fn new(n: usize) -> Self {
        Barrier {
            count: n as u32,
            state: AtomicU32::new(0),
            generation: AtomicU32::new(0),
        }
    }

    /// Waits at the barrier.
    pub fn wait(&self) -> BarrierWaitResult {
        let gen = self.generation.load(Ordering::SeqCst);
        let old = self.state.fetch_add(1, Ordering::SeqCst);

        if old + 1 == self.count {
            // Last thread
            self.state.store(0, Ordering::SeqCst);
            self.generation.fetch_add(1, Ordering::SeqCst);
            BarrierWaitResult { is_leader: true }
        } else {
            // Wait for generation to change
            while self.generation.load(Ordering::SeqCst) == gen {
                core::hint::spin_loop();
            }
            BarrierWaitResult { is_leader: false }
        }
    }
}

/// Result from waiting at a barrier.
pub struct BarrierWaitResult {
    is_leader: bool,
}

impl BarrierWaitResult {
    /// Returns true if this thread is the "leader".
    pub fn is_leader(&self) -> bool {
        self.is_leader
    }
}

/// Arc (Atomically Reference Counted)
pub use alloc::sync::Arc;

/// Weak reference to Arc
pub use alloc::sync::Weak;
