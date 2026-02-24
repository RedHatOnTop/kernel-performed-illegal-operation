//! VirtIO device drivers.
//!
//! VirtIO is a standardized interface for virtual I/O devices,
//! providing efficient communication between guest and host.
//!
//! # Architecture
//!
//! VirtIO devices communicate through virtqueues - ring buffers
//! shared between driver and device. Each queue consists of:
//! - Descriptor ring: Describes memory buffers
//! - Available ring: Driver tells device which descriptors are ready
//! - Used ring: Device tells driver which descriptors are consumed
//!
//! # References
//!
//! - VirtIO Specification 1.1+

pub mod block;
pub mod block_adapter;
pub mod queue;
