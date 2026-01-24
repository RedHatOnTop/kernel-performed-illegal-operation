//! std::env compatibility layer for KPIO
//!
//! Provides environment variable and argument access via KPIO syscalls.

use alloc::string::String;
use alloc::vec::Vec;

use crate::syscall;

/// Returns the arguments which this program was started with.
pub fn args() -> Args {
    Args {
        inner: syscall::get_args().unwrap_or_default().into_iter(),
    }
}

/// An iterator over the arguments of a process.
pub struct Args {
    inner: alloc::vec::IntoIter<String>,
}

impl Iterator for Args {
    type Item = String;
    
    fn next(&mut self) -> Option<String> {
        self.inner.next()
    }
    
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for Args {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

/// Fetches the environment variable `key`.
pub fn var(key: &str) -> Result<String, VarError> {
    syscall::env_get(key)
        .map_err(|_| VarError::NotPresent)
}

/// Sets the environment variable `key` to `value`.
pub fn set_var(key: &str, value: &str) {
    let _ = syscall::env_set(key, value);
}

/// Removes an environment variable.
pub fn remove_var(key: &str) {
    let _ = syscall::env_remove(key);
}

/// Returns an iterator of environment variables.
pub fn vars() -> Vars {
    Vars {
        inner: syscall::env_list().unwrap_or_default().into_iter(),
    }
}

/// An iterator over environment variables.
pub struct Vars {
    inner: alloc::vec::IntoIter<(String, String)>,
}

impl Iterator for Vars {
    type Item = (String, String);
    
    fn next(&mut self) -> Option<(String, String)> {
        self.inner.next()
    }
}

/// Error type for environment variable operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarError {
    NotPresent,
    NotUnicode,
}

/// Returns the current working directory.
pub fn current_dir() -> Result<String, super::net::IoError> {
    syscall::getcwd()
        .map_err(|_| super::net::IoError::Other)
}

/// Changes the current working directory.
pub fn set_current_dir(path: &str) -> Result<(), super::net::IoError> {
    syscall::chdir(path)
        .map(|_| ())
        .map_err(|_| super::net::IoError::NotFound)
}

/// Returns the path of the current executable.
pub fn current_exe() -> Result<String, super::net::IoError> {
    syscall::current_exe()
        .map_err(|_| super::net::IoError::Other)
}

/// Returns the full filesystem path of the current running executable.
pub fn temp_dir() -> String {
    String::from("/tmp")
}

/// Returns the path of a temporary directory.
pub fn home_dir() -> Option<String> {
    var("HOME").ok()
}

/// Constants for target information
pub mod consts {
    /// Target architecture
    pub const ARCH: &str = "x86_64";
    
    /// Target OS
    pub const OS: &str = "kpio";
    
    /// Target family
    pub const FAMILY: &str = "kpio";
    
    /// DLL prefix
    pub const DLL_PREFIX: &str = "lib";
    
    /// DLL suffix
    pub const DLL_SUFFIX: &str = ".so";
    
    /// DLL extension
    pub const DLL_EXTENSION: &str = "so";
    
    /// Executable suffix
    pub const EXE_SUFFIX: &str = "";
    
    /// Executable extension
    pub const EXE_EXTENSION: &str = "";
}
