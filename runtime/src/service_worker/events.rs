//! Service Worker Events
//!
//! Common event types and utilities for service workers.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use super::ServiceWorkerId;

/// Event type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    /// Install event
    Install,
    /// Activate event
    Activate,
    /// Fetch event
    Fetch,
    /// Push event
    Push,
    /// Sync event
    Sync,
    /// Periodic sync event
    PeriodicSync,
    /// Notification click event
    NotificationClick,
    /// Notification close event
    NotificationClose,
    /// Message event
    Message,
}

/// Extendable event trait
pub trait ExtendableEvent {
    /// Get event type
    fn event_type(&self) -> EventType;

    /// Wait until a promise-like operation completes
    fn wait_until(&mut self);

    /// Check if wait_until was called
    fn has_wait_until(&self) -> bool;
}

/// Message event data
#[derive(Debug, Clone)]
pub struct MessageEvent {
    /// Event type
    event_type: EventType,
    /// Message data (serialized)
    data: Vec<u8>,
    /// Origin
    origin: String,
    /// Last event ID
    last_event_id: String,
    /// Source (client ID or worker ID as string)
    source: Option<String>,
    /// Ports (MessagePort handles)
    ports: Vec<u64>,
    /// Whether wait_until was called
    wait_until: bool,
}

impl MessageEvent {
    /// Create new message event
    pub fn new(data: Vec<u8>, origin: impl Into<String>) -> Self {
        Self {
            event_type: EventType::Message,
            data,
            origin: origin.into(),
            last_event_id: String::new(),
            source: None,
            ports: Vec::new(),
            wait_until: false,
        }
    }

    /// Get data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get origin
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Get source
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// Set source
    pub fn set_source(&mut self, source: impl Into<String>) {
        self.source = Some(source.into());
    }

    /// Get ports
    pub fn ports(&self) -> &[u64] {
        &self.ports
    }
}

impl ExtendableEvent for MessageEvent {
    fn event_type(&self) -> EventType {
        self.event_type
    }

    fn wait_until(&mut self) {
        self.wait_until = true;
    }

    fn has_wait_until(&self) -> bool {
        self.wait_until
    }
}

/// Push event data
#[derive(Debug, Clone)]
pub struct PushEvent {
    /// Event type
    event_type: EventType,
    /// Push data
    data: Option<Vec<u8>>,
    /// Whether wait_until was called
    wait_until: bool,
}

impl PushEvent {
    /// Create new push event
    pub fn new(data: Option<Vec<u8>>) -> Self {
        Self {
            event_type: EventType::Push,
            data,
            wait_until: false,
        }
    }

    /// Get data
    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    /// Get data as text
    pub fn text(&self) -> Option<String> {
        self.data.as_ref().and_then(|d| {
            core::str::from_utf8(d).ok().map(|s| s.to_string())
        })
    }
}

impl ExtendableEvent for PushEvent {
    fn event_type(&self) -> EventType {
        self.event_type
    }

    fn wait_until(&mut self) {
        self.wait_until = true;
    }

    fn has_wait_until(&self) -> bool {
        self.wait_until
    }
}

use alloc::string::ToString;

/// Notification click event
#[derive(Debug, Clone)]
pub struct NotificationClickEvent {
    /// Event type
    event_type: EventType,
    /// Notification tag
    notification_tag: Option<String>,
    /// Notification data
    notification_data: Option<Vec<u8>>,
    /// Action clicked
    action: Option<String>,
    /// Whether wait_until was called
    wait_until: bool,
}

impl NotificationClickEvent {
    /// Create new notification click event
    pub fn new() -> Self {
        Self {
            event_type: EventType::NotificationClick,
            notification_tag: None,
            notification_data: None,
            action: None,
            wait_until: false,
        }
    }

    /// Set notification tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.notification_tag = Some(tag.into());
        self
    }

    /// Set action
    pub fn with_action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    /// Get notification tag
    pub fn notification_tag(&self) -> Option<&str> {
        self.notification_tag.as_deref()
    }

    /// Get action
    pub fn action(&self) -> Option<&str> {
        self.action.as_deref()
    }
}

