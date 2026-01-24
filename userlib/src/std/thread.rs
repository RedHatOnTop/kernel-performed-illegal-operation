//! std::thread compatibility layer for KPIO
//!
//! Provides threading functionality via KPIO syscalls.

use alloc::boxed::Box;
use alloc::string::String;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::syscall;
use super::time::Duration;

static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);

/// Thread ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThreadId(u64);

impl ThreadId {
    fn new() -> Self {
        ThreadId(NEXT_THREAD_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Thread handle
pub struct Thread {
    id: ThreadId,
    name: Option<String>,
}

impl Thread {
    /// Gets the thread's unique identifier.
    pub fn id(&self) -> ThreadId {
        self.id
    }
    
    /// Gets the thread's name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    
    /// Unparks (wakes) this thread.
    pub fn unpark(&self) {
        let _ = syscall::thread_unpark(self.id.0);
    }
}

/// Returns a handle to the current thread.
pub fn current() -> Thread {
    let id = syscall::thread_id().unwrap_or(0);
    Thread {
        id: ThreadId(id),
        name: None,
    }
}

/// Cooperatively gives up a timeslice to the OS scheduler.
pub fn yield_now() {
    let _ = syscall::sched_yield();
}

/// Puts the current thread to sleep for at least the specified duration.
pub fn sleep(dur: Duration) {
    super::time::sleep(dur);
}

/// Blocks unless or until the current thread's token is made available.
pub fn park() {
    let _ = syscall::thread_park();
}

/// Blocks unless or until the current thread's token is made available,
/// or the specified timeout has elapsed.
pub fn park_timeout(dur: Duration) {
    let _ = syscall::thread_park_timeout(dur.as_nanos() as u64);
}

/// Join handle for a spawned thread
pub struct JoinHandle<T> {
    thread: Thread,
    result: Option<T>,
    kernel_handle: u64,
}

impl<T> JoinHandle<T> {
    /// Gets a handle to the underlying thread.
    pub fn thread(&self) -> &Thread {
        &self.thread
    }
    
    /// Waits for the thread to finish.
    pub fn join(self) -> Result<T, Box<dyn core::any::Any + Send + 'static>> {
        // Wait for thread completion
        let _ = syscall::thread_join(self.kernel_handle);
        
        // In real implementation, result would be retrieved from shared memory
        // For now, this is a placeholder
        Err(Box::new(()))
    }
    
    /// Checks if the thread has finished.
    pub fn is_finished(&self) -> bool {
        syscall::thread_is_finished(self.kernel_handle).unwrap_or(true)
    }
}

/// Thread builder
pub struct Builder {
    name: Option<String>,
    stack_size: Option<usize>,
}

impl Builder {
    /// Creates a new thread builder.
    pub fn new() -> Self {
        Builder {
            name: None,
            stack_size: None,
        }
    }
    
    /// Sets the name of the thread.
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }
    
    /// Sets the stack size for the new thread.
    pub fn stack_size(mut self, size: usize) -> Self {
        self.stack_size = Some(size);
        self
    }
    
    /// Spawns a new thread.
    pub fn spawn<F, T>(self, f: F) -> Result<JoinHandle<T>, SpawnError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let stack_size = self.stack_size.unwrap_or(2 * 1024 * 1024); // 2MB default
        
        // Box the closure (use double indirection to avoid fat pointer issues)
        let boxed: Box<Box<dyn FnOnce() -> T + Send + 'static>> = Box::new(Box::new(f));
        let raw = Box::into_raw(boxed) as *mut () as usize;
        
        // Create thread via syscall
        let kernel_handle = syscall::thread_spawn(
            thread_entry::<T> as usize,
            raw,
            stack_size,
        ).map_err(|_| SpawnError)?;
        
        let thread = Thread {
            id: ThreadId::new(),
            name: self.name,
        };
        
        Ok(JoinHandle {
            thread,
            result: None,
            kernel_handle,
        })
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry point for spawned threads
extern "C" fn thread_entry<T>(closure_ptr: usize) -> ! {
    let closure: Box<Box<dyn FnOnce() -> T + Send + 'static>> = 
        unsafe { Box::from_raw(closure_ptr as *mut Box<dyn FnOnce() -> T + Send + 'static>) };
    
    let _result = (*closure)();
    
    // Store result somewhere for join() to retrieve
    // Then exit the thread
    syscall::thread_exit(0);
    
    // This should never be reached
    loop {
        core::hint::spin_loop();
    }
}

/// Error returned when thread spawn fails.
#[derive(Debug)]
pub struct SpawnError;

/// Spawns a new thread.
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    Builder::new().spawn(f).expect("failed to spawn thread")
}

/// Returns the number of hardware threads available.
pub fn available_parallelism() -> Result<core::num::NonZeroUsize, ()> {
    let count = syscall::cpu_count().unwrap_or(1);
    core::num::NonZeroUsize::new(count).ok_or(())
}

/// Scope for scoped threads (allows borrowing)
pub struct Scope<'scope, 'env: 'scope> {
    _marker: core::marker::PhantomData<(&'scope mut &'env (), *mut ())>,
}

impl<'scope, 'env> Scope<'scope, 'env> {
    /// Spawns a scoped thread.
    pub fn spawn<F, T>(&'scope self, f: F) -> ScopedJoinHandle<'scope, T>
    where
        F: FnOnce() -> T + Send + 'scope,
        T: Send + 'scope,
    {
        // For now, just run synchronously
        let result = f();
        ScopedJoinHandle {
            result: Some(result),
            _marker: core::marker::PhantomData,
        }
    }
}

/// Join handle for scoped threads
pub struct ScopedJoinHandle<'scope, T> {
    result: Option<T>,
    _marker: core::marker::PhantomData<&'scope ()>,
}

impl<'scope, T> ScopedJoinHandle<'scope, T> {
    /// Waits for the thread to finish.
    pub fn join(mut self) -> Result<T, Box<dyn core::any::Any + Send + 'static>> {
        self.result.take().ok_or_else(|| Box::new(()) as Box<dyn core::any::Any + Send>)
    }
}

/// Creates a scope for spawning scoped threads.
pub fn scope<'env, F, T>(f: F) -> T
where
    F: for<'scope> FnOnce(&'scope Scope<'scope, 'env>) -> T,
{
    let scope = Scope {
        _marker: core::marker::PhantomData,
    };
    f(&scope)
}

/// Local key for thread-local storage
pub struct LocalKey<T: 'static> {
    init: fn() -> T,
    // In real implementation, would use TLS slots
}

impl<T: 'static> LocalKey<T> {
    /// Creates a new local key.
    pub const fn new(init: fn() -> T) -> Self {
        LocalKey { init }
    }
    
    /// Acquires a reference to the value in this TLS slot.
    pub fn with<F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        // Simplified: just call init every time
        // Real implementation would use actual TLS
        let value = (self.init)();
        f(&value)
    }
}
