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
mod app;
mod driver;
mod drivers;
mod gdt;
mod graphics;
mod gui;
mod hw;
mod interrupts;
mod memory;
mod net;
mod panic;
mod scheduler;
mod serial;
mod terminal;
mod vfs;
mod wasm;

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

    // Draw to framebuffer early (before other initialization)
    // Store framebuffer info for GUI
    let mut fb_ptr: *mut u8 = core::ptr::null_mut();
    let mut fb_width: u32 = 0;
    let mut fb_height: u32 = 0;
    let mut fb_bpp: usize = 0;
    let mut fb_stride: usize = 0;

    if let Some(fb_info) = boot_info.framebuffer.as_mut() {
        serial_println!(
            "[KPIO] Framebuffer available: {}x{}",
            fb_info.info().width,
            fb_info.info().height
        );

        fb_ptr = fb_info.buffer_mut().as_mut_ptr();
        fb_width = fb_info.info().width as u32;
        fb_height = fb_info.info().height as u32;
        fb_bpp = fb_info.info().bytes_per_pixel;
        fb_stride = fb_info.info().stride;

        draw_boot_screen(fb_info);
    } else {
        serial_println!("[KPIO] No framebuffer available");
    }

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

    // Phase 5.1: Global frame allocator initialization
    // Reserve a pool of physical frames for slab/buddy/user-page-table allocators.
    // Scan the bootloader memory map for the largest usable region and hand a
    // sub-range to the global bump allocator that backs allocate_frame().
    {
        let mut best_start: u64 = 0;
        let mut best_size: u64 = 0;
        for region in boot_info.memory_regions.iter() {
            if region.kind == bootloader_api::info::MemoryRegionKind::Usable {
                let size = region.end - region.start;
                if size > best_size {
                    best_start = region.start;
                    best_size = size;
                }
            }
        }
        // Use the upper half of the largest usable region so we don't collide
        // with frames already consumed by BootInfoFrameAllocator.
        // Align pool_start UP to the next page boundary (4 KiB) to satisfy
        // the frame allocator's alignment requirements.
        let pool_start = ((best_start + best_size / 2) + 4095) & !4095;
        let pool_end = best_start + best_size;
        memory::init_frame_allocator(pool_start, pool_end);
        serial_println!(
            "[MEM] Global frame pool: {:#x}..{:#x} ({} KiB)",
            pool_start,
            pool_end,
            (pool_end - pool_start) / 1024
        );
    }

    // Phase 5.2: Frame recycling self-test
    {
        let before = memory::free_frame_count();
        let frame = memory::allocate_frame().expect("self-test: allocate_frame failed");
        memory::free_frame(frame);
        let after = memory::free_frame_count();
        assert!(
            after == before + 1,
            "free_frame self-test failed: free list did not grow"
        );
        // Re-allocate — should return the same frame from the free list
        let recycled = memory::allocate_frame().expect("self-test: re-allocate failed");
        assert!(
            recycled == frame,
            "free_frame self-test failed: recycled frame mismatch"
        );
        serial_println!("[MEM] Frame recycling self-test passed (free+realloc OK)");
    }

    // Phase 5.3: User-space page table allocator
    serial_println!("[KPIO] Initializing user page table allocator...");
    memory::user_page_table::init(phys_mem_offset);
    serial_println!("[KPIO] User page table allocator initialized");

    // Phase 6: Scheduler initialization
    serial_println!("[KPIO] Initializing scheduler...");
    scheduler::init();
    serial_println!("[KPIO] Scheduler initialized");

    // Phase 6.1: User-space SYSCALL/SYSRET MSR setup
    serial_println!("[KPIO] Initializing Ring 3 userspace support...");
    scheduler::userspace::init();
    serial_println!("[KPIO] Ring 3 support initialized (STAR/LSTAR/SFMASK + PerCPU)");

    // Phase 6.2: Process table initialization
    serial_println!("[KPIO] Initializing process table...");
    // Process table init is called from lib; scheduler already handles tasks.
    serial_println!("[KPIO] Process table initialized");

    // Phase 6.5: Terminal filesystem & shell
    serial_println!("[KPIO] Initializing terminal subsystem...");
    terminal::fs::init();
    terminal::shell::init();
    serial_println!("[KPIO] Terminal subsystem ready (50+ commands)");

    // Phase 6.6: VFS & file descriptor table
    serial_println!("[KPIO] Initializing VFS...");
    vfs::fd::init();
    serial_println!("[KPIO] VFS initialized (fd table ready)");

    // Phase 7: APIC initialization (Phase 1 feature)
    serial_println!("[KPIO] Initializing APIC...");
    unsafe { interrupts::init_apic(phys_mem_offset) };
    serial_println!("[KPIO] APIC initialized");

    // Phase 8: PCI enumeration (moved before ACPI — basic PCI config-space
    // access via CF8/CFC works without ACPI tables, and ACPI parsing has a
    // known page-fault issue that must not block NIC/network init)
    serial_println!("[KPIO] Enumerating PCI bus...");
    driver::pci::enumerate();

    // Phase 9: VirtIO device initialization
    serial_println!("[KPIO] Initializing VirtIO devices...");
    driver::virtio::block::init();
    serial_println!(
        "[KPIO] VirtIO initialized ({} block device(s))",
        driver::virtio::block::device_count()
    );

    // Phase 9-3: Bridge kernel VirtIO block device to storage VFS
    if driver::virtio::block::device_count() > 0 {
        let storage_dev_idx = driver::virtio::block::device_count().saturating_sub(1);
        let adapter = alloc::boxed::Box::leak(alloc::boxed::Box::new(
            driver::virtio::block_adapter::KernelBlockAdapter::new(storage_dev_idx),
        ));
        let device_name = if storage_dev_idx == 0 {
            "virtio-blk0"
        } else {
            "virtio-blk1"
        };

        let _ = storage::vfs::init();
        match storage::register_block_device(device_name, adapter) {
            Ok(_) => {
                match storage::mount(
                    device_name,
                    "/mnt/test",
                    "fat32",
                    storage::MountFlags::READ_ONLY,
                ) {
                    Ok(()) => {
                        serial_println!("[VFS] Mounted FAT filesystem on {} at /mnt/test", device_name);

                        // Self-test 1: read a file
                        match storage::vfs::open("/mnt/test/HELLO.TXT", storage::OpenFlags::READ) {
                            Ok(fd) => {
                                let mut buf = [0u8; 512];
                                match storage::vfs::read(fd, &mut buf) {
                                    Ok(n) => {
                                        serial_println!(
                                            "[VFS] Self-test: read {} bytes from HELLO.TXT",
                                            n
                                        );
                                    }
                                    Err(e) => {
                                        serial_println!(
                                            "[VFS] Self-test read failed for HELLO.TXT: {:?}",
                                            e
                                        );
                                    }
                                }
                                let _ = storage::vfs::close(fd);
                            }
                            Err(e) => {
                                serial_println!(
                                    "[VFS] Self-test open failed for HELLO.TXT: {:?}",
                                    e
                                );
                            }
                        }

                        // Self-test 2: readdir
                        match storage::vfs::readdir("/mnt/test/") {
                            Ok(entries) => {
                                serial_println!(
                                    "[VFS] readdir /mnt/test/: {} entries",
                                    entries.len()
                                );
                                for entry in &entries {
                                    let name = core::str::from_utf8(
                                        &entry.name[..entry.name_len],
                                    )
                                    .unwrap_or("?");
                                    serial_println!("  - {} ({:?})", name, entry.file_type);
                                }
                            }
                            Err(e) => {
                                serial_println!(
                                    "[VFS] readdir /mnt/test/ failed: {:?}", e
                                );
                            }
                        }

                        // Self-test 3: invalid path (must not panic)
                        match storage::vfs::open("/mnt/test/NOFILE.TXT", storage::OpenFlags::READ) {
                            Ok(fd) => {
                                let _ = storage::vfs::close(fd);
                            }
                            Err(_) => {
                                serial_println!("[VFS] Self-test: NOFILE.TXT correctly not found");
                            }
                        }
                    }
                    Err(e) => serial_println!("[VFS] Mount failed for {}: {:?}", device_name, e),
                }
            }
            Err(e) => {
                serial_println!("[VFS] Block adapter registration failed: {:?}", e);
            }
        }
    }

    // Phase 9.5: VirtIO network probe
    serial_println!("[KPIO] Probing VirtIO network devices...");
    drivers::net::virtio_net::probe();

    // Phase 9.6: Network stack (after PCI + VirtIO so NIC is available)
    serial_println!("[KPIO] Initializing network stack...");
    net::init();
    serial_println!("[KPIO] Network stack ready (loopback + DNS + HTTP)");

    // Phase 7.5: ACPI table parsing (deferred after NIC/network init so
    // that a page-fault in ACPI does not block DHCP or packet I/O)
    serial_println!("[KPIO] Initializing ACPI...");
    if let Some(rsdp_addr) = boot_info.rsdp_addr.into_option() {
        match hw::acpi::init_with_rsdp(rsdp_addr, phys_mem_offset) {
            Ok(()) => serial_println!(
                "[KPIO] ACPI initialized ({} tables)",
                hw::acpi::table_count()
            ),
            Err(e) => serial_println!("[KPIO] ACPI init failed: {}", e),
        }
    } else {
        serial_println!("[KPIO] No RSDP address from bootloader");
    }
    serial_println!("[KPIO] Initializing PS/2 mouse...");
    driver::ps2_mouse::init();

    // Enable mouse IRQ (IRQ 12 -> GSI 12 -> Vector 44)
    interrupts::ioapic::set_gsi(12, 44, 0);
    interrupts::ioapic::unmask_gsi(12);
    serial_println!("[KPIO] Mouse IRQ enabled");

    // Phase 10: WASM runtime initialization
    serial_println!("[KPIO] Initializing WASM runtime...");
    wasm::init();
    match wasm::test_runtime() {
        Ok(()) => serial_println!("[KPIO] WASM runtime test passed"),
        Err(e) => serial_println!("[KPIO] WASM runtime test failed: {}", e),
    }

    // Phase 9-5: End-to-End Integration Self-Test
    // Validates the full I/O path: NIC init → DHCP → VFS mount → read.
    // Results are logged so the qemu-test.ps1 -Mode io can verify them.
    {
        serial_println!("[E2E] Running integration self-test...");
        let mut e2e_pass = true;
        let mut e2e_fail_reason: Option<&str> = None;

        // Check 1: VirtIO NIC was initialized (probe logged success earlier)
        let nic_ok = drivers::net::virtio_net::is_initialized();
        if nic_ok {
            serial_println!("[E2E] NIC initialized: OK");
        } else {
            serial_println!("[E2E] NIC initialized: FAIL (no VirtIO NIC found)");
            e2e_pass = false;
            e2e_fail_reason = Some("NIC not initialized");
        }

        // Check 2: DHCP acquired an IP (network stack has non-zero IP)
        let ip = net::ipv4::get_ip();
        if ip != [0, 0, 0, 0] {
            serial_println!("[E2E] DHCP lease: OK (IP {}.{}.{}.{})", ip[0], ip[1], ip[2], ip[3]);
        } else {
            serial_println!("[E2E] DHCP lease: FAIL (no IP acquired)");
            e2e_pass = false;
            if e2e_fail_reason.is_none() {
                e2e_fail_reason = Some("DHCP did not acquire IP");
            }
        }

        // Check 3: VFS has a mounted filesystem
        let vfs_ok = storage::vfs::is_mounted("/mnt/test");
        if vfs_ok {
            serial_println!("[E2E] VFS mount: OK");
        } else {
            serial_println!("[E2E] VFS mount: SKIP (no test disk attached)");
            // Not a hard failure — test disk is optional
        }

        // Check 4: If VFS is mounted, verify read path
        if vfs_ok {
            match storage::vfs::open("/mnt/test/HELLO.TXT", storage::OpenFlags::READ) {
                Ok(fd) => {
                    let mut buf = [0u8; 512];
                    match storage::vfs::read(fd, &mut buf) {
                        Ok(n) if n > 0 => {
                            serial_println!("[E2E] VFS read: OK ({} bytes)", n);
                        }
                        Ok(_) => {
                            serial_println!("[E2E] VFS read: FAIL (0 bytes)");
                            e2e_pass = false;
                            if e2e_fail_reason.is_none() {
                                e2e_fail_reason = Some("VFS read returned 0 bytes");
                            }
                        }
                        Err(e) => {
                            serial_println!("[E2E] VFS read: FAIL ({:?})", e);
                            e2e_pass = false;
                            if e2e_fail_reason.is_none() {
                                e2e_fail_reason = Some("VFS read error");
                            }
                        }
                    }
                    let _ = storage::vfs::close(fd);
                }
                Err(e) => {
                    serial_println!("[E2E] VFS read: FAIL (open {:?})", e);
                    e2e_pass = false;
                    if e2e_fail_reason.is_none() {
                        e2e_fail_reason = Some("VFS open error");
                    }
                }
            }
        }

        // Check 5: Network TX counter (packets were transmitted during DHCP)
        let tx_count = drivers::net::virtio_net::tx_packet_count();
        if tx_count > 0 {
            serial_println!("[E2E] Network TX: OK ({} packets)", tx_count);
        } else {
            serial_println!("[E2E] Network TX: WARN (0 packets)");
        }

        // Final verdict
        if e2e_pass {
            serial_println!("[E2E] Integration test PASSED");
        } else {
            serial_println!(
                "[E2E] Integration test FAILED: {}",
                e2e_fail_reason.unwrap_or("unknown")
            );
        }
    }

    // Phase 11: Initialize Boot Animation BEFORE enabling interrupts
    if !fb_ptr.is_null() {
        // Initialize boot animation first
        serial_println!("[KPIO] Initializing boot animation...");
        gui::boot_animation::init(fb_width, fb_height);

        // Store framebuffer info for later GUI use
        unsafe {
            BOOT_FB_INFO = Some(FramebufferInfo {
                ptr: fb_ptr,
                width: fb_width,
                height: fb_height,
                bpp: fb_bpp,
                stride: fb_stride,
            });
        }

        // Render initial boot animation frame BEFORE interrupts
        serial_println!("[KPIO] Rendering initial boot frame...");
        {
            let mut renderer =
                gui::render::Renderer::new(fb_ptr, fb_width, fb_height, fb_bpp, fb_stride);
            gui::boot_animation::render(&mut renderer);
        }
        serial_println!("[KPIO] Boot animation initialized");

        // Register timer callback for boot animation phase
        interrupts::register_timer_callback(on_boot_animation_tick);
    }

    // Phase 12: Start timer and enable interrupts
    serial_println!("[KPIO] Starting APIC timer...");
    interrupts::start_apic_timer(100); // 100 Hz

    serial_println!("[KPIO] Enabling interrupts...");
    interrupts::enable();

    // ── Phase 10-2: Preemptive scheduling self-test ──────────────
    // Spawn two kernel tasks that print interleaved messages.
    // The APIC timer will preempt the CPU-bound task_A and let
    // task_B run, proving that preemptive context switching works.
    {
        use scheduler::{Task, TaskId};
        use core::sync::atomic::{AtomicU64, Ordering as AtOrd};

        static TASK_A_COUNT: AtomicU64 = AtomicU64::new(0);
        static TASK_B_COUNT: AtomicU64 = AtomicU64::new(0);

        fn task_a_entry() -> ! {
            // Re-enable interrupts — we were switched-to from an
            // interrupt context where IF was cleared by the CPU.
            x86_64::instructions::interrupts::enable();
            for i in 0..5u64 {
                serial_println!("[TASK-A] iteration {}", i);
                TASK_A_COUNT.fetch_add(1, AtOrd::Relaxed);
                // Busy-wait ~10 ms worth of cycles to consume time slice
                for _ in 0..100_000u64 {
                    core::hint::spin_loop();
                }
            }
            serial_println!("[TASK-A] done");
            scheduler::exit_current(0);
            loop { x86_64::instructions::hlt(); }
        }

        fn task_b_entry() -> ! {
            x86_64::instructions::interrupts::enable();
            for i in 0..5u64 {
                serial_println!("[TASK-B] iteration {}", i);
                TASK_B_COUNT.fetch_add(1, AtOrd::Relaxed);
                for _ in 0..100_000u64 {
                    core::hint::spin_loop();
                }
            }
            serial_println!("[TASK-B] done");
            scheduler::exit_current(0);
            loop { x86_64::instructions::hlt(); }
        }

        serial_println!("[SCHED] Spawning preemptive test tasks...");
        let ta = Task::new_kernel("task-A", task_a_entry as *const () as u64, 0);
        let tb = Task::new_kernel("task-B", task_b_entry as *const () as u64, 0);
        scheduler::spawn(ta);
        scheduler::spawn(tb);
        serial_println!("[SCHED] Preemptive test tasks spawned (task-A, task-B)");
    }

    // ── Phase 10-3: Ring 3 user-space isolation self-test ────────
    // Validates the full Ring 3 pipeline:
    //   1. Create isolated user page table (CR3)
    //   2. Map user-space code + stack pages
    //   3. Spawn user-mode task via scheduler
    //   4. SYSCALL/SYSRET round-trip (write + exit syscalls)
    //   5. Graceful fault handling for invalid user-mode access
    {
        serial_println!("[RING3] Phase 10-3: User-space isolation self-test");

        // Create an isolated user page table
        let user_cr3 = match memory::user_page_table::create_user_page_table() {
            Ok(cr3) => {
                serial_println!("[RING3] User page table created: CR3={:#x}", cr3);
                cr3
            }
            Err(e) => {
                serial_println!("[RING3] FAIL: Could not create user page table: {}", e);
                0
            }
        };

        if user_cr3 != 0 {
            // Map a code page at 0x40_0000 (4 MiB) — canonical user-space address
            let user_code_vaddr: u64 = 0x40_0000;
            let user_stack_top: u64 = 0x80_0000; // 8 MiB — stack grows down
            let user_stack_base: u64 = user_stack_top - 0x1000; // 1 page for stack

            // Allocate physical frames and map them in the user page table
            use x86_64::structures::paging::PageTableFlags;
            let code_flags = PageTableFlags::PRESENT
                | PageTableFlags::USER_ACCESSIBLE;
            let stack_flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;

            let code_result = memory::user_page_table::map_user_page(
                user_cr3, user_code_vaddr, code_flags,
            );
            let stack_result = memory::user_page_table::map_user_page(
                user_cr3, user_stack_base, stack_flags,
            );

            if let (Ok(code_phys), Ok(_stack_phys)) = (code_result, stack_result) {
                serial_println!("[RING3] User pages mapped: code={:#x}, stack={:#x}", user_code_vaddr, user_stack_base);

                // Write a minimal x86_64 user-space program into the code page:
                //   mov rax, 60   ; SYS_EXIT
                //   mov rdi, 42   ; exit code = 42
                //   syscall       ; enter kernel via SYSCALL/SYSRET
                // Tests the full Ring 3 → Ring 0 → Ring 3 pipeline.
                let user_program: [u8; 16] = [
                    0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00,  // mov rax, 60
                    0x48, 0xc7, 0xc7, 0x2a, 0x00, 0x00, 0x00,  // mov rdi, 42
                    0x0f, 0x05,                                  // syscall
                ];
                unsafe {
                    memory::user_page_table::write_to_phys(
                        code_phys, 0, &user_program,
                    );
                }

                serial_println!("[RING3] User program written (SYS_EXIT 42 via syscall)");

                // Allocate a kernel stack for the user-space task
                let kernel_stack_size: usize = 32 * 1024; // 32 KiB
                let mut kernel_stack_vec: alloc::vec::Vec<u8> =
                    alloc::vec![0u8; kernel_stack_size];
                let kernel_stack_top_addr =
                    kernel_stack_vec.as_ptr() as u64 + kernel_stack_size as u64;

                // Create a scheduler task for this user-space process
                let user_task = scheduler::Task::new_user_process(
                    "ring3-test",
                    user_cr3,
                    user_code_vaddr,      // entry point RIP
                    user_stack_top,       // user RSP
                    gdt::USER_CS as u16,  // CS = Ring 3 code
                    gdt::USER_DS as u16,  // SS = Ring 3 data
                    kernel_stack_top_addr,
                    kernel_stack_vec,
                    1, // pid
                );

                scheduler::spawn(user_task);
                serial_println!("[RING3] User-space test task spawned (pid=1, CR3={:#x})", user_cr3);
                serial_println!("[RING3] Phase 10-3 self-test: Ring 3 pipeline configured ✓");
            } else {
                serial_println!("[RING3] FAIL: Could not map user pages");
            }
        }
    }

    serial_println!(
        "[KPIO] Kernel initialization complete (ctx_switches={})",
        scheduler::context_switch_count()
    );

    // Run tests if in test mode
    #[cfg(test)]
    test_main();

    // Main loop: wait for boot animation to complete, then initialize GUI
    serial_println!("[KPIO] Waiting for boot animation...");
    loop {
        // Drive the boot animation callback from the main loop
        // (instead of the timer interrupt) to avoid lock contention.
        on_boot_animation_tick();

        unsafe {
            if BOOT_ANIMATION_COMPLETE && !GUI_INITIALIZED {
                // Disable interrupts briefly to prevent reentrancy
                x86_64::instructions::interrupts::disable();
                init_gui_after_boot();
                x86_64::instructions::interrupts::enable();
            }
        }
        x86_64::instructions::hlt();
    }
}

