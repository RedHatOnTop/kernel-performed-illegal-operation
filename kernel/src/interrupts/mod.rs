//! Interrupt handling subsystem.
//!
//! This module sets up the Interrupt Descriptor Table (IDT) and handles
//! CPU exceptions and hardware interrupts.
//!
//! # Architecture
//!
//! - **IDT**: Interrupt Descriptor Table for exception and interrupt handlers.
//! - **PIC**: Legacy 8259 Programmable Interrupt Controller (Phase 0).
//! - **APIC**: Advanced Programmable Interrupt Controller (Phase 1+).
//! - **I/O APIC**: External interrupt routing (Phase 1+).

mod idt;
mod pic;
pub mod apic;
pub mod ioapic;

use core::sync::atomic::{AtomicU64, AtomicPtr, Ordering};
use crate::gdt;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

/// Interrupt index type for IDT indexing.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = 32,
    Keyboard = 33,
    Mouse = 44,  // IRQ 12
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }
    
    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

/// Interrupt vector numbers.
pub mod vectors {
    /// APIC Timer interrupt vector.
    pub const TIMER: u8 = 32;
    /// Keyboard interrupt vector (PS/2).
    pub const KEYBOARD: u8 = 33;
    /// Mouse interrupt vector (PS/2).
    pub const MOUSE: u8 = 44;
    /// Spurious interrupt vector.
    pub const SPURIOUS: u8 = 0xFF;
}

/// Timer tick counter.
static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);

/// Get the current timer tick count.
pub fn timer_ticks() -> u64 {
    TIMER_TICKS.load(Ordering::Relaxed)
}

// ==================== GUI Callback System ====================

/// Keyboard event callback type
pub type KeyCallback = fn(char, u8, bool);
/// Mouse byte callback type  
pub type MouseByteCallback = fn(u8);
/// Timer callback type
pub type TimerCallback = fn();

static KEY_CALLBACK: Mutex<Option<KeyCallback>> = Mutex::new(None);
static MOUSE_CALLBACK: Mutex<Option<MouseByteCallback>> = Mutex::new(None);
static TIMER_CALLBACK: Mutex<Option<TimerCallback>> = Mutex::new(None);

/// Register keyboard callback
pub fn register_key_callback(cb: KeyCallback) {
    *KEY_CALLBACK.lock() = Some(cb);
}

/// Register mouse callback
pub fn register_mouse_callback(cb: MouseByteCallback) {
    *MOUSE_CALLBACK.lock() = Some(cb);
}

/// Register timer callback
pub fn register_timer_callback(cb: TimerCallback) {
    *TIMER_CALLBACK.lock() = Some(cb);
}

lazy_static! {
    /// The interrupt descriptor table.
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // CPU Exceptions
        idt.breakpoint.set_handler_fn(breakpoint_handler);

        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);

        // Hardware interrupts (vectors 32-255)
        idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);
        idt[InterruptIndex::Mouse.as_u8()].set_handler_fn(mouse_interrupt_handler);
        
        // Spurious interrupt handler (vector 0xFF)
        idt[vectors::SPURIOUS].set_handler_fn(spurious_interrupt_handler);

        idt
    };
}

/// Initialize the interrupt subsystem.
pub fn init() {
    IDT.load();
    crate::serial_println!("[IDT] Interrupt Descriptor Table loaded");
}

/// Initialize APIC-based interrupts.
///
/// Call this after memory initialization when physical memory offset is known.
///
/// # Safety
///
/// Physical memory offset must be valid.
pub unsafe fn init_apic(phys_mem_offset: u64) {
    // CRITICAL: Disable legacy 8259 PIC before using APIC
    // The PIC might have pending interrupts that would cause havoc
    disable_pic();
    
    // Set physical memory offset for APIC MMIO access.
    unsafe { apic::init(phys_mem_offset) };
    ioapic::set_physical_memory_offset(phys_mem_offset);
    
    // Initialize I/O APIC with default settings (ACPI parsing in Phase 1.3).
    unsafe { ioapic::init_default() };
    
    crate::serial_println!("[APIC] APIC subsystem initialized");
}

/// Disable the legacy 8259 PIC.
///
/// This is required before using APIC to prevent spurious interrupts
/// from the PIC interfering with APIC operation.
fn disable_pic() {
    use x86_64::instructions::port::Port;
    
    unsafe {
        // Mask all interrupts on both PICs
        let mut pic1_data: Port<u8> = Port::new(0x21);
        let mut pic2_data: Port<u8> = Port::new(0xA1);
        
        pic1_data.write(0xFF);  // Mask all on master PIC
        pic2_data.write(0xFF);  // Mask all on slave PIC
    }
    
    crate::serial_println!("[PIC] Legacy 8259 PIC disabled");
}

