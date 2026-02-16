//! Browser Services Interface
//!
//! This module defines the IPC protocols and service interfaces
//! for communication between the kernel and the Servo browser engine.
//!
//! # Architecture
//!
//! The browser runs in userspace as a Servo process. It communicates
//! with the kernel through IPC channels for:
//!
//! - GPU memory allocation and command submission
//! - Network stack access
//! - Input event delivery
//! - Window management
//!
//! # Service Model
//!
//! Each tab/renderer process has dedicated channels:
//!
//! ```text
//! ┌──────────────────┐     ┌──────────────────┐
//! │   Servo Tab 1    │     │   Servo Tab 2    │
//! │  (Userspace)     │     │  (Userspace)     │
//! └────────┬─────────┘     └────────┬─────────┘
//!          │                        │
//!          │ IPC Channels           │ IPC Channels
//!          │                        │
//! ┌────────┴────────────────────────┴─────────┐
//! │              Browser Coordinator           │
//! │                 (Kernel)                   │
//! └──────────────────┬────────────────────────┘
//!                    │
//!          ┌─────────┴─────────┐
//!          ▼                   ▼
//!     GPU Driver          Network Driver
//! ```

pub mod compositor;
pub mod coordinator;
pub mod gpu;
pub mod input;
pub mod integration;
pub mod memory;
pub mod network;
pub mod origin;
pub mod protocol;
pub mod pwa_bridge;

pub use coordinator::{BrowserCoordinator, TabId, TabInfo, TabState};
pub use memory::{MemoryPressure, MemoryStats, TabManager, TabMemoryStats, TabProcess};
pub use origin::{CoepPolicy, CoopPolicy, CorbResult, Origin, SiteId};
pub use protocol::{BrowserMessage, BrowserRequest, BrowserResponse};
