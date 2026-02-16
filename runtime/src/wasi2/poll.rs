// WASI Preview 2 — Poll (wasi:io/poll)
//
// This module implements the `wasi:io/poll` interface, which provides
// a way to wait for I/O readiness. In our single-threaded kernel
// environment, all pollables are immediately ready (no real async I/O).

extern crate alloc;

use alloc::vec::Vec;
use super::{ResourceTable, ResourceHandle, ResourceType, ResourceData, ResourceError};

// ---------------------------------------------------------------------------
// Pollable State
// ---------------------------------------------------------------------------

/// The internal state of a `Pollable` resource.
#[derive(Debug, Clone)]
pub enum PollableState {
    /// Always ready immediately (used for memory streams, etc.).
    Immediate,
    /// Ready when a specific stream has data available.
    /// Stores the handle of the associated stream resource.
    StreamReady(ResourceHandle),
    /// Ready after a specific point in time (for timers).
    /// Stores a deadline in nanoseconds (monotonic clock).
    Timer(u64),
}

/// Check whether a single pollable is ready.
pub fn pollable_ready(state: &PollableState, _current_time_ns: u64) -> bool {
    match state {
        PollableState::Immediate => true,
        PollableState::StreamReady(_) => {
            // In our implementation, all streams are always ready
            // (no real async I/O in the kernel).
            true
        }
        PollableState::Timer(deadline) => {
            // Timer is ready when current time >= deadline
            _current_time_ns >= *deadline
        }
    }
}

/// Block on a single pollable until it is ready.
///
/// In our single-threaded kernel, this is a no-op since all pollables
/// are effectively ready immediately.
pub fn pollable_block(state: &PollableState) {
    match state {
        PollableState::Immediate => {}
        PollableState::StreamReady(_) => {}
        PollableState::Timer(_) => {
            // In a real implementation, we would spin-wait here.
            // For now, no-op.
        }
    }
}

// ---------------------------------------------------------------------------
// Poll — poll a list of pollables
// ---------------------------------------------------------------------------

/// Poll a list of pollable handles and return the indices of those that
/// are ready.
///
/// Corresponds to `wasi:io/poll.poll(list<borrow<pollable>>) → list<u32>`.
pub fn poll_list(
    table: &ResourceTable,
    handles: &[ResourceHandle],
    current_time_ns: u64,
) -> Result<Vec<u32>, ResourceError> {
    let mut ready_indices = Vec::new();

    for (i, &handle) in handles.iter().enumerate() {
        let data = table.get(handle, ResourceType::Pollable)?;
        if let ResourceData::Pollable(state) = data {
            if pollable_ready(state, current_time_ns) {
                ready_indices.push(i as u32);
            }
        } else {
            return Err(ResourceError::TypeMismatch {
                expected: ResourceType::Pollable,
                actual: ResourceType::InputStream, // placeholder
            });
        }
    }

    // WASI P2 spec: if no pollables are ready, block until at least one is.
    // In our implementation, at least one is always ready unless the list is empty.
    if ready_indices.is_empty() && !handles.is_empty() {
        // Fallback: return all as ready (since we can't truly block).
        for i in 0..handles.len() {
            ready_indices.push(i as u32);
        }
    }

    Ok(ready_indices)
}

/// Create a pollable that is immediately ready.
pub fn create_immediate_pollable(
    table: &mut ResourceTable,
) -> Result<ResourceHandle, ResourceError> {
    table.push(
        ResourceType::Pollable,
        ResourceData::Pollable(PollableState::Immediate),
    )
}

/// Create a pollable associated with a stream resource.
pub fn create_stream_pollable(
    table: &mut ResourceTable,
    stream_handle: ResourceHandle,
) -> Result<ResourceHandle, ResourceError> {
    table.push(
        ResourceType::Pollable,
        ResourceData::Pollable(PollableState::StreamReady(stream_handle)),
    )
}

/// Create a timer pollable that becomes ready at the given deadline.
pub fn create_timer_pollable(
    table: &mut ResourceTable,
    deadline_ns: u64,
) -> Result<ResourceHandle, ResourceError> {
    table.push(
        ResourceType::Pollable,
        ResourceData::Pollable(PollableState::Timer(deadline_ns)),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immediate_pollable_is_ready() {
        let state = PollableState::Immediate;
        assert!(pollable_ready(&state, 0));
    }

    #[test]
    fn stream_pollable_is_ready() {
        let handle = ResourceHandle::from_u32(0);
        let state = PollableState::StreamReady(handle);
        assert!(pollable_ready(&state, 0));
    }

    #[test]
    fn timer_pollable_before_deadline() {
        let state = PollableState::Timer(1000);
        assert!(!pollable_ready(&state, 500));
    }

    #[test]
    fn timer_pollable_at_deadline() {
        let state = PollableState::Timer(1000);
        assert!(pollable_ready(&state, 1000));
    }

    #[test]
    fn timer_pollable_after_deadline() {
        let state = PollableState::Timer(1000);
        assert!(pollable_ready(&state, 2000));
    }

    #[test]
    fn poll_list_all_ready() {
        let mut table = ResourceTable::new();
        let h1 = create_immediate_pollable(&mut table).unwrap();
        let h2 = create_immediate_pollable(&mut table).unwrap();
        let h3 = create_immediate_pollable(&mut table).unwrap();

        let ready = poll_list(&table, &[h1, h2, h3], 0).unwrap();
        assert_eq!(ready, alloc::vec![0, 1, 2]);
    }

    #[test]
    fn poll_list_partial_ready() {
        let mut table = ResourceTable::new();
        let h1 = create_immediate_pollable(&mut table).unwrap();
        let h2 = create_timer_pollable(&mut table, 1000).unwrap();
        let h3 = create_immediate_pollable(&mut table).unwrap();

        // At time 500, timer is not ready
        let ready = poll_list(&table, &[h1, h2, h3], 500).unwrap();
        assert_eq!(ready, alloc::vec![0, 2]);
    }

    #[test]
    fn poll_list_empty() {
        let table = ResourceTable::new();
        let ready = poll_list(&table, &[], 0).unwrap();
        assert!(ready.is_empty());
    }

    #[test]
    fn create_stream_pollable_works() {
        let mut table = ResourceTable::new();
        let stream_h = table
            .push(
                ResourceType::InputStream,
                ResourceData::InputStream(
                    crate::wasi2::streams::InputStreamData::Memory(
                        crate::wasi2::streams::MemoryInputStream::new(alloc::vec![1]),
                    ),
                ),
            )
            .unwrap();
        let poll_h = create_stream_pollable(&mut table, stream_h).unwrap();
        assert_eq!(table.resource_type(poll_h).unwrap(), ResourceType::Pollable);
    }

    #[test]
    fn poll_invalid_handle() {
        let table = ResourceTable::new();
        let fake = ResourceHandle::from_u32(99);
        let result = poll_list(&table, &[fake], 0);
        assert!(result.is_err());
    }
}
