//! Download Manager
//!
//! Download progress, pause/resume, file management.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::boxed::Box;
use spin::RwLock;

/// Download ID.
pub type DownloadId = u64;

/// Download state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    /// Queued, not started.
    Pending,
    /// Currently downloading.
    InProgress,
    /// Paused.
    Paused,
    /// Completed successfully.
    Complete,
    /// Failed.
    Failed,
    /// Cancelled by user.
    Cancelled,
}

/// Download danger type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DangerType {
    /// Safe file.
    Safe,
    /// Potentially dangerous file type.
    File,
    /// URL is suspicious.
    Url,
    /// Content is suspicious.
    Content,
    /// Uncommon file.
    Uncommon,
    /// User accepted the danger.
    Accepted,
}

impl Default for DangerType {
    fn default() -> Self {
        Self::Safe
    }
}

/// Download item.
#[derive(Debug, Clone)]
pub struct Download {
    /// ID.
    pub id: DownloadId,
    /// URL.
    pub url: String,
    /// Referrer URL.
    pub referrer: Option<String>,
    /// Filename.
    pub filename: String,
    /// Save path.
    pub save_path: String,
    /// MIME type.
    pub mime_type: String,
    /// Start time.
    pub start_time: u64,
    /// End time.
    pub end_time: Option<u64>,
    /// Current state.
    pub state: DownloadState,
    /// Bytes received.
    pub bytes_received: u64,
    /// Total bytes (0 if unknown).
    pub total_bytes: u64,
    /// Danger type.
    pub danger: DangerType,
    /// Can resume if paused.
    pub can_resume: bool,
    /// Exists on disk.
    pub exists: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

impl Download {
    /// Create a new download.
    pub fn new(id: DownloadId, url: &str, filename: &str, save_path: &str) -> Self {
        Self {
            id,
            url: url.to_string(),
            referrer: None,
            filename: filename.to_string(),
            save_path: save_path.to_string(),
            mime_type: String::new(),
            start_time: 0,
            end_time: None,
            state: DownloadState::Pending,
            bytes_received: 0,
            total_bytes: 0,
            danger: DangerType::Safe,
            can_resume: false,
            exists: false,
            error: None,
        }
    }
    
    /// Get progress (0.0 - 1.0).
    pub fn progress(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.bytes_received as f64) / (self.total_bytes as f64)
        }
    }
    
    /// Get progress as percentage.
    pub fn progress_percent(&self) -> u32 {
        (self.progress() * 100.0) as u32
    }
    
    /// Is downloading.
    pub fn is_active(&self) -> bool {
        matches!(self.state, DownloadState::InProgress | DownloadState::Pending)
    }
    
    /// Is finished (complete, failed, or cancelled).
    pub fn is_finished(&self) -> bool {
        matches!(self.state, DownloadState::Complete | DownloadState::Failed | DownloadState::Cancelled)
    }
    
    /// Get estimated time remaining (seconds).
    pub fn time_remaining(&self, speed_bytes_per_sec: u64) -> Option<u64> {
        if speed_bytes_per_sec == 0 || self.total_bytes == 0 {
            return None;
        }
        
        let remaining = self.total_bytes.saturating_sub(self.bytes_received);
        Some(remaining / speed_bytes_per_sec)
    }
}

/// Download event.
#[derive(Debug, Clone)]
pub enum DownloadEvent {
    /// Download created.
    Created(Download),
    /// Download updated.
    Updated(Download),
    /// Download removed from list.
    Removed(DownloadId),
}

/// Download observer callback.
type DownloadObserver = Box<dyn Fn(&DownloadEvent) + Send + Sync>;

/// Download manager.
pub struct DownloadManager {
    /// Downloads.
    downloads: RwLock<Vec<Download>>,
    /// Next ID.
    next_id: RwLock<DownloadId>,
    /// Download directory.
    download_dir: RwLock<String>,
    /// Ask before download.
    ask_before_download: RwLock<bool>,
    /// Max concurrent downloads.
    max_concurrent: usize,
    /// Observers.
    observers: RwLock<Vec<DownloadObserver>>,
}

impl DownloadManager {
    /// Create a new download manager.
    pub fn new() -> Self {
        Self {
            downloads: RwLock::new(Vec::new()),
            next_id: RwLock::new(1),
            download_dir: RwLock::new("/home/user/Downloads".to_string()),
            ask_before_download: RwLock::new(false),
            max_concurrent: 5,
            observers: RwLock::new(Vec::new()),
        }
    }
    
    /// Set download directory.
    pub fn set_download_dir(&self, path: &str) {
        *self.download_dir.write() = path.to_string();
    }
    
