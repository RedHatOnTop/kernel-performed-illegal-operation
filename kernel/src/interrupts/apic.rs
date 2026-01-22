//! Local APIC (Advanced Programmable Interrupt Controller) driver.
//!
//! The Local APIC is a per-CPU interrupt controller that handles:
//! - Inter-processor interrupts (IPI)
//! - Local timer interrupts
//! - Performance monitoring interrupts
//! - Thermal sensor interrupts
//! - LINT0/LINT1 (local interrupts)
//!
//! # Memory-Mapped Registers
//!
//! The Local APIC registers are memory-mapped starting at a base address
//! (default: 0xFEE0_0000). Each register is 32 bits wide and aligned to
//! 16-byte boundaries.
//!
//! # References
//!
//! - Intel SDM Volume 3, Chapter 10: Advanced Programmable Interrupt Controller
//! - AMD64 Architecture Programmer's Manual, Volume 2: System Programming

use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::registers::model_specific::Msr;

/// Default Local APIC base address.
///
/// This is the standard x86_64 default. The actual address should be
/// obtained from ACPI MADT (Multiple APIC Description Table) at runtime.
const LAPIC_BASE_DEFAULT: u64 = 0xFEE0_0000;

/// Local APIC base address (set from ACPI MADT).
static LAPIC_BASE: AtomicU64 = AtomicU64::new(LAPIC_BASE_DEFAULT);

/// Virtual address offset for physical memory access.
///
/// Set during memory initialization from bootloader info.
static PHYS_MEM_OFFSET: AtomicU64 = AtomicU64::new(0);

/// Local APIC register offsets.
///
/// All offsets are from the LAPIC base address.
/// Registers are 32-bit wide, 16-byte aligned.
#[allow(dead_code)]
mod regs {
    /// Local APIC ID Register (RO for xAPIC, RW for x2APIC).
    pub const ID: u32 = 0x020;
    
    /// Local APIC Version Register (RO).
    pub const VERSION: u32 = 0x030;
    
    /// Task Priority Register (TPR).
    /// Controls which interrupts can be delivered to the processor.
    pub const TPR: u32 = 0x080;
    
    /// Arbitration Priority Register (RO, deprecated in modern CPUs).
    pub const APR: u32 = 0x090;
    
    /// Processor Priority Register (RO).
    pub const PPR: u32 = 0x0A0;
    
    /// End of Interrupt Register (WO).
    /// Write 0 to signal end of interrupt handling.
    pub const EOI: u32 = 0x0B0;
    
    /// Remote Read Register (RO, deprecated).
    pub const RRD: u32 = 0x0C0;
    
    /// Logical Destination Register.
    pub const LDR: u32 = 0x0D0;
    
    /// Destination Format Register.
    pub const DFR: u32 = 0x0E0;
    
    /// Spurious Interrupt Vector Register.
    /// Bit 8: APIC Software Enable/Disable.
    /// Bits 0-7: Spurious interrupt vector number.
    pub const SVR: u32 = 0x0F0;
    
    /// In-Service Register (ISR) - 256 bits across 8 registers.
    pub const ISR_BASE: u32 = 0x100;
    
    /// Trigger Mode Register (TMR) - 256 bits across 8 registers.
    pub const TMR_BASE: u32 = 0x180;
    
    /// Interrupt Request Register (IRR) - 256 bits across 8 registers.
    pub const IRR_BASE: u32 = 0x200;
    
    /// Error Status Register.
    pub const ESR: u32 = 0x280;
    
    /// LVT CMCI Register (Corrected Machine Check Interrupt).
    pub const LVT_CMCI: u32 = 0x2F0;
    
    /// Interrupt Command Register (low 32 bits).
    pub const ICR_LOW: u32 = 0x300;
    
    /// Interrupt Command Register (high 32 bits).
    pub const ICR_HIGH: u32 = 0x310;
    
    /// LVT Timer Register.
    pub const LVT_TIMER: u32 = 0x320;
    
    /// LVT Thermal Sensor Register.
    pub const LVT_THERMAL: u32 = 0x330;
    
    /// LVT Performance Monitoring Counters Register.
    pub const LVT_PMC: u32 = 0x340;
    
    /// LVT LINT0 Register.
    pub const LVT_LINT0: u32 = 0x350;
    
    /// LVT LINT1 Register.
    pub const LVT_LINT1: u32 = 0x360;
    
    /// LVT Error Register.
    pub const LVT_ERROR: u32 = 0x370;
    
    /// Timer Initial Count Register.
    pub const TIMER_INIT: u32 = 0x380;
    
