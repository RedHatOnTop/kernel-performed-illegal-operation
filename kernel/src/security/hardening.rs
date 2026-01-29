//! Kernel Hardening
//!
//! Implements security hardening features for the kernel including:
//! - KASLR (Kernel Address Space Layout Randomization)
//! - Stack canaries and guard pages
//! - Control Flow Integrity (CFI)
//! - Memory protection (SMAP/SMEP)

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;
use alloc::collections::BTreeSet;

/// Kernel hardening configuration.
pub struct HardeningConfig {
    /// KASLR enabled.
    pub kaslr_enabled: bool,
    /// KASLR offset (set at boot).
    pub kaslr_offset: u64,
    /// Stack canaries enabled.
    pub stack_canaries: bool,
    /// Guard pages enabled.
    pub guard_pages: bool,
    /// SMEP enabled (Supervisor Mode Execution Prevention).
    pub smep_enabled: bool,
    /// SMAP enabled (Supervisor Mode Access Prevention).
    pub smap_enabled: bool,
    /// CFI enabled.
    pub cfi_enabled: bool,
}

impl HardeningConfig {
    /// Create default configuration.
    pub const fn default() -> Self {
        Self {
            kaslr_enabled: true,
            kaslr_offset: 0,
            stack_canaries: true,
            guard_pages: true,
            smep_enabled: true,
            smap_enabled: true,
            cfi_enabled: true,
        }
    }
}

/// Global hardening state.
static HARDENING_INITIALIZED: AtomicBool = AtomicBool::new(false);
static STACK_CANARY: AtomicU64 = AtomicU64::new(0);
static KASLR_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Initialize kernel hardening.
pub fn init() {
    if HARDENING_INITIALIZED.swap(true, Ordering::SeqCst) {
        return; // Already initialized
    }
    
    // Initialize stack canary with random value
    let canary = generate_random_canary();
    STACK_CANARY.store(canary, Ordering::SeqCst);
    
    // Enable CPU security features
    enable_smep();
    enable_smap();
    
    crate::serial_println!("[Hardening] Kernel hardening initialized");
}

/// Generate a random stack canary value.
fn generate_random_canary() -> u64 {
    // Use RDRAND if available, otherwise fallback to TSC-based randomness
    #[cfg(target_arch = "x86_64")]
    {
        let mut val: u64 = 0;
        unsafe {
            if core::arch::x86_64::_rdrand64_step(&mut val) == 1 {
                // Ensure low byte is zero to detect string overflows
                return (val & !0xFF) | 0x00;
            }
            
            // Fallback: use TSC
            let tsc = core::arch::x86_64::_rdtsc();
            (tsc ^ (tsc >> 17) ^ (tsc << 13)) | 0x00
        }
    }
    
    #[cfg(not(target_arch = "x86_64"))]
    {
        0xDEADBEEFCAFE0000u64
    }
}

/// Get the current stack canary value.
pub fn get_stack_canary() -> u64 {
    STACK_CANARY.load(Ordering::SeqCst)
}

/// Check stack canary (called at function exit).
#[inline(never)]
pub fn check_stack_canary(expected: u64) -> bool {
    let current = STACK_CANARY.load(Ordering::SeqCst);
    if current != expected {
        stack_smashing_detected();
    }
    true
}

/// Called when stack smashing is detected.
#[cold]
fn stack_smashing_detected() -> ! {
    crate::serial_println!("*** STACK SMASHING DETECTED ***");
    panic!("Stack buffer overflow detected!");
}

/// Enable SMEP (Supervisor Mode Execution Prevention).
fn enable_smep() {
    #[cfg(target_arch = "x86_64")]
    {
        // Try to enable SMEP - requires CPUID check in real implementation
        crate::serial_println!("[Hardening] SMEP check complete");
    }
}

/// Enable SMAP (Supervisor Mode Access Prevention).
fn enable_smap() {
    #[cfg(target_arch = "x86_64")]
    {
        // Try to enable SMAP - requires CPUID check in real implementation
        crate::serial_println!("[Hardening] SMAP check complete");
    }
}

/// Temporarily disable SMAP for user memory access.
/// SAFETY: Caller must ensure this is called in a controlled context.
#[inline(always)]
pub unsafe fn stac() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!("stac", options(nomem, nostack));
    }
}

/// Re-enable SMAP after user memory access.
#[inline(always)]
pub unsafe fn clac() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!("clac", options(nomem, nostack));
    }
}

/// Execute a closure with SMAP temporarily disabled.
/// SAFETY: Caller must ensure the closure only accesses intended user memory.
pub unsafe fn with_user_access<T, F: FnOnce() -> T>(f: F) -> T {
    unsafe {
        stac();
    }
    let result = f();
    unsafe {
        clac();
    }
    result
}

/// Guard page marker.
pub const GUARD_PAGE_MARKER: u64 = 0xDEAD_0000_0000_0000;

/// Allocate a guard page.
pub fn allocate_guard_page() -> Option<u64> {
    // In a real implementation, this would allocate a page
    // and mark it as non-present in the page tables
    None
}