/// Start the APIC timer for preemptive scheduling.
///
/// # Arguments
///
/// - `frequency_hz`: Desired timer frequency in Hz.
pub fn start_apic_timer(frequency_hz: u32) {
    let lapic = apic::local_apic();
    
    // Use divider of 16 for reasonable granularity.
    // Initial count is calibrated based on CPU frequency.
    // For now, use a rough estimate (assuming ~100MHz APIC clock after divider).
    // Real calibration would use PIT or HPET.
    let divider = apic::TimerDivider::Div16;
    
    // Rough estimate: APIC bus clock ~100MHz, divider 16 = 6.25MHz tick rate.
    // For 100 Hz (10ms intervals): 6.25MHz / 100 = 62500 ticks.
    let initial_count = 62500 * (100 / frequency_hz);
    
    lapic.setup_timer(
        vectors::TIMER,
        initial_count,
        divider,
        apic::TimerMode::Periodic,
    );
    
    crate::serial_println!("[APIC] Timer started at ~{} Hz", frequency_hz);
}

/// Enable hardware interrupts.
pub fn enable() {
    x86_64::instructions::interrupts::enable();
    crate::serial_println!("[INT] Hardware interrupts enabled");
}

/// Check if interrupts are enabled.
pub fn are_enabled() -> bool {
    x86_64::instructions::interrupts::are_enabled()
}

/// Run a closure with interrupts disabled.
pub fn without_interrupts<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    x86_64::instructions::interrupts::without_interrupts(f)
}

/// Halt loop.
///
/// Loops indefinitely, halting the CPU until the next interrupt.
/// This is the idle loop used when there is no work to do.
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

// Exception Handlers

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    crate::serial_println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!(
        "EXCEPTION: GENERAL PROTECTION FAULT (error code: {})\n{:#?}",
        error_code, stack_frame
    );
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    let faulting_address = Cr2::read();

    crate::serial_println!(
        "EXCEPTION: PAGE FAULT\nAccessed Address: {:?}\nError Code: {:?}\n{:#?}",
        faulting_address,
        error_code,
        stack_frame
    );

    // TODO: Handle recoverable page faults (demand paging, copy-on-write)

    panic!("Unrecoverable page fault");
}

// Hardware Interrupt Handlers

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Increment tick counter
    let ticks = TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
    
    // Log every 100 ticks (~1 second at 100 Hz)
    if ticks % 100 == 0 {
        crate::serial_println!("[TIMER] Tick {}", ticks);
    }
    
    // Call timer callback (for GUI processing)
    if let Some(cb) = *TIMER_CALLBACK.lock() {
        cb();
    }
    
    // Send EOI to Local APIC
    apic::end_of_interrupt();
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    
    // Read scancode from PS/2 keyboard controller.
    let mut port: Port<u8> = Port::new(0x60);
    let scancode = unsafe { port.read() };
    
    // Convert scancode to character (basic US QWERTY layout)
    let pressed = scancode & 0x80 == 0;
    let key = scancode & 0x7F;
    
    // Basic scancode to ASCII mapping
    let ch = match key {
        0x02 => '1', 0x03 => '2', 0x04 => '3', 0x05 => '4', 0x06 => '5',
        0x07 => '6', 0x08 => '7', 0x09 => '8', 0x0A => '9', 0x0B => '0',
        0x10 => 'q', 0x11 => 'w', 0x12 => 'e', 0x13 => 'r', 0x14 => 't',
        0x15 => 'y', 0x16 => 'u', 0x17 => 'i', 0x18 => 'o', 0x19 => 'p',
        0x1E => 'a', 0x1F => 's', 0x20 => 'd', 0x21 => 'f', 0x22 => 'g',
        0x23 => 'h', 0x24 => 'j', 0x25 => 'k', 0x26 => 'l',
        0x2C => 'z', 0x2D => 'x', 0x2E => 'c', 0x2F => 'v', 0x30 => 'b',
        0x31 => 'n', 0x32 => 'm',
        0x39 => ' ',  // space
        0x1C => '\n', // enter
        0x0E => '\x08', // backspace
        _ => '\0',
    };
    
    // Call keyboard callback
    if let Some(cb) = *KEY_CALLBACK.lock() {
        cb(ch, scancode, pressed);
    }
    
    // Send EOI to Local APIC.
    apic::end_of_interrupt();
}

extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    
    // Read data from PS/2 mouse controller
    let mut port: Port<u8> = Port::new(0x60);
    let byte = unsafe { port.read() };
    
    // Call mouse callback
    if let Some(cb) = *MOUSE_CALLBACK.lock() {
        cb(byte);
    }
    
    // Send EOI to Local APIC
    apic::end_of_interrupt();
}

extern "x86-interrupt" fn spurious_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Spurious interrupts should NOT send EOI.
    // Just ignore them silently or log if debugging.
    // crate::serial_println!("[APIC] Spurious interrupt");
}
