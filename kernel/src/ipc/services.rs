//! Service Registry for IPC
//!
//! This module implements a named service registration and discovery system
//! that allows WASM processes to find and connect to system services.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::sync::Arc;
use spin::RwLock;

use super::capability::{Capability, CapabilityRights};
use super::channel::{Channel, ChannelId};

/// Service identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceId(pub u64);

impl ServiceId {
    /// Create new service ID.
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    /// Get raw ID value.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// Well-known system service names.
pub mod well_known {
    /// Virtual filesystem service.
    pub const VFS: &str = "kpio.vfs";
    /// Network service.
    pub const NETWORK: &str = "kpio.network";
    /// Display/compositor service.
    pub const DISPLAY: &str = "kpio.display";
    /// Input service.
    pub const INPUT: &str = "kpio.input";
    /// Audio service.
    pub const AUDIO: &str = "kpio.audio";
    /// Power management service.
    pub const POWER: &str = "kpio.power";
    /// Device manager service.
    pub const DEVICES: &str = "kpio.devices";
    /// Security/policy service.
    pub const SECURITY: &str = "kpio.security";
    /// Process manager service.
    pub const PROCESS: &str = "kpio.process";
    /// Timer service.
    pub const TIMER: &str = "kpio.timer";
}

/// Service state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    /// Service is starting up.
    Starting,
    /// Service is running and accepting connections.
    Running,
    /// Service is stopping.
    Stopping,
    /// Service has stopped.
    Stopped,
    /// Service has failed.
    Failed,
}

/// Service metadata.
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    /// Service unique ID.
    pub id: ServiceId,
    /// Service name (hierarchical, e.g., "kpio.vfs").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Service version.
    pub version: u32,
    /// Current state.
    pub state: ServiceState,
    /// Required capabilities to connect.
    pub required_rights: CapabilityRights,
    /// Maximum concurrent connections.
    pub max_connections: u32,
    /// Current connection count.
    pub current_connections: u32,
    /// Owner process ID.
    pub owner_pid: u64,
}

impl ServiceInfo {
    /// Create new service info.
    pub fn new(id: ServiceId, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            description: String::new(),
            version: 1,
            state: ServiceState::Starting,
            required_rights: CapabilityRights::CONNECT,
            max_connections: 256,
            current_connections: 0,
            owner_pid: 0,
        }
    }
    
    /// Set description.
    pub fn description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }
    
    /// Set version.
    pub fn version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }
    
    /// Set required rights.
    pub fn required_rights(mut self, rights: CapabilityRights) -> Self {
        self.required_rights = rights;
        self
    }
    
    /// Set max connections.
    pub fn max_connections(mut self, max: u32) -> Self {
        self.max_connections = max;
        self
    }
    
    /// Set owner PID.
    pub fn owner(mut self, pid: u64) -> Self {
        self.owner_pid = pid;
        self
    }
    
    /// Check if service can accept new connection.
    pub fn can_accept_connection(&self) -> bool {
        self.state == ServiceState::Running 
            && self.current_connections < self.max_connections
    }
}

/// Service connection handle.
pub struct ServiceConnection {
    /// Service ID.
    pub service_id: ServiceId,
    /// Client capability.
    pub capability: Capability,
    /// Communication channel.
    pub channel: Arc<Channel>,
    /// Connection ID.
    pub connection_id: u64,
}

/// Registered service.
struct RegisteredService {
    /// Service info.
    info: ServiceInfo,
    /// Server-side channel for accepting connections.
    accept_channel: Arc<Channel>,
    /// Active connections.
    connections: Vec<u64>,
}

/// Service registry errors.
#[derive(Debug, Clone)]
pub enum ServiceError {
    /// Service not found.
    NotFound(String),
    /// Service already exists.
    AlreadyExists(String),
    /// Permission denied.
    PermissionDenied,
    /// Service is not running.
    NotRunning,
    /// Maximum connections reached.
    TooManyConnections,
    /// Invalid service name.
    InvalidName(String),
    /// Internal error.
    InternalError(String),
}

/// Global service registry.
pub struct ServiceRegistry {
    /// Services by name.
    by_name: RwLock<BTreeMap<String, RegisteredService>>,
    /// Services by ID.
    by_id: RwLock<BTreeMap<ServiceId, String>>,
    /// Next service ID.
    next_id: spin::Mutex<u64>,
    /// Next connection ID.
    next_connection_id: spin::Mutex<u64>,
}

impl ServiceRegistry {
    /// Create new service registry.
    pub const fn new() -> Self {
        Self {
            by_name: RwLock::new(BTreeMap::new()),
            by_id: RwLock::new(BTreeMap::new()),
            next_id: spin::Mutex::new(1),
            next_connection_id: spin::Mutex::new(1),
        }
    }
    
