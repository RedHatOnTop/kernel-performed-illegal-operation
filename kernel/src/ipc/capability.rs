//! Capability-based security for IPC.
//!
//! This module implements the capability system that controls
//! access to kernel resources and IPC channels.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use bitflags::bitflags;
use core::sync::atomic::{AtomicU64, Ordering};

/// Unique capability identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityId(pub u64);

impl CapabilityId {
    /// The null capability (no permissions).
    pub const NULL: CapabilityId = CapabilityId(0);
}

/// Capability type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityType {
    /// Channel endpoint capability.
    Channel,
    /// Memory region capability.
    Memory,
    /// Device capability.
    Device,
    /// File/directory capability.
    File,
    /// Process capability.
    Process,
    /// Network capability.
    Network,
    /// Graphics capability.
    Graphics,
}

bitflags! {
    /// Capability rights.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CapabilityRights: u32 {
        /// Read permission.
        const READ = 0b00000001;
        /// Write permission.
        const WRITE = 0b00000010;
        /// Execute permission.
        const EXECUTE = 0b00000100;
        /// Can derive new capabilities.
        const DERIVE = 0b00001000;
        /// Can revoke this capability.
        const REVOKE = 0b00010000;
        /// Can transfer to other processes.
        const TRANSFER = 0b00100000;
        /// Exclusive access.
        const EXCLUSIVE = 0b01000000;
        /// Connect to service permission.
        const CONNECT = 0b10000000;
        /// All rights.
        const ALL = 0b11111111;
    }
}

/// A capability granting access to a resource.
#[derive(Debug, Clone)]
pub struct Capability {
    /// Unique ID.
    id: CapabilityId,

    /// Capability type.
    cap_type: CapabilityType,

    /// Rights granted.
    rights: CapabilityRights,

    /// Resource identifier (meaning depends on type).
    resource_id: u64,

    /// Owner process ID.
    owner: u64,

    /// Child capabilities derived from this one.
    children: Vec<CapabilityId>,

    /// Parent capability (if derived).
    parent: Option<CapabilityId>,

    /// Creation timestamp.
    created_at: u64,

    /// Whether the capability is revoked.
    revoked: bool,
}

impl Capability {
    /// Create a new capability.
    pub fn new(id: CapabilityId, cap_type: CapabilityType) -> Self {
        Capability {
            id,
            cap_type,
            rights: CapabilityRights::empty(),
            resource_id: 0,
            owner: 0,
            children: Vec::new(),
            parent: None,
            created_at: 0,
            revoked: false,
        }
    }

    /// Create a capability with full rights.
    pub fn new_full(
        id: CapabilityId,
        cap_type: CapabilityType,
        resource_id: u64,
        owner: u64,
    ) -> Self {
        Capability {
            id,
            cap_type,
            rights: CapabilityRights::ALL,
            resource_id,
            owner,
            children: Vec::new(),
            parent: None,
            created_at: 0,
            revoked: false,
        }
    }

    /// Get the capability ID.
    pub fn id(&self) -> CapabilityId {
        self.id
    }

    /// Get the capability type.
    pub fn cap_type(&self) -> CapabilityType {
        self.cap_type
    }

    /// Get the rights.
    pub fn rights(&self) -> CapabilityRights {
        self.rights
    }

    /// Set the rights.
    pub fn set_rights(&mut self, rights: CapabilityRights) {
        self.rights = rights;
    }

    /// Get the resource ID.
    pub fn resource_id(&self) -> u64 {
        self.resource_id
    }

    /// Set the resource ID.
    pub fn set_resource_id(&mut self, id: u64) {
        self.resource_id = id;
    }

    /// Get the owner process ID.
    pub fn owner(&self) -> u64 {
        self.owner
    }

    /// Set the owner.
    pub fn set_owner(&mut self, owner: u64) {
        self.owner = owner;
    }

    /// Check if the capability has a specific right.
    pub fn has_right(&self, right: CapabilityRights) -> bool {
        self.rights.contains(right)
    }

    /// Check if the capability is valid (not revoked).
    pub fn is_valid(&self) -> bool {
        !self.revoked
    }

    /// Revoke this capability.
    pub fn revoke(&mut self) {
        self.revoked = true;
    }

    /// Derive a new capability with restricted rights.
    pub fn derive(&self, new_id: CapabilityId, rights: CapabilityRights) -> Option<Capability> {
        if !self.has_right(CapabilityRights::DERIVE) {
            return None;
        }

        // New capability can only have rights that we have
        let new_rights = self.rights & rights;

        let mut new_cap = Capability::new(new_id, self.cap_type);
        new_cap.rights = new_rights;
        new_cap.resource_id = self.resource_id;
        new_cap.parent = Some(self.id);

        Some(new_cap)
    }

    /// Add a child capability.
    pub fn add_child(&mut self, child_id: CapabilityId) {
        self.children.push(child_id);
    }

    /// Get child capabilities.
    pub fn children(&self) -> &[CapabilityId] {
        &self.children
    }

    /// Get the parent capability.
    pub fn parent(&self) -> Option<CapabilityId> {
        self.parent
    }
}

/// Capability space for a process.
#[derive(Debug)]
pub struct CapabilitySpace {
    /// Process ID.
    process_id: u64,

    /// Held capabilities.
    capabilities: BTreeSet<CapabilityId>,
}

impl CapabilitySpace {
    /// Create a new capability space.
    pub fn new(process_id: u64) -> Self {
        CapabilitySpace {
            process_id,
            capabilities: BTreeSet::new(),
        }
    }

    /// Add a capability to this space.
    pub fn add(&mut self, cap_id: CapabilityId) {
        self.capabilities.insert(cap_id);
    }

    /// Remove a capability from this space.
    pub fn remove(&mut self, cap_id: CapabilityId) -> bool {
        self.capabilities.remove(&cap_id)
    }

    /// Check if this space contains a capability.
    pub fn contains(&self, cap_id: CapabilityId) -> bool {
        self.capabilities.contains(&cap_id)
    }

    /// Get all capabilities.
    pub fn all(&self) -> impl Iterator<Item = &CapabilityId> {
        self.capabilities.iter()
    }

    /// Get the number of capabilities.
    pub fn count(&self) -> usize {
        self.capabilities.len()
    }
}
