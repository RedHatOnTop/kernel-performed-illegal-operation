//! App Management Error Types
//!
//! Defines all error types used by the app management subsystem.

use alloc::string::String;
use core::fmt;

/// App management error.
#[derive(Debug, Clone)]
pub enum AppError {
    /// App with this ID was not found.
    NotFound,
    /// An app with this name or scope is already registered.
    AlreadyRegistered,
    /// Permission denied for the requested operation.
    PermissionDenied,
    /// Failed to launch the app.
    LaunchFailed(String),
    /// Resource limit exceeded (memory, storage, etc.).
    ResourceExhausted,
    /// App instance not found.
    InstanceNotFound,
    /// Invalid app descriptor or manifest data.
    InvalidDescriptor(String),
    /// VFS I/O error during registry persistence.
    IoError,
    /// App is in an invalid state for the requested operation.
    InvalidState {
        current: &'static str,
        expected: &'static str,
    },
    /// Maximum number of app instances reached.
    TooManyInstances,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotFound => write!(f, "app not found"),
            AppError::AlreadyRegistered => write!(f, "app already registered"),
            AppError::PermissionDenied => write!(f, "permission denied"),
            AppError::LaunchFailed(msg) => write!(f, "launch failed: {}", msg),
            AppError::ResourceExhausted => write!(f, "resource exhausted"),
            AppError::InstanceNotFound => write!(f, "app instance not found"),
            AppError::InvalidDescriptor(msg) => write!(f, "invalid descriptor: {}", msg),
            AppError::IoError => write!(f, "I/O error"),
            AppError::InvalidState { current, expected } => {
                write!(
                    f,
                    "invalid state: current={}, expected={}",
                    current, expected
                )
            }
            AppError::TooManyInstances => write!(f, "too many app instances"),
        }
    }
}
