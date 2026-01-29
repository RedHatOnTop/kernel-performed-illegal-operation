//! Cloud Synchronization Engine
//!
//! Provides data synchronization across devices with conflict resolution.

pub mod bookmarks;
pub mod history;
pub mod settings;
pub mod tabs;
pub mod engine;

pub use bookmarks::*;
pub use history::*;
pub use settings::*;
pub use tabs::*;
pub use engine::*;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

/// Sync error types
#[derive(Debug, Clone)]
pub enum SyncError {
    /// Not authenticated
    NotAuthenticated,
    /// Network error
    NetworkError(String),
    /// Server error
    ServerError(u16, String),
    /// Conflict error
    ConflictError(String),
    /// Encryption error
    EncryptionError(String),
    /// Storage error
    StorageError(String),
    /// Quota exceeded
    QuotaExceeded,
}

/// Sync status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStatus {
    /// Idle
    Idle,
    /// Syncing
    Syncing,
    /// Paused
    Paused,
    /// Error
    Error,
    /// Disabled
    Disabled,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self::Idle
    }
}

/// Sync direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDirection {
    /// Upload only
    Upload,
    /// Download only
    Download,
    /// Bidirectional
    Bidirectional,
}

impl Default for SyncDirection {
    fn default() -> Self {
        Self::Bidirectional
    }
}

/// Sync item type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SyncItemType {
    /// Bookmark
    Bookmark,
    /// History entry
    History,
    /// Setting
    Setting,
    /// Open tab
    Tab,
    /// Extension
    Extension,
    /// Password
    Password,
}

/// Sync item
#[derive(Debug, Clone)]
pub struct SyncItem {
    /// Item ID
    pub id: String,
    /// Item type
    pub item_type: SyncItemType,
    /// Version (for conflict detection)
    pub version: u64,
    /// Last modified timestamp
    pub modified_at: u64,
    /// Device that last modified
    pub modified_by: String,
    /// Is deleted
    pub deleted: bool,
    /// Encrypted data
    pub data: Vec<u8>,
}

impl SyncItem {
    /// Create new sync item
    pub fn new(id: String, item_type: SyncItemType, data: Vec<u8>) -> Self {
        Self {
            id,
            item_type,
            version: 1,
            modified_at: 0,
            modified_by: String::new(),
            deleted: false,
            data,
        }
    }

    /// Mark as deleted
    pub fn mark_deleted(&mut self) {
        self.deleted = true;
        self.version += 1;
    }

    /// Update data
    pub fn update(&mut self, data: Vec<u8>) {
        self.data = data;
        self.version += 1;
    }

    /// Is newer than
    pub fn is_newer_than(&self, other: &SyncItem) -> bool {
        self.version > other.version || 
            (self.version == other.version && self.modified_at > other.modified_at)
    }
}

/// Sync change
#[derive(Debug, Clone)]
pub struct SyncChange {
    /// Item
    pub item: SyncItem,
    /// Change type
    pub change_type: ChangeType,
}

/// Change type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    /// Created
    Created,
    /// Modified
    Modified,
    /// Deleted
    Deleted,
}

/// Conflict resolution strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy {
    /// Server wins
    ServerWins,
    /// Client wins
    ClientWins,
    /// Newest wins
    NewestWins,
    /// Manual resolution
    Manual,
    /// Merge
    Merge,
}

impl Default for ConflictStrategy {
    fn default() -> Self {
        Self::NewestWins
    }
}

/// Sync conflict
#[derive(Debug, Clone)]
pub struct SyncConflict {
    /// Local item
    pub local: SyncItem,
    /// Remote item
    pub remote: SyncItem,
    /// Resolved item (if auto-resolved)
    pub resolved: Option<SyncItem>,
    /// Resolution strategy used
    pub strategy: ConflictStrategy,
}

impl SyncConflict {
    /// Create new conflict
    pub fn new(local: SyncItem, remote: SyncItem) -> Self {
        Self {
            local,
            remote,
            resolved: None,
            strategy: ConflictStrategy::default(),
        }
    }

    /// Resolve with strategy
    pub fn resolve(&mut self, strategy: ConflictStrategy) -> &SyncItem {
        self.strategy = strategy;

        let resolved = match strategy {
            ConflictStrategy::ServerWins => self.remote.clone(),
            ConflictStrategy::ClientWins => self.local.clone(),
            ConflictStrategy::NewestWins => {
                if self.local.is_newer_than(&self.remote) {
                    self.local.clone()
                } else {
                    self.remote.clone()
                }
            }
            ConflictStrategy::Manual | ConflictStrategy::Merge => {
                // Would need user input or merge logic
                self.remote.clone()
            }
        };

        self.resolved = Some(resolved);
        self.resolved.as_ref().unwrap()
    }
}

/// Sync configuration
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Enabled
    pub enabled: bool,
    /// Sync interval (seconds)
    pub interval: u64,
    /// Sync direction
    pub direction: SyncDirection,
    /// Conflict strategy
    pub conflict_strategy: ConflictStrategy,
    /// Item types to sync
    pub sync_types: Vec<SyncItemType>,
    /// Encryption enabled
    pub encryption_enabled: bool,
    /// Bandwidth limit (bytes/sec, 0 = unlimited)
    pub bandwidth_limit: u64,
}

