//! File Explorer
//!
//! File browser with navigation, file operations, and views.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// File explorer instance
#[derive(Debug, Clone)]
pub struct FileExplorer {
    /// Current path
    pub current_path: String,
    /// Navigation history (back)
    pub history_back: Vec<String>,
    /// Navigation history (forward)
    pub history_forward: Vec<String>,
    /// Current view mode
    pub view_mode: ViewMode,
    /// Sort by
    pub sort_by: SortBy,
    /// Sort order
    pub sort_order: SortOrder,
    /// Show hidden files
    pub show_hidden: bool,
    /// Current entries
    pub entries: Vec<FileEntry>,
    /// Selected entries
    pub selected: Vec<usize>,
    /// Clipboard operation
    pub clipboard: Option<ClipboardOperation>,
    /// Sidebar items
    pub sidebar: Sidebar,
    /// Preview panel open
    pub preview_open: bool,
    /// Address bar mode
    pub address_bar_mode: AddressBarMode,
    /// Search query
    pub search_query: Option<String>,
    /// Is searching
    pub searching: bool,
}

impl FileExplorer {
    /// Create new file explorer
    pub fn new() -> Self {
        Self {
            current_path: String::from("/home"),
            history_back: Vec::new(),
            history_forward: Vec::new(),
            view_mode: ViewMode::Icons,
            sort_by: SortBy::Name,
            sort_order: SortOrder::Ascending,
            show_hidden: false,
            entries: Vec::new(),
            selected: Vec::new(),
            clipboard: None,
            sidebar: Sidebar::default(),
            preview_open: false,
            address_bar_mode: AddressBarMode::Breadcrumb,
            search_query: None,
            searching: false,
        }
    }

    /// Navigate to path
    pub fn navigate(&mut self, path: &str) {
        // Add current to history
        self.history_back.push(self.current_path.clone());
        self.history_forward.clear();
        
        // Update path
        self.current_path = path.to_string();
        self.selected.clear();
        
        // Load entries would happen here
        self.entries.clear();
    }

    /// Go back in history
    pub fn go_back(&mut self) -> bool {
        if let Some(path) = self.history_back.pop() {
            self.history_forward.push(self.current_path.clone());
            self.current_path = path;
            self.selected.clear();
            true
        } else {
            false
        }
    }

    /// Go forward in history
    pub fn go_forward(&mut self) -> bool {
        if let Some(path) = self.history_forward.pop() {
            self.history_back.push(self.current_path.clone());
            self.current_path = path;
            self.selected.clear();
            true
        } else {
            false
        }
    }

    /// Go to parent directory
    pub fn go_up(&mut self) -> bool {
        if self.current_path == "/" {
            return false;
        }

        if let Some(idx) = self.current_path.rfind('/') {
            let parent = if idx == 0 {
                String::from("/")
            } else {
                self.current_path[..idx].to_string()
            };
            self.navigate(&parent);
            true
        } else {
            false
        }
    }

    /// Select entry
    pub fn select(&mut self, index: usize, extend: bool, range: bool) {
        if index >= self.entries.len() {
            return;
        }

        if range && !self.selected.is_empty() {
            let last = *self.selected.last().unwrap();
            let start = last.min(index);
            let end = last.max(index);
            for i in start..=end {
                if !self.selected.contains(&i) {
                    self.selected.push(i);
                }
            }
        } else if extend {
            if let Some(pos) = self.selected.iter().position(|&i| i == index) {
                self.selected.remove(pos);
            } else {
                self.selected.push(index);
            }
        } else {
            self.selected = alloc::vec![index];
        }
    }

