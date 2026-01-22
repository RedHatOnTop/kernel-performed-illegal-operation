//! Architecture-specific code for x86_64.
//!
//! This module contains all architecture-dependent functionality.

pub mod gdt;
pub mod cpu;

use x86_64::instructions::port::Port;
use x86_64::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};

/// Initialize architecture-specific features.
pub fn init() {
    // Initialize GDT
    gdt::init();
    
    // Enable CPU features
    enable_cpu_features();
    
    // Initialize APIC (if available)
    // init_apic();
}

/// Enable CPU features.
fn enable_cpu_features() {
    // Enable SSE
    enable_sse();
    
    // Enable FSGSBASE if available
    if cpu::has_fsgsbase() {
        enable_fsgsbase();
    }
}

/// Enable SSE instructions.
fn enable_sse() {
    unsafe {
        // Clear EM bit, set MP bit in CR0
        let mut cr0 = Cr0::read();
        cr0.remove(Cr0Flags::EMULATE_COPROCESSOR);
        cr0.insert(Cr0Flags::MONITOR_COPROCESSOR);
        Cr0::write(cr0);
        
        // Set OSFXSR and OSXMMEXCPT in CR4
        let mut cr4 = Cr4::read();
        cr4.insert(Cr4Flags::OSFXSR);
        cr4.insert(Cr4Flags::OSXMMEXCPT_ENABLE);
        Cr4::write(cr4);
    }
}

/// Enable FSGSBASE instructions.
fn enable_fsgsbase() {
    unsafe {
        let mut cr4 = Cr4::read();
        cr4.insert(Cr4Flags::FSGSBASE);
        Cr4::write(cr4);
    }
}

/// Halt the CPU until the next interrupt.
#[inline]
pub fn halt() {
    x86_64::instructions::hlt();
}

/// Disable interrupts.
#[inline]
pub fn disable_interrupts() {
    x86_64::instructions::interrupts::disable();
}

/// Enable interrupts.
#[inline]
pub fn enable_interrupts() {
    x86_64::instructions::interrupts::enable();
}

/// Check if interrupts are enabled.
#[inline]
pub fn interrupts_enabled() -> bool {
    x86_64::instructions::interrupts::are_enabled()
}

/// Execute code with interrupts disabled.
#[inline]
pub fn without_interrupts<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    x86_64::instructions::interrupts::without_interrupts(f)
}

/// Read from an I/O port.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    Port::new(port).read()
}

/// Write to an I/O port.
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    Port::new(port).write(value)
}

/// Read from an I/O port (16-bit).
#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    Port::new(port).read()
}

/// Write to an I/O port (16-bit).
#[inline]
pub unsafe fn outw(port: u16, value: u16) {
    Port::new(port).write(value)
}

/// Read from an I/O port (32-bit).
#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    Port::new(port).read()
}

/// Write to an I/O port (32-bit).
#[inline]
pub unsafe fn outl(port: u16, value: u32) {
    Port::new(port).write(value)
}

/// Get the current CPU ID (using CPUID or APIC ID).
pub fn current_cpu_id() -> u32 {
    // For now, assume single CPU
    0
}

/// Read a Model-Specific Register.
#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    x86_64::registers::model_specific::Msr::new(msr).read()
}

/// Write a Model-Specific Register.
#[inline]
pub unsafe fn wrmsr(msr: u32, value: u64) {
    x86_64::registers::model_specific::Msr::new(msr).write(value)
}

/// Read Time Stamp Counter.
#[inline]
pub fn rdtsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// Memory fence (full barrier).
#[inline]
pub fn memory_fence() {
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
}

/// Invalidate TLB entry for a virtual address.
#[inline]
pub fn invlpg(addr: u64) {
    unsafe {
        core::arch::asm!("invlpg [{}]", in(reg) addr, options(nostack, preserves_flags));
    }
}

/// Flush the entire TLB.
pub fn flush_tlb() {
    use x86_64::registers::control::Cr3;
    
    let (frame, flags) = Cr3::read();
    unsafe {
        Cr3::write(frame, flags);
    }
}