    /// Register a new service.
    pub fn register(
        &self,
        name: &str,
        description: &str,
        owner_pid: u64,
        required_rights: CapabilityRights,
    ) -> Result<(ServiceId, Arc<Channel>), ServiceError> {
        // Validate name
        if name.is_empty() || !Self::validate_name(name) {
            return Err(ServiceError::InvalidName(name.to_string()));
        }
        
        // Check if already exists
        {
            let by_name = self.by_name.read();
            if by_name.contains_key(name) {
                return Err(ServiceError::AlreadyExists(name.to_string()));
            }
        }
        
        // Allocate ID
        let id = {
            let mut next = self.next_id.lock();
            let id = ServiceId(*next);
            *next += 1;
            id
        };
        
        // Create accept channel
        let channel_id = ChannelId(id.0);
        let channel = Arc::new(Channel::new(channel_id, channel_id));
        
        // Create service info
        let info = ServiceInfo::new(id, name)
            .description(description)
            .required_rights(required_rights)
            .owner(owner_pid);
        
        // Register
        {
            let mut by_name = self.by_name.write();
            let mut by_id = self.by_id.write();
            
            by_name.insert(name.to_string(), RegisteredService {
                info,
                accept_channel: channel.clone(),
                connections: Vec::new(),
            });
            by_id.insert(id, name.to_string());
        }
        
        Ok((id, channel))
    }
    
    /// Set service state.
    pub fn set_state(&self, id: ServiceId, state: ServiceState) -> Result<(), ServiceError> {
        let by_id = self.by_id.read();
        let name = by_id.get(&id)
            .ok_or_else(|| ServiceError::NotFound(format!("ID: {:?}", id)))?
            .clone();
        drop(by_id);
        
        let mut by_name = self.by_name.write();
        if let Some(service) = by_name.get_mut(&name) {
            service.info.state = state;
            Ok(())
        } else {
            Err(ServiceError::NotFound(name))
        }
    }
    
    /// Unregister a service.
    pub fn unregister(&self, id: ServiceId) -> Result<(), ServiceError> {
        let by_id = self.by_id.read();
        let name = by_id.get(&id)
            .ok_or_else(|| ServiceError::NotFound(format!("ID: {:?}", id)))?
            .clone();
        drop(by_id);
        
        let mut by_name = self.by_name.write();
        let mut by_id = self.by_id.write();
        
        by_name.remove(&name);
        by_id.remove(&id);
        
        Ok(())
    }
    
    /// Lookup service by name.
    pub fn lookup(&self, name: &str) -> Option<ServiceInfo> {
        let by_name = self.by_name.read();
        by_name.get(name).map(|s| s.info.clone())
    }
    
    /// Lookup service by ID.
    pub fn lookup_by_id(&self, id: ServiceId) -> Option<ServiceInfo> {
        let by_id = self.by_id.read();
        let name = by_id.get(&id)?;
        
        let by_name = self.by_name.read();
        by_name.get(name).map(|s| s.info.clone())
    }
    
    /// Connect to a service.
    pub fn connect(
        &self,
        name: &str,
        client_capability: Capability,
    ) -> Result<ServiceConnection, ServiceError> {
        let mut by_name = self.by_name.write();
        let service = by_name.get_mut(name)
            .ok_or_else(|| ServiceError::NotFound(name.to_string()))?;
        
        // Check state
        if service.info.state != ServiceState::Running {
            return Err(ServiceError::NotRunning);
        }
        
        // Check capacity
        if !service.info.can_accept_connection() {
            return Err(ServiceError::TooManyConnections);
        }
        
        // Check permissions
        if !client_capability.rights().contains(service.info.required_rights) {
            return Err(ServiceError::PermissionDenied);
        }
        
        // Create connection
        let connection_id = {
            let mut next = self.next_connection_id.lock();
            let id = *next;
            *next += 1;
            id
        };
        
        // Create client channel (connected to server's accept channel)
        let client_channel_id = ChannelId(connection_id);
        let server_channel_id = ChannelId(service.info.id.0);
        let channel = Arc::new(Channel::new(client_channel_id, server_channel_id));
        
        // Update connection count
        service.info.current_connections += 1;
        service.connections.push(connection_id);
        
        Ok(ServiceConnection {
            service_id: service.info.id,
            capability: client_capability,
            channel,
            connection_id,
        })
    }
    
