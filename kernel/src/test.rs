//! 커널 테스트 프레임워크
//!
//! `cargo test`를 위한 커스텀 테스트 러너입니다.
//! QEMU에서 테스트를 실행하고 결과를 시리얼 포트로 출력합니다.

use crate::{serial_print, serial_println};

/// 테스트 종료 코드.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    /// 테스트 성공 (QEMU 종료 코드: (0x10 << 1) | 1 = 33)
    Success = 0x10,
    /// 테스트 실패 (QEMU 종료 코드: (0x11 << 1) | 1 = 35)
    Failed = 0x11,
}

/// QEMU 종료.
///
/// isa-debug-exit 장치를 사용하여 QEMU를 종료합니다.
pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        // isa-debug-exit 장치의 기본 포트 (0xf4)
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

/// 테스트 가능 트레이트.
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

/// 테스트 러너.
///
/// 모든 테스트를 실행하고 결과를 보고합니다.
pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());

    for test in tests {
        test.run();
    }

    serial_println!();
    serial_println!("All tests passed!");

    exit_qemu(QemuExitCode::Success);
}

/// 테스트 패닉 핸들러.
///
/// 테스트 모드에서 패닉 발생 시 호출됩니다.
pub fn test_panic_handler(info: &core::panic::PanicInfo) -> ! {
    serial_println!("[failed]");
    serial_println!();
    serial_println!("Error: {}", info);

    exit_qemu(QemuExitCode::Failed);

    // exit_qemu가 실패하면 (QEMU가 아닌 환경)
    loop {
        x86_64::instructions::hlt();
    }
}
