//! Security sandbox for WASM execution.
//!
//! This module implements the security sandbox that isolates
//! WASM instances from each other and controls their access
//! to system resources.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use bitflags::bitflags;

use crate::RuntimeError;

/// Sandbox configuration.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Memory limit in bytes.
    pub memory_limit: usize,
    
    /// CPU time limit in nanoseconds (0 = unlimited).
    pub cpu_time_limit: u64,
    
    /// Maximum number of open file descriptors.
    pub max_fds: u32,
    
    /// Maximum number of spawned processes.
    pub max_processes: u32,
    
    /// Allowed filesystem paths.
    pub allowed_paths: Vec<PathPermission>,
    
    /// Allowed network access.
    pub network_permissions: NetworkPermissions,
    
    /// Allowed capabilities.
    pub capabilities: BTreeSet<u64>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        SandboxConfig {
            memory_limit: 64 * 1024 * 1024, // 64 MB
            cpu_time_limit: 0, // Unlimited
            max_fds: 64,
            max_processes: 0, // No process spawning
            allowed_paths: Vec::new(),
            network_permissions: NetworkPermissions::empty(),
            capabilities: BTreeSet::new(),
        }
    }
}

/// Path permission.
#[derive(Debug, Clone)]
pub struct PathPermission {
    /// Path prefix.
    pub path: String,
    
    /// Allowed operations.
    pub permissions: FilePermissions,
}

bitflags! {
    /// File permissions.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FilePermissions: u32 {
        /// Read files.
        const READ = 0b0001;
        /// Write files.
        const WRITE = 0b0010;
        /// Create files.
        const CREATE = 0b0100;
        /// Delete files.
        const DELETE = 0b1000;
        /// List directories.
        const LIST = 0b10000;
        /// All permissions.
        const ALL = 0b11111;
    }
}

bitflags! {
    /// Network permissions.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NetworkPermissions: u32 {
        /// Create TCP connections.
        const TCP_CONNECT = 0b0001;
        /// Listen for TCP connections.
        const TCP_LISTEN = 0b0010;
        /// Send/receive UDP datagrams.
        const UDP = 0b0100;
        /// DNS resolution.
        const DNS = 0b1000;
        /// All network permissions.
        const ALL = 0b1111;
    }
}

/// A security sandbox for a WASM instance.
pub struct Sandbox {
    /// Sandbox configuration.
    config: SandboxConfig,
    
    /// Current memory usage.
    memory_used: usize,
    
    /// Current CPU time used.
    cpu_time_used: u64,
    
    /// Number of open file descriptors.
    open_fds: u32,
    
    /// Number of spawned processes.
    spawned_processes: u32,
    
    /// Security violations log.
    violations: Vec<SecurityViolation>,
}

impl Sandbox {
    /// Create a new sandbox with the given configuration.
    pub fn new(config: SandboxConfig) -> Self {
        Sandbox {
            config,
            memory_used: 0,
            cpu_time_used: 0,
            open_fds: 0,
            spawned_processes: 0,
            violations: Vec::new(),
        }
    }
    
    /// Check if memory allocation is allowed.
    pub fn check_memory_alloc(&mut self, size: usize) -> Result<(), RuntimeError> {
        let new_usage = self.memory_used.saturating_add(size);
        
        if new_usage > self.config.memory_limit {
            self.log_violation(SecurityViolation::MemoryLimitExceeded {
                requested: size,
                limit: self.config.memory_limit,
            });
            return Err(RuntimeError::ResourceLimit(
                "Memory limit exceeded".into()
            ));
        }
        
        self.memory_used = new_usage;
        Ok(())
    }
    
    /// Release memory.
    pub fn release_memory(&mut self, size: usize) {
        self.memory_used = self.memory_used.saturating_sub(size);
    }
    
