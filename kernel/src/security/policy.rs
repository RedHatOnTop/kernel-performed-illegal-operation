//! Security Policy Management
//!
//! This module defines security policies that control what browser
//! processes and tabs can do.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use spin::RwLock;

use crate::browser::coordinator::TabId;
use crate::ipc::{CapabilityId, CapabilityType, CapabilityRights};

/// Security domain (isolation boundary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DomainId(pub u32);

impl DomainId {
    /// Kernel domain (trusted).
    pub const KERNEL: DomainId = DomainId(0);
    /// System services domain.
    pub const SYSTEM: DomainId = DomainId(1);
    /// Browser coordinator domain.
    pub const BROWSER_COORD: DomainId = DomainId(2);
    /// Renderer domain (untrusted web content).
    pub const RENDERER: DomainId = DomainId(100);
}

/// Permission for a specific operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// Allow the operation.
    Allow,
    /// Deny the operation.
    Deny,
    /// Prompt user for decision.
    Prompt,
    /// Allow with audit logging.
    AllowWithAudit,
}

/// Network access policy.
#[derive(Debug, Clone)]
pub struct NetworkPolicy {
    /// Allow any network access.
    pub allow_any: bool,
    /// Allowed hosts (wildcard patterns).
    pub allowed_hosts: Vec<String>,
    /// Blocked hosts.
    pub blocked_hosts: Vec<String>,
    /// Allow localhost.
    pub allow_localhost: bool,
    /// Maximum connections.
    pub max_connections: u32,
    /// Maximum bandwidth (bytes per second, 0 = unlimited).
    pub bandwidth_limit: u64,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        NetworkPolicy {
            allow_any: false,
            allowed_hosts: Vec::new(),
            blocked_hosts: Vec::new(),
            allow_localhost: true,
            max_connections: 16,
            bandwidth_limit: 0,
        }
    }
}

/// File system access policy.
#[derive(Debug, Clone)]
pub struct FileSystemPolicy {
    /// Allow any file access.
    pub allow_any: bool,
    /// Allowed paths (prefix match).
    pub allowed_paths: Vec<String>,
    /// Read-only paths.
    pub readonly_paths: Vec<String>,
    /// Denied paths.
    pub denied_paths: Vec<String>,
    /// Maximum file size.
    pub max_file_size: u64,
    /// Maximum total storage.
    pub max_storage: u64,
}

impl Default for FileSystemPolicy {
    fn default() -> Self {
        FileSystemPolicy {
            allow_any: false,
            allowed_paths: Vec::new(),
            readonly_paths: Vec::new(),
            denied_paths: Vec::new(),
            max_file_size: 100 * 1024 * 1024,  // 100MB
            max_storage: 1024 * 1024 * 1024,    // 1GB
        }
    }
}

/// GPU access policy.
#[derive(Debug, Clone, Copy)]
pub struct GpuPolicy {
    /// Allow GPU access.
    pub allow: bool,
    /// Maximum GPU memory.
    pub max_memory: u64,
    /// Allow WebGL.
    pub allow_webgl: bool,
    /// Allow compute shaders.
    pub allow_compute: bool,
}

impl Default for GpuPolicy {
    fn default() -> Self {
        GpuPolicy {
            allow: true,
            max_memory: 256 * 1024 * 1024,  // 256MB
            allow_webgl: true,
            allow_compute: false,
        }
    }
}

/// IPC policy.
#[derive(Debug, Clone)]
pub struct IpcPolicy {
    /// Allowed channel destinations.
    pub allowed_destinations: Vec<DomainId>,
    /// Maximum message size.
    pub max_message_size: usize,
    /// Maximum pending messages.
    pub max_pending: usize,
    /// Can create channels.
    pub can_create_channels: bool,
    /// Can share memory.
    pub can_share_memory: bool,
}

impl Default for IpcPolicy {
    fn default() -> Self {
        IpcPolicy {
            allowed_destinations: Vec::new(),
            max_message_size: 64 * 1024,
            max_pending: 256,
            can_create_channels: true,
            can_share_memory: true,
        }
    }
}

/// Complete security policy for a domain/process.
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    /// Domain ID.
    pub domain: DomainId,
    
    /// Policy name.
    pub name: String,
    
    /// Network policy.
    pub network: NetworkPolicy,
    
    /// File system policy.
    pub filesystem: FileSystemPolicy,
    
    /// GPU policy.
    pub gpu: GpuPolicy,
    
    /// IPC policy.
    pub ipc: IpcPolicy,
    
    /// Can spawn child processes.
    pub can_spawn: bool,
    
    /// Can access clipboard.
    pub clipboard_access: Permission,
    
    /// Can access geolocation.
    pub geolocation_access: Permission,
    
    /// Can access camera.
    pub camera_access: Permission,
    
    /// Can access microphone.
    pub microphone_access: Permission,
    
    /// Can show notifications.
    pub notification_access: Permission,
    
    /// Maximum CPU time per second (milliseconds, 0 = unlimited).
    pub cpu_time_limit: u32,
    
    /// Priority ceiling.
    pub max_priority: u32,
}

