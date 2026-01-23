//! KPIO Kernel Library
//!
//! 이 크레이트는 커널의 공개 모듈과 테스트 프레임워크를 제공합니다.
//!
//! # 용도
//!
//! - `cargo test` 통합 테스트 지원
//! - 커널 내부 모듈의 공개 인터페이스 제공

#![no_std]
#![cfg_attr(test, no_main)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

pub mod allocator;
pub mod driver;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod scheduler;
pub mod serial;
pub mod test;

#[cfg(test)]
mod tests;

/// 커널 테스트 매크로.
///
/// 테스트 케이스를 간편하게 정의할 수 있습니다.
///
/// # Example
///
/// ```ignore
/// kernel_test!(test_example, {
///     assert_eq!(1 + 1, 2);
/// });
/// ```
#[macro_export]
macro_rules! kernel_test {
    ($name:ident, $body:block) => {
        #[test_case]
        fn $name() {
            $body
        }
    };
}

#[cfg(test)]
use bootloader_api::{entry_point, BootInfo, BootloaderConfig};

#[cfg(test)]
pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    config
};

#[cfg(test)]
entry_point!(test_kernel_main, config = &BOOTLOADER_CONFIG);

/// 테스트 모드 커널 엔트리 포인트.
#[cfg(test)]
fn test_kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // 최소 초기화
    serial::init();
    gdt::init();
    interrupts::init();

    let phys_mem_offset = boot_info
        .physical_memory_offset
        .into_option()
        .expect("Physical memory offset not provided");

    memory::validate_physical_memory_offset(phys_mem_offset);

    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator =
        unsafe { memory::BootInfoFrameAllocator::new(boot_info.memory_regions.into_iter()) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    test_main();

    interrupts::hlt_loop();
}

#[cfg(test)]
mod panic_handler {
    use crate::test::test_panic_handler;
    use core::panic::PanicInfo;

    #[panic_handler]
    fn panic(info: &PanicInfo) -> ! {
        test_panic_handler(info)
    }
}