    /// Add CPU time used.
    pub fn add_cpu_time(&mut self, nanoseconds: u64) -> Result<(), RuntimeError> {
        self.cpu_time_used = self.cpu_time_used.saturating_add(nanoseconds);
        
        if self.config.cpu_time_limit > 0 && self.cpu_time_used > self.config.cpu_time_limit {
            self.log_violation(SecurityViolation::CpuTimeLimitExceeded {
                used: self.cpu_time_used,
                limit: self.config.cpu_time_limit,
            });
            return Err(RuntimeError::ResourceLimit(
                "CPU time limit exceeded".into()
            ));
        }
        
        Ok(())
    }
    
    /// Check if opening a file descriptor is allowed.
    pub fn check_open_fd(&mut self) -> Result<(), RuntimeError> {
        if self.open_fds >= self.config.max_fds {
            self.log_violation(SecurityViolation::FdLimitExceeded {
                limit: self.config.max_fds,
            });
            return Err(RuntimeError::ResourceLimit(
                "File descriptor limit exceeded".into()
            ));
        }
        
        self.open_fds += 1;
        Ok(())
    }
    
    /// Release a file descriptor.
    pub fn release_fd(&mut self) {
        self.open_fds = self.open_fds.saturating_sub(1);
    }
    
    /// Check if file access is allowed.
    pub fn check_file_access(&mut self, path: &str, permissions: FilePermissions) -> Result<(), RuntimeError> {
        for allowed in &self.config.allowed_paths {
            if path.starts_with(&allowed.path) && allowed.permissions.contains(permissions) {
                return Ok(());
            }
        }
        
        self.log_violation(SecurityViolation::FileAccessDenied {
            path: path.into(),
            permissions,
        });
        
        Err(RuntimeError::PermissionDenied(
            alloc::format!("File access denied: {}", path)
        ))
    }
    
    /// Check if network access is allowed.
    pub fn check_network_access(&mut self, permission: NetworkPermissions) -> Result<(), RuntimeError> {
        if !self.config.network_permissions.contains(permission) {
            self.log_violation(SecurityViolation::NetworkAccessDenied {
                permission,
            });
            return Err(RuntimeError::PermissionDenied(
                "Network access denied".into()
            ));
        }
        
        Ok(())
    }
    
    /// Check if a capability is held.
    pub fn check_capability(&mut self, cap_id: u64) -> Result<(), RuntimeError> {
        if !self.config.capabilities.contains(&cap_id) {
            self.log_violation(SecurityViolation::CapabilityDenied {
                capability: cap_id,
            });
            return Err(RuntimeError::PermissionDenied(
                "Capability not held".into()
            ));
        }
        
        Ok(())
    }
    
    /// Grant a capability to this sandbox.
    pub fn grant_capability(&mut self, cap_id: u64) {
        self.config.capabilities.insert(cap_id);
    }
    
    /// Revoke a capability from this sandbox.
    pub fn revoke_capability(&mut self, cap_id: u64) {
        self.config.capabilities.remove(&cap_id);
    }
    
    /// Log a security violation.
    fn log_violation(&mut self, violation: SecurityViolation) {
        self.violations.push(violation);
    }
    
    /// Get all logged violations.
    pub fn violations(&self) -> &[SecurityViolation] {
        &self.violations
    }
    
    /// Get current memory usage.
    pub fn memory_used(&self) -> usize {
        self.memory_used
    }
    
    /// Get current CPU time used.
    pub fn cpu_time_used(&self) -> u64 {
        self.cpu_time_used
    }
}

/// Security violation types.
#[derive(Debug, Clone)]
pub enum SecurityViolation {
    /// Memory limit exceeded.
    MemoryLimitExceeded {
        requested: usize,
        limit: usize,
    },
    /// CPU time limit exceeded.
    CpuTimeLimitExceeded {
        used: u64,
        limit: u64,
    },
    /// File descriptor limit exceeded.
    FdLimitExceeded {
        limit: u32,
    },
    /// File access denied.
    FileAccessDenied {
        path: String,
        permissions: FilePermissions,
    },
    /// Network access denied.
    NetworkAccessDenied {
        permission: NetworkPermissions,
    },
    /// Capability denied.
    CapabilityDenied {
        capability: u64,
    },
}
