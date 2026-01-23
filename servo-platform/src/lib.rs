//! KPIO Platform Abstraction Layer for Servo
//!
//! This crate provides the platform-specific implementations that Servo
//! needs to run on KPIO OS. It replaces standard library functionality
//! with KPIO syscalls and IPC mechanisms.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │              Servo Engine               │
//! ├─────────────────────────────────────────┤
//! │         kpio_platform (this crate)      │
//! │  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐      │
//! │  │ net │ │ gpu │ │ fs  │ │thread│      │
//! │  └──┬──┘ └──┬──┘ └──┬──┘ └──┬──┘      │
//! └─────┼───────┼───────┼───────┼──────────┘
//!       │       │       │       │
//!       └───────┴───────┴───────┘
//!                   │
//!            KPIO Syscalls
//! ```
//!
//! # Modules
//!
//! - `net`: Network abstraction (TCP, UDP, DNS)
//! - `gpu`: GPU/rendering abstraction
//! - `fs`: File system abstraction
//! - `thread`: Threading and synchronization
//! - `time`: Time and timers
//! - `window`: Window management and input
//! - `ipc`: Inter-process communication helpers

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod net;
pub mod gpu;
pub mod fs;
pub mod thread;
pub mod time;
pub mod window;
pub mod ipc;
pub mod error;

// Re-exports for convenience
pub use error::{PlatformError, Result};
pub use net::{TcpStream, TcpListener, SocketAddr, IpAddr};
pub use gpu::{Device as GpuDevice, BufferHandle, TextureHandle};
pub use fs::{File, OpenOptions};
pub use thread::{Mutex, RwLock, spawn as spawn_thread};
pub use time::{Instant, SystemTime};
pub use window::{Window, WindowBuilder, EventLoop, Event};
pub use ipc::{ServiceChannel, SharedMemory};

/// Platform initialization
pub fn init() {
    // Initialize platform subsystems
    log::info!("[KPIO Platform] Initializing...");
    
    // Initialize IPC first (other subsystems depend on it)
    ipc::init();
    
    // These will connect to kernel services via IPC
    net::init();
    gpu::init();
    fs::init();
    thread::init();
    time::init();
    window::init();
    
    log::info!("[KPIO Platform] Initialized");
}

/// Check if platform is ready
pub fn is_initialized() -> bool {
    // TODO: Check all subsystems
    true
}

/// Platform version
pub const VERSION: &str = "0.1.0";

/// Platform identifier
pub const PLATFORM_ID: &str = "kpio-servo";