    /// Select all
    pub fn select_all(&mut self) {
        self.selected = (0..self.entries.len()).collect();
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    /// Get selected entries
    pub fn selected_entries(&self) -> Vec<&FileEntry> {
        self.selected
            .iter()
            .filter_map(|&i| self.entries.get(i))
            .collect()
    }

    /// Cut selected files
    pub fn cut(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        let paths: Vec<String> = self.selected_entries()
            .iter()
            .map(|e| e.path.clone())
            .collect();
        self.clipboard = Some(ClipboardOperation::Cut(paths));
    }

    /// Copy selected files
    pub fn copy(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        let paths: Vec<String> = self.selected_entries()
            .iter()
            .map(|e| e.path.clone())
            .collect();
        self.clipboard = Some(ClipboardOperation::Copy(paths));
    }

    /// Paste files (returns operation details)
    pub fn paste(&mut self) -> Option<PasteOperation> {
        let clipboard = self.clipboard.take()?;
        let destination = self.current_path.clone();
        
        match clipboard {
            ClipboardOperation::Cut(paths) => Some(PasteOperation {
                action: PasteAction::Move,
                sources: paths,
                destination,
            }),
            ClipboardOperation::Copy(paths) => {
                // Put back for next paste
                self.clipboard = Some(ClipboardOperation::Copy(paths.clone()));
                Some(PasteOperation {
                    action: PasteAction::Copy,
                    sources: paths,
                    destination,
                })
            }
        }
    }

    /// Delete selected files
    pub fn delete(&self) -> Vec<String> {
        self.selected_entries()
            .iter()
            .map(|e| e.path.clone())
            .collect()
    }

    /// Create new folder
    pub fn new_folder(&self) -> String {
        alloc::format!("{}/새 폴더", self.current_path)
    }

    /// Create new file
    pub fn new_file(&self) -> String {
        alloc::format!("{}/새 파일.txt", self.current_path)
    }

    /// Set view mode
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
    }

    /// Set sort
    pub fn set_sort(&mut self, sort_by: SortBy, order: SortOrder) {
        self.sort_by = sort_by;
        self.sort_order = order;
        self.sort_entries();
    }

    /// Sort entries
    fn sort_entries(&mut self) {
        let order_mult = match self.sort_order {
            SortOrder::Ascending => 1,
            SortOrder::Descending => -1,
        };

        self.entries.sort_by(|a, b| {
            // Folders first
            if a.is_directory != b.is_directory {
                return if a.is_directory {
                    core::cmp::Ordering::Less
                } else {
                    core::cmp::Ordering::Greater
                };
            }

            let ord = match self.sort_by {
                SortBy::Name => a.name.cmp(&b.name),
                SortBy::Size => a.size.cmp(&b.size),
                SortBy::Modified => a.modified.cmp(&b.modified),
                SortBy::Type => a.extension.cmp(&b.extension),
            };

            if order_mult < 0 {
                ord.reverse()
            } else {
                ord
            }
        });
    }

    /// Start search
    pub fn start_search(&mut self, query: &str) {
        self.search_query = Some(query.to_string());
        self.searching = true;
    }

    /// Cancel search
    pub fn cancel_search(&mut self) {
        self.search_query = None;
        self.searching = false;
    }
}

impl Default for FileExplorer {
    fn default() -> Self {
        Self::new()
    }
}

/// File entry
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// File name
    pub name: String,
    /// Full path
    pub path: String,
    /// Is directory
    pub is_directory: bool,
    /// File extension
    pub extension: Option<String>,
    /// Size in bytes
    pub size: u64,
    /// Modified timestamp
    pub modified: u64,
    /// Is hidden
    pub hidden: bool,
    /// Is symlink
    pub symlink: bool,
    /// Symlink target
    pub symlink_target: Option<String>,
    /// Thumbnail URL (for images)
    pub thumbnail: Option<String>,
}

impl FileEntry {
    /// Get icon for entry
    pub fn icon(&self) -> &'static str {
        if self.is_directory {
            return "folder";
        }

        match self.extension.as_deref() {
            Some("txt" | "md" | "rtf") => "file-text",
            Some("pdf") => "file-text",
            Some("doc" | "docx" | "odt") => "file-text",
            Some("xls" | "xlsx" | "ods") => "file-spreadsheet",
            Some("ppt" | "pptx" | "odp") => "file-presentation",
            Some("jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "bmp") => "image",
            Some("mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a") => "music",
            Some("mp4" | "mkv" | "avi" | "mov" | "webm" | "wmv") => "video",
            Some("zip" | "tar" | "gz" | "7z" | "rar" | "bz2") => "archive",
            Some("rs" | "js" | "ts" | "py" | "java" | "c" | "cpp" | "h") => "code",
            Some("html" | "css" | "scss") => "code",
            Some("json" | "xml" | "yaml" | "toml") => "settings",
            Some("exe" | "app" | "sh" | "bat") => "app",
            _ => "file",
        }
    }

