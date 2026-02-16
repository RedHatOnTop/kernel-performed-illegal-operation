//! Bookmark Manager
//!
//! Bookmark bar, folder organization, and import/export.

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

/// Bookmark ID.
pub type BookmarkId = u64;

/// Bookmark node.
#[derive(Debug, Clone)]
pub enum BookmarkNode {
    /// URL bookmark.
    Bookmark(Bookmark),
    /// Folder.
    Folder(BookmarkFolder),
    /// Separator.
    Separator(BookmarkId),
}

impl BookmarkNode {
    /// Get ID.
    pub fn id(&self) -> BookmarkId {
        match self {
            Self::Bookmark(b) => b.id,
            Self::Folder(f) => f.id,
            Self::Separator(id) => *id,
        }
    }

    /// Get parent ID.
    pub fn parent_id(&self) -> Option<BookmarkId> {
        match self {
            Self::Bookmark(b) => b.parent_id,
            Self::Folder(f) => f.parent_id,
            Self::Separator(_) => None,
        }
    }
}

/// URL bookmark.
#[derive(Debug, Clone)]
pub struct Bookmark {
    /// ID.
    pub id: BookmarkId,
    /// Parent folder ID.
    pub parent_id: Option<BookmarkId>,
    /// Title.
    pub title: String,
    /// URL.
    pub url: String,
    /// Favicon.
    pub favicon: Option<String>,
    /// Date added (timestamp).
    pub date_added: u64,
    /// Last modified.
    pub date_modified: u64,
    /// Position in parent.
    pub index: u32,
}

impl Bookmark {
    /// Create a new bookmark.
    pub fn new(id: BookmarkId, title: &str, url: &str) -> Self {
        Self {
            id,
            parent_id: None,
            title: title.to_string(),
            url: url.to_string(),
            favicon: None,
            date_added: 0,
            date_modified: 0,
            index: 0,
        }
    }
}

/// Bookmark folder.
#[derive(Debug, Clone)]
pub struct BookmarkFolder {
    /// ID.
    pub id: BookmarkId,
    /// Parent folder ID.
    pub parent_id: Option<BookmarkId>,
    /// Title.
    pub title: String,
    /// Children IDs.
    pub children: Vec<BookmarkId>,
    /// Date added.
    pub date_added: u64,
    /// Date modified.
    pub date_modified: u64,
    /// Position in parent.
    pub index: u32,
}

impl BookmarkFolder {
    /// Create a new folder.
    pub fn new(id: BookmarkId, title: &str) -> Self {
        Self {
            id,
            parent_id: None,
            title: title.to_string(),
            children: Vec::new(),
            date_added: 0,
            date_modified: 0,
            index: 0,
        }
    }
}

/// Special folder type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialFolder {
    /// Bookmarks bar.
    BookmarksBar,
    /// Other bookmarks.
    OtherBookmarks,
    /// Mobile bookmarks (synced from mobile).
    MobileBookmarks,
}

/// Bookmark manager.
pub struct BookmarkManager {
    /// All bookmarks.
    nodes: RwLock<BTreeMap<BookmarkId, BookmarkNode>>,
    /// Next ID.
    next_id: RwLock<BookmarkId>,
    /// Bookmarks bar folder ID.
    bookmarks_bar_id: BookmarkId,
    /// Other bookmarks folder ID.
    other_bookmarks_id: BookmarkId,
    /// Mobile bookmarks folder ID.
    mobile_bookmarks_id: BookmarkId,
}

impl BookmarkManager {
    /// Create a new bookmark manager.
    pub fn new() -> Self {
        let mut nodes = BTreeMap::new();

        // Create root folders
        let bar = BookmarkFolder {
            id: 1,
            parent_id: None,
            title: "Bookmarks Bar".to_string(),
            children: Vec::new(),
            date_added: 0,
            date_modified: 0,
            index: 0,
        };
        let other = BookmarkFolder {
            id: 2,
            parent_id: None,
            title: "Other Bookmarks".to_string(),
            children: Vec::new(),
            date_added: 0,
            date_modified: 0,
            index: 1,
        };
        let mobile = BookmarkFolder {
            id: 3,
            parent_id: None,
            title: "Mobile Bookmarks".to_string(),
            children: Vec::new(),
            date_added: 0,
            date_modified: 0,
            index: 2,
        };

        nodes.insert(1, BookmarkNode::Folder(bar));
        nodes.insert(2, BookmarkNode::Folder(other));
        nodes.insert(3, BookmarkNode::Folder(mobile));

        Self {
            nodes: RwLock::new(nodes),
            next_id: RwLock::new(4),
            bookmarks_bar_id: 1,
            other_bookmarks_id: 2,
            mobile_bookmarks_id: 3,
        }
    }

    /// Get special folder ID.
    pub fn get_special_folder(&self, folder: SpecialFolder) -> BookmarkId {
        match folder {
            SpecialFolder::BookmarksBar => self.bookmarks_bar_id,
            SpecialFolder::OtherBookmarks => self.other_bookmarks_id,
            SpecialFolder::MobileBookmarks => self.mobile_bookmarks_id,
        }
    }