impl SecurityPolicy {
    /// Create a new restrictive policy.
    pub fn new_restrictive(name: &str, domain: DomainId) -> Self {
        SecurityPolicy {
            domain,
            name: String::from(name),
            network: NetworkPolicy::default(),
            filesystem: FileSystemPolicy::default(),
            gpu: GpuPolicy::default(),
            ipc: IpcPolicy::default(),
            can_spawn: false,
            clipboard_access: Permission::Prompt,
            geolocation_access: Permission::Deny,
            camera_access: Permission::Deny,
            microphone_access: Permission::Deny,
            notification_access: Permission::Prompt,
            cpu_time_limit: 900,  // 900ms per second
            max_priority: 16,
        }
    }
    
    /// Create a permissive policy (for trusted processes).
    pub fn new_permissive(name: &str, domain: DomainId) -> Self {
        SecurityPolicy {
            domain,
            name: String::from(name),
            network: NetworkPolicy { allow_any: true, ..Default::default() },
            filesystem: FileSystemPolicy { allow_any: true, ..Default::default() },
            gpu: GpuPolicy { allow: true, allow_compute: true, ..Default::default() },
            ipc: IpcPolicy {
                allowed_destinations: alloc::vec![
                    DomainId::KERNEL,
                    DomainId::SYSTEM,
                    DomainId::BROWSER_COORD,
                ],
                can_create_channels: true,
                can_share_memory: true,
                ..Default::default()
            },
            can_spawn: true,
            clipboard_access: Permission::Allow,
            geolocation_access: Permission::Allow,
            camera_access: Permission::Allow,
            microphone_access: Permission::Allow,
            notification_access: Permission::Allow,
            cpu_time_limit: 0,
            max_priority: 24,
        }
    }
    
    /// Create renderer policy (web content).
    pub fn new_renderer(name: &str) -> Self {
        let mut policy = Self::new_restrictive(name, DomainId::RENDERER);
        
        // Renderers can talk to coordinator only
        policy.ipc.allowed_destinations = alloc::vec![DomainId::BROWSER_COORD];
        policy.gpu.allow = true;
        policy.gpu.allow_webgl = true;
        
        policy
    }
    
    /// Check if network access is allowed.
    pub fn check_network(&self, host: &str) -> Permission {
        // Check blocked hosts first
        for blocked in &self.network.blocked_hosts {
            if host_matches(host, blocked) {
                return Permission::Deny;
            }
        }
        
        // Check localhost
        if (host == "localhost" || host == "127.0.0.1" || host == "::1")
            && self.network.allow_localhost {
            return Permission::Allow;
        }
        
        // Check allowed hosts
        if self.network.allow_any {
            return Permission::AllowWithAudit;
        }
        
        for allowed in &self.network.allowed_hosts {
            if host_matches(host, allowed) {
                return Permission::Allow;
            }
        }
        
        Permission::Deny
    }
    
    /// Check if file access is allowed.
    pub fn check_file(&self, path: &str, write: bool) -> Permission {
        // Check denied paths first
        for denied in &self.filesystem.denied_paths {
            if path.starts_with(denied) {
                return Permission::Deny;
            }
        }
        
        // Check readonly paths for writes
        if write {
            for readonly in &self.filesystem.readonly_paths {
                if path.starts_with(readonly) {
                    return Permission::Deny;
                }
            }
        }
        
        // Check allowed paths
        if self.filesystem.allow_any {
            return Permission::AllowWithAudit;
        }
        
        for allowed in &self.filesystem.allowed_paths {
            if path.starts_with(allowed) {
                return Permission::Allow;
            }
        }
        
        Permission::Deny
    }
    
    /// Check if IPC to destination is allowed.
    pub fn check_ipc(&self, dest_domain: DomainId) -> Permission {
        if self.ipc.allowed_destinations.contains(&dest_domain) {
            Permission::Allow
        } else {
            Permission::Deny
        }
    }
}

/// Simple host pattern matching.
fn host_matches(host: &str, pattern: &str) -> bool {
    if pattern.starts_with("*.") {
        let suffix = &pattern[1..];  // Keep the dot
        host.ends_with(suffix)
    } else {
        host == pattern
    }
}

/// Policy error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    /// Policy not found.
    NotFound,
    /// Access denied.
    AccessDenied,
    /// Invalid policy configuration.
    InvalidPolicy,
    /// Policy already exists.
    AlreadyExists,
}

