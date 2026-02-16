//! Browser Extension APIs
//!
//! Implements chrome.* APIs for extensions.

#![allow(dead_code)]

extern crate alloc;

pub mod runtime;
pub mod storage;
pub mod tabs;
pub mod web_request;

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::{ExtensionError, ExtensionId};

/// API call result.
pub type ApiResult<T> = Result<T, ApiError>;

/// API error.
#[derive(Debug, Clone)]
pub struct ApiError {
    /// Error message.
    pub message: String,
}

impl ApiError {
    /// Create a new API error.
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }

    /// Permission denied error.
    pub fn permission_denied(resource: &str) -> Self {
        Self::new(&alloc::format!("Permission denied: {}", resource))
    }

    /// Invalid argument error.
    pub fn invalid_argument(arg: &str) -> Self {
        Self::new(&alloc::format!("Invalid argument: {}", arg))
    }

    /// Not found error.
    pub fn not_found(resource: &str) -> Self {
        Self::new(&alloc::format!("{} not found", resource))
    }

    /// Quota exceeded error.
    pub fn quota_exceeded(reason: &str) -> Self {
        Self::new(&alloc::format!("Quota exceeded: {}", reason))
    }
}

impl From<ExtensionError> for ApiError {
    fn from(err: ExtensionError) -> Self {
        match err {
            ExtensionError::NotFound => Self::not_found("Extension"),
            ExtensionError::PermissionDenied => Self::permission_denied("Extension"),
            _ => Self::new("Extension error"),
        }
    }
}

/// API context for an extension call.
#[derive(Debug, Clone)]
pub struct ApiContext {
    /// Extension ID making the call.
    pub extension_id: ExtensionId,
    /// Tab ID (if called from a tab context).
    pub tab_id: Option<u32>,
    /// Frame ID (if called from a frame context).
    pub frame_id: Option<u32>,
    /// URL of the calling context.
    pub url: Option<String>,
}

impl ApiContext {
    /// Create a new API context.
    pub fn new(extension_id: ExtensionId) -> Self {
        Self {
            extension_id,
            tab_id: None,
            frame_id: None,
            url: None,
        }
    }

    /// With tab context.
    pub fn with_tab(mut self, tab_id: u32, frame_id: u32) -> Self {
        self.tab_id = Some(tab_id);
        self.frame_id = Some(frame_id);
        self
    }

    /// With URL.
    pub fn with_url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }
}

/// API callback.
pub type ApiCallback<T> = Box<dyn FnOnce(ApiResult<T>) + Send>;

/// Async API call handle.
pub struct AsyncCall<T> {
    /// Result (once available).
    result: Option<ApiResult<T>>,
    /// Callback.
    callback: Option<ApiCallback<T>>,
}

impl<T> AsyncCall<T> {
    /// Create a new async call.
    pub fn new() -> Self {
        Self {
            result: None,
            callback: None,
        }
    }

    /// Set callback.
    pub fn then(mut self, callback: ApiCallback<T>) -> Self {
        if let Some(result) = self.result.take() {
            callback(result);
        } else {
            self.callback = Some(callback);
        }
        self
    }

    /// Resolve with result.
    pub fn resolve(mut self, result: ApiResult<T>) {
        if let Some(callback) = self.callback.take() {
            callback(result);
        } else {
            self.result = Some(result);
        }
    }
}

impl<T> Default for AsyncCall<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Event listener.
pub type EventListener<T> = Box<dyn Fn(&T) + Send + Sync>;

/// Event emitter.
pub struct EventEmitter<T> {
    listeners: Vec<EventListener<T>>,
}

impl<T> EventEmitter<T> {
    /// Create a new event emitter.
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    /// Add a listener.
    pub fn add_listener(&mut self, listener: EventListener<T>) {
        self.listeners.push(listener);
    }

    /// Remove all listeners.
    pub fn remove_all_listeners(&mut self) {
        self.listeners.clear();
    }

    /// Emit an event.
    pub fn emit(&self, event: &T) {
        for listener in &self.listeners {
            listener(event);
        }
    }

    /// Check if there are listeners.
    pub fn has_listeners(&self) -> bool {
        !self.listeners.is_empty()
    }
}

impl<T> Default for EventEmitter<T> {
    fn default() -> Self {
        Self::new()
    }
}
