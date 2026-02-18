//! Process Manager
//!
//! High-level process management including creation, loading, and lifecycle.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::context::ProcessContext;
use super::table::{Process, ProcessId, ProcessState, Thread, ThreadId, PROCESS_TABLE};
use crate::loader::elf::{Elf64Loader, ElfError};
use crate::loader::program::UserProgram;

/// Kernel stack size (16KB)
const KERNEL_STACK_SIZE: usize = 16 * 1024;

/// User code segment selector (Ring 3) — from GDT
/// GDT index 4, byte offset 0x20, RPL 3 → 0x23
const USER_CS: u16 = crate::gdt::USER_CS;

/// User data segment selector (Ring 3) — from GDT
/// GDT index 3, byte offset 0x18, RPL 3 → 0x1B
const USER_DS: u16 = crate::gdt::USER_DS;

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
}

impl From<ElfError> for ProcessError {
    fn from(e: ElfError) -> Self {
        ProcessError::ElfError(e)
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

    /// Create a new process from ELF binary
    ///
    /// # Arguments
    ///
    /// * `name` - Process name
    /// * `binary` - ELF binary data
    /// * `args` - Command line arguments
    /// * `envp` - Environment variables
    /// * `parent` - Parent process ID
    ///
    /// # Returns
    ///
    /// Process ID of the new process
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