    /// Get download directory.
    pub fn download_dir(&self) -> String {
        self.download_dir.read().clone()
    }
    
    /// Set ask before download.
    pub fn set_ask_before_download(&self, ask: bool) {
        *self.ask_before_download.write() = ask;
    }
    
    /// Start a download.
    pub fn start_download(&self, url: &str, filename: Option<&str>) -> DownloadId {
        let mut next_id = self.next_id.write();
        let id = *next_id;
        *next_id += 1;
        drop(next_id);
        
        let filename = filename.unwrap_or_else(|| {
            url.rsplit('/').next().unwrap_or("download")
        });
        
        let download_dir = self.download_dir.read().clone();
        let save_path = alloc::format!("{}/{}", download_dir, filename);
        
        let mut download = Download::new(id, url, filename, &save_path);
        download.state = DownloadState::InProgress;
        download.start_time = 0; // Would use current timestamp
        
        let event = DownloadEvent::Created(download.clone());
        self.downloads.write().push(download);
        self.notify(&event);
        
        id
    }
    
    /// Update download progress.
    pub fn update_progress(&self, id: DownloadId, bytes_received: u64, total_bytes: u64) {
        let mut downloads = self.downloads.write();
        
        if let Some(download) = downloads.iter_mut().find(|d| d.id == id) {
            download.bytes_received = bytes_received;
            if total_bytes > 0 {
                download.total_bytes = total_bytes;
            }
            
            let event = DownloadEvent::Updated(download.clone());
            drop(downloads);
            self.notify(&event);
        }
    }
    
    /// Complete a download.
    pub fn complete(&self, id: DownloadId) {
        let mut downloads = self.downloads.write();
        
        if let Some(download) = downloads.iter_mut().find(|d| d.id == id) {
            download.state = DownloadState::Complete;
            download.end_time = Some(0); // Would use current timestamp
            download.exists = true;
            download.bytes_received = download.total_bytes;
            
            let event = DownloadEvent::Updated(download.clone());
            drop(downloads);
            self.notify(&event);
        }
    }
    
    /// Fail a download.
    pub fn fail(&self, id: DownloadId, error: &str) {
        let mut downloads = self.downloads.write();
        
        if let Some(download) = downloads.iter_mut().find(|d| d.id == id) {
            download.state = DownloadState::Failed;
            download.end_time = Some(0);
            download.error = Some(error.to_string());
            
            let event = DownloadEvent::Updated(download.clone());
            drop(downloads);
            self.notify(&event);
        }
    }
    
    /// Pause a download.
    pub fn pause(&self, id: DownloadId) -> bool {
        let mut downloads = self.downloads.write();
        
        if let Some(download) = downloads.iter_mut().find(|d| d.id == id) {
            if download.state == DownloadState::InProgress && download.can_resume {
                download.state = DownloadState::Paused;
                
                let event = DownloadEvent::Updated(download.clone());
                drop(downloads);
                self.notify(&event);
                return true;
            }
        }
        
        false
    }
    
    /// Resume a download.
    pub fn resume(&self, id: DownloadId) -> bool {
        let mut downloads = self.downloads.write();
        
        if let Some(download) = downloads.iter_mut().find(|d| d.id == id) {
            if download.state == DownloadState::Paused {
                download.state = DownloadState::InProgress;
                
                let event = DownloadEvent::Updated(download.clone());
                drop(downloads);
                self.notify(&event);
                return true;
            }
        }
        
        false
    }
    
    /// Cancel a download.
    pub fn cancel(&self, id: DownloadId) -> bool {
        let mut downloads = self.downloads.write();
        
        if let Some(download) = downloads.iter_mut().find(|d: &&mut Download| d.id == id) {
            if download.is_active() {
                download.state = DownloadState::Cancelled;
                download.end_time = Some(0);
                
                let event = DownloadEvent::Updated(download.clone());
                drop(downloads);
                self.notify(&event);
                return true;
            }
        }
        
        false
    }
    
    /// Retry a failed download.
    pub fn retry(&self, id: DownloadId) -> Option<DownloadId> {
        let downloads = self.downloads.read();
        
        if let Some(download) = downloads.iter().find(|d| d.id == id) {
            if download.state == DownloadState::Failed {
                let url = download.url.clone();
                let filename = download.filename.clone();
                drop(downloads);
                
                return Some(self.start_download(&url, Some(&filename)));
            }
        }
        
        None
    }
    
    /// Remove a download from the list.
    pub fn remove(&self, id: DownloadId) {
        self.downloads.write().retain(|d| d.id != id);
        self.notify(&DownloadEvent::Removed(id));
    }
    
