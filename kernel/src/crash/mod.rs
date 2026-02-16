//! Crash Reporting System
//!
//! Kernel-level crash handling, dump generation, and reporting.

mod dump;
mod handler;
mod reporter;
mod symbols;

pub use dump::*;
pub use handler::*;
pub use reporter::*;
pub use symbols::*;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::Mutex;

/// Crash type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashType {
    /// Kernel panic
    Panic,
    /// Page fault
    PageFault,
    /// General protection fault
    GeneralProtection,
    /// Double fault
    DoubleFault,
    /// Stack overflow
    StackOverflow,
    /// Division by zero
    DivisionByZero,
    /// Invalid opcode
    InvalidOpcode,
    /// Assertion failure
    Assertion,
    /// Watchdog timeout
    Watchdog,
    /// Out of memory
    OutOfMemory,
    /// Unknown
    Unknown,
}

impl CrashType {
    /// Get crash type name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Panic => "Kernel Panic",
            Self::PageFault => "Page Fault",
            Self::GeneralProtection => "General Protection Fault",
            Self::DoubleFault => "Double Fault",
            Self::StackOverflow => "Stack Overflow",
            Self::DivisionByZero => "Division by Zero",
            Self::InvalidOpcode => "Invalid Opcode",
            Self::Assertion => "Assertion Failure",
            Self::Watchdog => "Watchdog Timeout",
            Self::OutOfMemory => "Out of Memory",
            Self::Unknown => "Unknown Crash",
        }
    }

    /// Get crash severity
    pub fn severity(&self) -> CrashSeverity {
        match self {
            Self::DoubleFault | Self::StackOverflow => CrashSeverity::Critical,
            Self::Panic | Self::PageFault | Self::GeneralProtection => CrashSeverity::High,
            Self::OutOfMemory | Self::Watchdog => CrashSeverity::Medium,
            _ => CrashSeverity::Low,
        }
    }
}

/// Crash severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CrashSeverity {
    /// Low - recoverable
    Low,
    /// Medium - may be recoverable
    Medium,
    /// High - likely unrecoverable
    High,
    /// Critical - system halt
    Critical,
}

/// CPU state at crash
#[derive(Debug, Clone, Default)]
pub struct CpuState {
    /// RAX register
    pub rax: u64,
    /// RBX register
    pub rbx: u64,
    /// RCX register
    pub rcx: u64,
    /// RDX register
    pub rdx: u64,
    /// RSI register
    pub rsi: u64,
    /// RDI register
    pub rdi: u64,
    /// RBP register
    pub rbp: u64,
    /// RSP register
    pub rsp: u64,
    /// R8-R15 registers
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    /// Instruction pointer
    pub rip: u64,
    /// Flags register
    pub rflags: u64,
    /// CR2 (page fault address)
    pub cr2: u64,
    /// CR3 (page table base)
    pub cr3: u64,
    /// Error code (if applicable)
    pub error_code: u64,
}

impl CpuState {
    /// Capture current CPU state
    pub fn capture() -> Self {
        // Would use inline assembly
        Self::default()
    }
}

/// Stack frame for backtrace
#[derive(Debug, Clone)]
pub struct StackFrame {
    /// Return address
    pub address: u64,
    /// Symbol name (if resolved)
    pub symbol: Option<String>,
    /// Offset within symbol
    pub offset: u64,
    /// Module name
    pub module: Option<String>,
}

impl StackFrame {
    /// Create new stack frame
    pub fn new(address: u64) -> Self {
        Self {
            address,
            symbol: None,
            offset: 0,
            module: None,
        }
    }

    /// Set symbol info
    pub fn with_symbol(mut self, symbol: String, offset: u64) -> Self {
        self.symbol = Some(symbol);
        self.offset = offset;
        self
    }

    /// Format as string
    pub fn to_string(&self) -> String {
        if let Some(ref sym) = self.symbol {
            if let Some(ref module) = self.module {
                alloc::format!(
                    "{:#018x} {}!{} + {:#x}",
                    self.address,
                    module,
                    sym,
                    self.offset
                )
            } else {
                alloc::format!("{:#018x} {} + {:#x}", self.address, sym, self.offset)
            }
        } else {
            alloc::format!("{:#018x} <unknown>", self.address)
        }
    }
}

