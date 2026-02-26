//! Process Table
//!
//! Maintains the global table of all processes in the system.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::RwLock;

use super::context::ProcessContext;
use crate::loader::program::UserProgram;
use crate::process::signal::SignalState;

// ═══════════════════════════════════════════════════════════════════════
// Linux memory management structures
// ═══════════════════════════════════════════════════════════════════════

/// Virtual Memory Area — tracks a mapped region in the process address space.
#[derive(Debug, Clone)]
pub struct Vma {
    /// Start virtual address (page-aligned)
    pub start: u64,
    /// End virtual address (exclusive, page-aligned)
    pub end: u64,
    /// Protection flags (Linux PROT_READ=1, PROT_WRITE=2, PROT_EXEC=4)
    pub prot: u32,
    /// Map flags (Linux MAP_PRIVATE=0x02, MAP_ANONYMOUS=0x20, etc.)
    pub flags: u32,
}

/// Linux-specific memory management state per process.
///
/// Tracks the program break (brk/sbrk) and memory mapped regions (mmap).
#[derive(Debug, Clone)]
pub struct LinuxMemoryInfo {
    /// Page table root physical address (CR3)
    pub cr3: u64,
    /// Initial program break (page-aligned end of loaded segments)
    pub brk_start: u64,
    /// Current program break
    pub brk_current: u64,
    /// List of mapped virtual memory areas (mmap'd regions)
    pub vma_list: Vec<Vma>,
    /// Next mmap hint address (starts at 0x7F00_0000_0000, decrements)
    pub mmap_next_addr: u64,
}

/// Starting address for mmap allocations (descends from here).
pub const MMAP_BASE: u64 = 0x7F00_0000_0000;

/// Maximum heap size (256 MB).
pub const MAX_HEAP_SIZE: u64 = 256 * 1024 * 1024;

/// Process ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(pub u64);

impl ProcessId {
    /// Kernel process ID (always 0)
    pub const KERNEL: ProcessId = ProcessId(0);

    /// Init process ID (always 1)
    pub const INIT: ProcessId = ProcessId(1);

    /// Generate a new unique process ID
    pub fn new() -> Self {
        static NEXT_PID: AtomicU64 = AtomicU64::new(2);
        ProcessId(NEXT_PID.fetch_add(1, Ordering::SeqCst))
    }

    /// Create a ProcessId from a raw u64 value
    pub fn from_u64(val: u64) -> Self {
        ProcessId(val)
    }

    /// Get the raw ID value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for ProcessId {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Display for ProcessId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Process state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    /// Process is being created
    Creating,
    /// Process is ready to run
    Ready,
    /// Process is currently running
    Running,
    /// Process is blocked waiting for something
    Blocked(BlockReason),
    /// Process has exited but not yet cleaned up (zombie)
    Zombie(i32),
    /// Process has been terminated
    Dead,
}

/// Reason for blocking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    /// Waiting for I/O
    Io,
    /// Waiting for a child process
    WaitChild,
    /// Waiting for a futex
    Futex,
    /// Waiting for IPC message
    Ipc,
    /// Sleeping
    Sleep,
    /// Waiting for GPU operation
    Gpu,
}

/// Thread ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThreadId(pub u64);

