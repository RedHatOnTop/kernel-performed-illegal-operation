//! Panic handler for the kernel.
//!
//! This module provides the panic handler that is called when
//! the kernel encounters an unrecoverable error.

use crate::serial_println;
use core::panic::PanicInfo;

/// Panic handler implementation.
///
/// In test mode, the test module's panic handler is used;
/// in normal mode, output to serial and halt.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!();
    serial_println!("========================================");
    serial_println!("KERNEL PANIC");
    serial_println!("========================================");

    if let Some(location) = info.location() {
        serial_println!(
            "Location: {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
    }

    serial_println!("Message: {}", info.message());

    serial_println!();
    serial_println!("System halted.");
    serial_println!("========================================");

    crate::interrupts::hlt_loop()
}

/// Panic handler for test mode.
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    crate::test::test_panic_handler(info)
}