impl Default for SyncConfig {
    fn default() -> Self {
        use alloc::vec;
        Self {
            enabled: true,
            interval: 300, // 5 minutes
            direction: SyncDirection::Bidirectional,
            conflict_strategy: ConflictStrategy::NewestWins,
            sync_types: vec![
                SyncItemType::Bookmark,
                SyncItemType::History,
                SyncItemType::Setting,
                SyncItemType::Tab,
            ],
            encryption_enabled: true,
            bandwidth_limit: 0,
        }
    }
}

/// Sync statistics
#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    /// Items uploaded
    pub uploaded: u64,
    /// Items downloaded
    pub downloaded: u64,
    /// Conflicts resolved
    pub conflicts_resolved: u64,
    /// Errors
    pub errors: u64,
    /// Last sync time
    pub last_sync: Option<u64>,
    /// Next sync time
    pub next_sync: Option<u64>,
    /// Bytes uploaded
    pub bytes_uploaded: u64,
    /// Bytes downloaded
    pub bytes_downloaded: u64,
}

/// Sync manager
pub struct SyncManager {
    /// Configuration
    config: SyncConfig,
    /// Current status
    status: SyncStatus,
    /// Statistics
    stats: SyncStats,
    /// Pending changes
    pending_changes: Vec<SyncChange>,
    /// Unresolved conflicts
    conflicts: Vec<SyncConflict>,
    /// Device ID
    device_id: String,
}

impl SyncManager {
    /// Create new sync manager
    pub fn new(device_id: String) -> Self {
        Self {
            config: SyncConfig::default(),
            status: SyncStatus::Idle,
            stats: SyncStats::default(),
            pending_changes: Vec::new(),
            conflicts: Vec::new(),
            device_id,
        }
    }

    /// Get configuration
    pub fn config(&self) -> &SyncConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: SyncConfig) {
        self.config = config;
    }

    /// Get status
    pub fn status(&self) -> SyncStatus {
        self.status
    }

    /// Get statistics
    pub fn stats(&self) -> &SyncStats {
        &self.stats
    }

    /// Add change to sync
    pub fn add_change(&mut self, item: SyncItem, change_type: ChangeType) {
        self.pending_changes.push(SyncChange { item, change_type });
    }

    /// Get pending changes count
    pub fn pending_count(&self) -> usize {
        self.pending_changes.len()
    }

    /// Get unresolved conflicts
    pub fn conflicts(&self) -> &[SyncConflict] {
        &self.conflicts
    }

    /// Get mutable unresolved conflicts
    pub fn conflicts_mut(&mut self) -> &mut Vec<SyncConflict> {
        &mut self.conflicts
    }

    /// Start sync
    pub fn sync(&mut self) -> Result<(), SyncError> {
        if !self.config.enabled {
            return Err(SyncError::StorageError("Sync disabled".into()));
        }

        self.status = SyncStatus::Syncing;

        // Would perform actual sync here:
        // 1. Get server changes since last sync
        // 2. Detect conflicts
        // 3. Resolve conflicts
        // 4. Upload local changes
        // 5. Download remote changes

        self.status = SyncStatus::Idle;
        self.stats.last_sync = Some(0); // Would get current time

        Ok(())
    }

    /// Pause sync
    pub fn pause(&mut self) {
        self.status = SyncStatus::Paused;
    }

    /// Resume sync
    pub fn resume(&mut self) {
        if self.status == SyncStatus::Paused {
            self.status = SyncStatus::Idle;
        }
    }

    /// Reset sync state
    pub fn reset(&mut self) {
        self.pending_changes.clear();
        self.conflicts.clear();
        self.stats = SyncStats::default();
        self.status = SyncStatus::Idle;
    }

    /// Enable/disable sync type
    pub fn set_sync_type_enabled(&mut self, item_type: SyncItemType, enabled: bool) {
        if enabled {
            if !self.config.sync_types.contains(&item_type) {
                self.config.sync_types.push(item_type);
            }
        } else {
            self.config.sync_types.retain(|t| *t != item_type);
        }
    }

    /// Check if sync type enabled
    pub fn is_sync_type_enabled(&self, item_type: SyncItemType) -> bool {
        self.config.sync_types.contains(&item_type)
    }
}

impl Default for SyncManager {
    fn default() -> Self {
        Self::new("default_device".to_string())
    }
}

/// Global sync manager
pub static SYNC_MANAGER: RwLock<SyncManager> = RwLock::new(SyncManager {
    config: SyncConfig {
        enabled: true,
        interval: 300,
        direction: SyncDirection::Bidirectional,
        conflict_strategy: ConflictStrategy::NewestWins,
        sync_types: Vec::new(),
        encryption_enabled: true,
        bandwidth_limit: 0,
    },
    status: SyncStatus::Idle,
    stats: SyncStats {
        uploaded: 0,
        downloaded: 0,
        conflicts_resolved: 0,
        errors: 0,
        last_sync: None,
        next_sync: None,
        bytes_uploaded: 0,
        bytes_downloaded: 0,
    },
    pending_changes: Vec::new(),
    conflicts: Vec::new(),
    device_id: String::new(),
});
