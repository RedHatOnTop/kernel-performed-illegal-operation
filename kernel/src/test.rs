//! Kernel test framework
//!
//! Custom test runner for `cargo test`.
//! Runs tests in QEMU and outputs results through the serial port.

use crate::{serial_print, serial_println};

/// Test exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    /// Test success (QEMU exit code: (0x10 << 1) | 1 = 33)
    Success = 0x10,
    /// Test failure (QEMU exit code: (0x11 << 1) | 1 = 35)
    Failed = 0x11,
}

/// Exit QEMU.
///
/// Exits QEMU using the isa-debug-exit device.
pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        // Default port for the isa-debug-exit device (0xf4)
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

/// Testable trait.
pub trait Testable {
    fn run(&self);
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

/// Test runner.
///
/// Runs all tests and reports results.
pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());

    for test in tests {
        test.run();
    }

    serial_println!();
    serial_println!("All tests passed!");

    exit_qemu(QemuExitCode::Success);
}

/// Test panic handler.
///
/// Called when a panic occurs in test mode.
pub fn test_panic_handler(info: &core::panic::PanicInfo) -> ! {
    serial_println!("[failed]");
    serial_println!();
    serial_println!("Error: {}", info);

    exit_qemu(QemuExitCode::Failed);

    // If exit_qemu fails (non-QEMU environment)
    loop {
        x86_64::instructions::hlt();
    }
}
