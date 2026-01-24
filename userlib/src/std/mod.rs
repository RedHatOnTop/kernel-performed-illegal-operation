//! KPIO std compatibility layer
//!
//! This module provides std-compatible APIs that work with KPIO syscalls.
//! Programs using `std::net`, `std::fs`, etc. can be compiled against
//! this shim layer to run on KPIO.
//!
//! # Usage
//!
//! Instead of `use std::net::TcpStream`, use:
//! ```rust
//! use userlib::std::net::TcpStream;
//! ```

pub mod net;
pub mod fs;
pub mod io;
pub mod time;
pub mod sync;
pub mod thread;
pub mod env;

/// Prelude for std compatibility
pub mod prelude {
    pub use super::io::{Read, Write, BufRead};
    pub use super::fs::File;
    pub use super::net::TcpStream;
}