/// Policy manager.
pub struct PolicyManager {
    /// Policies by domain.
    policies: BTreeMap<DomainId, SecurityPolicy>,
    /// Tab-to-domain mapping.
    tab_domains: BTreeMap<TabId, DomainId>,
    /// Capability validations.
    capability_policies: BTreeMap<CapabilityId, CapabilityPolicy>,
}

/// Per-capability policy.
#[derive(Debug, Clone)]
pub struct CapabilityPolicy {
    /// Allowed rights.
    pub allowed_rights: CapabilityRights,
    /// Expiration (ticks since boot, 0 = never).
    pub expires_at: u64,
    /// Use count limit (0 = unlimited).
    pub use_limit: u32,
    /// Current use count.
    pub use_count: u32,
    /// Audit usage.
    pub audit: bool,
}

impl PolicyManager {
    /// Create new policy manager.
    pub fn new() -> Self {
        let mut manager = PolicyManager {
            policies: BTreeMap::new(),
            tab_domains: BTreeMap::new(),
            capability_policies: BTreeMap::new(),
        };
        
        // Register built-in policies
        manager.register_policy(SecurityPolicy::new_permissive(
            "kernel",
            DomainId::KERNEL,
        ));
        manager.register_policy(SecurityPolicy::new_permissive(
            "system",
            DomainId::SYSTEM,
        ));
        manager.register_policy(SecurityPolicy::new_permissive(
            "browser_coordinator",
            DomainId::BROWSER_COORD,
        ));
        
        manager
    }
    
    /// Register a policy.
    pub fn register_policy(&mut self, policy: SecurityPolicy) {
        crate::serial_println!(
            "[Security] Registered policy '{}' for domain {}",
            policy.name, policy.domain.0
        );
        self.policies.insert(policy.domain, policy);
    }
    
    /// Get policy for domain.
    pub fn get_policy(&self, domain: DomainId) -> Option<&SecurityPolicy> {
        self.policies.get(&domain)
    }
    
    /// Assign tab to domain.
    pub fn assign_tab(&mut self, tab: TabId, domain: DomainId) {
        self.tab_domains.insert(tab, domain);
    }
    
    /// Get domain for tab.
    pub fn tab_domain(&self, tab: TabId) -> Option<DomainId> {
        self.tab_domains.get(&tab).copied()
    }
    
    /// Check permission for tab.
    pub fn check_tab_network(&self, tab: TabId, host: &str) -> Permission {
        self.tab_domains.get(&tab)
            .and_then(|d| self.policies.get(d))
            .map(|p| p.check_network(host))
            .unwrap_or(Permission::Deny)
    }
    
    /// Check file permission for tab.
    pub fn check_tab_file(&self, tab: TabId, path: &str, write: bool) -> Permission {
        self.tab_domains.get(&tab)
            .and_then(|d| self.policies.get(d))
            .map(|p| p.check_file(path, write))
            .unwrap_or(Permission::Deny)
    }
    
    /// Check capability usage.
    pub fn check_capability(
        &mut self,
        cap_id: CapabilityId,
        rights: CapabilityRights,
    ) -> Result<(), PolicyError> {
        let policy = self.capability_policies.get_mut(&cap_id)
            .ok_or(PolicyError::NotFound)?;
        
        // Check rights
        if !policy.allowed_rights.contains(rights) {
            return Err(PolicyError::AccessDenied);
        }
        
        // Check use limit
        if policy.use_limit > 0 && policy.use_count >= policy.use_limit {
            return Err(PolicyError::AccessDenied);
        }
        
        // Increment use count
        policy.use_count += 1;
        
        Ok(())
    }
}

/// Global policy manager.
static POLICY_MANAGER: RwLock<Option<PolicyManager>> = RwLock::new(None);

/// Initialize policy manager.
pub fn init() {
    let mut mgr = POLICY_MANAGER.write();
    *mgr = Some(PolicyManager::new());
    crate::serial_println!("[Security] Policy manager initialized");
}

/// Get policy for domain.
pub fn get_policy(domain: DomainId) -> Option<SecurityPolicy> {
    POLICY_MANAGER.read().as_ref()?.get_policy(domain).cloned()
}

/// Assign tab to domain.
pub fn assign_tab_domain(tab: TabId, domain: DomainId) {
    if let Some(mgr) = POLICY_MANAGER.write().as_mut() {
        mgr.assign_tab(tab, domain);
    }
}

/// Check network permission.
pub fn check_network(tab: TabId, host: &str) -> Permission {
    POLICY_MANAGER.read()
        .as_ref()
        .map(|m| m.check_tab_network(tab, host))
        .unwrap_or(Permission::Deny)
}
