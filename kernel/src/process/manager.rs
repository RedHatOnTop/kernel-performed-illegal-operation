//! Process Manager
//!
//! High-level process management including creation, loading, and lifecycle.
//! `spawn_from_vfs()` loads an ELF binary from the VFS, creates a user page
//! table, loads segments, allocates a kernel stack, and enqueues the new
//! process for scheduling.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::context::ProcessContext;
use super::table::{
    LinuxMemoryInfo, Process, ProcessId, ProcessState, Thread, ThreadId, Vma, MMAP_BASE,
    PROCESS_TABLE,
};
use crate::loader::elf::{Elf64Loader, ElfError};
use crate::loader::program::UserProgram;
use crate::loader::segment_loader::{self, SegmentLoadError};

/// Kernel stack size (16KB)
const KERNEL_STACK_SIZE: usize = 16 * 1024;

/// User code segment selector (Ring 3) — from GDT
/// GDT index 4, byte offset 0x20, RPL 3 → 0x23
const USER_CS: u16 = crate::gdt::USER_CS;

/// User data segment selector (Ring 3) — from GDT
/// GDT index 3, byte offset 0x18, RPL 3 → 0x1B
const USER_DS: u16 = crate::gdt::USER_DS;

/// Kernel stack allocation size for user-space process tasks (32 KiB).
const USER_KERNEL_STACK_SIZE: usize = 32 * 1024;