    /// Format size
    pub fn format_size(&self) -> String {
        if self.is_directory {
            return String::from("--");
        }

        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        const TB: u64 = GB * 1024;

        if self.size >= TB {
            alloc::format!("{:.1} TB", self.size as f64 / TB as f64)
        } else if self.size >= GB {
            alloc::format!("{:.1} GB", self.size as f64 / GB as f64)
        } else if self.size >= MB {
            alloc::format!("{:.1} MB", self.size as f64 / MB as f64)
        } else if self.size >= KB {
            alloc::format!("{:.1} KB", self.size as f64 / KB as f64)
        } else {
            alloc::format!("{} B", self.size)
        }
    }
}

/// View mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// Small icons
    SmallIcons,
    /// Medium icons
    #[default]
    Icons,
    /// Large icons
    LargeIcons,
    /// List view
    List,
    /// Details view
    Details,
    /// Tiles view
    Tiles,
    /// Content view
    Content,
}

/// Sort by
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortBy {
    #[default]
    Name,
    Size,
    Modified,
    Type,
}

/// Sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

/// Clipboard operation
#[derive(Debug, Clone)]
pub enum ClipboardOperation {
    Cut(Vec<String>),
    Copy(Vec<String>),
}

/// Paste operation details
#[derive(Debug, Clone)]
pub struct PasteOperation {
    /// Action type
    pub action: PasteAction,
    /// Source paths
    pub sources: Vec<String>,
    /// Destination path
    pub destination: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteAction {
    Copy,
    Move,
}

/// Address bar mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AddressBarMode {
    #[default]
    Breadcrumb,
    Path,
}

/// Sidebar
#[derive(Debug, Clone)]
pub struct Sidebar {
    /// Quick access items
    pub quick_access: Vec<SidebarItem>,
    /// Favorites
    pub favorites: Vec<SidebarItem>,
    /// Drives/Volumes
    pub drives: Vec<SidebarItem>,
    /// Recent folders
    pub recent: Vec<SidebarItem>,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            quick_access: alloc::vec![
                SidebarItem {
                    name: String::from("홈"),
                    path: String::from("/home"),
                    icon: String::from("home"),
                    item_type: SidebarItemType::QuickAccess,
                },
                SidebarItem {
                    name: String::from("문서"),
                    path: String::from("/home/documents"),
                    icon: String::from("file-text"),
                    item_type: SidebarItemType::QuickAccess,
                },
                SidebarItem {
                    name: String::from("다운로드"),
                    path: String::from("/home/downloads"),
                    icon: String::from("download"),
                    item_type: SidebarItemType::QuickAccess,
                },
                SidebarItem {
                    name: String::from("사진"),
                    path: String::from("/home/pictures"),
                    icon: String::from("image"),
                    item_type: SidebarItemType::QuickAccess,
                },
                SidebarItem {
                    name: String::from("음악"),
                    path: String::from("/home/music"),
                    icon: String::from("music"),
                    item_type: SidebarItemType::QuickAccess,
                },
                SidebarItem {
                    name: String::from("동영상"),
                    path: String::from("/home/videos"),
                    icon: String::from("video"),
                    item_type: SidebarItemType::QuickAccess,
                },
            ],
            favorites: Vec::new(),
            drives: alloc::vec![
                SidebarItem {
                    name: String::from("시스템"),
                    path: String::from("/"),
                    icon: String::from("hard-drive"),
                    item_type: SidebarItemType::Drive,
                },
            ],
            recent: Vec::new(),
        }
    }
}

/// Sidebar item
#[derive(Debug, Clone)]
pub struct SidebarItem {
    /// Display name
    pub name: String,
    /// Path
    pub path: String,
    /// Icon
    pub icon: String,
    /// Item type
    pub item_type: SidebarItemType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarItemType {
    QuickAccess,
    Favorite,
    Drive,
    Recent,
}

/// File properties
#[derive(Debug, Clone)]
pub struct FileProperties {
    /// Entry
    pub entry: FileEntry,
    /// MIME type
    pub mime_type: Option<String>,
    /// Created timestamp
    pub created: Option<u64>,
    /// Accessed timestamp
    pub accessed: Option<u64>,
    /// Permissions
    pub permissions: Option<String>,
    /// Owner
    pub owner: Option<String>,
    /// Group
    pub group: Option<String>,
}

/// File conflict resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    /// Replace existing
    Replace,
    /// Keep both (rename new)
    KeepBoth,
    /// Skip
    Skip,
    /// Apply to all
    ApplyToAll,
}