impl ThreadId {
    /// Generate a new unique thread ID
    pub fn new() -> Self {
        static NEXT_TID: AtomicU64 = AtomicU64::new(1);
        ThreadId(NEXT_TID.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for ThreadId {
    fn default() -> Self {
        Self::new()
    }
}

/// A single thread within a process
#[derive(Debug)]
pub struct Thread {
    /// Thread ID
    pub tid: ThreadId,
    /// Thread state
    pub state: ProcessState,
    /// CPU context
    pub context: ProcessContext,
    /// Kernel stack base address
    pub kernel_stack: u64,
    /// Kernel stack size
    pub kernel_stack_size: usize,
    /// User stack base address
    pub user_stack: u64,
    /// User stack size
    pub user_stack_size: usize,
    /// Thread-local storage pointer
    pub tls: u64,
}

/// A process (address space + threads)
pub struct Process {
    /// Process ID
    pub pid: ProcessId,
    /// Parent process ID
    pub parent: ProcessId,
    /// Process name
    pub name: String,
    /// Process state (based on main thread)
    pub state: ProcessState,
    /// Page table root physical address (CR3 value)
    pub page_table_root: u64,
    /// Threads in this process
    pub threads: Vec<Thread>,
    /// Main thread ID
    pub main_thread: ThreadId,
    /// User program info
    pub program: Option<UserProgram>,
    /// Exit code (if exited)
    pub exit_code: Option<i32>,
    /// Open file descriptors
    pub file_descriptors: BTreeMap<u32, FileDescriptor>,
    /// Next file descriptor number
    pub next_fd: u32,
    /// Working directory
    pub cwd: String,
    /// User ID
    pub uid: u32,
    /// Group ID
    pub gid: u32,
    /// Linux memory management state (brk, mmap VMAs)
    pub linux_memory: Option<LinuxMemoryInfo>,
    /// Signal state (pending signals, blocked mask, handlers)
    pub signals: SignalState,
}

/// File descriptor entry
#[derive(Debug, Clone)]
pub struct FileDescriptor {
    /// File descriptor number
    pub fd: u32,
    /// Resource type
    pub resource: FileResource,
    /// Flags
    pub flags: u32,
    /// Current offset
    pub offset: u64,
}

/// Type of file resource
#[derive(Debug, Clone)]
pub enum FileResource {
    /// Regular file
    File { path: String },
    /// Pipe
    Pipe { buffer_id: u64 },
    /// Socket
    Socket { socket_id: u64 },
    /// Standard I/O
    Stdio(StdioType),
    /// IPC channel
    Channel { channel_id: u64 },
}

/// Standard I/O type
#[derive(Debug, Clone, Copy)]
pub enum StdioType {
    Stdin,
    Stdout,
    Stderr,
}

impl Process {
    /// Create a new kernel process
    pub fn kernel() -> Self {
        Self {
            pid: ProcessId::KERNEL,
            parent: ProcessId::KERNEL,
            name: String::from("kernel"),
            state: ProcessState::Running,
            page_table_root: 0, // Kernel uses identity mapping
            threads: Vec::new(),
            main_thread: ThreadId(0),
            program: None,
            exit_code: None,
            file_descriptors: BTreeMap::new(),
            next_fd: 3, // 0, 1, 2 reserved for stdio
            cwd: String::from("/"),
            uid: 0,
            gid: 0,
            linux_memory: None,
            signals: SignalState::new(),
        }
    }

    /// Create a new user process
    pub fn new(name: String, parent: ProcessId, page_table_root: u64) -> Self {
        let pid = ProcessId::new();

        // Set up standard file descriptors
        let mut file_descriptors = BTreeMap::new();
        file_descriptors.insert(
            0,
            FileDescriptor {
                fd: 0,
                resource: FileResource::Stdio(StdioType::Stdin),
                flags: 0,
                offset: 0,
            },
        );
        file_descriptors.insert(
            1,
            FileDescriptor {
                fd: 1,
                resource: FileResource::Stdio(StdioType::Stdout),
                flags: 0,
                offset: 0,
            },
        );
        file_descriptors.insert(
            2,
            FileDescriptor {
                fd: 2,
                resource: FileResource::Stdio(StdioType::Stderr),
                flags: 0,
                offset: 0,
            },
        );

        Self {
            pid,
            parent,
            name,
            state: ProcessState::Creating,
            page_table_root,
            threads: Vec::new(),
            main_thread: ThreadId(0),
            program: None,
            exit_code: None,
            file_descriptors,
            next_fd: 3,
            cwd: String::from("/"),
            uid: 0,
            gid: 0,
            linux_memory: None,
            signals: SignalState::new(),
        }
    }

    /// Add a thread to this process
    pub fn add_thread(&mut self, thread: Thread) -> ThreadId {
        let tid = thread.tid;
        if self.threads.is_empty() {
            self.main_thread = tid;
        }
        self.threads.push(thread);
        tid
    }

    /// Get a thread by ID
    pub fn get_thread(&self, tid: ThreadId) -> Option<&Thread> {
        self.threads.iter().find(|t| t.tid == tid)
    }

    /// Get a mutable thread by ID
    pub fn get_thread_mut(&mut self, tid: ThreadId) -> Option<&mut Thread> {
        self.threads.iter_mut().find(|t| t.tid == tid)
    }

    /// Get the main thread
    pub fn main_thread(&self) -> Option<&Thread> {
        self.get_thread(self.main_thread)
    }

    /// Allocate a new file descriptor
    pub fn alloc_fd(&mut self) -> u32 {
        let fd = self.next_fd;
        self.next_fd += 1;
        fd
    }

    /// Add a file descriptor
    pub fn add_fd(&mut self, fd: FileDescriptor) {
        self.file_descriptors.insert(fd.fd, fd);
    }

    /// Get a file descriptor
    pub fn get_fd(&self, fd: u32) -> Option<&FileDescriptor> {
        self.file_descriptors.get(&fd)
    }

    /// Remove a file descriptor
    pub fn remove_fd(&mut self, fd: u32) -> Option<FileDescriptor> {
        self.file_descriptors.remove(&fd)
    }

    /// Mark process as ready
    pub fn set_ready(&mut self) {
        self.state = ProcessState::Ready;
        for thread in &mut self.threads {
            thread.state = ProcessState::Ready;
        }
    }

    /// Mark process as exited
    pub fn set_exited(&mut self, exit_code: i32) {
        self.state = ProcessState::Zombie(exit_code);
        self.exit_code = Some(exit_code);
    }
}

/// Global process table
pub struct ProcessTable {
    /// All processes indexed by PID
    processes: RwLock<BTreeMap<ProcessId, Process>>,
    /// Currently running process on each CPU
    current: RwLock<BTreeMap<usize, ProcessId>>,
}

impl ProcessTable {
    /// Create a new empty process table
    pub const fn new() -> Self {
        Self {
            processes: RwLock::new(BTreeMap::new()),
            current: RwLock::new(BTreeMap::new()),
        }
    }

    /// Initialize with kernel process
    pub fn init(&self) {
        let kernel = Process::kernel();
        self.processes.write().insert(ProcessId::KERNEL, kernel);
    }

    /// Add a new process
    pub fn add(&self, process: Process) -> ProcessId {
        let pid = process.pid;
        self.processes.write().insert(pid, process);
        pid
    }

    /// Remove a process
    pub fn remove(&self, pid: ProcessId) -> Option<Process> {
        self.processes.write().remove(&pid)
    }

    /// Get a process by PID (read-only)
    pub fn get(
        &self,
        pid: ProcessId,
    ) -> Option<spin::RwLockReadGuard<'_, BTreeMap<ProcessId, Process>>> {
        let guard = self.processes.read();
        if guard.contains_key(&pid) {
            Some(guard)
        } else {
            None
        }
    }

    /// Execute a closure with mutable access to a process.
    ///
    /// Returns `None` if the process does not exist.
    pub fn with_process_mut<F, R>(&self, pid: ProcessId, f: F) -> Option<R>
    where
        F: FnOnce(&mut Process) -> R,
    {
        let mut guard = self.processes.write();
        guard.get_mut(&pid).map(f)
    }

    /// Set the current process for a CPU
    pub fn set_current(&self, cpu: usize, pid: ProcessId) {
        self.current.write().insert(cpu, pid);
    }

    /// Get the current process for a CPU
    pub fn get_current(&self, cpu: usize) -> Option<ProcessId> {
        self.current.read().get(&cpu).copied()
    }

    /// Get count of processes
    pub fn count(&self) -> usize {
        self.processes.read().len()
    }

    /// Iterate over all processes (for debugging)
    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&ProcessId, &Process),
    {
        for (pid, proc) in self.processes.read().iter() {
            f(pid, proc);
        }
    }

    /// Get a snapshot of all processes (cloned PIDs and basic info).
    ///
    /// Returns a Vec of (ProcessId, ProcessSnapshot) for iteration
    /// without holding the lock.
    pub fn processes_snapshot(&self) -> alloc::vec::Vec<(ProcessId, ProcessSnapshot)> {
        let guard = self.processes.read();
        guard
            .iter()
            .map(|(pid, proc)| {
                (
                    *pid,
                    ProcessSnapshot {
                        pid: proc.pid,
                        parent: proc.parent,
                        state: proc.state,
                        name: proc.name.clone(),
                    },
                )
            })
            .collect()
    }
}

/// Lightweight snapshot of a process (for wait4 and other lookups).
#[derive(Debug, Clone)]
pub struct ProcessSnapshot {
    pub pid: ProcessId,
    pub parent: ProcessId,
    pub state: ProcessState,
    pub name: String,
}

/// Global process table instance
pub static PROCESS_TABLE: ProcessTable = ProcessTable::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid_generation() {
        let pid1 = ProcessId::new();
        let pid2 = ProcessId::new();
        assert_ne!(pid1, pid2);
        assert!(pid1.0 < pid2.0);
    }

    #[test]
    fn test_process_creation() {
        let proc = Process::new(String::from("test"), ProcessId::KERNEL, 0x1000);

        assert_ne!(proc.pid, ProcessId::KERNEL);
        assert_eq!(proc.parent, ProcessId::KERNEL);
        assert_eq!(proc.state, ProcessState::Creating);
        assert!(proc.get_fd(0).is_some()); // stdin
        assert!(proc.get_fd(1).is_some()); // stdout
        assert!(proc.get_fd(2).is_some()); // stderr
    }
}
