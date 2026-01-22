//! Interrupt Descriptor Table structures.
//!
//! This module defines the IDT entry format and provides utilities
//! for creating and managing IDT entries.

use core::arch::asm;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use x86_64::VirtAddr;

/// IDT entry options.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct EntryOptions(u16);

impl EntryOptions {
    /// Create minimal entry options (present, ring 0).
    const fn minimal() -> Self {
        EntryOptions(0b1110_0000_0000)
    }

    /// Set the present bit.
    pub fn set_present(&mut self, present: bool) -> &mut Self {
        if present {
            self.0 |= 1 << 15;
        } else {
            self.0 &= !(1 << 15);
        }
        self
    }

    /// Set the privilege level (0-3).
    pub fn set_privilege_level(&mut self, dpl: u8) -> &mut Self {
        self.0 = (self.0 & !0x6000) | ((dpl as u16 & 0b11) << 13);
        self
    }

    /// Set the interrupt stack table index (0-7).
    pub fn set_stack_index(&mut self, index: u8) -> &mut Self {
        self.0 = (self.0 & !0b111) | (index as u16 & 0b111);
        self
    }
}

/// Raw IDT entry (64-bit).
#[derive(Clone, Copy)]
#[repr(C)]
pub struct RawIdtEntry {
    /// Lower 16 bits of handler address.
    offset_low: u16,
    /// Segment selector.
    selector: u16,
    /// Options.
    options: EntryOptions,
    /// Middle 16 bits of handler address.
    offset_middle: u16,
    /// Upper 32 bits of handler address.
    offset_high: u32,
    /// Reserved (must be zero).
    reserved: u32,
}

impl RawIdtEntry {
    /// Create a missing (not present) entry.
    pub const fn missing() -> Self {
        RawIdtEntry {
            offset_low: 0,
            selector: 0,
            options: EntryOptions::minimal(),
            offset_middle: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    /// Create an entry with the given handler address.
    pub fn new(handler: u64, selector: u16) -> Self {
        let mut entry = Self::missing();
        entry.offset_low = handler as u16;
        entry.offset_middle = (handler >> 16) as u16;
        entry.offset_high = (handler >> 32) as u32;
        entry.selector = selector;
        entry.options.set_present(true);
        entry
    }
}

/// IDT pointer structure for LIDT instruction.
#[repr(C, packed)]
pub struct IdtPointer {
    /// IDT size minus 1.
    pub limit: u16,
    /// IDT base address.
    pub base: u64,
}

impl IdtPointer {
    /// Create a new IDT pointer.
    pub fn new(base: u64, entries: u16) -> Self {
        IdtPointer {
            limit: entries * 16 - 1,
            base,
        }
    }

    /// Load the IDT.
    pub unsafe fn load(&self) {
        unsafe {
            asm!("lidt [{}]", in(reg) self, options(readonly, nostack, preserves_flags));
        }
    }
}

/// Interrupt stack frame layout pushed by the CPU.
#[repr(C)]
pub struct InterruptFrame {
    /// Instruction pointer at time of interrupt.
    pub instruction_pointer: u64,
    /// Code segment.
    pub code_segment: u64,
    /// CPU flags.
    pub cpu_flags: u64,
    /// Stack pointer at time of interrupt.
    pub stack_pointer: u64,
    /// Stack segment.
    pub stack_segment: u64,
}

impl InterruptFrame {
    /// Get the instruction pointer.
    pub fn ip(&self) -> VirtAddr {
        VirtAddr::new(self.instruction_pointer)
    }

    /// Get the stack pointer.
    pub fn sp(&self) -> VirtAddr {
        VirtAddr::new(self.stack_pointer)
    }
}

/// Interrupt stack frame with error code.
#[repr(C)]
pub struct InterruptFrameWithError {
    /// Error code pushed by CPU.
    pub error_code: u64,
    /// Interrupt frame.
    pub frame: InterruptFrame,
}

/// Interrupt handler type (without error code).
pub type HandlerFunc = extern "x86-interrupt" fn(InterruptStackFrame);

/// Interrupt handler type (with error code).
pub type HandlerFuncWithError = extern "x86-interrupt" fn(InterruptStackFrame, u64);

/// Page fault handler type (with error code and CR2).
pub type PageFaultHandler =
    extern "x86-interrupt" fn(InterruptStackFrame, x86_64::structures::idt::PageFaultErrorCode);
