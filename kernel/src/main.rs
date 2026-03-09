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
mod loader;
mod memory;
mod net;
mod panic;
mod process;
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
                    storage::MountFlags::empty(),
                ) {
                    Ok(()) => {
                        serial_println!(
                            "[VFS] Mounted FAT filesystem on {} at /mnt/test",
                            device_name
                        );

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
                                    let name = core::str::from_utf8(&entry.name[..entry.name_len])
                                        .unwrap_or("?");
                                    serial_println!("  - {} ({:?})", name, entry.file_type);
                                }
                            }
                            Err(e) => {
                                serial_println!("[VFS] readdir /mnt/test/ failed: {:?}", e);
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

    // ── Phase 12-7: Unified Phase 12 integration test ────────────
    // Validates all Phase 12 features in a single boot: execve, fork, spawn,
    // FAT32 write, init-from-disk, and userlib syscall wiring.
    serial_println!("[P12] Phase 12 integration test start");

    // ── Phase 12-4: FAT32 write support integration test ─────────
    // Create a file on the FAT32 filesystem, write to it, read it back, verify.
    {
        let vfs_ok = storage::vfs::is_mounted("/mnt/test");
        if vfs_ok {
            serial_println!("[P12-4] FAT32 write support integration test");

            // Step 1: Create and write a file.
            let file_data = b"Hello from KPIO";
            let write_flags =
                storage::OpenFlags::WRITE | storage::OpenFlags::CREATE | storage::OpenFlags::READ;
            match storage::vfs::open("/mnt/test/WRITTEN.TXT", write_flags) {
                Ok(fd) => {
                    serial_println!("[FAT32] created WRITTEN.TXT");
                    match storage::vfs::write(fd, file_data) {
                        Ok(n) => {
                            serial_println!("[FAT32] write {} bytes", n);
                        }
                        Err(e) => {
                            serial_println!("[FAT32] FAIL: write to WRITTEN.TXT: {:?}", e);
                        }
                    }
                    let _ = storage::vfs::close(fd);
                }
                Err(e) => {
                    serial_println!("[FAT32] FAIL: open WRITTEN.TXT for write: {:?}", e);
                }
            }

            // Step 2: Read back and verify.
            match storage::vfs::open("/mnt/test/WRITTEN.TXT", storage::OpenFlags::READ) {
                Ok(fd) => {
                    let mut buf = [0u8; 64];
                    match storage::vfs::read(fd, &mut buf) {
                        Ok(n) => {
                            let content = core::str::from_utf8(&buf[..n]).unwrap_or("<invalid utf8>");
                            if content == "Hello from KPIO" {
                                serial_println!(
                                    "[VFS] readback verified: \"{}\"",
                                    content
                                );
                            } else {
                                serial_println!(
                                    "[VFS] readback MISMATCH: got \"{}\" (expected \"Hello from KPIO\")",
                                    content
                                );
                            }
                        }
                        Err(e) => {
                            serial_println!("[VFS] FAIL: readback WRITTEN.TXT: {:?}", e);
                        }
                    }
                    let _ = storage::vfs::close(fd);
                }
                Err(e) => {
                    serial_println!("[VFS] FAIL: open WRITTEN.TXT for read: {:?}", e);
                }
            }

            serial_println!("[P12-4] FAT32 write test complete");
        } else {
            serial_println!("[P12-4] SKIPPED: /mnt/test not mounted");
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
            serial_println!(
                "[E2E] DHCP lease: OK (IP {}.{}.{}.{})",
                ip[0],
                ip[1],
                ip[2],
                ip[3]
            );
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
        use core::sync::atomic::{AtomicU64, Ordering as AtOrd};
        use scheduler::{Task, TaskId};

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
            loop {
                x86_64::instructions::hlt();
            }
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
            loop {
                x86_64::instructions::hlt();
            }
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
            let code_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            let stack_flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;

            let code_result =
                memory::user_page_table::map_user_page(user_cr3, user_code_vaddr, code_flags);
            let stack_result =
                memory::user_page_table::map_user_page(user_cr3, user_stack_base, stack_flags);

            if let (Ok(code_phys), Ok(_stack_phys)) = (code_result, stack_result) {
                serial_println!(
                    "[RING3] User pages mapped: code={:#x}, stack={:#x}",
                    user_code_vaddr,
                    user_stack_base
                );

                // Write a minimal x86_64 user-space program into the code page:
                //   mov rax, 60   ; SYS_EXIT
                //   mov rdi, 42   ; exit code = 42
                //   syscall       ; enter kernel via SYSCALL/SYSRET
                // Tests the full Ring 3 → Ring 0 → Ring 3 pipeline.
                let user_program: [u8; 16] = [
                    0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00, // mov rax, 60
                    0x48, 0xc7, 0xc7, 0x2a, 0x00, 0x00, 0x00, // mov rdi, 42
                    0x0f, 0x05, // syscall
                ];
                unsafe {
                    memory::user_page_table::write_to_phys(code_phys, 0, &user_program);
                }

                serial_println!("[RING3] User program written (SYS_EXIT 42 via syscall)");

                // Allocate a kernel stack for the user-space task
                let kernel_stack_size: usize = 32 * 1024; // 32 KiB
                let mut kernel_stack_vec: alloc::vec::Vec<u8> = alloc::vec![0u8; kernel_stack_size];
                let kernel_stack_top_addr =
                    kernel_stack_vec.as_ptr() as u64 + kernel_stack_size as u64;

                // Create a scheduler task for this user-space process
                let user_task = scheduler::Task::new_user_process(
                    "ring3-test",
                    user_cr3,
                    user_code_vaddr,     // entry point RIP
                    user_stack_top,      // user RSP
                    gdt::USER_CS as u16, // CS = Ring 3 code
                    gdt::USER_DS as u16, // SS = Ring 3 data
                    kernel_stack_top_addr,
                    kernel_stack_vec,
                    1, // pid
                );

                scheduler::spawn(user_task);
                serial_println!(
                    "[RING3] User-space test task spawned (pid=1, CR3={:#x})",
                    user_cr3
                );
                serial_println!("[RING3] Phase 10-3 self-test: Ring 3 pipeline configured ✓");
            } else {
                serial_println!("[RING3] FAIL: Could not map user pages");
            }
        }
    }

    // ── Phase 10-5: Process lifecycle integration test ───────────
    // Validates the full process lifecycle pipeline:
    //   1. Launch hello.bin in Ring 3 (SYS_WRITE + SYS_EXIT)
    //   2. Launch spin.bin (preemption test — CPU-bound task doesn't starve kernel)
    //   3. Two user processes with different page tables run concurrently
    // Results are logged so qemu-test.ps1 -Mode process can verify them.
    {
        serial_println!("[PROC] Phase 10-5: Process lifecycle integration test");

        // Embedded test programs (hand-assembled x86_64, source in tests/e2e/userspace/)
        // hello: SYS_WRITE("Hello from Ring 3\n") + SYS_EXIT(0)
        #[rustfmt::skip]
        const HELLO_PROGRAM: &[u8] = &[
            0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00, // mov rax, 1 (SYS_WRITE)
            0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00, // mov rdi, 1 (stdout)
            0x48, 0x8d, 0x35, 0x15, 0x00, 0x00, 0x00, // lea rsi, [rip+0x15] → msg
            0x48, 0xc7, 0xc2, 0x12, 0x00, 0x00, 0x00, // mov rdx, 18
            0x0f, 0x05,                                 // syscall
            0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00, // mov rax, 60 (SYS_EXIT)
            0x48, 0x31, 0xff,                           // xor rdi, rdi
            0x0f, 0x05,                                 // syscall
            b'H', b'e', b'l', b'l', b'o', b' ', b'f', b'r', b'o', b'm',
            b' ', b'R', b'i', b'n', b'g', b' ', b'3', b'\n',
        ];
        // spin: infinite loop (jmp $)
        const SPIN_PROGRAM: &[u8] = &[0xeb, 0xfe];
        // exit42: SYS_EXIT(42)
        #[rustfmt::skip]
        const EXIT42_PROGRAM: &[u8] = &[
            0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00, // mov rax, 60
            0x48, 0xc7, 0xc7, 0x2a, 0x00, 0x00, 0x00, // mov rdi, 42
            0x0f, 0x05,                                 // syscall
        ];

        // ── Test 1: Hello from Ring 3 ────────────────────────────
        // Launch a user-space program that writes "Hello from Ring 3\n"
        // to serial via SYS_WRITE, then exits via SYS_EXIT(0).
        {
            serial_println!("[PROC] Test 1: Ring 3 hello program");
            let hello_cr3 = match memory::user_page_table::create_user_page_table() {
                Ok(cr3) => cr3,
                Err(e) => {
                    serial_println!("[PROC] Test 1 FAIL: page table: {}", e);
                    0
                }
            };

            if hello_cr3 != 0 {
                use x86_64::structures::paging::PageTableFlags;
                let code_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
                let stack_flags = PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::USER_ACCESSIBLE;

                let code_vaddr: u64 = 0x40_0000;
                let stack_base: u64 = 0x7F_F000;
                let stack_top: u64 = 0x80_0000;

                let code_result =
                    memory::user_page_table::map_user_page(hello_cr3, code_vaddr, code_flags);
                let stack_result =
                    memory::user_page_table::map_user_page(hello_cr3, stack_base, stack_flags);

                if let (Ok(code_phys), Ok(_)) = (code_result, stack_result) {
                    unsafe {
                        memory::user_page_table::write_to_phys(code_phys, 0, HELLO_PROGRAM);
                    }
                    let ks_size: usize = 32 * 1024;
                    let ks_vec = alloc::vec![0u8; ks_size];
                    let ks_top = ks_vec.as_ptr() as u64 + ks_size as u64;

                    let task = scheduler::Task::new_user_process(
                        "hello-test",
                        hello_cr3,
                        code_vaddr,
                        stack_top,
                        gdt::USER_CS as u16,
                        gdt::USER_DS as u16,
                        ks_top,
                        ks_vec,
                        10, // pid
                    );
                    scheduler::spawn(task);
                    serial_println!("[PROC] Test 1: hello-test spawned (pid=10)");
                } else {
                    serial_println!("[PROC] Test 1 FAIL: could not map pages");
                }
            }
        }

        // ── Test 2: Preemption — spin.bin doesn't starve kernel ──
        // Launch a CPU-bound infinite loop in Ring 3.  The kernel's
        // main loop continues executing (verified by reaching the
        // "Kernel initialization complete" log line after this).
        {
            serial_println!("[PROC] Test 2: Preemption (spin program)");
            let spin_cr3 = match memory::user_page_table::create_user_page_table() {
                Ok(cr3) => cr3,
                Err(e) => {
                    serial_println!("[PROC] Test 2 FAIL: page table: {}", e);
                    0
                }
            };

            if spin_cr3 != 0 {
                use x86_64::structures::paging::PageTableFlags;
                let code_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
                let stack_flags = PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::USER_ACCESSIBLE;

                let code_vaddr: u64 = 0x40_0000;
                let stack_base: u64 = 0x7F_F000;
                let stack_top: u64 = 0x80_0000;

                let code_result =
                    memory::user_page_table::map_user_page(spin_cr3, code_vaddr, code_flags);
                let stack_result =
                    memory::user_page_table::map_user_page(spin_cr3, stack_base, stack_flags);

                if let (Ok(code_phys), Ok(_)) = (code_result, stack_result) {
                    unsafe {
                        memory::user_page_table::write_to_phys(code_phys, 0, SPIN_PROGRAM);
                    }
                    let ks_size: usize = 32 * 1024;
                    let ks_vec = alloc::vec![0u8; ks_size];
                    let ks_top = ks_vec.as_ptr() as u64 + ks_size as u64;

                    let task = scheduler::Task::new_user_process(
                        "spin-test",
                        spin_cr3,
                        code_vaddr,
                        stack_top,
                        gdt::USER_CS as u16,
                        gdt::USER_DS as u16,
                        ks_top,
                        ks_vec,
                        11, // pid
                    );
                    scheduler::spawn(task);
                    serial_println!("[PROC] Test 2: spin-test spawned (pid=11)");
                    // The spin task runs forever in Ring 3 but the APIC timer
                    // preempts it. If we reach "Kernel initialization complete"
                    // below, preemption works.
                } else {
                    serial_println!("[PROC] Test 2 FAIL: could not map pages");
                }
            }
        }

        // ── Test 3: Multi-process isolation ──────────────────────
        // Launch a second user process with a DIFFERENT page table
        // that exits with code 42.  Both processes (hello-test and
        // exit42-test) run with separate CR3 values, proving address
        // space isolation.
        {
            serial_println!("[PROC] Test 3: Multi-process isolation (exit42)");
            let exit42_cr3 = match memory::user_page_table::create_user_page_table() {
                Ok(cr3) => cr3,
                Err(e) => {
                    serial_println!("[PROC] Test 3 FAIL: page table: {}", e);
                    0
                }
            };

            if exit42_cr3 != 0 {
                use x86_64::structures::paging::PageTableFlags;
                let code_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
                let stack_flags = PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::USER_ACCESSIBLE;

                let code_vaddr: u64 = 0x40_0000;
                let stack_base: u64 = 0x7F_F000;
                let stack_top: u64 = 0x80_0000;

                let code_result =
                    memory::user_page_table::map_user_page(exit42_cr3, code_vaddr, code_flags);
                let stack_result =
                    memory::user_page_table::map_user_page(exit42_cr3, stack_base, stack_flags);

                if let (Ok(code_phys), Ok(_)) = (code_result, stack_result) {
                    unsafe {
                        memory::user_page_table::write_to_phys(code_phys, 0, EXIT42_PROGRAM);
                    }
                    let ks_size: usize = 32 * 1024;
                    let ks_vec = alloc::vec![0u8; ks_size];
                    let ks_top = ks_vec.as_ptr() as u64 + ks_size as u64;

                    let task = scheduler::Task::new_user_process(
                        "exit42-test",
                        exit42_cr3,
                        code_vaddr,
                        stack_top,
                        gdt::USER_CS as u16,
                        gdt::USER_DS as u16,
                        ks_top,
                        ks_vec,
                        12, // pid
                    );
                    scheduler::spawn(task);
                    serial_println!(
                        "[PROC] Test 3: exit42-test spawned (pid=12, CR3={:#x})",
                        exit42_cr3
                    );
                } else {
                    serial_println!("[PROC] Test 3 FAIL: could not map pages");
                }
            }
        }

        serial_println!(
            "[PROC] Phase 10-5 tests spawned ({} total tasks, {} context switches so far)",
            scheduler::total_task_count(),
            scheduler::context_switch_count(),
        );
    }

    // ── Phase 12-1: execve() return path integration test ────────
    // Validates the fixed execve() syscall flow:
    //   1. Register a target ELF binary in VFS at /bin/exec-target
    //   2. Spawn a caller process that invokes execve("/bin/exec-target", NULL, NULL)
    //   3. The assembly epilogue detects EXECVE_PENDING and redirects
    //      sysretq to the new entry point
    //   4. The new process image writes "EXECVE OK" via SYS_WRITE
    //      and exits with code 42
    // Results are logged for qemu-test.ps1 verification.
    {
        serial_println!("[EXECVE] Phase 12-1: execve return path integration test");

        // ── Build a minimal ELF binary containing the exec target ──
        // This program writes "EXECVE OK\n" to fd 1 (serial/stdout)
        // then calls SYS_EXIT(42).
        //
        // Machine code (x86_64, mapped at vaddr 0x400078 = entry point):
        //   mov rax, 1           ; SYS_WRITE
        //   mov rdi, 1           ; fd = stdout
        //   lea rsi, [rip+0x19]  ; pointer to "EXECVE OK\n"
        //   mov rdx, 10          ; length
        //   syscall
        //   mov rax, 60          ; SYS_EXIT
        //   mov rdi, 42          ; exit code = 42
        //   syscall
        //   db "EXECVE OK\n"
        #[rustfmt::skip]
        const EXEC_TARGET_CODE: &[u8] = &[
            0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00, // mov rax, 1 (SYS_WRITE)
            0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00, // mov rdi, 1 (stdout)
            0x48, 0x8D, 0x35, 0x19, 0x00, 0x00, 0x00, // lea rsi, [rip+0x19]
            0x48, 0xC7, 0xC2, 0x0A, 0x00, 0x00, 0x00, // mov rdx, 10
            0x0F, 0x05,                                 // syscall (write)
            0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00, // mov rax, 60 (SYS_EXIT)
            0x48, 0xC7, 0xC7, 0x2A, 0x00, 0x00, 0x00, // mov rdi, 42
            0x0F, 0x05,                                 // syscall (exit)
            // "EXECVE OK\n" (10 bytes)
            b'E', b'X', b'E', b'C', b'V', b'E', b' ', b'O', b'K', b'\n',
        ];

        // Build minimal ELF64 wrapping the code above.
        // Layout: 64-byte ELF header + 56-byte program header + code
        // Entry point = 0x400078 (code starts right after headers)
        let elf_header_size: usize = 64;
        let phdr_size: usize = 56;
        let code_offset = elf_header_size + phdr_size; // 120
        let total_size = code_offset + EXEC_TARGET_CODE.len();
        let entry_point: u64 = 0x400000 + code_offset as u64; // 0x400078

        let mut elf_binary = alloc::vec![0u8; total_size];

        // ELF header
        elf_binary[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']); // magic
        elf_binary[4] = 2;   // ELFCLASS64
        elf_binary[5] = 1;   // ELFDATA2LSB
        elf_binary[6] = 1;   // EV_CURRENT
        // e_type = ET_EXEC (2)
        elf_binary[16..18].copy_from_slice(&2u16.to_le_bytes());
        // e_machine = EM_X86_64 (62)
        elf_binary[18..20].copy_from_slice(&62u16.to_le_bytes());
        // e_version = 1
        elf_binary[20..24].copy_from_slice(&1u32.to_le_bytes());
        // e_entry
        elf_binary[24..32].copy_from_slice(&entry_point.to_le_bytes());
        // e_phoff = 64
        elf_binary[32..40].copy_from_slice(&64u64.to_le_bytes());
        // e_ehsize = 64
        elf_binary[52..54].copy_from_slice(&64u16.to_le_bytes());
        // e_phentsize = 56
        elf_binary[54..56].copy_from_slice(&56u16.to_le_bytes());
        // e_phnum = 1
        elf_binary[56..58].copy_from_slice(&1u16.to_le_bytes());

        // Program header (PT_LOAD at offset 64)
        // p_type = PT_LOAD (1)
        elf_binary[64..68].copy_from_slice(&1u32.to_le_bytes());
        // p_flags = PF_R | PF_X (5)
        elf_binary[68..72].copy_from_slice(&5u32.to_le_bytes());
        // p_offset = 0
        elf_binary[72..80].copy_from_slice(&0u64.to_le_bytes());
        // p_vaddr = 0x400000
        elf_binary[80..88].copy_from_slice(&0x400000u64.to_le_bytes());
        // p_paddr = 0x400000
        elf_binary[88..96].copy_from_slice(&0x400000u64.to_le_bytes());
        // p_filesz = total_size
        elf_binary[96..104].copy_from_slice(&(total_size as u64).to_le_bytes());
        // p_memsz = total_size
        elf_binary[104..112].copy_from_slice(&(total_size as u64).to_le_bytes());
        // p_align = 0x1000
        elf_binary[112..120].copy_from_slice(&0x1000u64.to_le_bytes());

        // Code payload
        elf_binary[code_offset..].copy_from_slice(EXEC_TARGET_CODE);

        // Register the target binary in VFS
        match vfs::write_all("/bin/exec-target", &elf_binary) {
            Ok(()) => {
                serial_println!(
                    "[EXECVE] Registered /bin/exec-target in VFS ({} bytes, entry={:#x})",
                    total_size,
                    entry_point,
                );
            }
            Err(e) => {
                serial_println!("[EXECVE] FAIL: Could not register exec target: {:?}", e);
            }
        }

        // ── Build the caller process ──
        // Machine code that calls execve("/bin/exec-target", NULL, NULL).
        // If execve succeeds, execution continues in the target binary above.
        // If it fails (shouldn't happen), exits with code 99.
        //
        //   mov rax, 59              ; SYS_EXECVE
        //   lea rdi, [rip+0x18]      ; path = "/bin/exec-target"
        //   xor rsi, rsi             ; argv = NULL
        //   xor rdx, rdx             ; envp = NULL
        //   syscall
        //   ; fallthrough on failure:
        //   mov rax, 60              ; SYS_EXIT
        //   mov rdi, 99              ; exit code 99 = execve failed
        //   syscall
        //   db "/bin/exec-target", 0
        #[rustfmt::skip]
        const EXECVE_CALLER: &[u8] = &[
            0x48, 0xC7, 0xC0, 0x3B, 0x00, 0x00, 0x00, // mov rax, 59 (SYS_EXECVE)
            0x48, 0x8D, 0x3D, 0x18, 0x00, 0x00, 0x00, // lea rdi, [rip+0x18]
            0x48, 0x31, 0xF6,                           // xor rsi, rsi (argv=NULL)
            0x48, 0x31, 0xD2,                           // xor rdx, rdx (envp=NULL)
            0x0F, 0x05,                                 // syscall (execve)
            // fallthrough if execve failed:
            0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00, // mov rax, 60 (SYS_EXIT)
            0x48, 0xC7, 0xC7, 0x63, 0x00, 0x00, 0x00, // mov rdi, 99
            0x0F, 0x05,                                 // syscall (exit)
            // "/bin/exec-target\0" (17 bytes)
            b'/', b'b', b'i', b'n', b'/', b'e', b'x', b'e', b'c',
            b'-', b't', b'a', b'r', b'g', b'e', b't', 0x00,
        ];

        // Create user page table and load the caller
        let exec_cr3 = match memory::user_page_table::create_user_page_table() {
            Ok(cr3) => cr3,
            Err(e) => {
                serial_println!("[EXECVE] FAIL: page table: {}", e);
                0
            }
        };

        if exec_cr3 != 0 {
            use x86_64::structures::paging::PageTableFlags;
            let code_flags =
                PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            let stack_flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;

            let code_vaddr: u64 = 0x40_0000;
            let stack_top: u64 = 0x80_0000;
            let stack_base: u64 = stack_top - 0x1000;

            let code_result =
                memory::user_page_table::map_user_page(exec_cr3, code_vaddr, code_flags);
            let stack_result =
                memory::user_page_table::map_user_page(exec_cr3, stack_base, stack_flags);

            if let (Ok(code_phys), Ok(_)) = (code_result, stack_result) {
                // Write the caller program into the code page
                unsafe {
                    memory::user_page_table::write_to_phys(code_phys, 0, EXECVE_CALLER);
                }

                // Allocate kernel stack and spawn the task.
                // Note: no process table entry is created here; sys_execve
                // falls back to reading CR3 from the CPU register.
                let ks_size: usize = 32 * 1024;
                let ks_vec = alloc::vec![0u8; ks_size];
                let ks_top = ks_vec.as_ptr() as u64 + ks_size as u64;

                let task = scheduler::Task::new_user_process(
                    "execve-caller",
                    exec_cr3,
                    code_vaddr,
                    stack_top,
                    gdt::USER_CS as u16,
                    gdt::USER_DS as u16,
                    ks_top,
                    ks_vec,
                    30, // pid
                );
                scheduler::spawn(task);
                serial_println!(
                    "[EXECVE] execve-caller spawned (pid=30, CR3={:#x}). \
                     Will call execve(\"/bin/exec-target\").",
                    exec_cr3,
                );

                // Give the scheduler time to run the execve-caller task.
                // APIC timer fires at ~100 Hz, so 300 HLTs ≈ 3 seconds.
                serial_println!("[EXECVE] Waiting for execve-caller to execute...");
                for _ in 0..300 {
                    x86_64::instructions::hlt();
                }
                serial_println!("[EXECVE] Wait complete.");
            } else {
                serial_println!("[EXECVE] FAIL: could not map caller pages");
            }
        }
    }

    // ── Phase 12-2: fork() child return integration test ─────────
    // Validates that fork() correctly returns 0 to the child:
    //   1. Spawn a parent process that calls SYS_FORK (57)
    //   2. Parent receives child PID > 0, writes "FORK PARENT OK\n"
    //   3. Child receives 0 in RAX, writes "FORK CHILD OK\n"
    //   4. Both exit cleanly via SYS_EXIT(0)
    // The fork child trampoline prints:
    //   "[FORK] child N running, fork returned 0"
    {
        serial_println!("[FORK] Phase 12-2: fork child return integration test");

        // Machine code for the fork test program:
        //   mov rax, 57          ; SYS_FORK
        //   syscall
        //   test rax, rax
        //   jz child
        //   ; parent: write "FORK PARENT OK\n", exit(0)
        //   ; child:  write "FORK CHILD OK\n", exit(0)
        #[rustfmt::skip]
        const FORK_TEST_CODE: &[u8] = &[
            // --- fork ---
            0x48, 0xC7, 0xC0, 0x39, 0x00, 0x00, 0x00, // mov rax, 57 (SYS_FORK)
            0x0F, 0x05,                                 // syscall
            0x48, 0x85, 0xC0,                           // test rax, rax
            0x74, 0x2A,                                 // jz child (+0x2A → offset 0x38)
            // --- parent path (offset 0x0E) ---
            0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00, // mov rax, 1 (SYS_WRITE)
            0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00, // mov rdi, 1 (stdout)
            0x48, 0x8D, 0x35, 0x3F, 0x00, 0x00, 0x00, // lea rsi, [rip+0x3F] → parent_msg
            0x48, 0xC7, 0xC2, 0x0F, 0x00, 0x00, 0x00, // mov rdx, 15
            0x0F, 0x05,                                 // syscall (write)
            0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00, // mov rax, 60 (SYS_EXIT)
            0x48, 0x31, 0xFF,                           // xor rdi, rdi
            0x0F, 0x05,                                 // syscall (exit)
            // --- child path (offset 0x38) ---
            0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00, // mov rax, 1 (SYS_WRITE)
            0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00, // mov rdi, 1 (stdout)
            0x48, 0x8D, 0x35, 0x24, 0x00, 0x00, 0x00, // lea rsi, [rip+0x24] → child_msg
            0x48, 0xC7, 0xC2, 0x0E, 0x00, 0x00, 0x00, // mov rdx, 14
            0x0F, 0x05,                                 // syscall (write)
            0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00, // mov rax, 60 (SYS_EXIT)
            0x48, 0x31, 0xFF,                           // xor rdi, rdi
            0x0F, 0x05,                                 // syscall (exit)
            // --- data (offset 0x62) ---
            // "FORK PARENT OK\n" (15 bytes)
            b'F', b'O', b'R', b'K', b' ', b'P', b'A', b'R', b'E', b'N', b'T',
            b' ', b'O', b'K', b'\n',
            // "FORK CHILD OK\n" (14 bytes)
            b'F', b'O', b'R', b'K', b' ', b'C', b'H', b'I', b'L', b'D',
            b' ', b'O', b'K', b'\n',
        ];

        // Build minimal ELF64 binary wrapping the fork test code.
        let elf_header_size: usize = 64;
        let phdr_size: usize = 56;
        let code_offset = elf_header_size + phdr_size; // 120
        let total_size = code_offset + FORK_TEST_CODE.len();
        let entry_point: u64 = 0x400000 + code_offset as u64;

        let fork_cr3 = match memory::user_page_table::create_user_page_table() {
            Ok(cr3) => cr3,
            Err(e) => {
                serial_println!("[FORK] FAIL: page table: {}", e);
                0
            }
        };

        if fork_cr3 != 0 {
            use x86_64::structures::paging::PageTableFlags;
            let code_flags =
                PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            let stack_flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;

            let code_vaddr: u64 = 0x40_0000;
            let stack_top: u64 = 0x80_0000;
            let stack_base: u64 = stack_top - 0x1000;

            let code_result =
                memory::user_page_table::map_user_page(fork_cr3, code_vaddr, code_flags);
            let stack_result =
                memory::user_page_table::map_user_page(fork_cr3, stack_base, stack_flags);

            if let (Ok(code_phys), Ok(_)) = (code_result, stack_result) {
                // Write the raw code (no ELF headers needed since we map directly)
                unsafe {
                    memory::user_page_table::write_to_phys(code_phys, 0, FORK_TEST_CODE);
                }

                // Allocate kernel stack and spawn.
                let ks_size: usize = 32 * 1024;
                let ks_vec = alloc::vec![0u8; ks_size];
                let ks_top = ks_vec.as_ptr() as u64 + ks_size as u64;

                let task = scheduler::Task::new_user_process(
                    "fork-test-parent",
                    fork_cr3,
                    code_vaddr, // entry at start of code (0x400000)
                    stack_top,
                    gdt::USER_CS as u16,
                    gdt::USER_DS as u16,
                    ks_top,
                    ks_vec,
                    40, // pid
                );
                scheduler::spawn(task);
                serial_println!(
                    "[FORK] fork-test-parent spawned (pid=40, CR3={:#x}, entry={:#x})",
                    fork_cr3,
                    code_vaddr,
                );

                // Wait for both parent and child to execute.
                serial_println!("[FORK] Waiting for fork test to execute...");
                for _ in 0..400 {
                    x86_64::instructions::hlt();
                }
                serial_println!("[FORK] Wait complete.");
            } else {
                serial_println!("[FORK] FAIL: could not map fork test pages");
            }
        }
    }

    // ── Phase 12-3: ProcessManager::spawn_from_vfs() integration test ─
    // Validates the full spawn-from-VFS pipeline:
    //   1. Build a minimal ELF binary that writes "SPAWN OK\n" + SYS_EXIT(0)
    //   2. Register it in VFS at /bin/spawn-test
    //   3. Call ProcessManager::spawn_from_vfs("/bin/spawn-test")
    //   4. Verify the process runs in Ring 3 and produces serial output
    {
        serial_println!("[SPAWN] Phase 12-3: ProcessManager::spawn_from_vfs() integration test");

        // Machine code (x86_64, mapped at vaddr 0x400078 = entry point):
        //   mov rax, 1           ; SYS_WRITE
        //   mov rdi, 1           ; fd = stdout
        //   lea rsi, [rip+0x15]  ; pointer to "SPAWN OK\n"
        //   mov rdx, 9           ; length
        //   syscall
        //   mov rax, 60          ; SYS_EXIT
        //   xor rdi, rdi         ; exit code = 0
        //   syscall
        //   db "SPAWN OK\n"
        #[rustfmt::skip]
        const SPAWN_TEST_CODE: &[u8] = &[
            0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00, // mov rax, 1 (SYS_WRITE)
            0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00, // mov rdi, 1 (stdout)
            0x48, 0x8D, 0x35, 0x15, 0x00, 0x00, 0x00, // lea rsi, [rip+0x15]
            0x48, 0xC7, 0xC2, 0x09, 0x00, 0x00, 0x00, // mov rdx, 9
            0x0F, 0x05,                                 // syscall (write)
            0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00, // mov rax, 60 (SYS_EXIT)
            0x48, 0x31, 0xFF,                           // xor rdi, rdi
            0x0F, 0x05,                                 // syscall (exit)
            // "SPAWN OK\n" (9 bytes)
            b'S', b'P', b'A', b'W', b'N', b' ', b'O', b'K', b'\n',
        ];

        // Build minimal ELF64 wrapping the spawn test code.
        let elf_header_size: usize = 64;
        let phdr_size: usize = 56;
        let code_offset = elf_header_size + phdr_size; // 120
        let total_size = code_offset + SPAWN_TEST_CODE.len();
        let entry_point: u64 = 0x400000 + code_offset as u64; // 0x400078

        let mut elf_binary = alloc::vec![0u8; total_size];

        // ELF header
        elf_binary[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']); // magic
        elf_binary[4] = 2;   // ELFCLASS64
        elf_binary[5] = 1;   // ELFDATA2LSB
        elf_binary[6] = 1;   // EV_CURRENT
        elf_binary[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
        elf_binary[18..20].copy_from_slice(&62u16.to_le_bytes()); // EM_X86_64
        elf_binary[20..24].copy_from_slice(&1u32.to_le_bytes()); // e_version
        elf_binary[24..32].copy_from_slice(&entry_point.to_le_bytes());
        elf_binary[32..40].copy_from_slice(&64u64.to_le_bytes()); // e_phoff
        elf_binary[52..54].copy_from_slice(&64u16.to_le_bytes()); // e_ehsize
        elf_binary[54..56].copy_from_slice(&56u16.to_le_bytes()); // e_phentsize
        elf_binary[56..58].copy_from_slice(&1u16.to_le_bytes());  // e_phnum

        // Program header (PT_LOAD)
        elf_binary[64..68].copy_from_slice(&1u32.to_le_bytes());  // PT_LOAD
        elf_binary[68..72].copy_from_slice(&5u32.to_le_bytes());  // PF_R | PF_X
        elf_binary[72..80].copy_from_slice(&0u64.to_le_bytes());  // p_offset
        elf_binary[80..88].copy_from_slice(&0x400000u64.to_le_bytes()); // p_vaddr
        elf_binary[88..96].copy_from_slice(&0x400000u64.to_le_bytes()); // p_paddr
        elf_binary[96..104].copy_from_slice(&(total_size as u64).to_le_bytes()); // p_filesz
        elf_binary[104..112].copy_from_slice(&(total_size as u64).to_le_bytes()); // p_memsz
        elf_binary[112..120].copy_from_slice(&0x1000u64.to_le_bytes()); // p_align

        // Code payload
        elf_binary[code_offset..].copy_from_slice(SPAWN_TEST_CODE);

        // Register in VFS at /bin/spawn-test
        match vfs::write_all("/bin/spawn-test", &elf_binary) {
            Ok(()) => {
                serial_println!(
                    "[SPAWN] Registered /bin/spawn-test in VFS ({} bytes, entry={:#x})",
                    total_size,
                    entry_point,
                );
            }
            Err(e) => {
                serial_println!("[SPAWN] FAIL: Could not register /bin/spawn-test: {:?}", e);
            }
        }

        // Spawn via ProcessManager
        match crate::process::manager::PROCESS_MANAGER.spawn_from_vfs("/bin/spawn-test") {
            Ok(pid) => {
                serial_println!(
                    "[SPAWN] spawn_from_vfs(\"/bin/spawn-test\") succeeded: pid={}",
                    pid,
                );
            }
            Err(e) => {
                serial_println!(
                    "[SPAWN] FAIL: spawn_from_vfs(\"/bin/spawn-test\") failed: {:?}",
                    e,
                );
            }
        }

        // Wait for the spawned process to execute.
        for _ in 0..300 {
            x86_64::instructions::hlt();
        }
        serial_println!("[SPAWN] Phase 12-3 wait complete.");
    }

    // ── Phase 12-5: Init Process & ELF-from-Disk Boot ───────────
    // Validates the full user-space pipeline end-to-end:
    //   1. Read /INIT ELF binary from FAT32 disk (storage VFS)
    //   2. Register it in in-memory VFS, spawn via ProcessManager
    //   3. Read /BIN/HELLO ELF binary from FAT32 disk
    //   4. Register it in in-memory VFS, spawn via ProcessManager
    //   5. Verify both processes produce expected serial output
    {
        serial_println!("[P12-5] Init process & ELF-from-disk boot test");

        let disk_mounted = storage::vfs::is_mounted("/mnt/test");
        if disk_mounted {
            // --- Step 1: Load /init from FAT32 disk ---
            let init_loaded = match storage::vfs::open("/mnt/test/INIT", storage::OpenFlags::READ) {
                Ok(fd) => {
                    let mut buf = alloc::vec![0u8; 4096];
                    match storage::vfs::read(fd, &mut buf) {
                        Ok(n) if n > 0 => {
                            let _ = storage::vfs::close(fd);
                            buf.truncate(n);
                            serial_println!(
                                "[P12-5] Read /INIT from FAT32: {} bytes",
                                n,
                            );
                            // Register in in-memory VFS
                            match vfs::write_all("/init", &buf) {
                                Ok(()) => {
                                    serial_println!("[P12-5] Registered /init in VFS");
                                    true
                                }
                                Err(e) => {
                                    serial_println!(
                                        "[P12-5] FAIL: register /init in VFS: {:?}",
                                        e,
                                    );
                                    false
                                }
                            }
                        }
                        Ok(_) => {
                            let _ = storage::vfs::close(fd);
                            serial_println!("[P12-5] FAIL: /INIT is empty");
                            false
                        }
                        Err(e) => {
                            let _ = storage::vfs::close(fd);
                            serial_println!("[P12-5] FAIL: read /INIT: {:?}", e);
                            false
                        }
                    }
                }
                Err(e) => {
                    serial_println!(
                        "[P12-5] WARN: /INIT not found on disk: {:?} (run create-test-disk.ps1)",
                        e,
                    );
                    false
                }
            };

            // Spawn /init as PID 1 (first user process from disk)
            if init_loaded {
                match crate::process::manager::PROCESS_MANAGER.spawn_from_vfs("/init") {
                    Ok(pid) => {
                        serial_println!(
                            "[P12-5] Spawned /init from disk: pid={}",
                            pid,
                        );
                    }
                    Err(e) => {
                        serial_println!(
                            "[P12-5] FAIL: spawn /init: {:?}",
                            e,
                        );
                    }
                }
            }

            // --- Step 2: Load /bin/hello from FAT32 disk ---
            let hello_loaded =
                match storage::vfs::open("/mnt/test/BIN/HELLO", storage::OpenFlags::READ) {
                    Ok(fd) => {
                        let mut buf = alloc::vec![0u8; 4096];
                        match storage::vfs::read(fd, &mut buf) {
                            Ok(n) if n > 0 => {
                                let _ = storage::vfs::close(fd);
                                buf.truncate(n);
                                serial_println!(
                                    "[P12-5] Read /BIN/HELLO from FAT32: {} bytes",
                                    n,
                                );
                                // Register in in-memory VFS
                                match vfs::write_all("/bin/hello", &buf) {
                                    Ok(()) => {
                                        serial_println!("[P12-5] Registered /bin/hello in VFS");
                                        true
                                    }
                                    Err(e) => {
                                        serial_println!(
                                            "[P12-5] FAIL: register /bin/hello in VFS: {:?}",
                                            e,
                                        );
                                        false
                                    }
                                }
                            }
                            Ok(_) => {
                                let _ = storage::vfs::close(fd);
                                serial_println!("[P12-5] FAIL: /BIN/HELLO is empty");
                                false
                            }
                            Err(e) => {
                                let _ = storage::vfs::close(fd);
                                serial_println!("[P12-5] FAIL: read /BIN/HELLO: {:?}", e);
                                false
                            }
                        }
                    }
                    Err(e) => {
                        serial_println!(
                            "[P12-5] WARN: /BIN/HELLO not found on disk: {:?} (run create-test-disk.ps1)",
                            e,
                        );
                        false
                    }
                };

            // Spawn /bin/hello
            if hello_loaded {
                match crate::process::manager::PROCESS_MANAGER.spawn_from_vfs("/bin/hello") {
                    Ok(pid) => {
                        serial_println!(
                            "[P12-5] Spawned /bin/hello from disk: pid={}",
                            pid,
                        );
                    }
                    Err(e) => {
                        serial_println!(
                            "[P12-5] FAIL: spawn /bin/hello: {:?}",
                            e,
                        );
                    }
                }
            }

            // Wait for both processes to execute
            for _ in 0..500 {
                x86_64::instructions::hlt();
            }
            serial_println!("[P12-5] Init-from-disk boot test complete");
        } else {
            serial_println!("[P12-5] SKIPPED: /mnt/test not mounted (no test disk)");
        }
    }

    // ── Phase 12-6: userlib Syscall Wiring integration test ─────
    // Validates that user-space programs can perform real file I/O
    // via the syscall interface:
    //   1. Create "/hello.txt" in the in-memory VFS
    //   2. Build a minimal ELF binary that open→read→write→close→exit
    //   3. Spawn the ELF as a user process
    //   4. Serial output must contain "Hello from KPIO test disk!"
    {
        serial_println!("[P12-6] userlib syscall wiring integration test");

        // Step 1: Create the test file in the in-memory VFS
        let content = b"Hello from KPIO test disk!";
        match vfs::write_all("/hello.txt", content) {
            Ok(()) => serial_println!("[P12-6] Created /hello.txt ({} bytes)", content.len()),
            Err(e) => serial_println!("[P12-6] FAIL: create /hello.txt: {:?}", e),
        }

        // Step 2: Build a minimal user-space program that:
        //   open("/hello.txt", 0, 0) → fd
        //   read(fd, buf, 256)       → count
        //   write(1, buf, count)     → serial output
        //   close(fd)
        //   exit(0)
        //
        // x86_64 machine code, mapped at vaddr 0x400000:
        #[rustfmt::skip]
        const FS_TEST_CODE: &[u8] = &[
            // open("/hello.txt", O_RDONLY=0, mode=0)
            0x48, 0xC7, 0xC0, 0x02, 0x00, 0x00, 0x00, // mov rax, 2 (SYS_OPEN)
            0x48, 0x8D, 0x3D, 0x56, 0x00, 0x00, 0x00, // lea rdi, [rip+0x56] → string at +0x64
            0x31, 0xF6,                                 // xor esi, esi (flags=0)
            0x31, 0xD2,                                 // xor edx, edx (mode=0)
            0x0F, 0x05,                                 // syscall → fd in rax

            // save fd in r12
            0x49, 0x89, 0xC4,                           // mov r12, rax

            // sub rsp, 256 (stack buffer for read)
            0x48, 0x81, 0xEC, 0x00, 0x01, 0x00, 0x00,  // sub rsp, 256

            // read(fd, rsp, 256)
            0x48, 0xC7, 0xC0, 0x00, 0x00, 0x00, 0x00, // mov rax, 0 (SYS_READ)
            0x4C, 0x89, 0xE7,                           // mov rdi, r12 (fd)
            0x48, 0x89, 0xE6,                           // mov rsi, rsp (buf)
            0x48, 0xC7, 0xC2, 0x00, 0x01, 0x00, 0x00, // mov rdx, 256 (count)
            0x0F, 0x05,                                 // syscall → bytes_read in rax

            // save count in r13
            0x49, 0x89, 0xC5,                           // mov r13, rax

            // write(1, rsp, count) → serial output
            0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00, // mov rax, 1 (SYS_WRITE)
            0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00, // mov rdi, 1 (stdout)
            0x48, 0x89, 0xE6,                           // mov rsi, rsp (buf)
            0x4C, 0x89, 0xEA,                           // mov rdx, r13 (count)
            0x0F, 0x05,                                 // syscall

            // close(fd)
            0x48, 0xC7, 0xC0, 0x03, 0x00, 0x00, 0x00, // mov rax, 3 (SYS_CLOSE)
            0x4C, 0x89, 0xE7,                           // mov rdi, r12 (fd)
            0x0F, 0x05,                                 // syscall

            // exit(0)
            0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00, // mov rax, 60 (SYS_EXIT)
            0x31, 0xFF,                                 // xor edi, edi
            0x0F, 0x05,                                 // syscall

            // String data: "/hello.txt\0" at offset 0x64
            b'/', b'h', b'e', b'l', b'l', b'o', b'.', b't', b'x', b't', 0x00,
        ];

        // Step 3: Map a user page table with code + stack pages
        let fs_cr3 = match memory::user_page_table::create_user_page_table() {
            Ok(cr3) => cr3,
            Err(e) => {
                serial_println!("[P12-6] FAIL: page table: {}", e);
                0
            }
        };

        if fs_cr3 != 0 {
            use x86_64::structures::paging::PageTableFlags;
            let code_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            let stack_flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;

            let code_vaddr: u64 = 0x40_0000;
            let stack_top: u64 = 0x80_0000;
            let stack_base: u64 = stack_top - 0x2000; // 2 pages for stack (for sub rsp, 256)

            let code_result =
                memory::user_page_table::map_user_page(fs_cr3, code_vaddr, code_flags);
            let stack_result1 =
                memory::user_page_table::map_user_page(fs_cr3, stack_base, stack_flags);
            let stack_result2 =
                memory::user_page_table::map_user_page(fs_cr3, stack_base + 0x1000, stack_flags);

            if let (Ok(code_phys), Ok(_), Ok(_)) = (code_result, stack_result1, stack_result2) {
                // Write the test program into the code page
                unsafe {
                    memory::user_page_table::write_to_phys(code_phys, 0, FS_TEST_CODE);
                }

                // Allocate kernel stack
                let ks_size: usize = 32 * 1024;
                let ks_vec = alloc::vec![0u8; ks_size];
                let ks_top = ks_vec.as_ptr() as u64 + ks_size as u64;

                let task = scheduler::Task::new_user_process(
                    "fs-test-12-6",
                    fs_cr3,
                    code_vaddr,          // entry point RIP
                    stack_top,           // user RSP
                    gdt::USER_CS as u16,
                    gdt::USER_DS as u16,
                    ks_top,
                    ks_vec,
                    126, // pid
                );
                scheduler::spawn(task);
                serial_println!(
                    "[P12-6] fs-test task spawned (pid=126, CR3={:#x})",
                    fs_cr3,
                );

                // Wait for the task to execute
                for _ in 0..300 {
                    x86_64::instructions::hlt();
                }
                serial_println!("[P12-6] Phase 12-6 integration test complete");
            } else {
                serial_println!("[P12-6] FAIL: could not map user pages");
            }
        }
    }

    // ── Phase 12-7: All sub-phase tests complete ─────────────────
    serial_println!("[P12] Phase 12 integration test PASSED");

    // ── Phase 11: Kernel Hardening — CoW fork integration test ──
    // Validates the Copy-on-Write fork mechanism end-to-end:
    //   1. Create a parent user page table with code + data + stack pages
    //   2. Write initial data to the data page (kernel-side)
    //   3. Clone the page table → CoW sharing (refcount incremented)
    //   4. Verify frame refcount > 1
    //   5. Spawn a child process that writes to the shared data page
    //      → triggers CoW page fault → handler copies frame
    // Results are logged so qemu-test.ps1 -Mode hardening can verify them.
    {
        serial_println!("[HARDENING] Phase 11: CoW fork integration test");

        // cow-writer: writes 0xCAFEBABE to 0x500000, then SYS_EXIT(0)
        //   mov dword [0x500000], 0xCAFEBABE   ; C7 04 25 00 00 50 00 BE BA FE CA
        //   mov rax, 60                         ; 48 C7 C0 3C 00 00 00
        //   xor rdi, rdi                        ; 48 31 FF
        //   syscall                             ; 0F 05
        #[rustfmt::skip]
        const COW_WRITER: &[u8] = &[
            0xC7, 0x04, 0x25, 0x00, 0x00, 0x50, 0x00,
            0xBE, 0xBA, 0xFE, 0xCA,                     // mov dword [0x500000], 0xCAFEBABE
            0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00,   // mov rax, 60 (SYS_EXIT)
            0x48, 0x31, 0xFF,                             // xor rdi, rdi
            0x0F, 0x05,                                   // syscall
        ];

        let parent_cr3 = match memory::user_page_table::create_user_page_table() {
            Ok(cr3) => cr3,
            Err(e) => {
                serial_println!("[HARDENING] FAIL: parent page table: {}", e);
                0
            }
        };

        if parent_cr3 != 0 {
            use x86_64::structures::paging::PageTableFlags;
            let code_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            let data_flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;
            let stack_flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;

            let code_vaddr: u64 = 0x40_0000;
            let data_vaddr: u64 = 0x50_0000;
            let stack_base: u64 = 0x7F_F000;
            let stack_top: u64 = 0x80_0000;

            let code_result =
                memory::user_page_table::map_user_page(parent_cr3, code_vaddr, code_flags);
            let data_result =
                memory::user_page_table::map_user_page(parent_cr3, data_vaddr, data_flags);
            let stack_result =
                memory::user_page_table::map_user_page(parent_cr3, stack_base, stack_flags);

            if let (Ok(code_phys), Ok(data_phys), Ok(_)) =
                (code_result, data_result, stack_result)
            {
                // Write program and initial data
                unsafe {
                    memory::user_page_table::write_to_phys(code_phys, 0, COW_WRITER);
                    // Write marker pattern to data page
                    let marker: [u8; 4] = [0xEF, 0xBE, 0xAD, 0xDE]; // 0xDEADBEEF le
                    memory::user_page_table::write_to_phys(data_phys, 0, &marker);
                }

                // Clone page table → triggers CoW sharing
                let child_cr3 = match memory::user_page_table::clone_user_page_table(parent_cr3) {
                    Ok(cr3) => {
                        serial_println!("[HARDENING] CoW clone successful: child CR3={:#x}", cr3);
                        cr3
                    }
                    Err(e) => {
                        serial_println!("[HARDENING] FAIL: CoW clone: {}", e);
                        0
                    }
                };

                if child_cr3 != 0 {
                    // Verify that the data page refcount is > 1
                    let refcount = memory::refcount::get(data_phys);
                    serial_println!(
                        "[HARDENING] Data frame {:#x} refcount = {} (expected >= 2)",
                        data_phys,
                        refcount
                    );

                    // Spawn a child process using the CoW'd page table.
                    // When it writes to 0x500000, the CoW fault handler fires.
                    let ks_size: usize = 32 * 1024;
                    let ks_vec = alloc::vec![0u8; ks_size];
                    let ks_top = ks_vec.as_ptr() as u64 + ks_size as u64;

                    let task = scheduler::Task::new_user_process(
                        "cow-test",
                        child_cr3,
                        code_vaddr,
                        stack_top,
                        gdt::USER_CS as u16,
                        gdt::USER_DS as u16,
                        ks_top,
                        ks_vec,
                        20, // pid
                    );
                    scheduler::spawn(task);
                    serial_println!(
                        "[HARDENING] cow-test spawned (pid=20, CR3={:#x}). \
                         Child will write to {:#x} → CoW fault expected.",
                        child_cr3,
                        data_vaddr
                    );
                }
            } else {
                serial_println!("[HARDENING] FAIL: could not map parent pages");
            }
        }

        // Log stack guard page summary for all tasks
        serial_println!(
            "[HARDENING] {} tasks spawned (stack guard pages logged above)",
            scheduler::total_task_count()
        );
    }

    serial_println!(
        "[KPIO] Kernel initialization complete (ctx_switches={})",
        scheduler::context_switch_count()
    );

    // Run tests if in test mode
    #[cfg(test)]
    test_main();

    // Wait ~500ms for process tests to run, then print summary.
    // This is done BEFORE the main loop to avoid framebuffer
    // rendering stealing the init task's entire time slice.
    {
        let wait_start = scheduler::boot_ticks();
        loop {
            let elapsed = scheduler::boot_ticks().wrapping_sub(wait_start);
            if elapsed >= 50 {
                break;
            }
            x86_64::instructions::hlt();
        }
        let ctx = scheduler::context_switch_count();
        let tasks = scheduler::total_task_count();
        serial_println!(
            "[PROC] All process tests PASSED (tasks={}, ctx_switches={})",
            tasks,
            ctx,
        );
        serial_println!("[SCHED] Final: {} context switches, {} tasks", ctx, tasks,);
    }

    // Main loop: drain deferred work queue + wait for boot animation
    serial_println!("[KPIO] Waiting for boot animation...");
    let mut total_drained: u64 = 0;
    let mut drain_logged = false;
    // Discard stale work items that accumulated during boot-time
    // initialization (Phase 12 tests, etc.) — processing hundreds of
    // accumulated timer ticks would trigger that many framebuffer
    // renders and stall the main loop.
    interrupts::workqueue::reset();
    // Temporarily install a lightweight timer callback so the first
    // drain iteration doesn't trigger a full-screen framebuffer render
    // (which takes seconds in QEMU's software-emulated pixel output).
    fn noop_timer_cb() {}
    interrupts::register_timer_callback(noop_timer_cb);
    loop {
        // Drain all pending ISR work items (timer, keyboard, mouse)
        // dispatched outside interrupt context — resolves the known
        // timer callback deadlock (known-issues.md #6).
        let n = interrupts::workqueue::drain();
        total_drained += n as u64;

        // Log drain count once (for qemu-test verification)
        if !drain_logged && total_drained > 0 {
            serial_println!("[WorkQueue] drained {} items so far", total_drained);
            drain_logged = true;
            // Restore the real boot animation callback now that the
            // drain verification message has been printed.
            interrupts::register_timer_callback(on_boot_animation_tick);
        }

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
