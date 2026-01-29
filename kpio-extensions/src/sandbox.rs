//! Extension Sandbox
//!
//! Provides isolated execution environment for extensions.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::RwLock;

use crate::{ExtensionId, ExtensionError};

/// Sandbox ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SandboxId(pub u64);

/// Static counter for sandbox IDs.
static NEXT_SANDBOX_ID: AtomicU64 = AtomicU64::new(1);

impl SandboxId {
    /// Generate a new sandbox ID.
    pub fn new() -> Self {
        Self(NEXT_SANDBOX_ID.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for SandboxId {
    fn default() -> Self {
        Self::new()
    }
}

/// Sandbox configuration.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Memory limit (bytes).
    pub memory_limit: usize,
    /// CPU time limit (milliseconds).
    pub cpu_time_limit: u64,
    /// Allowed APIs.
    pub allowed_apis: Vec<String>,
    /// Network access enabled.
    pub network_access: bool,
    /// Storage quota (bytes).
    pub storage_quota: usize,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            memory_limit: 256 * 1024 * 1024, // 256 MB
            cpu_time_limit: 30_000,           // 30 seconds
            allowed_apis: Vec::new(),
            network_access: false,
            storage_quota: 10 * 1024 * 1024,  // 10 MB
        }
    }
}

impl SandboxConfig {
    /// Create config with permissions.
    pub fn with_permissions(permissions: &[String]) -> Self {
        let mut config = Self::default();
        
        for perm in permissions {
            match perm.as_str() {
                "storage" => {
                    config.allowed_apis.push("chrome.storage".to_string());
                }
                "tabs" => {
                    config.allowed_apis.push("chrome.tabs".to_string());
                }
                "webRequest" => {
                    config.allowed_apis.push("chrome.webRequest".to_string());
                    config.network_access = true;
                }
                "activeTab" => {
                    config.allowed_apis.push("chrome.tabs.query".to_string());
                    config.allowed_apis.push("chrome.tabs.get".to_string());
                }
                "unlimitedStorage" => {
                    config.storage_quota = usize::MAX;
                }
                _ => {}
            }
        }
        
        // Always allow runtime API
        config.allowed_apis.push("chrome.runtime".to_string());
        
        config
    }
}

/// Extension sandbox.
pub struct Sandbox {
    /// Sandbox ID.
    pub id: SandboxId,
    /// Extension ID.
    pub extension_id: ExtensionId,
    /// Configuration.
    pub config: SandboxConfig,
    /// State.
    state: SandboxState,
    /// Resource usage.
    usage: ResourceUsage,
    /// Message handlers.
    message_handlers: BTreeMap<String, MessageHandler>,
    /// Pending messages.
    pending_messages: Vec<Message>,
}

/// Sandbox state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxState {
    /// Not started.
    Created,
    /// Running.
    Running,
    /// Paused.
    Paused,
    /// Terminated.
    Terminated,
    /// Error state.
    Error,
}

/// Resource usage tracking.
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    /// Memory used (bytes).
    pub memory_used: usize,
    /// CPU time used (milliseconds).
    pub cpu_time_used: u64,
    /// Storage used (bytes).
    pub storage_used: usize,
    /// Network bytes sent.
    pub network_sent: usize,
    /// Network bytes received.
    pub network_received: usize,
}

impl ResourceUsage {
    /// Check if within limits.
    pub fn check_limits(&self, config: &SandboxConfig) -> Result<(), ResourceLimitExceeded> {
        if self.memory_used > config.memory_limit {
            return Err(ResourceLimitExceeded::Memory);
        }
        if self.cpu_time_used > config.cpu_time_limit {
            return Err(ResourceLimitExceeded::CpuTime);
        }
        if self.storage_used > config.storage_quota {
            return Err(ResourceLimitExceeded::Storage);
        }
        Ok(())
    }
}

/// Resource limit exceeded error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimitExceeded {
    Memory,
    CpuTime,
    Storage,
    Network,
}

/// Message between sandbox and browser.
#[derive(Debug, Clone)]
pub struct Message {
    /// Message ID.
    pub id: u64,
    /// Message type.
    pub message_type: MessageType,
    /// Payload.
    pub payload: MessagePayload,
    /// Sender.
    pub sender: MessageSender,
    /// Response channel ID.
    pub response_channel: Option<u64>,
}

/// Message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// API call request.
    ApiCall,
    /// API call response.
    ApiResponse,
    /// Event notification.
    Event,
    /// Port message.
    PortMessage,
    /// Port disconnect.
    PortDisconnect,
}

/// Message payload.
#[derive(Debug, Clone)]
pub enum MessagePayload {
    /// JSON-like data.
    Json(String),
    /// Binary data.
    Binary(Vec<u8>),
    /// Empty.
    Empty,
}

