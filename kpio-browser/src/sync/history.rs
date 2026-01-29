//! History Synchronization
//!
//! Privacy-preserving browser history sync.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{SyncItem, SyncItemType, SyncError};

/// History entry
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// Entry ID (hash of URL)
    pub id: String,
    /// URL
    pub url: String,
    /// Title
    pub title: String,
    /// Visit count
    pub visit_count: u32,
    /// Last visit timestamp
    pub last_visit: u64,
    /// First visit timestamp
    pub first_visit: u64,
    /// Typed count (manually entered URL)
    pub typed_count: u32,
    /// Hidden (e.g., redirect)
    pub hidden: bool,
}

impl HistoryEntry {
    /// Create new history entry
    pub fn new(url: String, title: String) -> Self {
        let id = hash_url(&url);
        Self {
            id,
            url,
            title,
            visit_count: 1,
            last_visit: 0,
            first_visit: 0,
            typed_count: 0,
            hidden: false,
        }
    }

    /// Record visit
    pub fn record_visit(&mut self, timestamp: u64, typed: bool) {
        self.visit_count += 1;
        self.last_visit = timestamp;
        if typed {
            self.typed_count += 1;
        }
    }

    /// Convert to sync item
    pub fn to_sync_item(&self) -> SyncItem {
        let data = self.serialize();
        SyncItem::new(self.id.clone(), SyncItemType::History, data)
    }

    /// Create from sync item
    pub fn from_sync_item(item: &SyncItem) -> Option<Self> {
        if item.item_type != SyncItemType::History {
            return None;
        }
        Self::deserialize(&item.data)
    }

    fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        
        serialize_string(&mut data, &self.id);
        serialize_string(&mut data, &self.url);
        serialize_string(&mut data, &self.title);
        data.extend_from_slice(&self.visit_count.to_le_bytes());
        data.extend_from_slice(&self.last_visit.to_le_bytes());
        data.extend_from_slice(&self.first_visit.to_le_bytes());
        data.extend_from_slice(&self.typed_count.to_le_bytes());
        data.push(if self.hidden { 1 } else { 0 });
        
        data
    }

    fn deserialize(data: &[u8]) -> Option<Self> {
        let mut cursor = 0;
        
        let id = deserialize_string(data, &mut cursor)?;
        let url = deserialize_string(data, &mut cursor)?;
        let title = deserialize_string(data, &mut cursor)?;
        
        if cursor + 4 > data.len() { return None; }
        let visit_count = u32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?);
        cursor += 4;
        
        if cursor + 8 > data.len() { return None; }
        let last_visit = u64::from_le_bytes(data[cursor..cursor+8].try_into().ok()?);
        cursor += 8;
        
        if cursor + 8 > data.len() { return None; }
        let first_visit = u64::from_le_bytes(data[cursor..cursor+8].try_into().ok()?);
        cursor += 8;
        
        if cursor + 4 > data.len() { return None; }
        let typed_count = u32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?);
        cursor += 4;
        
        if cursor >= data.len() { return None; }
        let hidden = data[cursor] != 0;
        
        Some(Self {
            id,
            url,
            title,
            visit_count,
            last_visit,
            first_visit,
            typed_count,
            hidden,
        })
    }
}

/// History sync options
#[derive(Debug, Clone)]
pub struct HistorySyncOptions {
    /// Sync enabled
    pub enabled: bool,
    /// Max age in days (0 = unlimited)
    pub max_age_days: u32,
    /// Exclude patterns
    pub exclude_patterns: Vec<String>,
    /// Include only typed URLs
    pub typed_only: bool,
    /// Anonymize URLs (remove query parameters)
    pub anonymize: bool,
}

impl Default for HistorySyncOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            max_age_days: 90,
            exclude_patterns: Vec::new(),
            typed_only: false,
            anonymize: false,
        }
    }
}

/// History sync handler
pub struct HistorySync {
    /// Local history entries
    entries: BTreeMap<String, HistoryEntry>,
    /// Options
    options: HistorySyncOptions,
    /// Last sync timestamp
    last_sync: u64,
}

