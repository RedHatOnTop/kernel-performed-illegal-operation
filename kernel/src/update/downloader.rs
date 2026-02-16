//! Update Downloader
//!
//! Handles downloading updates with resume support.

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

/// Download chunk size (1 MB)
pub const CHUNK_SIZE: usize = 1024 * 1024;

/// Download state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    /// Not started
    NotStarted,
    /// In progress
    InProgress,
    /// Paused
    Paused,
    /// Completed
    Completed,
    /// Error
    Error,
}

/// Download request
#[derive(Debug, Clone)]
pub struct DownloadRequest {
    /// URL
    pub url: String,
    /// Expected size
    pub expected_size: u64,
    /// Expected checksum
    pub checksum: String,
    /// Resume from offset
    pub resume_offset: u64,
}

/// Download chunk
#[derive(Debug)]
pub struct DownloadChunk {
    /// Chunk offset
    pub offset: u64,
    /// Chunk data
    pub data: Vec<u8>,
}

/// Downloader
pub struct Downloader {
    /// Current request
    request: Option<DownloadRequest>,
    /// Downloaded bytes
    downloaded: u64,
    /// State
    state: DownloadState,
    /// Error message
    error: Option<String>,
    /// Download buffer
    buffer: Vec<u8>,
    /// Maximum concurrent connections
    max_connections: usize,
}

impl Downloader {
    /// Create new downloader
    pub const fn new() -> Self {
        Self {
            request: None,
            downloaded: 0,
            state: DownloadState::NotStarted,
            error: None,
            buffer: Vec::new(),
            max_connections: 4,
        }
    }

    /// Start download
    pub fn start(&mut self, request: DownloadRequest) -> Result<(), String> {
        if self.state == DownloadState::InProgress {
            return Err("Download already in progress".into());
        }

        self.downloaded = request.resume_offset;
        self.request = Some(request);
        self.state = DownloadState::InProgress;
        self.error = None;

        Ok(())
    }

    /// Pause download
    pub fn pause(&mut self) {
        if self.state == DownloadState::InProgress {
            self.state = DownloadState::Paused;
        }
    }

    /// Resume download
    pub fn resume(&mut self) {
        if self.state == DownloadState::Paused {
            self.state = DownloadState::InProgress;
        }
    }

    /// Cancel download
    pub fn cancel(&mut self) {
        self.request = None;
        self.downloaded = 0;
        self.state = DownloadState::NotStarted;
        self.buffer.clear();
    }

    /// Process incoming chunk
    pub fn on_chunk(&mut self, chunk: DownloadChunk) -> Result<(), String> {
        if self.state != DownloadState::InProgress {
            return Err("Not downloading".into());
        }

        // Verify chunk offset
        if chunk.offset != self.downloaded {
            return Err("Chunk offset mismatch".into());
        }

        self.downloaded += chunk.data.len() as u64;
        self.buffer.extend_from_slice(&chunk.data);

        // Check if complete
        if let Some(ref request) = self.request {
            if self.downloaded >= request.expected_size {
                self.state = DownloadState::Completed;
            }
        }

        Ok(())
    }

    /// Get downloaded data
    pub fn data(&self) -> &[u8] {
        &self.buffer
    }

    /// Take downloaded data
    pub fn take_data(&mut self) -> Vec<u8> {
        core::mem::take(&mut self.buffer)
    }

    /// Get progress
    pub fn progress(&self) -> (u64, u64) {
        if let Some(ref request) = self.request {
            (self.downloaded, request.expected_size)
        } else {
            (0, 0)
        }
    }

    /// Get state
    pub fn state(&self) -> DownloadState {
        self.state
    }

    /// Get error
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Set error
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.state = DownloadState::Error;
    }
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new()
    }
}

/// Delta update support
pub struct DeltaUpdate {
    /// Base version
    pub base_version: String,
    /// Target version
    pub target_version: String,
    /// Patches
    pub patches: Vec<DeltaPatch>,
}

/// Delta patch
pub struct DeltaPatch {
    /// Patch type
    pub patch_type: DeltaPatchType,
    /// Source offset
    pub source_offset: u64,
    /// Source length
    pub source_length: u64,
    /// Target offset
    pub target_offset: u64,
    /// Data (for Add/Replace)
    pub data: Vec<u8>,
}

/// Delta patch type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaPatchType {
    /// Copy from source
    Copy,
    /// Add new data
    Add,
    /// Replace data
    Replace,
}

impl DeltaUpdate {
    /// Apply delta update
    pub fn apply(&self, source: &[u8]) -> Result<Vec<u8>, String> {
        // Calculate target size
        let target_size = self
            .patches
            .iter()
            .map(|p| match p.patch_type {
                DeltaPatchType::Copy => p.source_length,
                DeltaPatchType::Add | DeltaPatchType::Replace => p.data.len() as u64,
            })
            .sum::<u64>();

        let mut target = Vec::with_capacity(target_size as usize);

        for patch in &self.patches {
            match patch.patch_type {
                DeltaPatchType::Copy => {
                    let start = patch.source_offset as usize;
                    let end = start + patch.source_length as usize;
                    if end > source.len() {
                        return Err("Source out of bounds".into());
                    }
                    target.extend_from_slice(&source[start..end]);
                }
                DeltaPatchType::Add | DeltaPatchType::Replace => {
                    target.extend_from_slice(&patch.data);
                }
            }
        }

        Ok(target)
    }
}

/// Global downloader
pub static DOWNLOADER: Mutex<Downloader> = Mutex::new(Downloader::new());