    /// Timer Current Count Register (RO).
    pub const TIMER_CURR: u32 = 0x390;
    
    /// Timer Divide Configuration Register.
    pub const TIMER_DIV: u32 = 0x3E0;
}

/// LVT Timer modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TimerMode {
    /// One-shot mode: timer fires once.
    OneShot = 0b00,
    /// Periodic mode: timer fires repeatedly.
    Periodic = 0b01,
    /// TSC-Deadline mode: timer fires when TSC reaches deadline.
    TscDeadline = 0b10,
}

/// Timer divider values.
///
/// The actual division is: 2^(value+1) for values 0-6,
/// except value 7 which means divide by 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TimerDivider {
    /// Divide by 2.
    Div2 = 0b0000,
    /// Divide by 4.
    Div4 = 0b0001,
    /// Divide by 8.
    Div8 = 0b0010,
    /// Divide by 16.
    Div16 = 0b0011,
    /// Divide by 32.
    Div32 = 0b1000,
    /// Divide by 64.
    Div64 = 0b1001,
    /// Divide by 128.
    Div128 = 0b1010,
    /// Divide by 1.
    Div1 = 0b1011,
}

/// Local APIC driver.
pub struct LocalApic {
    /// Base physical address.
    base_phys: u64,
    /// Base virtual address (after adding physical memory offset).
    base_virt: u64,
}

impl LocalApic {
    /// Set the physical memory offset for MMIO access.
    ///
    /// This must be called during memory initialization before
    /// any APIC operations.
    pub fn set_physical_memory_offset(offset: u64) {
        PHYS_MEM_OFFSET.store(offset, Ordering::SeqCst);
    }
    
    /// Set the Local APIC base address from ACPI MADT.
    ///
    /// Call this if the MADT contains a Local APIC Address Override.
    pub fn set_base_address(addr: u64) {
        LAPIC_BASE.store(addr, Ordering::SeqCst);
        crate::serial_println!("[APIC] Local APIC base set to {:#x}", addr);
    }
    
    /// Initialize and enable the Local APIC.
    ///
    /// # Safety
    ///
    /// - Physical memory offset must be set correctly.
    /// - APIC base address must be valid and mapped.
    pub unsafe fn init() -> Self {
        let base_phys = LAPIC_BASE.load(Ordering::SeqCst);
        let offset = PHYS_MEM_OFFSET.load(Ordering::SeqCst);
        let base_virt = base_phys + offset;
        
        let lapic = LocalApic { base_phys, base_virt };
        
        // Enable APIC via MSR (IA32_APIC_BASE, MSR 0x1B).
        // Bit 11: APIC Global Enable.
        let mut apic_base_msr = Msr::new(0x1B);
        let msr_value = unsafe { apic_base_msr.read() };
        unsafe { apic_base_msr.write(msr_value | (1 << 11)) };
        
        // Set Spurious Interrupt Vector Register.
        // Bit 8: APIC Software Enable.
        // Bits 0-7: Spurious interrupt vector (use 0xFF).
        lapic.write(regs::SVR, 0x1FF);
        
        // Set Task Priority to 0 (accept all interrupts).
        lapic.write(regs::TPR, 0);
        
        // Clear any pending errors.
        lapic.write(regs::ESR, 0);
        let _ = lapic.read(regs::ESR);
        
        crate::serial_println!("[APIC] Local APIC initialized at {:#x} (virt: {:#x})", 
            base_phys, base_virt);
        crate::serial_println!("[APIC] APIC ID: {}", lapic.id());
        crate::serial_println!("[APIC] APIC Version: {:#x}", lapic.version());
        
        lapic
    }
    
    /// Read a 32-bit register.
    #[inline]
    fn read(&self, reg: u32) -> u32 {
        unsafe {
            let ptr = (self.base_virt + reg as u64) as *const u32;
            read_volatile(ptr)
        }
    }
    
    /// Write a 32-bit register.
    #[inline]
    fn write(&self, reg: u32, value: u32) {
        unsafe {
            let ptr = (self.base_virt + reg as u64) as *mut u32;
            write_volatile(ptr, value);
        }
    }
    
    /// Get the Local APIC ID.
    pub fn id(&self) -> u8 {
        ((self.read(regs::ID) >> 24) & 0xFF) as u8
    }
    
    /// Get the Local APIC version.
    pub fn version(&self) -> u32 {
        self.read(regs::VERSION)
    }
    