    /// Disconnect from a service.
    pub fn disconnect(&self, service_id: ServiceId, connection_id: u64) -> Result<(), ServiceError> {
        let by_id = self.by_id.read();
        let name = by_id.get(&service_id)
            .ok_or_else(|| ServiceError::NotFound(format!("ID: {:?}", service_id)))?
            .clone();
        drop(by_id);
        
        let mut by_name = self.by_name.write();
        if let Some(service) = by_name.get_mut(&name) {
            if let Some(pos) = service.connections.iter().position(|&id| id == connection_id) {
                service.connections.remove(pos);
                service.info.current_connections = 
                    service.info.current_connections.saturating_sub(1);
            }
            Ok(())
        } else {
            Err(ServiceError::NotFound(name))
        }
    }
    
    /// List all registered services.
    pub fn list_services(&self) -> Vec<ServiceInfo> {
        let by_name = self.by_name.read();
        by_name.values().map(|s| s.info.clone()).collect()
    }
    
    /// List services matching a prefix.
    pub fn list_by_prefix(&self, prefix: &str) -> Vec<ServiceInfo> {
        let by_name = self.by_name.read();
        by_name.iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(_, s)| s.info.clone())
            .collect()
    }
    
    /// Validate service name.
    fn validate_name(name: &str) -> bool {
        // Names must be:
        // - Non-empty
        // - Only contain alphanumeric, '.', '_', '-'
        // - Not start with '.'
        // - Max 255 characters
        
        if name.is_empty() || name.len() > 255 {
            return false;
        }
        
        if name.starts_with('.') {
            return false;
        }
        
        name.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-')
    }
}

/// Global service registry instance.
static GLOBAL_REGISTRY: ServiceRegistry = ServiceRegistry::new();

/// Get the global service registry.
pub fn global_registry() -> &'static ServiceRegistry {
    &GLOBAL_REGISTRY
}

/// Register a system service (convenience function).
pub fn register_service(
    name: &str,
    description: &str,
    owner_pid: u64,
) -> Result<(ServiceId, Arc<Channel>), ServiceError> {
    GLOBAL_REGISTRY.register(
        name,
        description,
        owner_pid,
        CapabilityRights::CONNECT,
    )
}

/// Start a registered service.
pub fn start_service(id: ServiceId) -> Result<(), ServiceError> {
    GLOBAL_REGISTRY.set_state(id, ServiceState::Running)
}

/// Stop a registered service.
pub fn stop_service(id: ServiceId) -> Result<(), ServiceError> {
    GLOBAL_REGISTRY.set_state(id, ServiceState::Stopped)
}

/// Lookup a service by name.
pub fn lookup_service(name: &str) -> Option<ServiceInfo> {
    GLOBAL_REGISTRY.lookup(name)
}

/// Connect to a service.
pub fn connect_to_service(
    name: &str,
    capability: Capability,
) -> Result<ServiceConnection, ServiceError> {
    GLOBAL_REGISTRY.connect(name, capability)
}

/// Service builder for fluent API.
pub struct ServiceBuilder {
    name: String,
    description: String,
    owner_pid: u64,
    required_rights: CapabilityRights,
    max_connections: u32,
}

impl ServiceBuilder {
    /// Create new service builder.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            owner_pid: 0,
            required_rights: CapabilityRights::CONNECT,
            max_connections: 256,
        }
    }
    
    /// Set description.
    pub fn description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }
    
    /// Set owner PID.
    pub fn owner(mut self, pid: u64) -> Self {
        self.owner_pid = pid;
        self
    }
    
    /// Set required rights.
    pub fn required_rights(mut self, rights: CapabilityRights) -> Self {
        self.required_rights = rights;
        self
    }
    
    /// Set max connections.
    pub fn max_connections(mut self, max: u32) -> Self {
        self.max_connections = max;
        self
    }
    
    /// Register the service.
    pub fn register(self) -> Result<(ServiceId, Arc<Channel>), ServiceError> {
        GLOBAL_REGISTRY.register(
            &self.name,
            &self.description,
            self.owner_pid,
            self.required_rights,
        )
    }
}

/// Request types for service communication.
#[derive(Debug, Clone)]
pub enum ServiceRequest {
    /// Ping request (health check).
    Ping,
    /// Get service info.
    GetInfo,
    /// Custom request with type ID.
    Custom { request_type: u32, payload: Vec<u8> },
}

/// Response types for service communication.
#[derive(Debug, Clone)]
pub enum ServiceResponse {
    /// Pong response.
    Pong,
    /// Service info response.
    Info(ServiceInfo),
    /// Success with optional payload.
    Success(Vec<u8>),
    /// Error response.
    Error { code: u32, message: String },
}
