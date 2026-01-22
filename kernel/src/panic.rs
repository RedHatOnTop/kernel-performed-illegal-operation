//! Panic handler for the kernel.
//!
//! This module provides the panic handler that is called when
//! the kernel encounters an unrecoverable error.

use crate::serial_println;
use core::panic::PanicInfo;

/// Panic handler implementation.
///
/// 테스트 모드에서는 test 모듈의 패닉 핸들러 사용,
/// 일반 모드에서는 시리얼로 출력 후 halt.
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

/// 테스트 모드용 패닉 핸들러.
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    crate::test::test_panic_handler(info)
}
