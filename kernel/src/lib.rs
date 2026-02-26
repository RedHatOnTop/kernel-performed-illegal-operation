//! KPIO Kernel Library
//!
//! This crate provides the kernel's public modules and test framework.
//!
//! # Purpose
//!
//! - Integration test support via `cargo test`
//! - Public interface for internal kernel modules

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
pub mod app;
pub mod browser;
pub mod crash;
pub mod driver;
pub mod drivers;
pub mod gdt;
pub mod hw;
pub mod i18n;
pub mod interrupts;
pub mod io;
pub mod ipc;
pub mod loader;
pub mod memory;
pub mod net;
pub mod process;
pub mod scheduler;
pub mod security;
pub mod serial;
pub mod sync;
pub mod syscall;
pub mod terminal;
pub mod test;
pub mod update;
pub mod vfs;

#[cfg(test)]
mod tests;

/// Kernel test macro.
///
/// Conveniently define test cases.
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

/// Test mode kernel entry point.
#[cfg(test)]
fn test_kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // Minimal initialization
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