    /// Create a bookmark.
    pub fn create_bookmark(
        &self,
        parent_id: BookmarkId,
        title: &str,
        url: &str,
    ) -> Option<BookmarkId> {
        let mut next_id = self.next_id.write();
        let id = *next_id;
        *next_id += 1;
        drop(next_id);

        let mut nodes = self.nodes.write();

        // Get parent folder
        let parent = nodes.get_mut(&parent_id)?;
        let children = match parent {
            BookmarkNode::Folder(f) => &mut f.children,
            _ => return None,
        };

        let index = children.len() as u32;
        children.push(id);

        let bookmark = Bookmark {
            id,
            parent_id: Some(parent_id),
            title: title.to_string(),
            url: url.to_string(),
            favicon: None,
            date_added: 0,
            date_modified: 0,
            index,
        };

        nodes.insert(id, BookmarkNode::Bookmark(bookmark));
        Some(id)
    }

    /// Create a folder.
    pub fn create_folder(&self, parent_id: BookmarkId, title: &str) -> Option<BookmarkId> {
        let mut next_id = self.next_id.write();
        let id = *next_id;
        *next_id += 1;
        drop(next_id);

        let mut nodes = self.nodes.write();

        // Get parent folder
        let parent = nodes.get_mut(&parent_id)?;
        let children = match parent {
            BookmarkNode::Folder(f) => &mut f.children,
            _ => return None,
        };

        let index = children.len() as u32;
        children.push(id);

        let folder = BookmarkFolder {
            id,
            parent_id: Some(parent_id),
            title: title.to_string(),
            children: Vec::new(),
            date_added: 0,
            date_modified: 0,
            index,
        };

        nodes.insert(id, BookmarkNode::Folder(folder));
        Some(id)
    }

    /// Delete a bookmark or folder.
    pub fn delete(&self, id: BookmarkId) -> bool {
        // Don't allow deleting special folders
        if id <= 3 {
            return false;
        }

        let mut nodes = self.nodes.write();

        // Get parent and remove from children
        let node_clone = nodes.get(&id).cloned();
        if let Some(node) = node_clone {
            if let Some(parent_id) = node.parent_id() {
                if let Some(BookmarkNode::Folder(parent)) = nodes.get_mut(&parent_id) {
                    parent.children.retain(|&child_id| child_id != id);
                }
            }

            // If folder, recursively delete children
            if let BookmarkNode::Folder(folder) = node.clone() {
                for child_id in folder.children {
                    drop(nodes);
                    self.delete(child_id);
                    nodes = self.nodes.write();
                }
            }

            nodes.remove(&id);
            true
        } else {
            false
        }
    }

    /// Update bookmark.
    pub fn update_bookmark(&self, id: BookmarkId, title: Option<&str>, url: Option<&str>) -> bool {
        let mut nodes = self.nodes.write();

        if let Some(BookmarkNode::Bookmark(bookmark)) = nodes.get_mut(&id) {
            if let Some(title) = title {
                bookmark.title = title.to_string();
            }
            if let Some(url) = url {
                bookmark.url = url.to_string();
            }
            bookmark.date_modified = 0; // Would use current timestamp
            true
        } else {
            false
        }
    }

    /// Update folder.
    pub fn update_folder(&self, id: BookmarkId, title: &str) -> bool {
        let mut nodes = self.nodes.write();

        if let Some(BookmarkNode::Folder(folder)) = nodes.get_mut(&id) {
            folder.title = title.to_string();
            folder.date_modified = 0;
            true
        } else {
            false
        }
    }

    /// Move bookmark/folder.
    pub fn move_node(&self, id: BookmarkId, new_parent_id: BookmarkId, index: Option<u32>) -> bool {
        if id <= 3 {
            return false; // Can't move special folders
        }

        let mut nodes = self.nodes.write();

        // Get current parent
        let old_parent_id = match nodes.get(&id).and_then(|n| n.parent_id()) {
            Some(p) => p,
            None => return false,
        };

        // Remove from old parent
        if let Some(BookmarkNode::Folder(old_parent)) = nodes.get_mut(&old_parent_id) {
            old_parent.children.retain(|&child_id| child_id != id);
        }

        // Add to new parent
        if let Some(BookmarkNode::Folder(new_parent)) = nodes.get_mut(&new_parent_id) {
            let insert_idx = index.unwrap_or(new_parent.children.len() as u32) as usize;
            let insert_idx = insert_idx.min(new_parent.children.len());
            new_parent.children.insert(insert_idx, id);
        } else {
            return false;
        }

        // Update node's parent
        match nodes.get_mut(&id) {
            Some(BookmarkNode::Bookmark(b)) => b.parent_id = Some(new_parent_id),
            Some(BookmarkNode::Folder(f)) => f.parent_id = Some(new_parent_id),
            _ => return false,
        }

        true
    }

    /// Get node by ID.
    pub fn get(&self, id: BookmarkId) -> Option<BookmarkNode> {
        self.nodes.read().get(&id).cloned()
    }