    /// Get download by ID.
    pub fn get(&self, id: DownloadId) -> Option<Download> {
        self.downloads.read().iter().find(|d| d.id == id).cloned()
    }
    
    /// Get all downloads.
    pub fn all(&self) -> Vec<Download> {
        self.downloads.read().clone()
    }
    
    /// Get active downloads.
    pub fn active(&self) -> Vec<Download> {
        self.downloads.read().iter()
            .filter(|d: &&Download| d.is_active())
            .cloned()
            .collect()
    }
    
    /// Get active download count.
    pub fn active_count(&self) -> usize {
        self.downloads.read().iter()
            .filter(|d: &&Download| d.is_active())
            .count()
    }
    
    /// Search downloads.
    pub fn search(&self, query: &str) -> Vec<Download> {
        let query = query.to_lowercase();
        self.downloads.read().iter()
            .filter(|d| {
                d.filename.to_lowercase().contains(&query) ||
                d.url.to_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }
    
    /// Clear completed downloads.
    pub fn clear_completed(&self) {
        let mut downloads = self.downloads.write();
        let removed: Vec<DownloadId> = downloads.iter()
            .filter(|d| d.state == DownloadState::Complete)
            .map(|d| d.id)
            .collect();
        
        downloads.retain(|d| d.state != DownloadState::Complete);
        drop(downloads);
        
        for id in removed {
            self.notify(&DownloadEvent::Removed(id));
        }
    }
    
    /// Add observer.
    pub fn observe<F>(&self, callback: F)
    where
        F: Fn(&DownloadEvent) + Send + Sync + 'static,
    {
        self.observers.write().push(Box::new(callback));
    }
    
    /// Notify observers.
    fn notify(&self, event: &DownloadEvent) {
        for observer in self.observers.read().iter() {
            observer(event);
        }
    }
    
    /// Get suggested filename from URL.
    pub fn suggest_filename(url: &str) -> String {
        url.rsplit('/').next()
            .unwrap_or("download")
            .split('?').next()
            .unwrap_or("download")
            .to_string()
    }
    
    /// Check if file type is safe.
    pub fn is_safe_file_type(filename: &str) -> bool {
        let dangerous_extensions = [
            ".exe", ".msi", ".bat", ".cmd", ".com", ".scr", ".pif",
            ".vbs", ".js", ".jse", ".wsf", ".wsh", ".ps1", ".psm1",
            ".jar", ".app", ".deb", ".rpm", ".dmg",
        ];
        
        let lower = filename.to_lowercase();
        !dangerous_extensions.iter().any(|ext| lower.ends_with(ext))
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Format bytes for display.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    
    if bytes >= GB {
        alloc::format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        alloc::format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        alloc::format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        alloc::format!("{} B", bytes)
    }
}

/// Format speed for display.
pub fn format_speed(bytes_per_sec: u64) -> String {
    alloc::format!("{}/s", format_bytes(bytes_per_sec))
}

/// Format time for display.
pub fn format_time(seconds: u64) -> String {
    if seconds >= 3600 {
        alloc::format!("{}:{:02}:{:02}", seconds / 3600, (seconds % 3600) / 60, seconds % 60)
    } else if seconds >= 60 {
        alloc::format!("{}:{:02}", seconds / 60, seconds % 60)
    } else {
        alloc::format!("{}s", seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_download_progress() {
        let mut download = Download::new(1, "https://example.com/file.zip", "file.zip", "/tmp/file.zip");
        download.total_bytes = 1000;
        download.bytes_received = 500;
        
        assert_eq!(download.progress(), 0.5);
        assert_eq!(download.progress_percent(), 50);
    }
    
    #[test]
    fn test_download_manager() {
        let manager = DownloadManager::new();
        
        let id = manager.start_download("https://example.com/file.zip", None);
        
        let download = manager.get(id).unwrap();
        assert_eq!(download.state, DownloadState::InProgress);
        
        manager.update_progress(id, 500, 1000);
        assert_eq!(manager.get(id).unwrap().progress_percent(), 50);
        
        manager.complete(id);
        assert_eq!(manager.get(id).unwrap().state, DownloadState::Complete);
    }
    
    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1_500_000), "1.43 MB");
    }
    
    #[test]
    fn test_safe_file_type() {
        assert!(DownloadManager::is_safe_file_type("document.pdf"));
        assert!(DownloadManager::is_safe_file_type("image.png"));
        assert!(!DownloadManager::is_safe_file_type("virus.exe"));
        assert!(!DownloadManager::is_safe_file_type("script.bat"));
    }
}