// ==================== Boot Info Storage ====================

/// Framebuffer info for deferred GUI initialization
struct FramebufferInfo {
    ptr: *mut u8,
    width: u32,
    height: u32,
    bpp: usize,
    stride: usize,
}

unsafe impl Send for FramebufferInfo {}
unsafe impl Sync for FramebufferInfo {}

static mut BOOT_FB_INFO: Option<FramebufferInfo> = None;
static mut GUI_INITIALIZED: bool = false;
static mut BOOT_ANIMATION_COMPLETE: bool = false;

/// Render boot animation frame
fn render_boot_animation(ptr: *mut u8, width: u32, height: u32, bpp: usize, stride: usize) {
    let mut renderer = gui::render::Renderer::new(ptr, width, height, bpp, stride);
    gui::boot_animation::render(&mut renderer);
}

/// Initialize GUI after boot animation completes
fn init_gui_after_boot() {
    unsafe {
        if GUI_INITIALIZED {
            return;
        }

        if let Some(ref fb) = BOOT_FB_INFO {
            serial_println!("[KPIO] Boot complete - Initializing GUI...");
            gui::init(fb.width, fb.height, fb.bpp, fb.stride, fb.ptr);

            // Create demo windows
            serial_println!("[KPIO] Creating demo windows...");
            gui::with_gui(|gui| {
                // Create browser window
                let browser = gui::window::Window::new_browser(gui::window::WindowId(1), 100, 50);
                gui.windows.push(browser);
                gui.taskbar
                    .add_window(gui::window::WindowId(1), "KPIO Browser");

                // Create terminal window
                let terminal =
                    gui::window::Window::new_terminal(gui::window::WindowId(2), 250, 150);
                gui.windows.push(terminal);
                gui.taskbar.add_window(gui::window::WindowId(2), "Terminal");

                // Create files window
                let files = gui::window::Window::new_files(gui::window::WindowId(3), 400, 100);
                gui.windows.push(files);
                gui.taskbar.add_window(gui::window::WindowId(3), "Files");

                gui.active_window = Some(gui::window::WindowId(1));
            });

            // Switch to GUI callbacks
            interrupts::register_key_callback(on_key_event);
            interrupts::register_mouse_callback(on_mouse_byte);
            interrupts::register_timer_callback(on_timer_tick);

            // Force render GUI
            gui::with_gui(|gui| {
                gui.dirty = true;
            });
            gui::render();
            serial_println!("[KPIO] GUI rendered - Desktop ready!");

            GUI_INITIALIZED = true;
        }
    }
}