impl Default for NotificationClickEvent {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtendableEvent for NotificationClickEvent {
    fn event_type(&self) -> EventType {
        self.event_type
    }

    fn wait_until(&mut self) {
        self.wait_until = true;
    }

    fn has_wait_until(&self) -> bool {
        self.wait_until
    }
}

/// Event dispatcher
pub struct EventDispatcher {
    /// Worker ID
    worker_id: ServiceWorkerId,
    /// Event queue
    event_queue: Vec<Box<dyn ExtendableEvent + Send + Sync>>,
}

impl EventDispatcher {
    /// Create new dispatcher
    pub fn new(worker_id: ServiceWorkerId) -> Self {
        Self {
            worker_id,
            event_queue: Vec::new(),
        }
    }

    /// Queue an event
    pub fn queue(&mut self, event: Box<dyn ExtendableEvent + Send + Sync>) {
        self.event_queue.push(event);
    }

    /// Dispatch next event
    pub fn dispatch_next(&mut self) -> Option<Box<dyn ExtendableEvent + Send + Sync>> {
        if self.event_queue.is_empty() {
            None
        } else {
            Some(self.event_queue.remove(0))
        }
    }

    /// Get queue length
    pub fn queue_len(&self) -> usize {
        self.event_queue.len()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.event_queue.is_empty()
    }
}

/// Client info
#[derive(Debug, Clone)]
pub struct ClientInfo {
    /// Client ID
    pub id: String,
    /// Client type
    pub client_type: ClientType,
    /// URL
    pub url: String,
    /// Frame type
    pub frame_type: FrameType,
    /// Visibility state
    pub visibility: VisibilityState,
    /// Whether focused
    pub focused: bool,
}

/// Client type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientType {
    /// Window client
    Window,
    /// Worker client
    Worker,
    /// SharedWorker client
    SharedWorker,
    /// All types
    All,
}

impl Default for ClientType {
    fn default() -> Self {
        Self::Window
    }
}

/// Frame type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    /// Auxiliary (opened via window.open)
    Auxiliary,
    /// Top-level
    TopLevel,
    /// Nested (iframe)
    Nested,
    /// None
    None,
}

impl Default for FrameType {
    fn default() -> Self {
        Self::TopLevel
    }
}

/// Visibility state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityState {
    /// Hidden
    Hidden,
    /// Visible
    Visible,
}

impl Default for VisibilityState {
    fn default() -> Self {
        Self::Visible
    }
}

/// Clients API
pub struct Clients {
    /// All clients
    clients: Vec<ClientInfo>,
}

impl Clients {
    /// Create new clients API
    pub fn new() -> Self {
        Self {
            clients: Vec::new(),
        }
    }

    /// Get a client by ID
    pub fn get(&self, id: &str) -> Option<&ClientInfo> {
        self.clients.iter().find(|c| c.id == id)
    }

    /// Match all clients
    pub fn match_all(&self, options: MatchAllOptions) -> Vec<&ClientInfo> {
        self.clients.iter().filter(|client| {
            // Filter by type
            if options.client_type != ClientType::All && client.client_type != options.client_type {
                return false;
            }
            
            // Filter by include_uncontrolled
            true // Would check if controlled
        }).collect()
    }

    /// Open a window
    pub fn open_window(&mut self, _url: &str) -> Result<ClientInfo, String> {
        // Would open a new window
        Err("Not implemented".into())
    }

    /// Claim all clients
    pub fn claim(&mut self) -> Result<(), String> {
        // Would take control of all clients in scope
        Ok(())
    }

    /// Add a client
    pub fn add(&mut self, client: ClientInfo) {
        self.clients.push(client);
    }

    /// Remove a client
    pub fn remove(&mut self, id: &str) -> bool {
        let len_before = self.clients.len();
        self.clients.retain(|c| c.id != id);
        self.clients.len() != len_before
    }
}

impl Default for Clients {
    fn default() -> Self {
        Self::new()
    }
}

/// Options for matchAll
#[derive(Debug, Clone, Default)]
pub struct MatchAllOptions {
    /// Include uncontrolled clients
    pub include_uncontrolled: bool,
    /// Client type filter
    pub client_type: ClientType,
}
