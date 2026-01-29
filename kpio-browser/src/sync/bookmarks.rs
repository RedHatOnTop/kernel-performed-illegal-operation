//! Bookmark Synchronization
//!
//! Two-way bookmark sync with folder structure preservation.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{SyncItem, SyncItemType, SyncChange, ChangeType, SyncError, ConflictStrategy};

/// Bookmark
#[derive(Debug, Clone)]
pub struct Bookmark {
    /// Bookmark ID
    pub id: String,
    /// Title
    pub title: String,
    /// URL
    pub url: String,
    /// Parent folder ID
    pub parent_id: Option<String>,
    /// Position in folder
    pub position: u32,
    /// Created timestamp
    pub created_at: u64,
    /// Last modified timestamp
    pub modified_at: u64,
    /// Favicon URL
    pub favicon: Option<String>,
    /// Tags
    pub tags: Vec<String>,
}

impl Bookmark {
    /// Create new bookmark
    pub fn new(id: String, title: String, url: String) -> Self {
        Self {
            id,
            title,
            url,
            parent_id: None,
            position: 0,
            created_at: 0,
            modified_at: 0,
            favicon: None,
            tags: Vec::new(),
        }
    }

    /// Set parent folder
    pub fn with_parent(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Add tag
    pub fn with_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    /// Convert to sync item
    pub fn to_sync_item(&self) -> SyncItem {
        let data = self.serialize();
        SyncItem::new(self.id.clone(), SyncItemType::Bookmark, data)
    }

    /// Create from sync item
    pub fn from_sync_item(item: &SyncItem) -> Option<Self> {
        if item.item_type != SyncItemType::Bookmark {
            return None;
        }
        Self::deserialize(&item.data)
    }

    /// Serialize to bytes
    fn serialize(&self) -> Vec<u8> {
        // Simple serialization format (would use proper format in production)
        let mut data = Vec::new();
        
        // Length-prefixed strings
        serialize_string(&mut data, &self.id);
        serialize_string(&mut data, &self.title);
        serialize_string(&mut data, &self.url);
        serialize_option_string(&mut data, &self.parent_id);
        data.extend_from_slice(&self.position.to_le_bytes());
        data.extend_from_slice(&self.created_at.to_le_bytes());
        data.extend_from_slice(&self.modified_at.to_le_bytes());
        serialize_option_string(&mut data, &self.favicon);
        
        // Tags count and strings
        data.extend_from_slice(&(self.tags.len() as u32).to_le_bytes());
        for tag in &self.tags {
            serialize_string(&mut data, tag);
        }
        
        data
    }

    /// Deserialize from bytes
    fn deserialize(data: &[u8]) -> Option<Self> {
        let mut cursor = 0;
        
        let id = deserialize_string(data, &mut cursor)?;
        let title = deserialize_string(data, &mut cursor)?;
        let url = deserialize_string(data, &mut cursor)?;
        let parent_id = deserialize_option_string(data, &mut cursor)?;
        
        if cursor + 4 > data.len() { return None; }
        let position = u32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?);
        cursor += 4;
        
        if cursor + 8 > data.len() { return None; }
        let created_at = u64::from_le_bytes(data[cursor..cursor+8].try_into().ok()?);
        cursor += 8;
        
        if cursor + 8 > data.len() { return None; }
        let modified_at = u64::from_le_bytes(data[cursor..cursor+8].try_into().ok()?);
        cursor += 8;
        
        let favicon = deserialize_option_string(data, &mut cursor)?;
        
        if cursor + 4 > data.len() { return None; }
        let tag_count = u32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?) as usize;
        cursor += 4;
        
        let mut tags = Vec::with_capacity(tag_count);
        for _ in 0..tag_count {
            tags.push(deserialize_string(data, &mut cursor)?);
        }
        
        Some(Self {
            id,
            title,
            url,
            parent_id,
            position,
            created_at,
            modified_at,
            favicon,
            tags,
        })
    }
}

/// Bookmark folder
#[derive(Debug, Clone)]
pub struct BookmarkFolder {
    /// Folder ID
    pub id: String,
    /// Title
    pub title: String,
    /// Parent folder ID
    pub parent_id: Option<String>,
    /// Position in parent
    pub position: u32,
    /// Created timestamp
    pub created_at: u64,
    /// Last modified timestamp
    pub modified_at: u64,
}

impl BookmarkFolder {
    /// Create new folder
    pub fn new(id: String, title: String) -> Self {
        Self {
            id,
            title,
            parent_id: None,
            position: 0,
            created_at: 0,
            modified_at: 0,
        }
    }

    /// Set parent
    pub fn with_parent(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }
}

/// Bookmark sync handler
pub struct BookmarkSync {
    /// Local bookmarks
    bookmarks: BTreeMap<String, Bookmark>,
    /// Local folders
    folders: BTreeMap<String, BookmarkFolder>,
    /// Pending changes
    pending: Vec<SyncChange>,
    /// Last sync version
    last_sync_version: u64,
}

impl BookmarkSync {
    /// Create new bookmark sync
    pub const fn new() -> Self {
        Self {
            bookmarks: BTreeMap::new(),
            folders: BTreeMap::new(),
            pending: Vec::new(),
            last_sync_version: 0,
        }
    }

