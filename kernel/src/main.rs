//! KPIO Kernel
//!
//! A WebAssembly-native operating system kernel with Vulkan-exclusive graphics.
//!
//! # Architecture
//!
//! The kernel implements a pure microkernel design where:
//! - Ring 0 contains only essential services (memory, scheduling, IPC)
//! - All drivers (including GPU) run in userspace as WASM modules
//! - Graphics uses Vulkan exclusively via Mesa drivers
//!
//! # Boot Process
//!
//! 1. UEFI firmware initializes
//! 2. bootloader loads and parses kernel ELF
//! 3. bootloader sets up framebuffer (GOP) and memory map
//! 4. bootloader jumps to kernel entry point
//! 5. Kernel initializes serial, GDT, IDT, memory, heap
//! 6. Kernel enters hlt loop (Phase 0) or starts scheduler (Phase 1+)

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

mod allocator;
mod gdt;
mod interrupts;
mod memory;
mod panic;
mod serial;

#[cfg(test)]
mod test;

use bootloader_api::{entry_point, BootInfo, BootloaderConfig};

/// Bootloader configuration.
///
/// - Physical memory mapping: Dynamic (bootloader chooses offset)
/// - Framebuffer: Enabled for potential early console
pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

/// Kernel entry point after bootloader handoff.
///
/// This function is called by the bootloader after setting up:
/// - Identity-mapped kernel code/data
/// - Physical memory mapping at configurable offset
/// - Page tables in higher-half kernel space
fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // Phase 1: Serial console initialization
    serial::init();
    serial_println!("Hello, Kernel");
    serial_println!("[KPIO] Boot info at: {:p}", boot_info);

    // Phase 2: GDT initialization (required before IDT for TSS)
    serial_println!("[KPIO] Initializing GDT...");
    gdt::init();
    serial_println!("[KPIO] GDT initialized");

    // Phase 3: IDT initialization
    serial_println!("[KPIO] Initializing IDT...");
    interrupts::init();
    serial_println!("[KPIO] IDT initialized");

    // Phase 4: Memory management initialization
    serial_println!("[KPIO] Initializing memory management...");

    let phys_mem_offset = boot_info.physical_memory_offset.into_option().expect(
        "Physical memory offset not provided by bootloader. \
                 Ensure BOOTLOADER_CONFIG.mappings.physical_memory is set.",
    );

    // Validate physical memory offset before use
    memory::validate_physical_memory_offset(phys_mem_offset);
    serial_println!("[KPIO] Physical memory offset: {:#x}", phys_mem_offset);

    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator =
        unsafe { memory::BootInfoFrameAllocator::new(boot_info.memory_regions.into_iter()) };
    serial_println!("[KPIO] Page mapper and frame allocator initialized");

    // Phase 5: Heap initialization
    serial_println!("[KPIO] Initializing heap...");
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");
    serial_println!("[KPIO] Heap initialized");

    // Phase 6: APIC initialization (Phase 1 feature)
    serial_println!("[KPIO] Initializing APIC...");
    unsafe { interrupts::init_apic(phys_mem_offset) };
    serial_println!("[KPIO] APIC initialized");

    // Phase 7: Enable interrupts and start timer
    serial_println!("[KPIO] Starting APIC timer...");
    interrupts::start_apic_timer(100); // 100 Hz
    interrupts::enable();
    serial_println!("[KPIO] Interrupts enabled");

    serial_println!("[KPIO] Kernel initialization complete");

    // Run tests if in test mode
    #[cfg(test)]
    test_main();

    // Enter halt loop
    serial_println!("[KPIO] Entering halt loop...");
    interrupts::hlt_loop();
}

// 매크로는 serial.rs에서 정의됨
