//! Hello World - KPIO OS Userspace Example
//!
//! This is the simplest possible userspace program that demonstrates
//! the basic system call interface.

#![no_std]
#![no_main]

use userlib::prelude::*;

/// Entry point for the userspace program.
///
/// This function is called by the CRT0 startup code after setting up
/// the stack and calling global constructors.
#[no_mangle]
pub extern "C" fn main(_argc: isize, _argv: *const *const u8) -> isize {
    // Print greeting
    println("Hello from KPIO OS userspace!");
    println("");

    // Get our process ID
    let pid = getpid();
    print("My PID is: ");
    print_number(pid);
    println("");

    // Test debug print (goes directly to serial)
    userlib::io::debug_print("[DEBUG] This message goes to serial\n");

    // Print some more info
    println("Successfully executed system calls:");
    println("  - write (for printing)");
    println("  - getpid (got our PID)");
    println("  - debug_print (serial output)");
    println("");
    println("Exiting with code 0...");

    // Exit successfully
    0
}

/// Simple function to print a number (no format! macro in no_std)
fn print_number(n: u64) {
    if n == 0 {
        print("0");
        return;
    }

    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut n = n;

    while n > 0 && i > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }

    if let Ok(s) = core::str::from_utf8(&buf[i..]) {
        print(s);
    }
}

/// Panic handler - required for no_std
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    userlib::io::eprint("PANIC: ");
    if let Some(location) = info.location() {
        userlib::io::eprint(location.file());
        userlib::io::eprint(":");
        // Can't easily print line number without format!
    }
    userlib::io::eprintln("");
    exit(-1);
}

/// CRT0 - Minimal C runtime startup
///
/// This is the actual entry point called by the kernel.
/// It sets up argc/argv and calls main().
#[no_mangle]
#[link_section = ".text.start"]
pub extern "C" fn _start() -> ! {
    // In a real implementation, argc and argv would be on the stack
    // set up by the kernel. For now, we just call main with dummy values.
    let result = main(0, core::ptr::null());
    exit(result as i32);
}