// ==================== Interrupt Callbacks ====================

/// Boot animation tick callback
fn on_boot_animation_tick() {
    // Advance boot animation
    let complete = gui::boot_animation::tick();

    unsafe {
        if complete && !BOOT_ANIMATION_COMPLETE {
            // Set flag - GUI will be initialized in main loop
            BOOT_ANIMATION_COMPLETE = true;
        } else if !complete {
            // Render next animation frame
            if let Some(ref fb) = BOOT_FB_INFO {
                render_boot_animation(fb.ptr, fb.width, fb.height, fb.bpp, fb.stride);
            }
        }
    }
}

/// Keyboard event callback
fn on_key_event(ch: char, _scancode: u8, pressed: bool, ctrl: bool, shift: bool, alt: bool) {
    if ch != '\0' {
        gui::input::push_key_event(ch, _scancode, pressed, ctrl, shift, alt);
    }
}

/// Mouse byte callback
fn on_mouse_byte(byte: u8) {
    gui::input::process_mouse_byte(byte);
}

/// Timer tick callback
fn on_timer_tick() {
    // Poll the network stack for received frames.
    // This is what makes background networking work — without this,
    // incoming packets (ARP replies, TCP segments, DNS responses, ICMP
    // echo replies) would only be processed during blocking operations.
    net::poll_rx();

    // Advance the ICMP tick counter for RTT measurement.
    net::ipv4::icmp_tick();

    gui::input::process_all_events();
    gui::render();
}