    /// Send End of Interrupt signal.
    ///
    /// Must be called at the end of every interrupt handler
    /// for APIC-delivered interrupts.
    #[inline]
    pub fn end_of_interrupt(&self) {
        self.write(regs::EOI, 0);
    }
    
    /// Set up the APIC timer.
    ///
    /// # Arguments
    ///
    /// - `vector`: Interrupt vector number (32-255).
    /// - `initial_count`: Initial countdown value.
    /// - `divider`: Clock divider.
    /// - `mode`: Timer mode (one-shot, periodic, TSC-deadline).
    pub fn setup_timer(
        &self,
        vector: u8,
        initial_count: u32,
        divider: TimerDivider,
        mode: TimerMode,
    ) {
        // Set divider.
        self.write(regs::TIMER_DIV, divider as u32);
        
        // Configure LVT Timer.
        // Bits 0-7: Vector.
        // Bit 16: Mask (0 = not masked).
        // Bits 17-18: Timer mode.
        let lvt_value = (vector as u32) | ((mode as u32) << 17);
        self.write(regs::LVT_TIMER, lvt_value);
        
        // Set initial count (starts the timer).
        self.write(regs::TIMER_INIT, initial_count);
        
        crate::serial_println!(
            "[APIC] Timer configured: vector={}, count={}, divider={:?}, mode={:?}",
            vector, initial_count, divider, mode
        );
    }
    
    /// Stop the APIC timer.
    pub fn stop_timer(&self) {
        self.write(regs::TIMER_INIT, 0);
    }
    
    /// Get current timer count.
    pub fn timer_current(&self) -> u32 {
        self.read(regs::TIMER_CURR)
    }
    
    /// Mask (disable) an LVT entry.
    pub fn mask_lvt(&self, reg: u32) {
        let value = self.read(reg);
        self.write(reg, value | (1 << 16));
    }
    
    /// Unmask (enable) an LVT entry.
    pub fn unmask_lvt(&self, reg: u32) {
        let value = self.read(reg);
        self.write(reg, value & !(1 << 16));
    }
    
    /// Send an Inter-Processor Interrupt (IPI).
    ///
    /// # Arguments
    ///
    /// - `dest_apic_id`: Destination APIC ID (0-255).
    /// - `vector`: Interrupt vector (0-255).
    pub fn send_ipi(&self, dest_apic_id: u8, vector: u8) {
        // Set destination in ICR high.
        self.write(regs::ICR_HIGH, (dest_apic_id as u32) << 24);
        
        // Send IPI via ICR low.
        // Bits 0-7: Vector.
        // Bits 8-10: Delivery mode (000 = Fixed).
        // Bit 11: Destination mode (0 = Physical).
        // Bit 14: Level (1 = Assert).
        self.write(regs::ICR_LOW, (vector as u32) | (1 << 14));
    }
    
    /// Send INIT IPI to another processor.
    pub fn send_init_ipi(&self, dest_apic_id: u8) {
        self.write(regs::ICR_HIGH, (dest_apic_id as u32) << 24);
        // Delivery mode 101 = INIT, Level = Assert.
        self.write(regs::ICR_LOW, (0b101 << 8) | (1 << 14));
    }
    
    /// Send Startup IPI (SIPI) to another processor.
    ///
    /// # Arguments
    ///
    /// - `dest_apic_id`: Destination APIC ID.
    /// - `vector`: Startup address / 4096 (page number).
    pub fn send_startup_ipi(&self, dest_apic_id: u8, vector: u8) {
        self.write(regs::ICR_HIGH, (dest_apic_id as u32) << 24);
        // Delivery mode 110 = Startup, Vector = page number.
        self.write(regs::ICR_LOW, (vector as u32) | (0b110 << 8));
    }
}

/// Global Local APIC instance.
static mut LOCAL_APIC: Option<LocalApic> = None;

/// Initialize the Local APIC.
///
/// # Safety
///
/// Must be called only once during kernel initialization,
/// after physical memory offset is known.
pub unsafe fn init(phys_mem_offset: u64) {
    LocalApic::set_physical_memory_offset(phys_mem_offset);
    unsafe {
        LOCAL_APIC = Some(LocalApic::init());
    }
}

/// Get a reference to the Local APIC.
///
/// # Panics
///
/// Panics if the APIC has not been initialized.
pub fn local_apic() -> &'static LocalApic {
    unsafe {
        LOCAL_APIC.as_ref().expect("Local APIC not initialized")
    }
}

/// Send End of Interrupt signal.
///
/// Convenience function for interrupt handlers.
#[inline]
pub fn end_of_interrupt() {
    local_apic().end_of_interrupt();
}
