//! Linux Binary Compatibility — Process Management
//!
//! Provides the `LinuxProcess` struct and `launch_linux_process()` function
//! for running statically-linked Linux ELF binaries on KPIO OS.
//!
//! # Execution Flow
//!
//! 1. Parse ELF binary → `LoadedProgram`
//! 2. Create per-process page table → CR3
//! 3. Load ELF segments into page table (Job 3)
//! 4. Set up user stack with argc/argv/envp/auxv
//! 5. Create `LinuxProcess` struct
//! 6. Build `ProcessContext` for Ring 3 entry
//! 7. Set TSS RSP0 for Ring 3→0 transitions
//! 8. Enter userspace via `iretq`

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::gdt;
use crate::loader::elf::{Elf64Loader, ElfError};
use crate::loader::program::{layout, UserProgram};
use crate::loader::segment_loader::{self, SegmentLoadError};
use crate::memory::user_page_table;
use crate::process::context::ProcessContext;
use crate::process::table::{Process, ProcessId, Thread, ThreadId, PROCESS_TABLE};

/// Kernel stack size for each user process (16 KiB).
const KERNEL_STACK_SIZE: usize = 16 * 1024;

/// Linux process handle returned after launching.
#[derive(Debug)]
pub struct ProcessHandle {
    /// Process ID
    pub pid: ProcessId,
    /// Entry point address
    pub entry_point: u64,
    /// Initial stack pointer
    pub initial_sp: u64,
    /// Page table root (CR3 value)
    pub cr3: u64,
}

/// Errors from Linux process creation.
#[derive(Debug)]
pub enum LinuxProcessError {
    /// ELF parsing failed
    ElfError(ElfError),
    /// Segment loading failed
    SegmentLoadError(SegmentLoadError),
    /// Page table creation failed
    PageTableError(&'static str),
    /// Memory allocation failed
    OutOfMemory,
    /// Process table full
    ProcessLimitReached,
}

impl From<ElfError> for LinuxProcessError {
    fn from(e: ElfError) -> Self {
        LinuxProcessError::ElfError(e)
    }
}

impl From<SegmentLoadError> for LinuxProcessError {
    fn from(e: SegmentLoadError) -> Self {
        LinuxProcessError::SegmentLoadError(e)
    }
}

impl core::fmt::Display for LinuxProcessError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ElfError(e) => write!(f, "ELF error: {:?}", e),
            Self::SegmentLoadError(e) => write!(f, "Segment load error: {}", e),
            Self::PageTableError(e) => write!(f, "Page table error: {}", e),
            Self::OutOfMemory => write!(f, "Out of memory"),
            Self::ProcessLimitReached => write!(f, "Process limit reached"),
        }
    }
}

/// Linux process state held in the process table.
///
/// This extends the generic `Process` with Linux-specific fields
/// needed for binary compatibility.
#[derive(Debug)]
pub struct LinuxProcessInfo {
    /// Page table root physical address (loaded into CR3)
    pub cr3: u64,
    /// Entry point of the ELF binary
    pub entry_point: u64,
    /// Current stack pointer
    pub stack_pointer: u64,
    /// Current program break (heap end)
    pub brk: u64,
    /// Working directory
    pub cwd: String,
    /// Process kernel stack (top address)
    pub kernel_stack_top: u64,
    /// Process kernel stack size
    pub kernel_stack_size: usize,
}