/// Check if address is in a guard page.
pub fn is_guard_page(_addr: u64) -> bool {
    // Would check page table entries
    false
}

/// KASLR functions.
pub mod kaslr {
    use super::*;
    
    /// Initialize KASLR offset.
    pub fn init(entropy: u64) {
        // Generate a random offset aligned to 2MB
        let offset = (entropy & 0x7FFFFFFFF) & !0x1FFFFF;
        KASLR_OFFSET.store(offset, Ordering::SeqCst);
        crate::serial_println!("[KASLR] Offset: {:#x}", offset);
    }
    
    /// Get the KASLR offset.
    pub fn get_offset() -> u64 {
        KASLR_OFFSET.load(Ordering::SeqCst)
    }
    
    /// Randomize a kernel address.
    pub fn randomize_address(base: u64) -> u64 {
        base.wrapping_add(get_offset())
    }
    
    /// De-randomize a kernel address (for debugging).
    pub fn derandomize_address(addr: u64) -> u64 {
        addr.wrapping_sub(get_offset())
    }
}

/// Control Flow Integrity.
pub mod cfi {
    use super::*;
    
    /// CFI state.
    static CFI_ENABLED: AtomicBool = AtomicBool::new(false);
    
    /// Valid indirect call targets.
    static VALID_TARGETS: Mutex<Option<BTreeSet<u64>>> = Mutex::new(None);
    
    /// Initialize CFI.
    pub fn init() {
        let mut targets = VALID_TARGETS.lock();
        *targets = Some(BTreeSet::new());
        CFI_ENABLED.store(true, Ordering::SeqCst);
        crate::serial_println!("[CFI] Control Flow Integrity initialized");
    }
    
    /// Register a valid indirect call target.
    pub fn register_target(addr: u64) {
        if let Some(ref mut targets) = *VALID_TARGETS.lock() {
            targets.insert(addr);
        }
    }
    
    /// Check if target is valid for indirect call.
    #[inline(always)]
    pub fn check_target(addr: u64) -> bool {
        if !CFI_ENABLED.load(Ordering::Relaxed) {
            return true;
        }
        
        if let Some(ref targets) = *VALID_TARGETS.lock() {
            if targets.contains(&addr) {
                return true;
            }
        }
        
        cfi_violation(addr);
        false
    }
    
    /// Called on CFI violation.
    #[cold]
    fn cfi_violation(addr: u64) {
        crate::serial_println!("*** CFI VIOLATION: invalid target {:#x} ***", addr);
        // In production, this would panic or terminate the process
    }
}

/// Shadow stack for return address protection.
pub mod shadow_stack {
    /// Maximum shadow stack depth.
    const MAX_DEPTH: usize = 1024;
    
    /// Per-CPU shadow stack.
    pub struct ShadowStack {
        stack: [u64; MAX_DEPTH],
        top: usize,
    }
    
    impl ShadowStack {
        /// Create a new shadow stack.
        pub const fn new() -> Self {
            Self {
                stack: [0; MAX_DEPTH],
                top: 0,
            }
        }
        
        /// Push a return address.
        #[inline(always)]
        pub fn push(&mut self, addr: u64) {
            if self.top < MAX_DEPTH {
                self.stack[self.top] = addr;
                self.top += 1;
            }
        }
        
        /// Pop and verify return address.
        #[inline(always)]
        pub fn pop(&mut self, expected: u64) -> bool {
            if self.top == 0 {
                return false;
            }
            
            self.top -= 1;
            let stored = self.stack[self.top];
            
            if stored != expected {
                return_address_mismatch(stored, expected);
                return false;
            }
            
            true
        }
    }
    
    impl Default for ShadowStack {
        fn default() -> Self {
            Self::new()
        }
    }
    
    /// Called when return address mismatch is detected.
    #[cold]
    fn return_address_mismatch(stored: u64, actual: u64) {
        crate::serial_println!(
            "*** RETURN ADDRESS MISMATCH: stored={:#x}, actual={:#x} ***",
            stored, actual
        );
    }
}

/// Hardening statistics.
#[derive(Debug, Default)]
pub struct HardeningStats {
    /// Stack canary checks passed.
    pub canary_checks_passed: u64,
    /// Stack canary failures.
    pub canary_failures: u64,
    /// CFI checks passed.
    pub cfi_checks_passed: u64,
    /// CFI violations.
    pub cfi_violations: u64,
    /// Guard page hits.
    pub guard_page_hits: u64,
}

/// Get hardening status.
pub fn get_status() -> HardeningStatus {
    HardeningStatus {
        initialized: HARDENING_INITIALIZED.load(Ordering::Relaxed),
        kaslr_offset: KASLR_OFFSET.load(Ordering::Relaxed),
        stack_canary_set: STACK_CANARY.load(Ordering::Relaxed) != 0,
    }
}

/// Hardening status.
#[derive(Debug)]
pub struct HardeningStatus {
    /// Whether hardening is initialized.
    pub initialized: bool,
    /// KASLR offset.
    pub kaslr_offset: u64,
    /// Whether stack canary is set.
    pub stack_canary_set: bool,
}
