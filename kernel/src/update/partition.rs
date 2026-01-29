//! A/B Partition Management
//!
//! Manages dual boot partitions for reliable updates.

use alloc::string::String;
use super::UpdateError;

/// Partition slot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionSlot {
    /// Slot A
    A,
    /// Slot B
    B,
}

impl PartitionSlot {
    /// Get other slot
    pub fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }

    /// Get slot name
    pub fn name(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
        }
    }
}

/// Partition status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionStatus {
    /// Active - currently booted from
    Active,
    /// Successful - verified working
    Successful,
    /// Unbootable - failed to boot
    Unbootable,
    /// Pending - waiting to be tested
    Pending,
}

/// Partition info
#[derive(Debug, Clone)]
pub struct PartitionInfo {
    /// Slot
    pub slot: PartitionSlot,
    /// Status
    pub status: PartitionStatus,
    /// Version installed
    pub version: String,
    /// Boot attempts
    pub boot_attempts: u32,
    /// Max boot attempts
    pub max_attempts: u32,
    /// Size in bytes
    pub size: u64,
    /// Used space
    pub used: u64,
    /// Verified
    pub verified: bool,
}

impl PartitionInfo {
    /// Create new partition info
    pub fn new(slot: PartitionSlot, size: u64) -> Self {
        Self {
            slot,
            status: PartitionStatus::Successful,
            version: String::new(),
            boot_attempts: 0,
            max_attempts: 3,
            size,
            used: 0,
            verified: false,
        }
    }

    /// Check if bootable
    pub fn is_bootable(&self) -> bool {
        match self.status {
            PartitionStatus::Active | PartitionStatus::Successful | PartitionStatus::Pending => true,
            PartitionStatus::Unbootable => false,
        }
    }

    /// Available space
    pub fn available(&self) -> u64 {
        self.size.saturating_sub(self.used)
    }
}

/// Partition manager
pub struct PartitionManager {
    /// Active slot
    pub active: PartitionSlot,
    /// Pending slot (if any)
    pub pending: Option<PartitionSlot>,
    /// Partition slots
    pub slots: [Option<PartitionInfo>; 2],
}

impl PartitionManager {
    /// Create new partition manager
    pub const fn new() -> Self {
        Self {
            active: PartitionSlot::A,
            pending: None,
            slots: [None, None],
        }
    }

    /// Initialize partitions
    pub fn init(&mut self, slot_a_size: u64, slot_b_size: u64) {
        let mut info_a = PartitionInfo::new(PartitionSlot::A, slot_a_size);
        info_a.status = PartitionStatus::Active;
        
        let info_b = PartitionInfo::new(PartitionSlot::B, slot_b_size);

        self.slots[0] = Some(info_a);
        self.slots[1] = Some(info_b);
    }

    /// Get active partition
    pub fn active_partition(&self) -> Option<&PartitionInfo> {
        match self.active {
            PartitionSlot::A => self.slots[0].as_ref(),
            PartitionSlot::B => self.slots[1].as_ref(),
        }
    }

    /// Get inactive partition
    pub fn inactive_partition(&self) -> Option<PartitionSlot> {
        Some(self.active.other())
    }

    /// Get partition info
    pub fn get_partition(&self, slot: PartitionSlot) -> Option<&PartitionInfo> {
        match slot {
            PartitionSlot::A => self.slots[0].as_ref(),
            PartitionSlot::B => self.slots[1].as_ref(),
        }
    }

    /// Get partition info mut
    pub fn get_partition_mut(&mut self, slot: PartitionSlot) -> Option<&mut PartitionInfo> {
        match slot {
            PartitionSlot::A => self.slots[0].as_mut(),
            PartitionSlot::B => self.slots[1].as_mut(),
        }
    }

    /// Check if has enough space
    pub fn has_space(&self, required: u64) -> bool {
        if let Some(slot) = self.inactive_partition() {
            if let Some(info) = self.get_partition(slot) {
                return info.available() >= required;
            }
        }
        false
    }