/// Crash info
#[derive(Debug, Clone)]
pub struct CrashInfo {
    /// Crash type
    pub crash_type: CrashType,
    /// Crash message
    pub message: String,
    /// CPU state
    pub cpu_state: CpuState,
    /// Stack trace
    pub backtrace: Vec<StackFrame>,
    /// Timestamp
    pub timestamp: u64,
    /// Kernel version
    pub kernel_version: String,
    /// Current process ID (if applicable)
    pub process_id: Option<u64>,
    /// Current thread ID (if applicable)
    pub thread_id: Option<u64>,
    /// CPU number
    pub cpu_number: u32,
}

impl CrashInfo {
    /// Create new crash info
    pub fn new(crash_type: CrashType, message: String) -> Self {
        Self {
            crash_type,
            message,
            cpu_state: CpuState::capture(),
            backtrace: Vec::new(),
            timestamp: 0, // Would get current time
            kernel_version: "0.1.0".to_string(),
            process_id: None,
            thread_id: None,
            cpu_number: 0,
        }
    }

    /// Capture backtrace
    pub fn capture_backtrace(&mut self, max_frames: usize) {
        self.backtrace = unwind_stack(self.cpu_state.rbp, max_frames);
    }

    /// Format crash report
    pub fn format_report(&self) -> String {
        let mut report = String::new();

        report.push_str("=== KPIO CRASH REPORT ===\n\n");
        report.push_str(&alloc::format!("Type: {}\n", self.crash_type.name()));
        report.push_str(&alloc::format!("Message: {}\n", self.message));
        report.push_str(&alloc::format!(
            "Severity: {:?}\n",
            self.crash_type.severity()
        ));
        report.push_str(&alloc::format!("Kernel Version: {}\n", self.kernel_version));
        report.push_str(&alloc::format!("CPU: {}\n", self.cpu_number));

        if let Some(pid) = self.process_id {
            report.push_str(&alloc::format!("Process ID: {}\n", pid));
        }
        if let Some(tid) = self.thread_id {
            report.push_str(&alloc::format!("Thread ID: {}\n", tid));
        }

        report.push_str("\n--- CPU State ---\n");
        report.push_str(&alloc::format!("RIP: {:#018x}\n", self.cpu_state.rip));
        report.push_str(&alloc::format!("RSP: {:#018x}\n", self.cpu_state.rsp));
        report.push_str(&alloc::format!("RBP: {:#018x}\n", self.cpu_state.rbp));
        report.push_str(&alloc::format!("RFLAGS: {:#018x}\n", self.cpu_state.rflags));

        if self.crash_type == CrashType::PageFault {
            report.push_str(&alloc::format!("CR2: {:#018x}\n", self.cpu_state.cr2));
        }

        report.push_str("\n--- Backtrace ---\n");
        for (i, frame) in self.backtrace.iter().enumerate() {
            report.push_str(&alloc::format!("#{}: {}\n", i, frame.to_string()));
        }

        report.push_str("\n=== END CRASH REPORT ===\n");

        report
    }
}

/// Unwind stack to get backtrace
fn unwind_stack(mut rbp: u64, max_frames: usize) -> Vec<StackFrame> {
    let mut frames = Vec::new();

    for _ in 0..max_frames {
        if rbp == 0 {
            break;
        }

        // Would read from memory safely
        // let return_addr = unsafe { *(rbp as *const u64).offset(1) };
        // let next_rbp = unsafe { *(rbp as *const u64) };

        // Placeholder
        let return_addr = 0u64;
        let next_rbp = 0u64;

        if return_addr == 0 {
            break;
        }

        frames.push(StackFrame::new(return_addr));
        rbp = next_rbp;
    }

    frames
}

/// Global crash handler state
pub static CRASH_HANDLER: Mutex<CrashHandler> = Mutex::new(CrashHandler::new());

/// Initialize crash handling
pub fn init() {
    let mut handler = CRASH_HANDLER.lock();
    handler.init();
}

/// Handle a crash
pub fn handle_crash(crash_type: CrashType, message: &str) -> ! {
    let mut handler = CRASH_HANDLER.lock();
    handler.handle(crash_type, message);

    // Halt the system
    loop {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
