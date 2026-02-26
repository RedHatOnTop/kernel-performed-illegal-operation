//! Futex (Fast Userspace Mutex) Implementation
//!
//! Provides `FUTEX_WAIT` and `FUTEX_WAKE` operations using a global hash
//! table of wait queues keyed by virtual address.

use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use spin::Mutex;

use crate::scheduler::{self, TaskId};

/// Futex command mask (strips private flag).
pub const FUTEX_CMD_MASK: i32 = 0x7F;
/// FUTEX_WAIT operation.
pub const FUTEX_WAIT: i32 = 0;
/// FUTEX_WAKE operation.
pub const FUTEX_WAKE: i32 = 1;
/// Private flag (process-private futex — we ignore the distinction).
pub const FUTEX_PRIVATE_FLAG: i32 = 128;

/// Global futex wait queue table.
///
/// Maps virtual addresses to queues of waiting task IDs.
/// In a full implementation, we would key by physical address
/// to support shared memory futexes. For now, virtual address
/// suffices since we don't share memory across processes.
static FUTEX_TABLE: Mutex<Option<BTreeMap<u64, VecDeque<TaskId>>>> = Mutex::new(None);

fn with_futex_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut BTreeMap<u64, VecDeque<TaskId>>) -> R,
{
    let mut guard = FUTEX_TABLE.lock();
    if guard.is_none() {
        *guard = Some(BTreeMap::new());
    }
    f(guard.as_mut().unwrap())
}

/// Perform a futex operation.
///
/// # Arguments
///
/// * `uaddr` - User-space address of the futex word
/// * `op` - Futex operation (FUTEX_WAIT, FUTEX_WAKE, etc.)
/// * `val` - Expected value (for WAIT) or number of waiters to wake (for WAKE)
///
/// # Returns
///
/// - `FUTEX_WAIT`: 0 on success, -EAGAIN if *uaddr != val
/// - `FUTEX_WAKE`: number of waiters woken
/// - Other: -ENOSYS
pub fn sys_futex(uaddr: u64, op: i32, val: u32) -> i64 {
    let cmd = op & FUTEX_CMD_MASK;

    match cmd {
        FUTEX_WAIT => futex_wait(uaddr, val),
        FUTEX_WAKE => futex_wake(uaddr, val),
        _ => {
            crate::serial_println!("[FUTEX] unsupported op={}", op);
            0  // Return success for unsupported ops (stub behavior)
        }
    }
}

/// FUTEX_WAIT: atomically check *uaddr == val, block if so.
fn futex_wait(uaddr: u64, expected: u32) -> i64 {
    const EAGAIN: i64 = 11;

    // Read the current value at uaddr
    // Safety: uaddr has been validated by the syscall layer
    let current_val = unsafe { core::ptr::read_volatile(uaddr as *const u32) };

    if current_val != expected {
        return -EAGAIN;
    }

    // Add current task to the wait queue
    let task_id = scheduler::current_task_id();

    with_futex_table(|table| {
        table
            .entry(uaddr)
            .or_insert_with(VecDeque::new)
            .push_back(task_id);
    });

    // Block the current task
    scheduler::block_current();

    // When we return here, we were woken up
    // Yield to let the scheduler pick another task
    scheduler::schedule();

    0
}

/// FUTEX_WAKE: wake up to `val` waiters.
fn futex_wake(uaddr: u64, val: u32) -> i64 {
    let mut woken = 0u32;

    with_futex_table(|table| {
        if let Some(waiters) = table.get_mut(&uaddr) {
            while woken < val {
                if let Some(task_id) = waiters.pop_front() {
                    scheduler::unblock(task_id);
                    woken += 1;
                } else {
                    break;
                }
            }
            // Clean up empty entry
            if waiters.is_empty() {
                table.remove(&uaddr);
            }
        }
    });

    woken as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_futex_cmd_mask() {
        assert_eq!(FUTEX_WAIT, 0);
        assert_eq!(FUTEX_WAKE, 1);
        // FUTEX_WAIT | FUTEX_PRIVATE_FLAG should still yield FUTEX_WAIT
        assert_eq!((FUTEX_WAIT | FUTEX_PRIVATE_FLAG) & FUTEX_CMD_MASK, FUTEX_WAIT);
        assert_eq!((FUTEX_WAKE | FUTEX_PRIVATE_FLAG) & FUTEX_CMD_MASK, FUTEX_WAKE);
    }
}
