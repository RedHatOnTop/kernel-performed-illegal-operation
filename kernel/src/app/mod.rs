//! App Management Module
//!
//! This module provides the kernel-level application management subsystem:
//!
//! - **Registry** — catalog of installed apps (persisted to VFS).
//! - **Lifecycle** — launch / suspend / resume / terminate / crash-restart.
//! - **Permissions** — capability-based access control per app.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────┐
//! │              App Module (kernel/src/app/)     │
//! │  ┌──────────┐  ┌───────────┐  ┌───────────┐ │
//! │  │ Registry │  │ Lifecycle │  │Permissions│ │
//! │  └──────────┘  └───────────┘  └───────────┘ │
//! │       ↕ VFS          ↕ Process       ↕ VFS   │
//! └──────────────────────────────────────────────┘
//! ```
//!
//! Other kernel subsystems (GUI, syscall, browser bridge) interact
//! with apps through the public API exported here.

pub mod error;
pub mod lifecycle;
pub mod permissions;
pub mod registry;
pub mod window_state;

// Re-export the most commonly used types.
pub use error::AppError;
pub use lifecycle::{AppInstanceId, AppInstanceInfo, AppLifecycle, AppRunState, APP_LIFECYCLE};
pub use permissions::{AppPermissions, FsScope, NetScope, PermissionChecker};
pub use registry::{
    AppRegistry, KernelAppDescriptor, KernelAppId, KernelAppType, APP_REGISTRY,
};

/// Initialise the app subsystem.
///
/// Call this during kernel boot after the VFS is ready.
pub fn init() {
    // Load the app registry from VFS (if a prior session saved one).
    {
        let mut reg = APP_REGISTRY.lock();
        if let Err(e) = reg.load_from_vfs() {
            crate::serial_println!("[KPIO/App] Warning: failed to load registry: {}", e);
        }
    }

    crate::serial_println!("[KPIO/App] App subsystem initialised");
}