    /// Get children of folder.
    pub fn get_children(&self, folder_id: BookmarkId) -> Vec<BookmarkNode> {
        let nodes = self.nodes.read();

        match nodes.get(&folder_id) {
            Some(BookmarkNode::Folder(folder)) => folder
                .children
                .iter()
                .filter_map(|id| nodes.get(id).cloned())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Search bookmarks.
    pub fn search(&self, query: &str) -> Vec<Bookmark> {
        let query = query.to_lowercase();
        let nodes = self.nodes.read();

        nodes
            .values()
            .filter_map(|node| match node {
                BookmarkNode::Bookmark(b) => {
                    if b.title.to_lowercase().contains(&query)
                        || b.url.to_lowercase().contains(&query)
                    {
                        Some(b.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect()
    }

    /// Get bookmarks bar items.
    pub fn get_bookmarks_bar(&self) -> Vec<BookmarkNode> {
        self.get_children(self.bookmarks_bar_id)
    }

    /// Export to Chrome bookmark format (JSON-like).
    pub fn export_chrome(&self) -> BookmarkExport {
        let nodes = self.nodes.read();

        fn export_folder(nodes: &BTreeMap<BookmarkId, BookmarkNode>, id: BookmarkId) -> ExportNode {
            if let Some(BookmarkNode::Folder(folder)) = nodes.get(&id) {
                ExportNode::Folder {
                    name: folder.title.clone(),
                    children: folder
                        .children
                        .iter()
                        .filter_map(|child_id| nodes.get(child_id))
                        .map(|node| match node {
                            BookmarkNode::Bookmark(b) => ExportNode::Url {
                                name: b.title.clone(),
                                url: b.url.clone(),
                            },
                            BookmarkNode::Folder(f) => export_folder(nodes, f.id),
                            BookmarkNode::Separator(id) => ExportNode::Separator { id: *id },
                        })
                        .collect(),
                }
            } else {
                ExportNode::Folder {
                    name: "Unknown".to_string(),
                    children: Vec::new(),
                }
            }
        }

        BookmarkExport {
            roots: ExportRoots {
                bookmark_bar: export_folder(&nodes, 1),
                other: export_folder(&nodes, 2),
                synced: export_folder(&nodes, 3),
            },
            version: 1,
        }
    }

    /// Import from Chrome bookmark format.
    pub fn import_chrome(&self, data: &BookmarkExport) {
        fn import_node(manager: &BookmarkManager, parent_id: BookmarkId, node: &ExportNode) {
            match node {
                ExportNode::Url { name, url } => {
                    manager.create_bookmark(parent_id, name, url);
                }
                ExportNode::Folder { name, children } => {
                    if let Some(folder_id) = manager.create_folder(parent_id, name) {
                        for child in children {
                            import_node(manager, folder_id, child);
                        }
                    }
                }
                ExportNode::Separator { .. } => {
                    // Skip separators for now
                }
            }
        }

        import_node(self, 1, &data.roots.bookmark_bar);
        import_node(self, 2, &data.roots.other);
    }
}

impl Default for BookmarkManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Export format.
#[derive(Debug, Clone)]
pub struct BookmarkExport {
    /// Roots.
    pub roots: ExportRoots,
    /// Version.
    pub version: u32,
}

/// Export roots.
#[derive(Debug, Clone)]
pub struct ExportRoots {
    /// Bookmark bar.
    pub bookmark_bar: ExportNode,
    /// Other bookmarks.
    pub other: ExportNode,
    /// Synced bookmarks.
    pub synced: ExportNode,
}

/// Export node.
#[derive(Debug, Clone)]
pub enum ExportNode {
    /// URL.
    Url { name: String, url: String },
    /// Folder.
    Folder {
        name: String,
        children: Vec<ExportNode>,
    },
    /// Separator.
    Separator { id: BookmarkId },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bookmark_manager() {
        let manager = BookmarkManager::new();

        // Create bookmark
        let bar_id = manager.get_special_folder(SpecialFolder::BookmarksBar);
        let bm_id = manager
            .create_bookmark(bar_id, "Google", "https://google.com")
            .unwrap();

        // Verify
        let bm = manager.get(bm_id).unwrap();
        match bm {
            BookmarkNode::Bookmark(b) => {
                assert_eq!(b.title, "Google");
                assert_eq!(b.url, "https://google.com");
            }
            _ => panic!("Expected bookmark"),
        }

        // Get children
        let children = manager.get_children(bar_id);
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn test_folders() {
        let manager = BookmarkManager::new();

        let bar_id = manager.get_special_folder(SpecialFolder::BookmarksBar);
        let folder_id = manager.create_folder(bar_id, "Work").unwrap();
        manager
            .create_bookmark(folder_id, "Jira", "https://jira.example.com")
            .unwrap();

        let children = manager.get_children(folder_id);
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn test_search() {
        let manager = BookmarkManager::new();

        let bar_id = manager.get_special_folder(SpecialFolder::BookmarksBar);
        manager
            .create_bookmark(bar_id, "Google", "https://google.com")
            .unwrap();
        manager
            .create_bookmark(bar_id, "Rust Lang", "https://rust-lang.org")
            .unwrap();

        let results = manager.search("rust");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Lang");
    }
}