/// Message sender.
#[derive(Debug, Clone)]
pub enum MessageSender {
    /// Extension.
    Extension {
        id: ExtensionId,
        tab_id: Option<u32>,
        frame_id: Option<u32>,
    },
    /// Content script.
    ContentScript {
        extension_id: ExtensionId,
        tab_id: u32,
        frame_id: u32,
        url: String,
    },
    /// Browser.
    Browser,
    /// Web page.
    WebPage {
        url: String,
        tab_id: u32,
    },
}

/// Message handler.
type MessageHandler = Box<dyn Fn(&Message) -> Option<MessagePayload> + Send + Sync>;

impl Sandbox {
    /// Create a new sandbox.
    pub fn new(extension_id: ExtensionId, config: SandboxConfig) -> Self {
        Self {
            id: SandboxId::new(),
            extension_id,
            config,
            state: SandboxState::Created,
            usage: ResourceUsage::default(),
            message_handlers: BTreeMap::new(),
            pending_messages: Vec::new(),
        }
    }
    
    /// Start the sandbox.
    pub fn start(&mut self) -> Result<(), SandboxError> {
        if self.state != SandboxState::Created {
            return Err(SandboxError::InvalidState);
        }
        
        self.state = SandboxState::Running;
        Ok(())
    }
    
    /// Pause the sandbox.
    pub fn pause(&mut self) -> Result<(), SandboxError> {
        if self.state != SandboxState::Running {
            return Err(SandboxError::InvalidState);
        }
        
        self.state = SandboxState::Paused;
        Ok(())
    }
    
    /// Resume the sandbox.
    pub fn resume(&mut self) -> Result<(), SandboxError> {
        if self.state != SandboxState::Paused {
            return Err(SandboxError::InvalidState);
        }
        
        self.state = SandboxState::Running;
        Ok(())
    }
    
    /// Terminate the sandbox.
    pub fn terminate(&mut self) {
        self.state = SandboxState::Terminated;
        self.pending_messages.clear();
    }
    
    /// Get sandbox state.
    pub fn state(&self) -> SandboxState {
        self.state
    }
    
    /// Get resource usage.
    pub fn usage(&self) -> &ResourceUsage {
        &self.usage
    }
    
    /// Check if API is allowed.
    pub fn is_api_allowed(&self, api: &str) -> bool {
        self.config.allowed_apis.iter().any(|allowed| {
            api.starts_with(allowed) || *allowed == api
        })
    }
    
    /// Send a message to the sandbox.
    pub fn send_message(&mut self, message: Message) -> Result<(), SandboxError> {
        if self.state != SandboxState::Running {
            return Err(SandboxError::NotRunning);
        }
        
        self.pending_messages.push(message);
        Ok(())
    }
    
    /// Process pending messages.
    pub fn process_messages(&mut self) -> Vec<(u64, Option<MessagePayload>)> {
        let messages = core::mem::take(&mut self.pending_messages);
        let mut responses = Vec::new();
        
        for message in messages {
            let handler_key = match message.message_type {
                MessageType::ApiCall => "api",
                MessageType::Event => "event",
                MessageType::PortMessage => "port",
                _ => continue,
            };
            
            if let Some(handler) = self.message_handlers.get(handler_key) {
                let response = handler(&message);
                responses.push((message.id, response));
            }
        }
        
        responses
    }
    
    /// Register a message handler.
    pub fn register_handler(&mut self, name: &str, handler: MessageHandler) {
        self.message_handlers.insert(name.to_string(), handler);
    }
    
    /// Update memory usage.
    pub fn update_memory_usage(&mut self, bytes: usize) -> Result<(), ResourceLimitExceeded> {
        self.usage.memory_used = bytes;
        self.usage.check_limits(&self.config)
    }
    
    /// Update CPU time.
    pub fn update_cpu_time(&mut self, ms: u64) -> Result<(), ResourceLimitExceeded> {
        self.usage.cpu_time_used += ms;
        self.usage.check_limits(&self.config)
    }
}

/// Sandbox error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxError {
    /// Invalid state transition.
    InvalidState,
    /// Sandbox not running.
    NotRunning,
    /// Resource limit exceeded.
    ResourceLimit(ResourceLimitExceeded),
    /// API not allowed.
    ApiNotAllowed,
    /// Message send failed.
    MessageFailed,
}

/// Sandbox manager.
pub struct SandboxManager {
    /// Active sandboxes.
    sandboxes: RwLock<BTreeMap<u64, Sandbox>>,
}

impl SandboxManager {
    /// Create a new sandbox manager.
    pub const fn new() -> Self {
        Self {
            sandboxes: RwLock::new(BTreeMap::new()),
        }
    }
    
