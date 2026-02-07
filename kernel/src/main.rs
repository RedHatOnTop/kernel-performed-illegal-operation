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
mod driver;
mod gdt;
mod graphics;
mod gui;
mod interrupts;
mod memory;
mod panic;
mod scheduler;
mod serial;
mod net;
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
        serial_println!("[KPIO] Framebuffer available: {}x{}", 
            fb_info.info().width, fb_info.info().height);
        
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

    // Phase 6: Scheduler initialization
    serial_println!("[KPIO] Initializing scheduler...");
    scheduler::init();
    serial_println!("[KPIO] Scheduler initialized");

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

    // Phase 6.7: Network stack
    serial_println!("[KPIO] Initializing network stack...");
    net::init();
    serial_println!("[KPIO] Network stack ready (loopback + DNS + HTTP)");

    // Phase 7: APIC initialization (Phase 1 feature)
    serial_println!("[KPIO] Initializing APIC...");
    unsafe { interrupts::init_apic(phys_mem_offset) };
    serial_println!("[KPIO] APIC initialized");

    // Phase 8: PCI enumeration
    serial_println!("[KPIO] Enumerating PCI bus...");
    driver::pci::enumerate();
    
    // Phase 9: VirtIO device initialization
    serial_println!("[KPIO] Initializing VirtIO devices...");
    driver::virtio::block::init();
    serial_println!("[KPIO] VirtIO initialized ({} block device(s))", 
        driver::virtio::block::device_count());
    
    // Phase 9.5: PS/2 Mouse initialization
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
            let mut renderer = gui::render::Renderer::new(fb_ptr, fb_width, fb_height, fb_bpp, fb_stride);
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

    serial_println!("[KPIO] Kernel initialization complete");

    // Run tests if in test mode
    #[cfg(test)]
    test_main();

    // Main loop: wait for boot animation to complete, then initialize GUI
    serial_println!("[KPIO] Waiting for boot animation...");
    loop {
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
                let browser = gui::window::Window::new_browser(
                    gui::window::WindowId(1),
                    100, 50
                );
                gui.windows.push(browser);
                gui.taskbar.add_window(gui::window::WindowId(1), "KPIO Browser");
                
                // Create terminal window
                let terminal = gui::window::Window::new_terminal(
                    gui::window::WindowId(2),
                    250, 150
                );
                gui.windows.push(terminal);
                gui.taskbar.add_window(gui::window::WindowId(2), "Terminal");
                
                // Create files window
                let files = gui::window::Window::new_files(
                    gui::window::WindowId(3),
                    400, 100
                );
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
fn on_key_event(ch: char, _scancode: u8, pressed: bool) {
    if ch != '\0' {
        gui::input::push_key_event(ch, _scancode, pressed);
    }
}

/// Mouse byte callback
fn on_mouse_byte(byte: u8) {
    gui::input::process_mouse_byte(byte);
}

/// Timer tick callback
fn on_timer_tick() {
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

    serial_println!("[FB] Drawing boot screen ({}x{}, {} bpp, stride {})", 
        width, height, bytes_per_pixel, stride);

    // Fill background with dark blue
    let bg_color: [u8; 4] = [0x40, 0x20, 0x10, 0xFF]; // BGR format for most displays
    
    for y in 0..height {
        for x in 0..width {
            let offset = (y * stride + x) * bytes_per_pixel;
            if offset + bytes_per_pixel <= buffer.len() {
                buffer[offset] = bg_color[0];     // Blue
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
        0b10000010,
        0b10000100,
        0b10001000,
        0b10010000,
        0b10100000,
        0b11010000,
        0b10001000,
        0b10000100,
    ];
    draw_char(buffer, text_x, text_y, &k_pattern, &black, bytes_per_pixel, stride);

    // Simple P letter (8x8)
    let p_pattern: [u8; 8] = [
        0b11111100,
        0b10000010,
        0b10000010,
        0b11111100,
        0b10000000,
        0b10000000,
        0b10000000,
        0b10000000,
    ];
    draw_char(buffer, text_x + 40, text_y, &p_pattern, &black, bytes_per_pixel, stride);

    // Simple I letter (8x8)
    let i_pattern: [u8; 8] = [
        0b11111110,
        0b00010000,
        0b00010000,
        0b00010000,
        0b00010000,
        0b00010000,
        0b00010000,
        0b11111110,
    ];
    draw_char(buffer, text_x + 80, text_y, &i_pattern, &black, bytes_per_pixel, stride);

    // Simple O letter (8x8)
    let o_pattern: [u8; 8] = [
        0b00111100,
        0b01000010,
        0b10000001,
        0b10000001,
        0b10000001,
        0b10000001,
        0b01000010,
        0b00111100,
    ];
    draw_char(buffer, text_x + 120, text_y, &o_pattern, &black, bytes_per_pixel, stride);

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
    stride: usize
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

// 매크로는 serial.rs에서 정의됨
