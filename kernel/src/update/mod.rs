//! System Update Module
//!
//! A/B partition updates with rollback capability.

mod downloader;
mod verifier;
mod partition;

pub use downloader::*;
pub use verifier::*;
pub use partition::*;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::Mutex;

/// Update error
#[derive(Debug, Clone)]
pub enum UpdateError {
    /// Network error
    NetworkError(String),
    /// Download failed
    DownloadFailed(String),
    /// Verification failed
    VerificationFailed,
    /// Insufficient space
    InsufficientSpace,
    /// Partition error
    PartitionError(String),
    /// Already updating
    AlreadyUpdating,
    /// Not available
    NotAvailable,
}

/// Update status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStatus {
    /// Idle - no update in progress
    Idle,
    /// Checking for updates
    Checking,
    /// Update available
    Available,
    /// Downloading
    Downloading,
    /// Verifying
    Verifying,
    /// Installing
    Installing,
    /// Pending reboot
    PendingReboot,
    /// Error
    Error,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self::Idle
    }
}

/// Update info
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// Version
    pub version: String,
    /// Release date
    pub release_date: String,
    /// Size in bytes
    pub size: u64,
    /// Changelog
    pub changelog: String,
    /// Is critical
    pub critical: bool,
    /// Download URL
    pub download_url: String,
    /// Checksum
    pub checksum: String,
    /// Signature
    pub signature: Vec<u8>,
}

/// Update progress
#[derive(Debug, Clone, Default)]
pub struct UpdateProgress {
    /// Total bytes
    pub total_bytes: u64,
    /// Downloaded bytes
    pub downloaded_bytes: u64,
    /// Current phase
    pub phase: UpdateStatus,
    /// Phase progress (0-100)
    pub phase_progress: u8,
    /// ETA in seconds
    pub eta_seconds: Option<u64>,
}

impl UpdateProgress {
    /// Get overall progress (0-100)
    pub fn overall_progress(&self) -> u8 {
        match self.phase {
            UpdateStatus::Checking => 5,
            UpdateStatus::Downloading => {
                if self.total_bytes > 0 {
                    10 + (self.downloaded_bytes * 50 / self.total_bytes) as u8
                } else {
                    10
                }
            }
            UpdateStatus::Verifying => 60 + self.phase_progress / 5,
            UpdateStatus::Installing => 80 + self.phase_progress / 5,
            UpdateStatus::PendingReboot => 100,
            _ => 0,
        }
    }
}

/// Update manager
pub struct UpdateManager {
    /// Current status
    status: UpdateStatus,
    /// Current version
    current_version: String,
    /// Available update
    available_update: Option<UpdateInfo>,
    /// Progress
    progress: UpdateProgress,
    /// Update server URL
    server_url: String,
    /// Auto update enabled
    auto_update: bool,
    /// Allow beta updates
    allow_beta: bool,
    /// Partition manager
    partitions: PartitionManager,
}

impl UpdateManager {
    /// Create new update manager
    pub fn new(current_version: String, server_url: String) -> Self {
        Self {
            status: UpdateStatus::Idle,
            current_version,
            available_update: None,
            progress: UpdateProgress::default(),
            server_url,
            auto_update: true,
            allow_beta: false,
            partitions: PartitionManager::new(),
        }
    }

    /// Get current status
    pub fn status(&self) -> UpdateStatus {
        self.status
    }

    /// Get progress
    pub fn progress(&self) -> &UpdateProgress {
        &self.progress
    }

    /// Get available update
    pub fn available_update(&self) -> Option<&UpdateInfo> {
        self.available_update.as_ref()
    }

    /// Check for updates
    pub fn check_for_updates(&mut self) -> Result<Option<&UpdateInfo>, UpdateError> {
        if self.status != UpdateStatus::Idle {
            return Err(UpdateError::AlreadyUpdating);
        }

        self.status = UpdateStatus::Checking;

        // Would query update server
        let update_check_url = alloc::format!(
            "{}/check?version={}&channel={}",
            self.server_url,
            self.current_version,
            if self.allow_beta { "beta" } else { "stable" },
        );

        // Mock response
        let update = UpdateInfo {
            version: "0.2.0".to_string(),
            release_date: "2026-02-01".to_string(),
            size: 100 * 1024 * 1024, // 100 MB
            changelog: "Bug fixes and performance improvements".to_string(),
            critical: false,
            download_url: alloc::format!("{}/download/0.2.0", self.server_url),
            checksum: "sha256:abcdef123456...".to_string(),
            signature: Vec::new(),
        };

        self.available_update = Some(update);
        self.status = UpdateStatus::Available;

        Ok(self.available_update.as_ref())
    }