    /// Create a sandbox for an extension.
    pub fn create_sandbox(
        &self,
        extension_id: ExtensionId,
        config: SandboxConfig,
    ) -> SandboxId {
        let sandbox = Sandbox::new(extension_id, config);
        let id = sandbox.id;
        self.sandboxes.write().insert(id.0, sandbox);
        id
    }
    
    /// Get a sandbox.
    pub fn get_sandbox(&self, id: SandboxId) -> Option<Sandbox> {
        self.sandboxes.read().get(&id.0).cloned()
    }
    
    /// Start a sandbox.
    pub fn start_sandbox(&self, id: SandboxId) -> Result<(), SandboxError> {
        let mut sandboxes = self.sandboxes.write();
        if let Some(sandbox) = sandboxes.get_mut(&id.0) {
            sandbox.start()
        } else {
            Err(SandboxError::InvalidState)
        }
    }
    
    /// Terminate a sandbox.
    pub fn terminate_sandbox(&self, id: SandboxId) {
        let mut sandboxes = self.sandboxes.write();
        if let Some(sandbox) = sandboxes.get_mut(&id.0) {
            sandbox.terminate();
        }
    }
    
    /// Remove a sandbox.
    pub fn remove_sandbox(&self, id: SandboxId) {
        self.sandboxes.write().remove(&id.0);
    }
    
    /// Get sandboxes for an extension.
    pub fn get_extension_sandboxes(&self, extension_id: &ExtensionId) -> Vec<SandboxId> {
        self.sandboxes.read()
            .values()
            .filter(|s| s.extension_id == *extension_id)
            .map(|s| s.id)
            .collect()
    }
}

impl Default for SandboxManager {
    fn default() -> Self {
        Self::new()
    }
}

// Note: Clone is not derivable due to Box<dyn Fn>, so we implement it manually
impl Clone for Sandbox {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            extension_id: self.extension_id.clone(),
            config: self.config.clone(),
            state: self.state,
            usage: self.usage.clone(),
            message_handlers: BTreeMap::new(), // Handlers are not cloned
            pending_messages: self.pending_messages.clone(),
        }
    }
}

/// Isolated world for content scripts.
pub struct IsolatedWorld {
    /// World ID.
    pub id: u64,
    /// Extension ID.
    pub extension_id: ExtensionId,
    /// Tab ID.
    pub tab_id: u32,
    /// Frame ID.
    pub frame_id: u32,
    /// Injected scripts.
    pub scripts: Vec<String>,
    /// Exposed APIs.
    pub exposed_apis: Vec<String>,
}

impl IsolatedWorld {
    /// Create a new isolated world.
    pub fn new(id: u64, extension_id: ExtensionId, tab_id: u32, frame_id: u32) -> Self {
        Self {
            id,
            extension_id,
            tab_id,
            frame_id,
            scripts: Vec::new(),
            exposed_apis: Vec::new(),
        }
    }
    
    /// Add a script to the world.
    pub fn add_script(&mut self, script: String) {
        self.scripts.push(script);
    }
    
    /// Expose an API to the world.
    pub fn expose_api(&mut self, api: &str) {
        if !self.exposed_apis.contains(&api.to_string()) {
            self.exposed_apis.push(api.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sandbox_lifecycle() {
        let mut sandbox = Sandbox::new(
            ExtensionId::new("test"),
            SandboxConfig::default(),
        );
        
        assert_eq!(sandbox.state(), SandboxState::Created);
        
        sandbox.start().unwrap();
        assert_eq!(sandbox.state(), SandboxState::Running);
        
        sandbox.pause().unwrap();
        assert_eq!(sandbox.state(), SandboxState::Paused);
        
        sandbox.resume().unwrap();
        assert_eq!(sandbox.state(), SandboxState::Running);
        
        sandbox.terminate();
        assert_eq!(sandbox.state(), SandboxState::Terminated);
    }
    
    #[test]
    fn test_api_permissions() {
        let config = SandboxConfig::with_permissions(&[
            "storage".to_string(),
            "tabs".to_string(),
        ]);
        
        let sandbox = Sandbox::new(ExtensionId::new("test"), config);
        
        assert!(sandbox.is_api_allowed("chrome.storage"));
        assert!(sandbox.is_api_allowed("chrome.storage.local.get"));
        assert!(sandbox.is_api_allowed("chrome.tabs"));
        assert!(sandbox.is_api_allowed("chrome.runtime")); // Always allowed
        assert!(!sandbox.is_api_allowed("chrome.webRequest"));
    }
    
    #[test]
    fn test_resource_limits() {
        let config = SandboxConfig {
            memory_limit: 1024,
            ..Default::default()
        };
        
        let mut sandbox = Sandbox::new(ExtensionId::new("test"), config);
        sandbox.start().unwrap();
        
        assert!(sandbox.update_memory_usage(512).is_ok());
        assert!(sandbox.update_memory_usage(2048).is_err());
    }
}
