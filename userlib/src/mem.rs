//! Memory management for userspace.
//!
//! This module provides memory mapping and allocation functions.

use crate::syscall::{syscall1, syscall2, syscall4, SyscallError, SyscallNumber, SyscallResult};

/// Memory protection flags.
pub mod prot {
    /// Pages may not be accessed.
    pub const PROT_NONE: u32 = 0;
    /// Pages may be read.
    pub const PROT_READ: u32 = 1;
    /// Pages may be written.
    pub const PROT_WRITE: u32 = 2;
    /// Pages may be executed.
    pub const PROT_EXEC: u32 = 4;
}

/// Memory mapping flags.
pub mod map {
    /// Share changes.
    pub const MAP_SHARED: u32 = 0x01;
    /// Changes are private.
    pub const MAP_PRIVATE: u32 = 0x02;
    /// Interpret addr exactly.
    pub const MAP_FIXED: u32 = 0x10;
    /// Don't use a file.
    pub const MAP_ANONYMOUS: u32 = 0x20;
}

/// Map memory pages.
///
/// # Arguments
///
/// * `addr` - Suggested address (0 for kernel to choose)
/// * `length` - Number of bytes to map
/// * `prot` - Protection flags (PROT_READ, PROT_WRITE, PROT_EXEC)
/// * `flags` - Mapping flags (MAP_PRIVATE, MAP_ANONYMOUS, etc.)
///
/// # Returns
///
/// Address of the mapped region.
pub fn mmap(addr: u64, length: usize, prot: u32, flags: u32) -> SyscallResult {
    unsafe {
        syscall4(
            SyscallNumber::Mmap,
            addr,
            length as u64,
            prot as u64,
            flags as u64,
        )
    }
}

/// Unmap memory pages.
pub fn munmap(addr: u64, length: usize) -> SyscallResult {
    unsafe { syscall2(SyscallNumber::Munmap, addr, length as u64) }
}

/// Set program break (heap boundary).
///
/// Returns the new program break on success.
pub fn brk(addr: u64) -> SyscallResult {
    unsafe { syscall1(SyscallNumber::Brk, addr) }
}

/// Get current program break.
pub fn sbrk(increment: isize) -> Result<u64, SyscallError> {
    // Get current brk
    let current = brk(0)?;

    if increment == 0 {
        return Ok(current);
    }

    // Calculate new brk
    let new_brk = if increment > 0 {
        current.wrapping_add(increment as u64)
    } else {
        current.wrapping_sub((-increment) as u64)
    };

    // Set new brk
    brk(new_brk)?;

    Ok(current)
}

/// Allocate anonymous memory.
///
/// This is a convenience wrapper around mmap for anonymous mappings.
pub fn alloc_pages(num_pages: usize) -> Result<*mut u8, SyscallError> {
    const PAGE_SIZE: usize = 4096;

    let length = num_pages * PAGE_SIZE;
    let flags = map::MAP_PRIVATE | map::MAP_ANONYMOUS;
    let prot = prot::PROT_READ | prot::PROT_WRITE;

    let addr = mmap(0, length, prot, flags)?;
    Ok(addr as *mut u8)
}

/// Free allocated pages.
pub fn free_pages(addr: *mut u8, num_pages: usize) -> Result<(), SyscallError> {
    const PAGE_SIZE: usize = 4096;

    munmap(addr as u64, num_pages * PAGE_SIZE)?;
    Ok(())
}

/// Create shared memory region.
///
/// Returns a handle that can be shared with other processes.
pub fn shm_create(size: usize, flags: u32) -> SyscallResult {
    unsafe { syscall2(SyscallNumber::ShmCreate, size as u64, flags as u64) }
}

/// Map shared memory into address space.
pub fn shm_map(shm_id: u64, addr_hint: u64, prot: u32) -> SyscallResult {
    unsafe { crate::syscall::syscall3(SyscallNumber::ShmMap, shm_id, addr_hint, prot as u64) }
}

/// Unmap shared memory.
pub fn shm_unmap(addr: u64, size: usize) -> SyscallResult {
    unsafe { syscall2(SyscallNumber::ShmUnmap, addr, size as u64) }
}