    /// Start downloading update
    pub fn start_download(&mut self) -> Result<(), UpdateError> {
        let update = self.available_update.as_ref()
            .ok_or(UpdateError::NotAvailable)?;

        if self.status != UpdateStatus::Available {
            return Err(UpdateError::AlreadyUpdating);
        }

        // Check space
        if !self.partitions.has_space(update.size) {
            return Err(UpdateError::InsufficientSpace);
        }

        self.status = UpdateStatus::Downloading;
        self.progress.total_bytes = update.size;
        self.progress.downloaded_bytes = 0;
        self.progress.phase = UpdateStatus::Downloading;

        // Would start async download
        // For now, simulate completion
        self.progress.downloaded_bytes = update.size;
        
        Ok(())
    }

    /// Verify downloaded update
    pub fn verify(&mut self) -> Result<(), UpdateError> {
        if self.status != UpdateStatus::Downloading {
            return Err(UpdateError::AlreadyUpdating);
        }

        self.status = UpdateStatus::Verifying;
        self.progress.phase = UpdateStatus::Verifying;

        let update = self.available_update.as_ref()
            .ok_or(UpdateError::NotAvailable)?;

        // Verify checksum
        if !self.verify_checksum(&update.checksum) {
            self.status = UpdateStatus::Error;
            return Err(UpdateError::VerificationFailed);
        }

        // Verify signature
        if !self.verify_signature(&update.signature) {
            self.status = UpdateStatus::Error;
            return Err(UpdateError::VerificationFailed);
        }

        self.progress.phase_progress = 100;
        Ok(())
    }

    /// Verify checksum
    fn verify_checksum(&self, _expected: &str) -> bool {
        // Would compute SHA256 of downloaded data
        true
    }

    /// Verify signature
    fn verify_signature(&self, _signature: &[u8]) -> bool {
        // Would verify Ed25519 signature
        true
    }

    /// Install update
    pub fn install(&mut self) -> Result<(), UpdateError> {
        if self.status != UpdateStatus::Verifying {
            return Err(UpdateError::AlreadyUpdating);
        }

        self.status = UpdateStatus::Installing;
        self.progress.phase = UpdateStatus::Installing;

        // Get inactive partition
        let target = self.partitions.inactive_partition()
            .ok_or_else(|| UpdateError::PartitionError("No inactive partition".into()))?;

        // Would write update to partition
        // ...

        // Mark new partition as pending
        self.partitions.set_pending(target)?;

        self.status = UpdateStatus::PendingReboot;
        self.progress.phase = UpdateStatus::PendingReboot;
        self.progress.phase_progress = 100;

        Ok(())
    }

    /// Apply update (reboot into new version)
    pub fn apply(&mut self) -> Result<(), UpdateError> {
        if self.status != UpdateStatus::PendingReboot {
            return Err(UpdateError::NotAvailable);
        }

        // Switch active partition
        self.partitions.commit_pending()?;

        // Would trigger reboot
        Ok(())
    }

    /// Rollback to previous version
    pub fn rollback(&mut self) -> Result<(), UpdateError> {
        self.partitions.rollback()?;
        Ok(())
    }

    /// Cancel update
    pub fn cancel(&mut self) {
        self.status = UpdateStatus::Idle;
        self.progress = UpdateProgress::default();
    }

    /// Set auto update
    pub fn set_auto_update(&mut self, enabled: bool) {
        self.auto_update = enabled;
    }

    /// Set allow beta
    pub fn set_allow_beta(&mut self, enabled: bool) {
        self.allow_beta = enabled;
    }
}

impl Default for UpdateManager {
    fn default() -> Self {
        Self::new(
            "0.1.0".to_string(),
            "https://update.kpios.local".to_string(),
        )
    }
}

/// Global update manager
pub static UPDATE_MANAGER: Mutex<UpdateManager> = Mutex::new(UpdateManager {
    status: UpdateStatus::Idle,
    current_version: String::new(),
    available_update: None,
    progress: UpdateProgress {
        total_bytes: 0,
        downloaded_bytes: 0,
        phase: UpdateStatus::Idle,
        phase_progress: 0,
        eta_seconds: None,
    },
    server_url: String::new(),
    auto_update: true,
    allow_beta: false,
    partitions: PartitionManager {
        active: PartitionSlot::A,
        pending: None,
        slots: [None, None],
    },
});
