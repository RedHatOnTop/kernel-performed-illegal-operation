//! Syscall Fuzzing Target
//!
//! Run with: cargo fuzz run syscall_fuzz

#![no_main]

use libfuzzer_sys::fuzz_target;

// Import the fuzzing harness
// use kpio_fuzz::syscall_fuzz;

fuzz_target!(|data: &[u8]| {
    // Skip if too short
    if data.len() < 4 {
        return;
    }

    // Extract syscall number and arguments
    let nr = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    
    // Skip invalid syscall numbers
    if nr > 512 {
        return;
    }

    // Extract up to 6 arguments (8 bytes each)
    let mut args = [0u64; 6];
    let mut offset = 4;
    
    for arg in &mut args {
        if offset + 8 <= data.len() {
            *arg = u64::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
            ]);
            offset += 8;
        }
    }

    // In actual fuzzing, this would dispatch to kernel syscall handler
    // kernel::syscall::dispatch(nr, args);
    
    // For now, simulate validation
    match nr {
        // SYS_read - validate fd, buffer, size
        0 => {
            let _fd = args[0] as i32;
            let _buf = args[1];
            let _size = args[2] as usize;
            // Should validate fd, check buf is user space, clamp size
        }
        
        // SYS_write
        1 => {
            let _fd = args[0] as i32;
            let _buf = args[1];
            let _size = args[2] as usize;
        }
        
        // SYS_open
        2 => {
            let _path = args[0];
            let _flags = args[1] as i32;
            let _mode = args[2] as u32;
        }
        
        // SYS_mmap
        9 => {
            let _addr = args[0];
            let _len = args[1] as usize;
            let _prot = args[2] as i32;
            let _flags = args[3] as i32;
            // Check alignment, size limits, permission combinations
        }
        
        _ => {
            // Unknown syscall - should return ENOSYS
        }
    }
});