impl HistorySync {
    /// Create new history sync
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            options: HistorySyncOptions::default(),
            last_sync: 0,
        }
    }

    /// Get options
    pub fn options(&self) -> &HistorySyncOptions {
        &self.options
    }

    /// Set options
    pub fn set_options(&mut self, options: HistorySyncOptions) {
        self.options = options;
    }

    /// Add or update entry
    pub fn record(&mut self, url: &str, title: &str, timestamp: u64, typed: bool) {
        // Check exclusions
        if self.should_exclude(url) {
            return;
        }

        // Anonymize if needed
        let url = if self.options.anonymize {
            strip_query_params(url)
        } else {
            url.to_string()
        };

        let id = hash_url(&url);

        if let Some(entry) = self.entries.get_mut(&id) {
            entry.record_visit(timestamp, typed);
            if !title.is_empty() {
                entry.title = title.to_string();
            }
        } else {
            let mut entry = HistoryEntry::new(url, title.to_string());
            entry.last_visit = timestamp;
            entry.first_visit = timestamp;
            if typed {
                entry.typed_count = 1;
            }
            self.entries.insert(id, entry);
        }
    }

    /// Check if URL should be excluded
    fn should_exclude(&self, url: &str) -> bool {
        for pattern in &self.options.exclude_patterns {
            if url.contains(pattern) {
                return true;
            }
        }
        false
    }

    /// Get entry by ID
    pub fn get(&self, id: &str) -> Option<&HistoryEntry> {
        self.entries.get(id)
    }

    /// Get entry by URL
    pub fn get_by_url(&self, url: &str) -> Option<&HistoryEntry> {
        let id = hash_url(url);
        self.entries.get(&id)
    }

    /// Get recent entries
    pub fn recent(&self, limit: usize) -> Vec<&HistoryEntry> {
        let mut entries: Vec<_> = self.entries.values().collect();
        entries.sort_by(|a, b| b.last_visit.cmp(&a.last_visit));
        entries.truncate(limit);
        entries
    }

    /// Get most visited
    pub fn most_visited(&self, limit: usize) -> Vec<&HistoryEntry> {
        let mut entries: Vec<_> = self.entries.values()
            .filter(|e| !e.hidden)
            .collect();
        entries.sort_by(|a, b| b.visit_count.cmp(&a.visit_count));
        entries.truncate(limit);
        entries
    }

    /// Search history
    pub fn search(&self, query: &str, limit: usize) -> Vec<&HistoryEntry> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<_> = self.entries
            .values()
            .filter(|e| {
                e.url.to_lowercase().contains(&query_lower) ||
                e.title.to_lowercase().contains(&query_lower)
            })
            .collect();
        
        // Sort by relevance (title match > url match, then by visit count)
        results.sort_by(|a, b| {
            let a_title_match = a.title.to_lowercase().contains(&query_lower);
            let b_title_match = b.title.to_lowercase().contains(&query_lower);
            
            match (a_title_match, b_title_match) {
                (true, false) => core::cmp::Ordering::Less,
                (false, true) => core::cmp::Ordering::Greater,
                _ => b.visit_count.cmp(&a.visit_count),
            }
        });
        
        results.truncate(limit);
        results
    }

    /// Delete entry
    pub fn delete(&mut self, id: &str) {
        self.entries.remove(id);
    }

    /// Delete by URL
    pub fn delete_url(&mut self, url: &str) {
        let id = hash_url(url);
        self.entries.remove(&id);
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Clear history older than
    pub fn clear_older_than(&mut self, timestamp: u64) {
        self.entries.retain(|_, e| e.last_visit >= timestamp);
    }

    /// Get entries modified since last sync
    pub fn changes_since(&self, timestamp: u64) -> Vec<&HistoryEntry> {
        self.entries
            .values()
            .filter(|e| e.last_visit > timestamp)
            .filter(|e| !self.options.typed_only || e.typed_count > 0)
            .collect()
    }

    /// Get sync items
    pub fn to_sync_items(&self) -> Vec<SyncItem> {
        let cutoff = if self.options.max_age_days > 0 {
            // Would calculate based on current time
            0
        } else {
            0
        };

        self.entries
            .values()
            .filter(|e| e.last_visit >= cutoff)
            .filter(|e| !self.options.typed_only || e.typed_count > 0)
            .map(|e| e.to_sync_item())
            .collect()
    }

    /// Apply remote entry
    pub fn apply_remote(&mut self, item: &SyncItem) -> Result<(), SyncError> {
        if let Some(remote) = HistoryEntry::from_sync_item(item) {
            if item.deleted {
                self.entries.remove(&remote.id);
            } else if let Some(local) = self.entries.get_mut(&remote.id) {
                // Merge: take max visit count and most recent visit
                local.visit_count = local.visit_count.max(remote.visit_count);
                local.typed_count = local.typed_count.max(remote.typed_count);
                
                if remote.last_visit > local.last_visit {
                    local.last_visit = remote.last_visit;
                    if !remote.title.is_empty() {
                        local.title = remote.title;
                    }
                }
                
                if remote.first_visit < local.first_visit || local.first_visit == 0 {
                    local.first_visit = remote.first_visit;
                }
            } else {
                self.entries.insert(remote.id.clone(), remote);
            }
        }
        Ok(())
    }

    /// Entry count
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for HistorySync {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions

fn hash_url(url: &str) -> String {
    // Simple hash for ID generation
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in url.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    alloc::format!("h_{:016x}", hash)
}

fn strip_query_params(url: &str) -> String {
    if let Some(pos) = url.find('?') {
        url[..pos].to_string()
    } else {
        url.to_string()
    }
}

fn serialize_string(data: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(bytes);
}

fn deserialize_string(data: &[u8], cursor: &mut usize) -> Option<String> {
    if *cursor + 4 > data.len() { return None; }
    let len = u32::from_le_bytes(data[*cursor..*cursor+4].try_into().ok()?) as usize;
    *cursor += 4;
    
    if *cursor + len > data.len() { return None; }
    let s = core::str::from_utf8(&data[*cursor..*cursor+len]).ok()?;
    *cursor += len;
    
    Some(s.to_string())
}
