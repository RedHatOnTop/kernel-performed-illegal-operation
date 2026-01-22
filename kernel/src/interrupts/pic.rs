//! Programmable Interrupt Controller (8259 PIC) driver.
//!
//! This module handles the legacy 8259 PIC which is used for hardware
//! interrupts on x86 systems. The PIC remaps IRQs to avoid conflicts
//! with CPU exceptions.

use pic8259::ChainedPics;
use spin::Mutex;

/// PIC1 offset (IRQ 0-7 mapped to interrupts 32-39).
const PIC1_OFFSET: u8 = 32;

/// PIC2 offset (IRQ 8-15 mapped to interrupts 40-47).
const PIC2_OFFSET: u8 = 40;

/// The chained PICs (master and slave).
static PICS: Mutex<ChainedPics> = Mutex::new(unsafe { ChainedPics::new(PIC1_OFFSET, PIC2_OFFSET) });

/// Initialize the PICs.
pub fn init() {
    unsafe {
        PICS.lock().initialize();
    }
}

/// Send end-of-interrupt signal for the given interrupt.
pub fn end_of_interrupt(interrupt_id: u8) {
    unsafe {
        PICS.lock().notify_end_of_interrupt(interrupt_id);
    }
}

/// Disable a specific IRQ.
pub fn disable_irq(irq: u8) {
    let mut pics = PICS.lock();

    if irq < 8 {
        // PIC1
        let mut mask = unsafe { x86_64::instructions::port::Port::<u8>::new(0x21).read() };
        mask |= 1 << irq;
        unsafe { x86_64::instructions::port::Port::<u8>::new(0x21).write(mask) };
    } else {
        // PIC2
        let irq = irq - 8;
        let mut mask = unsafe { x86_64::instructions::port::Port::<u8>::new(0xA1).read() };
        mask |= 1 << irq;
        unsafe { x86_64::instructions::port::Port::<u8>::new(0xA1).write(mask) };
    }
}

/// Enable a specific IRQ.
pub fn enable_irq(irq: u8) {
    if irq < 8 {
        // PIC1
        let mut mask = unsafe { x86_64::instructions::port::Port::<u8>::new(0x21).read() };
        mask &= !(1 << irq);
        unsafe { x86_64::instructions::port::Port::<u8>::new(0x21).write(mask) };
    } else {
        // PIC2
        let irq = irq - 8;
        let mut mask = unsafe { x86_64::instructions::port::Port::<u8>::new(0xA1).read() };
        mask &= !(1 << irq);
        unsafe { x86_64::instructions::port::Port::<u8>::new(0xA1).write(mask) };
    }
}