/// Process creation error
#[derive(Debug)]
pub enum ProcessError {
    /// ELF parsing failed
    ElfError(ElfError),
    /// Memory allocation failed
    OutOfMemory,
    /// Process limit reached
    ProcessLimitReached,
    /// Invalid argument
    InvalidArgument(&'static str),
    /// Parent process not found
    ParentNotFound,
    /// VFS error (file not found, I/O error, etc.)
    VfsError(&'static str),
    /// Segment loading error
    SegmentLoadError(SegmentLoadError),
}

impl From<ElfError> for ProcessError {
    fn from(e: ElfError) -> Self {
        ProcessError::ElfError(e)
    }
}

impl From<SegmentLoadError> for ProcessError {
    fn from(e: SegmentLoadError) -> Self {
        ProcessError::SegmentLoadError(e)
    }
}

/// Process manager
pub struct ProcessManager {
    /// Maximum number of processes
    max_processes: usize,
}

impl ProcessManager {
    /// Create a new process manager
    pub const fn new() -> Self {
        Self {
            max_processes: 1024,
        }
    }

    /// Initialize the process manager
    pub fn init(&self) {
        PROCESS_TABLE.init();
        crate::serial_println!("[KPIO] Process manager initialized");
    }

    /// Spawn a new process by loading an ELF binary from the VFS.
    ///
    /// This is the primary process creation entry point.  It performs
    /// the full pipeline: VFS read → ELF parse → page table creation →
    /// segment loading → kernel stack allocation → scheduler enqueue.
    ///
    /// # Arguments
    ///
    /// * `path` - VFS path to the ELF binary (e.g., "/test/hello")
    ///
    /// # Returns
    ///
    /// `Ok(ProcessId)` of the newly created process.
    pub fn spawn_from_vfs(&self, path: &str) -> Result<ProcessId, ProcessError> {
        self.spawn_from_vfs_with_args(path, Vec::new(), Vec::new())
    }

    /// Spawn a new process from a VFS path with arguments and environment.
    ///
    /// # Arguments
    ///
    /// * `path` - VFS path to the ELF binary
    /// * `argv` - Command-line arguments (argv[0] is typically the program name)
    /// * `envp` - Environment variables (KEY=VALUE strings)
    pub fn spawn_from_vfs_with_args(
        &self,
        path: &str,
        argv: Vec<String>,
        envp: Vec<String>,
    ) -> Result<ProcessId, ProcessError> {
        // 1. Check process limit
        if PROCESS_TABLE.count() >= self.max_processes {
            return Err(ProcessError::ProcessLimitReached);
        }

        // 2. Read ELF bytes from VFS
        let elf_bytes = crate::vfs::read_all(path).map_err(|_| ProcessError::VfsError("failed to read ELF from VFS"))?;
        if elf_bytes.is_empty() {
            return Err(ProcessError::VfsError("empty ELF binary"));
        }

        // 3. Parse ELF
        let loaded = Elf64Loader::parse(&elf_bytes)?;

        // 4. Create user page table
        let cr3 = crate::memory::user_page_table::create_user_page_table()
            .map_err(|_| ProcessError::OutOfMemory)?;

        // 5. Load ELF segments into the page table
        let pie_base = if loaded.is_pie {
            crate::loader::program::layout::PIE_BASE
        } else {
            0
        };
        let load_result = segment_loader::load_elf_segments(cr3, &loaded, &elf_bytes, pie_base)?;

        crate::serial_println!(
            "[SPAWN] loaded '{}' pid=pending cr3={:#x} entry={:#x} sp={:#x} brk={:#x} ({} pages)",
            path,
            cr3,
            load_result.entry_point,
            load_result.initial_sp,
            load_result.brk_start,
            load_result.pages_mapped,
        );

        // 6. Allocate kernel stack for the new process
        let kernel_stack_vec: Vec<u8> = vec![0u8; USER_KERNEL_STACK_SIZE];
        let kernel_stack_top = kernel_stack_vec.as_ptr() as u64 + USER_KERNEL_STACK_SIZE as u64;

        // 7. Create Process entry
        let name = extract_name_from_path(path);
        let mut process = Process::new(name.clone(), ProcessId::KERNEL, cr3);

        // 8. Set up Linux memory info (brk, mmap)
        process.linux_memory = Some(LinuxMemoryInfo {
            cr3,
            brk_start: load_result.brk_start,
            brk_current: load_result.brk_start,
            vma_list: Vec::new(),
            mmap_next_addr: MMAP_BASE,
        });

        let pid = process.pid;

        // 9. Create user program metadata
        let user_program = UserProgram::new(name.clone(), loaded, argv, envp);

        // 10. Create main thread (for ProcessTable bookkeeping)
        let context = ProcessContext::new_user(
            load_result.entry_point,
            load_result.initial_sp,
            USER_CS,
            USER_DS,
        );
        let main_thread = Thread {
            tid: ThreadId::new(),
            state: ProcessState::Ready,
            context,
            kernel_stack: kernel_stack_top,
            kernel_stack_size: USER_KERNEL_STACK_SIZE,
            user_stack: load_result.initial_sp,
            user_stack_size: crate::loader::program::layout::USER_STACK_SIZE as usize,
            tls: 0,
            clear_child_tid: 0,
        };
        process.add_thread(main_thread);
        process.program = Some(user_program);
        process.set_ready();

        // 11. Register in process table
        PROCESS_TABLE.add(process);

        // 12. Create scheduler task and enqueue
        let task = crate::scheduler::Task::new_user_process(
            &name,
            cr3,
            load_result.entry_point,
            load_result.initial_sp,
            USER_CS,
            USER_DS,
            kernel_stack_top,
            kernel_stack_vec,
            pid.as_u64(),
        );
        crate::scheduler::spawn(task);

        crate::serial_println!(
            "[SPAWN] loaded '{}' pid={} cr3={:#x}",
            path,
            pid,
            cr3,
        );

        Ok(pid)
    }

    /// Create a new process from an in-memory ELF binary (legacy API).
    ///
    /// This is the original `spawn()` that takes raw ELF bytes.
    /// Prefer `spawn_from_vfs()` for loading from the filesystem.
    pub fn spawn(
        &self,
        name: String,
        binary: &[u8],
        args: Vec<String>,
        envp: Vec<String>,
        parent: ProcessId,
    ) -> Result<ProcessId, ProcessError> {
        // Check process limit
        if PROCESS_TABLE.count() >= self.max_processes {
            return Err(ProcessError::ProcessLimitReached);
        }

        // Parse ELF
        let loaded = Elf64Loader::parse(binary)?;

        // Create user program
        let user_program = UserProgram::new(name.clone(), loaded, args, envp);

        // Create process (page table will be set up later)
        // For now, we use 0 as placeholder
        let mut process = Process::new(
            name, parent, 0, // Page table root - to be set up
        );

        // Calculate initial stack pointer
        let initial_sp = user_program.calculate_stack_layout();

        // Create main thread
        let context =
            ProcessContext::new_user(user_program.entry_point, initial_sp, USER_CS, USER_DS);

        let main_thread = Thread {
            tid: ThreadId::new(),
            state: ProcessState::Ready,
            context,
            kernel_stack: 0, // To be allocated
            kernel_stack_size: KERNEL_STACK_SIZE,
            user_stack: initial_sp,
            user_stack_size: crate::loader::program::layout::USER_STACK_SIZE as usize,
            tls: 0,
            clear_child_tid: 0,
        };

        process.add_thread(main_thread);
        process.program = Some(user_program);
        process.set_ready();

        // Add to process table
        let pid = PROCESS_TABLE.add(process);

        let process_name = PROCESS_TABLE
            .get(pid)
            .map(|g| {
                g.get(&pid)
                    .map(|p| alloc::string::String::from(p.name.as_str()))
                    .unwrap_or_else(|| alloc::string::String::from("?"))
            })
            .unwrap_or_else(|| alloc::string::String::from("?"));

        crate::serial_println!("[KPIO] Created process {} (PID {})", process_name, pid);

        Ok(pid)
    }

    /// Fork the current process
    ///
    /// Creates a copy of the calling process
    pub fn fork(&self, parent_pid: ProcessId) -> Result<ProcessId, ProcessError> {
        // This is a placeholder - full implementation requires:
        // 1. Copy page table with COW
        // 2. Copy file descriptors
        // 3. Copy thread state

        if PROCESS_TABLE.count() >= self.max_processes {
            return Err(ProcessError::ProcessLimitReached);
        }

        // For now, return an error as fork is complex
        Err(ProcessError::InvalidArgument("Fork not yet implemented"))
    }

    /// Exit a process
    pub fn exit(&self, pid: ProcessId, exit_code: i32) {
        // Get process and mark as zombie
        if let Some(guard) = PROCESS_TABLE.get(pid) {
            if let Some(_proc) = guard.get(&pid) {
                // Process will be cleaned up by parent's wait()
                drop(guard);

                // Actually modify the process
                // (This is a simplification - proper implementation would use interior mutability)
                crate::serial_println!("[KPIO] Process {} exited with code {}", pid, exit_code);
            }
        }
    }

    /// Kill a process
    pub fn kill(&self, pid: ProcessId, signal: i32) -> Result<(), ProcessError> {
        if pid == ProcessId::KERNEL {
            return Err(ProcessError::InvalidArgument("Cannot kill kernel"));
        }

        if let Some(_guard) = PROCESS_TABLE.get(pid) {
            // Send signal to process
            crate::serial_println!("[KPIO] Sending signal {} to process {}", signal, pid);
            Ok(())
        } else {
            Err(ProcessError::ParentNotFound)
        }
    }

    /// Get process info
    pub fn get_info(&self, pid: ProcessId) -> Option<ProcessInfo> {
        PROCESS_TABLE.get(pid).and_then(|guard| {
            guard.get(&pid).map(|proc| ProcessInfo {
                pid: proc.pid,
                parent: proc.parent,
                name: proc.name.clone(),
                state: proc.state,
                thread_count: proc.threads.len(),
            })
        })
    }

    /// List all processes
    pub fn list(&self) -> Vec<ProcessInfo> {
        let mut result = Vec::new();
        PROCESS_TABLE.for_each(|_pid, proc| {
            result.push(ProcessInfo {
                pid: proc.pid,
                parent: proc.parent,
                name: proc.name.clone(),
                state: proc.state,
                thread_count: proc.threads.len(),
            });
        });
        result
    }
}

/// Extract a short process name from a VFS path.
///
/// e.g., "/test/hello" → "hello", "/bin/init" → "init"
fn extract_name_from_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rsplit_once('/') {
        Some((_, name)) if !name.is_empty() => String::from(name),
        _ => String::from(trimmed),
    }
}

/// Process information (read-only view)
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: ProcessId,
    /// Parent process ID
    pub parent: ProcessId,
    /// Process name
    pub name: String,
    /// Current state
    pub state: ProcessState,
    /// Number of threads
    pub thread_count: usize,
}

/// Global process manager instance
pub static PROCESS_MANAGER: ProcessManager = ProcessManager::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = ProcessManager::new();
        assert_eq!(manager.max_processes, 1024);
    }
}
