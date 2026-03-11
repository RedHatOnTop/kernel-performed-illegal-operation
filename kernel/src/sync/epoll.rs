//! Epoll event multiplexing.
//!
//! Implements a minimal `epoll` subsystem for kernel-internal and Ring 3
//! use.  Each `EpollInstance` holds an interest list of file descriptors
//! and can be polled for readiness events via [`epoll_wait`].
//!
//! Only **level-triggered** semantics are supported (no `EPOLLET`).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

// ── Linux epoll constants ────────────────────────────────────────────

/// Readable (same as `POLLIN`).
pub const EPOLLIN: u32 = 0x001;
/// Writable (same as `POLLOUT`).
pub const EPOLLOUT: u32 = 0x004;
/// Error condition.
pub const EPOLLERR: u32 = 0x008;
/// Hang-up.
pub const EPOLLHUP: u32 = 0x010;

/// `epoll_ctl` operation: add an FD to the interest list.
pub const EPOLL_CTL_ADD: i32 = 1;
/// `epoll_ctl` operation: modify an existing interest entry.
pub const EPOLL_CTL_MOD: i32 = 2;
/// `epoll_ctl` operation: remove an FD from the interest list.
pub const EPOLL_CTL_DEL: i32 = 3;

// ── Types ────────────────────────────────────────────────────────────

/// An entry in the epoll interest list.
#[derive(Debug, Clone, Copy)]
pub struct EpollEntry {
    /// File descriptor being watched.
    pub fd: i32,
    /// Bitmask of requested events (`EPOLLIN | EPOLLOUT | …`).
    pub events: u32,
    /// User-supplied opaque data (the `epoll_data_t` union, stored as `u64`).
    pub data: u64,
}

/// A single readiness event returned by `epoll_wait`.
///
/// Binary-compatible with the Linux `struct epoll_event` layout:
/// `u32 events` followed by `u64 data`, packed to 12 bytes.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct EpollEvent {
    pub events: u32,
    pub data: u64,
}

/// A kernel epoll instance.
pub struct EpollInstance {
    /// Interest list: fd → entry.
    interest: BTreeMap<i32, EpollEntry>,
}

impl EpollInstance {
    fn new() -> Self {
        Self {
            interest: BTreeMap::new(),
        }
    }
}

// ── Global table ─────────────────────────────────────────────────────

/// Monotonically increasing ID generator for epoll instances.
static NEXT_EPOLL_ID: AtomicU64 = AtomicU64::new(1);

/// Global table mapping `epoll_id → EpollInstance`.
///
/// Process isolation is enforced by the FD table; an `epoll_id` is only
/// reachable through the owning process's FD table.
static EPOLL_TABLE: Mutex<Option<BTreeMap<u64, EpollInstance>>> = Mutex::new(None);

fn with_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut BTreeMap<u64, EpollInstance>) -> R,
{
    let mut guard = EPOLL_TABLE.lock();
    if guard.is_none() {
        *guard = Some(BTreeMap::new());
    }
    f(guard.as_mut().expect("epoll table init"))
}

// ── Public API ───────────────────────────────────────────────────────

/// Create a new epoll instance and return its `epoll_id`.
pub fn epoll_create() -> u64 {
    let id = NEXT_EPOLL_ID.fetch_add(1, Ordering::Relaxed);
    with_table(|t| {
        t.insert(id, EpollInstance::new());
    });
    id
}

/// Add, modify, or remove an FD from an epoll interest list.
///
/// Returns `Ok(())` on success, or a negative errno on failure.
pub fn epoll_ctl(epoll_id: u64, op: i32, fd: i32, events: u32, data: u64) -> Result<(), i64> {
    with_table(|t| {
        let inst = t.get_mut(&epoll_id).ok_or(-9i64)?; // -EBADF
        match op {
            EPOLL_CTL_ADD => {
                if inst.interest.contains_key(&fd) {
                    return Err(-17); // -EEXIST
                }
                inst.interest.insert(fd, EpollEntry { fd, events, data });
                Ok(())
            }
            EPOLL_CTL_MOD => {
                let entry = inst.interest.get_mut(&fd).ok_or(-2i64)?; // -ENOENT
                entry.events = events;
                entry.data = data;
                Ok(())
            }
            EPOLL_CTL_DEL => {
                inst.interest.remove(&fd).ok_or(-2i64)?; // -ENOENT
                Ok(())
            }
            _ => Err(-22), // -EINVAL
        }
    })
}

/// Poll for ready events across all FDs in the interest list.
///
/// `poll_fn` is called for each FD in the interest list; it must return
/// the current readiness flags for that FD (using `EPOLLIN`/`EPOLLOUT`
/// constants).  This callback allows the caller to query different FD
/// types (sockets, pipes, etc.) without the epoll module knowing about
/// their internals.
///
/// Returns a `Vec` of at most `max_events` ready events.
pub fn epoll_wait<F>(epoll_id: u64, max_events: usize, poll_fn: F) -> Result<Vec<EpollEvent>, i64>
where
    F: Fn(i32) -> u32,
{
    with_table(|t| {
        let inst = t.get(&epoll_id).ok_or(-9i64)?; // -EBADF
        let mut out = Vec::new();
        for entry in inst.interest.values() {
            if out.len() >= max_events {
                break;
            }
            let ready = poll_fn(entry.fd);
            let matched = ready & entry.events;
            if matched != 0 {
                out.push(EpollEvent {
                    events: matched,
                    data: entry.data,
                });
            }
        }
        Ok(out)
    })
}

/// Destroy an epoll instance (called when its FD is closed).
pub fn epoll_destroy(epoll_id: u64) {
    with_table(|t| {
        t.remove(&epoll_id);
    });
}