    /// Add bookmark
    pub fn add_bookmark(&mut self, bookmark: Bookmark) {
        let item = bookmark.to_sync_item();
        self.pending.push(SyncChange {
            item,
            change_type: ChangeType::Created,
        });
        self.bookmarks.insert(bookmark.id.clone(), bookmark);
    }

    /// Update bookmark
    pub fn update_bookmark(&mut self, bookmark: Bookmark) {
        let item = bookmark.to_sync_item();
        self.pending.push(SyncChange {
            item,
            change_type: ChangeType::Modified,
        });
        self.bookmarks.insert(bookmark.id.clone(), bookmark);
    }

    /// Delete bookmark
    pub fn delete_bookmark(&mut self, id: &str) {
        if let Some(bookmark) = self.bookmarks.remove(id) {
            let mut item = bookmark.to_sync_item();
            item.mark_deleted();
            self.pending.push(SyncChange {
                item,
                change_type: ChangeType::Deleted,
            });
        }
    }

    /// Get bookmark
    pub fn get_bookmark(&self, id: &str) -> Option<&Bookmark> {
        self.bookmarks.get(id)
    }

    /// Get all bookmarks
    pub fn all_bookmarks(&self) -> Vec<&Bookmark> {
        self.bookmarks.values().collect()
    }

    /// Get bookmarks in folder
    pub fn bookmarks_in_folder(&self, folder_id: Option<&str>) -> Vec<&Bookmark> {
        self.bookmarks
            .values()
            .filter(|b| b.parent_id.as_deref() == folder_id)
            .collect()
    }

    /// Add folder
    pub fn add_folder(&mut self, folder: BookmarkFolder) {
        self.folders.insert(folder.id.clone(), folder);
    }

    /// Get folder
    pub fn get_folder(&self, id: &str) -> Option<&BookmarkFolder> {
        self.folders.get(id)
    }

    /// Get subfolders
    pub fn subfolders(&self, parent_id: Option<&str>) -> Vec<&BookmarkFolder> {
        self.folders
            .values()
            .filter(|f| f.parent_id.as_deref() == parent_id)
            .collect()
    }

    /// Search bookmarks
    pub fn search(&self, query: &str) -> Vec<&Bookmark> {
        let query_lower = query.to_lowercase();
        self.bookmarks
            .values()
            .filter(|b| {
                b.title.to_lowercase().contains(&query_lower) ||
                b.url.to_lowercase().contains(&query_lower) ||
                b.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// Get pending changes
    pub fn pending_changes(&self) -> &[SyncChange] {
        &self.pending
    }

    /// Clear pending changes
    pub fn clear_pending(&mut self) {
        self.pending.clear();
    }

    /// Apply remote change
    pub fn apply_remote_change(&mut self, item: &SyncItem) -> Result<(), SyncError> {
        if item.item_type != SyncItemType::Bookmark {
            return Err(SyncError::StorageError("Invalid item type".into()));
        }

        if item.deleted {
            self.bookmarks.remove(&item.id);
        } else {
            if let Some(bookmark) = Bookmark::from_sync_item(item) {
                self.bookmarks.insert(bookmark.id.clone(), bookmark);
            }
        }

        Ok(())
    }

    /// Merge remote bookmark
    pub fn merge(&mut self, remote: &SyncItem, strategy: ConflictStrategy) 
        -> Result<Option<SyncItem>, SyncError> 
    {
        let remote_bookmark = Bookmark::from_sync_item(remote)
            .ok_or_else(|| SyncError::StorageError("Invalid bookmark data".into()))?;

        if let Some(local) = self.bookmarks.get(&remote.id) {
            let local_item = local.to_sync_item();
            
            // Check for conflict
            if local_item.version != remote.version {
                match strategy {
                    ConflictStrategy::ServerWins => {
                        self.bookmarks.insert(remote_bookmark.id.clone(), remote_bookmark);
                        Ok(None)
                    }
                    ConflictStrategy::ClientWins => {
                        Ok(Some(local_item))
                    }
                    ConflictStrategy::NewestWins => {
                        if local.modified_at > remote_bookmark.modified_at {
                            Ok(Some(local_item))
                        } else {
                            self.bookmarks.insert(remote_bookmark.id.clone(), remote_bookmark);
                            Ok(None)
                        }
                    }
                    _ => Ok(None),
                }
            } else {
                Ok(None)
            }
        } else {
            // No conflict, just add
            self.bookmarks.insert(remote_bookmark.id.clone(), remote_bookmark);
            Ok(None)
        }
    }
}

impl Default for BookmarkSync {
    fn default() -> Self {
        Self::new()
    }
}

// Serialization helpers

fn serialize_string(data: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(bytes);
}

fn serialize_option_string(data: &mut Vec<u8>, s: &Option<String>) {
    if let Some(ref s) = s {
        data.push(1);
        serialize_string(data, s);
    } else {
        data.push(0);
    }
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

fn deserialize_option_string(data: &[u8], cursor: &mut usize) -> Option<Option<String>> {
    if *cursor >= data.len() { return None; }
    let has_value = data[*cursor];
    *cursor += 1;
    
    if has_value == 1 {
        Some(Some(deserialize_string(data, cursor)?))
    } else {
        Some(None)
    }
}

use alloc::string::ToString as _;
