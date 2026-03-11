//! Synchronization primitives.
//!
//! Provides futex (fast userspace mutex) and epoll (event multiplexing) support.

pub mod epoll;
pub mod futex;