    /// Set partition as pending (ready to boot)
    pub fn set_pending(&mut self, slot: PartitionSlot) -> Result<(), UpdateError> {
        if let Some(info) = self.get_partition_mut(slot) {
            info.status = PartitionStatus::Pending;
            info.boot_attempts = 0;
            self.pending = Some(slot);
            Ok(())
        } else {
            Err(UpdateError::PartitionError("Partition not found".into()))
        }
    }

    /// Commit pending partition (make it active)
    pub fn commit_pending(&mut self) -> Result<(), UpdateError> {
        let pending = self.pending.ok_or_else(|| 
            UpdateError::PartitionError("No pending partition".into()))?;

        // Mark old active as successful (fallback)
        if let Some(info) = self.get_partition_mut(self.active) {
            info.status = PartitionStatus::Successful;
        }

        // Mark pending as active
        if let Some(info) = self.get_partition_mut(pending) {
            info.status = PartitionStatus::Active;
            info.boot_attempts += 1;
        }

        self.active = pending;
        self.pending = None;

        Ok(())
    }

    /// Mark boot as successful
    pub fn mark_successful(&mut self) -> Result<(), UpdateError> {
        if let Some(info) = self.get_partition_mut(self.active) {
            info.boot_attempts = 0;
            info.verified = true;
            Ok(())
        } else {
            Err(UpdateError::PartitionError("Active partition not found".into()))
        }
    }

    /// Handle boot failure
    pub fn handle_boot_failure(&mut self) -> bool {
        if let Some(info) = self.get_partition_mut(self.active) {
            info.boot_attempts += 1;
            
            if info.boot_attempts >= info.max_attempts {
                // Mark as unbootable
                info.status = PartitionStatus::Unbootable;
                
                // Try to rollback
                if let Ok(()) = self.rollback() {
                    return true;
                }
            }
        }
        false
    }

    /// Rollback to previous version
    pub fn rollback(&mut self) -> Result<(), UpdateError> {
        let other = self.active.other();
        
        // Check if other slot is bootable
        if let Some(info) = self.get_partition(other) {
            if !info.is_bootable() {
                return Err(UpdateError::PartitionError(
                    "No bootable fallback partition".into()
                ));
            }
        } else {
            return Err(UpdateError::PartitionError(
                "Fallback partition not found".into()
            ));
        }

        // Update statuses
        if let Some(info) = self.get_partition_mut(self.active) {
            if info.status != PartitionStatus::Unbootable {
                info.status = PartitionStatus::Successful;
            }
        }

        if let Some(info) = self.get_partition_mut(other) {
            info.status = PartitionStatus::Active;
        }

        self.active = other;
        self.pending = None;

        Ok(())
    }
}

impl Default for PartitionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Bootloader interface
pub struct BootControl {
    /// Current boot slot
    current_slot: PartitionSlot,
    /// Next boot slot
    next_slot: Option<PartitionSlot>,
    /// Rollback protection enabled
    rollback_protection: bool,
    /// Minimum version for rollback protection
    min_version: u32,
}

impl BootControl {
    /// Create new boot control
    pub const fn new() -> Self {
        Self {
            current_slot: PartitionSlot::A,
            next_slot: None,
            rollback_protection: true,
            min_version: 0,
        }
    }

    /// Get current boot slot
    pub fn current(&self) -> PartitionSlot {
        self.current_slot
    }

    /// Set next boot slot
    pub fn set_next_slot(&mut self, slot: PartitionSlot) {
        self.next_slot = Some(slot);
    }

    /// Clear next slot (boot from current)
    pub fn clear_next_slot(&mut self) {
        self.next_slot = None;
    }

    /// Check version for rollback protection
    pub fn check_version(&self, version: u32) -> bool {
        if self.rollback_protection {
            version >= self.min_version
        } else {
            true
        }
    }

    /// Update minimum version
    pub fn update_min_version(&mut self, version: u32) {
        if version > self.min_version {
            self.min_version = version;
        }
    }

    /// Enable/disable rollback protection
    pub fn set_rollback_protection(&mut self, enabled: bool) {
        self.rollback_protection = enabled;
    }
}

impl Default for BootControl {
    fn default() -> Self {
        Self::new()
    }
}
