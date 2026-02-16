//! App management syscall wrappers.
//!
//! Provides safe Rust wrappers around the kernel app management
//! system calls (106-111). These allow userspace programs and the
//! browser crate to install, launch, query, and uninstall apps.

use crate::syscall::{self, SyscallError, SyscallNumber, SyscallResult};

/// Install/register a new app.
///
/// # Arguments
/// - `app_type` — 0 = WebApp, 1 = WasmApp, 2 = NativeApp
/// - `name` — human-readable app name (UTF-8)
/// - `entry_point` — entry point path or URL (UTF-8)
///
/// # Returns
/// The kernel-assigned `app_id` (u64) on success.
pub fn app_install(app_type: u64, name: &str, entry_point: &str) -> SyscallResult {
    unsafe {
        syscall::syscall5(
            SyscallNumber::AppInstall,
            app_type,
            name.as_ptr() as u64,
            name.len() as u64,
            entry_point.as_ptr() as u64,
            entry_point.len() as u64,
        )
    }
}

/// Launch an installed app.
///
/// # Arguments
/// - `app_id` — the app ID returned from `app_install`.
///
/// # Returns
/// An `instance_id` (u64) identifying the running instance.
pub fn app_launch(app_id: u64) -> SyscallResult {
    unsafe { syscall::syscall1(SyscallNumber::AppLaunch, app_id) }
}

/// Terminate a running app instance.
///
/// # Arguments
/// - `instance_id` — the instance ID returned from `app_launch`.
pub fn app_terminate(instance_id: u64) -> Result<(), SyscallError> {
    unsafe { syscall::syscall1(SyscallNumber::AppTerminate, instance_id) }?;
    Ok(())
}

/// Query information about an installed app.
///
/// The kernel writes a text representation of the app descriptor
/// into `buf` and returns the number of bytes written.
///
/// Format: `id=<id>,name=<name>,entry=<entry>,type=<web|wasm|native>`
///
/// # Arguments
/// - `app_id` — the app to query.
/// - `buf` — output buffer for the info string.
///
/// # Returns
/// Number of bytes written into `buf`.
pub fn app_info(app_id: u64, buf: &mut [u8]) -> Result<usize, SyscallError> {
    let result = unsafe {
        syscall::syscall3(
            SyscallNumber::AppGetInfo,
            app_id,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
        )
    }?;
    Ok(result as usize)
}

/// List installed app IDs.
///
/// Writes up to `buf.len()` app IDs into the provided buffer.
///
/// # Returns
/// Number of app IDs actually written.
pub fn app_list(buf: &mut [u64]) -> Result<usize, SyscallError> {
    let result = unsafe {
        syscall::syscall2(
            SyscallNumber::AppList,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
        )
    }?;
    Ok(result as usize)
}

/// Uninstall an app by its ID.
///
/// This terminates all running instances, removes the app from the
/// registry, and deletes its sandbox data directory.
pub fn app_uninstall(app_id: u64) -> Result<(), SyscallError> {
    unsafe { syscall::syscall1(SyscallNumber::AppUninstall, app_id) }?;
    Ok(())
}