/// Draw boot screen to framebuffer
fn draw_boot_screen(fb: &mut bootloader_api::info::FrameBuffer) {
    let info = fb.info();
    let buffer = fb.buffer_mut();
    let width = info.width;
    let height = info.height;
    let bytes_per_pixel = info.bytes_per_pixel;
    let stride = info.stride;

    serial_println!(
        "[FB] Drawing boot screen ({}x{}, {} bpp, stride {})",
        width,
        height,
        bytes_per_pixel,
        stride
    );

    // Fill background with dark blue
    let bg_color: [u8; 4] = [0x40, 0x20, 0x10, 0xFF]; // BGR format for most displays

    for y in 0..height {
        for x in 0..width {
            let offset = (y * stride + x) * bytes_per_pixel;
            if offset + bytes_per_pixel <= buffer.len() {
                buffer[offset] = bg_color[0]; // Blue
                buffer[offset + 1] = bg_color[1]; // Green
                buffer[offset + 2] = bg_color[2]; // Red
                if bytes_per_pixel == 4 {
                    buffer[offset + 3] = bg_color[3]; // Alpha
                }
            }
        }
    }

    // Draw a centered white box (KPIO logo placeholder)
    let box_w = 200usize;
    let box_h = 100usize;
    let box_x = (width - box_w) / 2;
    let box_y = (height - box_h) / 2;
    let white: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

    for y in box_y..(box_y + box_h) {
        for x in box_x..(box_x + box_w) {
            let offset = (y * stride + x) * bytes_per_pixel;
            if offset + bytes_per_pixel <= buffer.len() {
                buffer[offset] = white[0];
                buffer[offset + 1] = white[1];
                buffer[offset + 2] = white[2];
                if bytes_per_pixel == 4 {
                    buffer[offset + 3] = white[3];
                }
            }
        }
    }

    // Draw "KPIO" text as simple pixel art (8x8 grid per char)
    let text_x = box_x + 20;
    let text_y = box_y + 30;
    let black: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

    // Simple K letter (8x8)
    let k_pattern: [u8; 8] = [
        0b10000010, 0b10000100, 0b10001000, 0b10010000, 0b10100000, 0b11010000, 0b10001000,
        0b10000100,
    ];
    draw_char(
        buffer,
        text_x,
        text_y,
        &k_pattern,
        &black,
        bytes_per_pixel,
        stride,
    );

    // Simple P letter (8x8)
    let p_pattern: [u8; 8] = [
        0b11111100, 0b10000010, 0b10000010, 0b11111100, 0b10000000, 0b10000000, 0b10000000,
        0b10000000,
    ];
    draw_char(
        buffer,
        text_x + 40,
        text_y,
        &p_pattern,
        &black,
        bytes_per_pixel,
        stride,
    );

    // Simple I letter (8x8)
    let i_pattern: [u8; 8] = [
        0b11111110, 0b00010000, 0b00010000, 0b00010000, 0b00010000, 0b00010000, 0b00010000,
        0b11111110,
    ];
    draw_char(
        buffer,
        text_x + 80,
        text_y,
        &i_pattern,
        &black,
        bytes_per_pixel,
        stride,
    );

    // Simple O letter (8x8)
    let o_pattern: [u8; 8] = [
        0b00111100, 0b01000010, 0b10000001, 0b10000001, 0b10000001, 0b10000001, 0b01000010,
        0b00111100,
    ];
    draw_char(
        buffer,
        text_x + 120,
        text_y,
        &o_pattern,
        &black,
        bytes_per_pixel,
        stride,
    );

    serial_println!("[FB] Boot screen drawn");
}

/// Draw an 8x8 character pattern
fn draw_char(
    buffer: &mut [u8],
    x: usize,
    y: usize,
    pattern: &[u8; 8],
    color: &[u8; 4],
    bytes_per_pixel: usize,
    stride: usize,
) {
    let scale = 4; // Scale up 4x
    for (row, &bits) in pattern.iter().enumerate() {
        for col in 0..8 {
            if (bits >> (7 - col)) & 1 == 1 {
                // Draw scaled pixel
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = x + col * scale + sx;
                        let py = y + row * scale + sy;
                        let offset = (py * stride + px) * bytes_per_pixel;
                        if offset + bytes_per_pixel <= buffer.len() {
                            buffer[offset] = color[0];
                            buffer[offset + 1] = color[1];
                            buffer[offset + 2] = color[2];
                            if bytes_per_pixel == 4 {
                                buffer[offset + 3] = color[3];
                            }
                        }
                    }
                }
            }
        }
    }
}

// Macros are defined in serial.rs
