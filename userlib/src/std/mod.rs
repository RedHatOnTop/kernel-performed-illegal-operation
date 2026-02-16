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

pub mod env;
pub mod fs;
pub mod io;
pub mod net;
pub mod sync;
pub mod thread;
pub mod time;

/// Prelude for std compatibility
pub mod prelude {
    pub use super::fs::File;
    pub use super::io::{BufRead, Read, Write};
    pub use super::net::TcpStream;
}
