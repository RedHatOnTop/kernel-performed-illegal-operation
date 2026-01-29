//! Crash Handler
//!
//! Core crash handling logic.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{CrashInfo, CrashType, CrashDump, DumpFormat, CrashReporter};

/// Crash handler
pub struct CrashHandler {
    /// Initialized
    initialized: bool,
    /// Dump configuration
    dump_format: DumpFormat,
    /// Crash history
    crashes: Vec<CrashInfo>,
    /// Max crashes to keep
    max_history: usize,
    /// Reporter
    reporter: Option<CrashReporter>,
    /// Custom panic hook installed
    panic_hook_installed: bool,
}

impl CrashHandler {
    /// Create new crash handler
    pub const fn new() -> Self {
        Self {
            initialized: false,
            dump_format: DumpFormat::Standard,
            crashes: Vec::new(),
            max_history: 10,
            reporter: None,
            panic_hook_installed: false,
        }
    }

    /// Initialize crash handler
    pub fn init(&mut self) {
        if self.initialized {
            return;
        }

        // Register exception handlers
        self.register_exception_handlers();

        // Set up panic hook
        self.install_panic_hook();

        self.initialized = true;
    }

    /// Register CPU exception handlers
    fn register_exception_handlers(&mut self) {
        // Would register IDT handlers for:
        // - Division by zero (#DE)
        // - Invalid opcode (#UD)
        // - Double fault (#DF)
        // - General protection fault (#GP)
        // - Page fault (#PF)
        // - Stack segment fault (#SS)
    }

    /// Install panic hook
    fn install_panic_hook(&mut self) {
        // Would install custom panic handler
        self.panic_hook_installed = true;
    }

    /// Handle a crash
    pub fn handle(&mut self, crash_type: CrashType, message: &str) {
        // Create crash info
        let mut crash = CrashInfo::new(crash_type, message.to_string());
        crash.capture_backtrace(32);

        // Store in history
        self.crashes.push(crash.clone());
        if self.crashes.len() > self.max_history {
            self.crashes.remove(0);
        }

        // Generate dump
        if let Some(dump) = self.generate_dump(&crash) {
            let _ = dump.write_to_storage();
        }

        // Report crash
        if let Some(ref reporter) = self.reporter {
            let _ = reporter.report(&crash);
        }

        // Display crash screen
        self.display_crash_screen(&crash);
    }

    /// Generate crash dump
    fn generate_dump(&self, crash: &CrashInfo) -> Option<CrashDump> {
        let mut dump = CrashDump::new(crash.clone(), self.dump_format);
        
        // Capture stack
        dump.capture_stack(crash.cpu_state.rsp, 0x4000);
        
        // Capture crash area
        if crash.crash_type == CrashType::PageFault {
            dump.capture_crash_area(crash.cpu_state.cr2);
        }
        dump.capture_crash_area(crash.cpu_state.rip);
        
        Some(dump)
    }

    /// Display crash screen (blue screen of death style)
    fn display_crash_screen(&self, crash: &CrashInfo) {
        // Would render to framebuffer
        // For now, just format the message

        let _screen_text = alloc::format!(
            r#"
    ╔══════════════════════════════════════════════════════════════════╗
    ║                        KPIO HAS CRASHED                          ║
    ╠══════════════════════════════════════════════════════════════════╣
    ║                                                                  ║
    ║  Error: {}                                             
    ║                                                                  ║
    ║  {}                                                    
    ║                                                                  ║
    ║  Technical Information:                                          ║
    ║  RIP: {:#018x}                                          ║
    ║  RSP: {:#018x}                                          ║
    ║                                                                  ║
    ║  A crash dump has been saved.                                    ║
    ║                                                                  ║
    ║  Press any key to restart, or wait for automatic restart.        ║
    ╚══════════════════════════════════════════════════════════════════╝
"#,
            crash.crash_type.name(),
            crash.message,
            crash.cpu_state.rip,
            crash.cpu_state.rsp,
        );
    }

    /// Set dump format
    pub fn set_dump_format(&mut self, format: DumpFormat) {
        self.dump_format = format;
    }

    /// Set reporter
    pub fn set_reporter(&mut self, reporter: CrashReporter) {
        self.reporter = Some(reporter);
    }

    /// Get crash history
    pub fn history(&self) -> &[CrashInfo] {
        &self.crashes
    }

    /// Clear crash history
    pub fn clear_history(&mut self) {
        self.crashes.clear();
    }

    /// Get last crash
    pub fn last_crash(&self) -> Option<&CrashInfo> {
        self.crashes.last()
    }
}

impl Default for CrashHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Exception handler for page fault
pub extern "x86-interrupt" fn page_fault_handler(
    _stack_frame: &InterruptStackFrame,
    _error_code: u64,
) {
    // Would be called by IDT
    // handle_crash(CrashType::PageFault, "Page fault");
}

/// Exception handler for double fault
pub extern "x86-interrupt" fn double_fault_handler(
    _stack_frame: &InterruptStackFrame,
    _error_code: u64,
) -> ! {
    // Would be called by IDT
    loop {}
}

/// Interrupt stack frame (placeholder)
#[repr(C)]
pub struct InterruptStackFrame {
    /// Instruction pointer
    pub instruction_pointer: u64,
    /// Code segment
    pub code_segment: u64,
    /// CPU flags
    pub cpu_flags: u64,
    /// Stack pointer
    pub stack_pointer: u64,
    /// Stack segment
    pub stack_segment: u64,
}
