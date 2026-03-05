//! Physical frame reference counting for Copy-on-Write support.
//!
//! Tracks how many page table entries reference each physical frame.
//! Frames with refcount > 1 are shared (CoW); writes to them trigger
//! a page fault that allocates a private copy.
//!
//! # Design
//!
//! - Default refcount for unmapped frames is implicitly 1 (not stored).
//! - Only frames with refcount >= 2 appear in the map.
//! - When `decrement()` returns 0, the caller is responsible for
//!   calling `free_frame()`.

use alloc::collections::BTreeMap;
use spin::Mutex;

/// Global frame reference counts.
///
/// Key: physical frame address (page-aligned).
/// Value: reference count (>= 2; frames with refcount 1 are not stored).
static FRAME_REFCOUNTS: Mutex<BTreeMap<u64, u32>> = Mutex::new(BTreeMap::new());

/// Increment the reference count for a physical frame.
///
/// If the frame is not yet tracked, its implicit refcount is 1,
/// so after this call it becomes 2.
pub fn increment(phys: u64) {
    let mut map = FRAME_REFCOUNTS.lock();
    let entry = map.entry(phys).or_insert(1);
    *entry += 1;
}

/// Decrement the reference count for a physical frame.
///
/// Returns the new reference count. When it returns 0 (or the frame
/// was not tracked, implying refcount was 1 → now 0), the caller
/// must free the frame.
pub fn decrement(phys: u64) -> u32 {
    let mut map = FRAME_REFCOUNTS.lock();
    if let Some(count) = map.get_mut(&phys) {
        *count -= 1;
        let new_count = *count;
        if new_count <= 1 {
            // No longer shared — remove from map to save memory.
            // A count of 1 means only one PTE references it (no longer CoW).
            map.remove(&phys);
        }
        new_count
    } else {
        // Not tracked → implicit refcount was 1, now 0.
        0
    }
}

/// Get the current reference count for a physical frame.
///
/// Returns 1 for untracked frames (the implicit default).
pub fn get(phys: u64) -> u32 {
    FRAME_REFCOUNTS.lock().get(&phys).copied().unwrap_or(1)
}

/// Total number of shared frames currently tracked.
///
/// Useful for diagnostics and serial log output.
pub fn shared_frame_count() -> usize {
    FRAME_REFCOUNTS.lock().len()
}