/// Launch a Linux ELF binary as a new process.
///
/// This is the main entry point for running Linux binaries on KPIO.
///
/// # Arguments
///
/// * `name` - Process name
/// * `elf_binary` - Raw ELF binary data
/// * `args` - Command line arguments (argv)
/// * `envp` - Environment variables
/// * `parent` - Parent process ID
///
/// # Returns
///
/// `ProcessHandle` with PID, entry point, and page table root.
///
/// # Process
///
/// 1. Parse ELF binary
/// 2. Create per-process page table (copy kernel half-space)
/// 3. Load ELF segments into the page table
/// 4. Set up user stack (argc/argv/envp/auxv)
/// 5. Allocate kernel stack for Ring 3→0 transitions
/// 6. Create Process + Thread in process table
/// 7. Return handle (actual scheduling happens separately)
pub fn launch_linux_process(
    name: String,
    elf_binary: &[u8],
    args: Vec<String>,
    envp: Vec<String>,
    parent: ProcessId,
) -> Result<ProcessHandle, LinuxProcessError> {
    // Step 1: Parse ELF
    let loaded = Elf64Loader::parse(elf_binary)?;

    crate::serial_println!(
        "[KPIO/Linux] Parsed ELF: entry={:#x}, {} segments, pie={}",
        loaded.entry_point,
        loaded.segments.len(),
        loaded.is_pie
    );

    // Step 2: Create per-process page table
    let cr3 = user_page_table::create_user_page_table()
        .map_err(LinuxProcessError::PageTableError)?;

    crate::serial_println!("[KPIO/Linux] Created page table: CR3={:#x}", cr3);

    // Step 3: Load ELF segments
    let pie_base = if loaded.is_pie { layout::PIE_BASE } else { 0 };
    let load_result = segment_loader::load_elf_segments(cr3, &loaded, elf_binary, pie_base)?;

    crate::serial_println!(
        "[KPIO/Linux] Loaded {} pages, entry={:#x}, brk={:#x}",
        load_result.pages_mapped,
        load_result.entry_point,
        load_result.brk_start
    );

    // Step 4: Build UserProgram for auxv/stack calculation
    let user_program = UserProgram::new(name.clone(), loaded, args.clone(), envp.clone());

    // Step 5: Set up user stack
    let initial_sp = segment_loader::setup_user_stack(
        cr3,
        load_result.initial_sp,
        &args,
        &envp,
        &user_program.auxv,
    )?;

    crate::serial_println!("[KPIO/Linux] Stack setup: SP={:#x}", initial_sp);

    // Step 6: Allocate kernel stack for this process
    // The kernel stack is used when Ring 3 → Ring 0 transitions occur
    let kernel_stack = allocate_kernel_stack(KERNEL_STACK_SIZE);
    // kernel_stack_top is stored in Thread for use during context switches
    let _kernel_stack_top = kernel_stack + KERNEL_STACK_SIZE as u64;

    // Step 7: Create process context for Ring 3 entry
    let context = ProcessContext::new_user(
        load_result.entry_point,
        initial_sp,
        gdt::USER_CS,
        gdt::USER_DS,
    );

    // Step 8: Create Process and Thread
    let mut process = Process::new(name.clone(), parent, cr3);

    let main_thread = Thread {
        tid: ThreadId::new(),
        state: crate::process::table::ProcessState::Ready,
        context,
        kernel_stack,
        kernel_stack_size: KERNEL_STACK_SIZE,
        user_stack: initial_sp,
        user_stack_size: layout::USER_STACK_SIZE as usize,
        tls: 0,
    };

    process.add_thread(main_thread);
    process.program = Some(user_program);
    process.set_ready();

    let pid = PROCESS_TABLE.add(process);

    crate::serial_println!(
        "[KPIO/Linux] Process '{}' created: PID={}, CR3={:#x}, entry={:#x}",
        name,
        pid,
        cr3,
        load_result.entry_point
    );

    Ok(ProcessHandle {
        pid,
        entry_point: load_result.entry_point,
        initial_sp,
        cr3,
    })
}

/// Switch to a Linux process's address space.
///
/// Updates CR3 to the process's page table and sets TSS RSP0
/// for Ring 3 → Ring 0 transitions.
///
/// # Safety
///
/// The CR3 value must be a valid page table with kernel half-space
/// properly mapped. Must be called with interrupts disabled.
pub unsafe fn switch_to_process(cr3: u64, kernel_stack_top: u64) {
    use x86_64::registers::control::Cr3;
    use x86_64::structures::paging::PhysFrame;
    use x86_64::PhysAddr;
    use x86_64::VirtAddr;

    // Set kernel stack in TSS for Ring 3 → Ring 0 transitions
    gdt::set_kernel_stack(VirtAddr::new(kernel_stack_top));

    // Switch page tables
    let frame = PhysFrame::containing_address(PhysAddr::new(cr3));
    unsafe {
        Cr3::write(frame, Cr3::read().1);
    }
}

/// Enter userspace for a Linux process.
///
/// This is the final step: switches to the process's address space
/// and performs `iretq` to begin executing user code.
///
/// # Safety
///
/// - The process context must have valid user CS/SS selectors
/// - The user page table must be properly set up
/// - The kernel stack must be valid
/// - This function never returns
pub unsafe fn enter_linux_process(
    cr3: u64,
    kernel_stack_top: u64,
    context: &ProcessContext,
) -> ! {
    // Switch address space and set kernel stack
    unsafe {
        switch_to_process(cr3, kernel_stack_top);
    }

    // Transfer control to userspace via iretq
    unsafe {
        crate::process::context::enter_userspace(context as *const ProcessContext);
    }
}

/// Allocate a kernel stack for a process.
///
/// Returns the base (bottom) address of the allocated stack.
/// The stack grows downward, so the top = base + size.
fn allocate_kernel_stack(size: usize) -> u64 {
    // Use the heap allocator to get kernel stack memory
    use alloc::alloc::{alloc, Layout};

    let layout = Layout::from_size_align(size, 16).expect("Invalid kernel stack layout");
    let ptr = unsafe { alloc(layout) };

    if ptr.is_null() {
        panic!("Failed to allocate kernel stack ({} bytes)", size);
    }

    ptr as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_stack_size() {
        assert_eq!(KERNEL_STACK_SIZE, 16 * 1024);
    }

    #[test]
    fn test_user_selectors() {
        // Verify we use the correct selectors from GDT
        assert_eq!(gdt::USER_CS, 0x23);
        assert_eq!(gdt::USER_DS, 0x1B);
    }

    #[test]
    fn test_process_handle_fields() {
        let handle = ProcessHandle {
            pid: ProcessId::new(),
            entry_point: 0x400000,
            initial_sp: 0x7FFFFFFFE000,
            cr3: 0x1000,
        };
        assert_eq!(handle.entry_point, 0x400000);
        assert_eq!(handle.initial_sp, 0x7FFFFFFFE000);
        assert_eq!(handle.cr3, 0x1000);
    }

    #[test]
    fn test_error_display() {
        let err = LinuxProcessError::OutOfMemory;
        let msg = alloc::format!("{}", err);
        assert_eq!(msg, "Out of memory");

        let err = LinuxProcessError::ProcessLimitReached;
        let msg = alloc::format!("{}", err);
        assert_eq!(msg, "Process limit reached");
    }
}
