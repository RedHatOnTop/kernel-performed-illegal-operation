//! Browser Tab Process and Memory Management
//!
//! This module implements the Tab = Process model and memory pressure handling
//! for browser tabs running as isolated WASM processes.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::{Mutex, RwLock};
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};

/// Tab process identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TabId(pub u64);

impl TabId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// Tab process state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabState {
    /// Tab is loading content.
    Loading,
    /// Tab is active and rendering.
    Active,
    /// Tab is in background but running.
    Background,
    /// Tab is suspended (frozen).
    Suspended,
    /// Tab is being compressed.
    Compressing,
    /// Tab is compressed.
    Compressed,
    /// Tab is being restored.
    Restoring,
    /// Tab has crashed.
    Crashed,
    /// Tab is closing.
    Closing,
}

/// Memory pressure level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryPressure {
    /// Normal operation.
    None = 0,
    /// Moderate pressure - consider releasing caches.
    Moderate = 1,
    /// Critical pressure - suspend/compress tabs.
    Critical = 2,
    /// Emergency - aggressive tab killing.
    Emergency = 3,
}

/// Tab memory statistics.
#[derive(Debug, Clone, Default)]
pub struct TabMemoryStats {
    /// WASM linear memory size.
    pub wasm_memory: u64,
    /// DOM tree memory.
    pub dom_memory: u64,
    /// Style data memory.
    pub style_memory: u64,
    /// Layout tree memory.
    pub layout_memory: u64,
    /// JavaScript heap size.
    pub js_heap: u64,
    /// Image cache size.
    pub image_cache: u64,
    /// Other memory.
    pub other: u64,
    /// Compressed size (if compressed).
    pub compressed_size: Option<u64>,
}

impl TabMemoryStats {
    /// Get total memory usage.
    pub fn total(&self) -> u64 {
        self.wasm_memory + self.dom_memory + self.style_memory +
        self.layout_memory + self.js_heap + self.image_cache + self.other
    }
    
    /// Get compression ratio.
    pub fn compression_ratio(&self) -> Option<f32> {
        self.compressed_size.map(|c| c as f32 / self.total() as f32)
    }
}

/// Tab process information.
pub struct TabProcess {
    /// Tab ID.
    pub id: TabId,
    /// Process ID (WASM instance ID).
    pub process_id: u64,
    /// Tab state.
    pub state: TabState,
    /// Tab title.
    pub title: String,
    /// Current URL.
    pub url: String,
    /// Memory statistics.
    pub memory: TabMemoryStats,
    /// Last active timestamp.
    pub last_active: u64,
    /// Creation timestamp.
    pub created_at: u64,
    /// Is tab pinned.
    pub pinned: bool,
    /// Tab priority (higher = more important).
    pub priority: u32,
    /// Compressed state data.
    compressed_state: Option<Vec<u8>>,
    /// Parent tab ID (for popups).
    pub parent: Option<TabId>,
    /// Child tab IDs.
    pub children: Vec<TabId>,
    /// Capability rights.
    pub capabilities: u64,
}

impl TabProcess {
    /// Create new tab process.
    pub fn new(id: TabId, process_id: u64, url: &str) -> Self {
        Self {
            id,
            process_id,
            state: TabState::Loading,
            title: String::new(),
            url: url.to_string(),
            memory: TabMemoryStats::default(),
            last_active: 0,
            created_at: 0,
            pinned: false,
            priority: 50,
            compressed_state: None,
            parent: None,
            children: Vec::new(),
            capabilities: 0,
        }
    }
    
    /// Check if tab can be suspended.
    pub fn can_suspend(&self) -> bool {
        !self.pinned && 
        self.state == TabState::Background &&
        self.children.is_empty()
    }
    
    /// Check if tab can be compressed.
    pub fn can_compress(&self) -> bool {
        self.state == TabState::Suspended &&
        self.compressed_state.is_none()
    }
    
    /// Check if tab can be killed for memory.
    pub fn can_kill(&self) -> bool {
        !self.pinned &&
        (self.state == TabState::Compressed || self.state == TabState::Suspended)
    }
    
    /// Get effective priority (considering state).
    pub fn effective_priority(&self) -> u32 {
        let mut priority = self.priority;
        
        match self.state {
            TabState::Active => priority += 100,
            TabState::Loading => priority += 80,
            TabState::Background => priority += 0,
            TabState::Suspended => priority -= 20,
            TabState::Compressed => priority -= 40,
            _ => {}
        }
        
        if self.pinned {
            priority += 50;
        }
        
        priority
    }
}

/// Tab process manager.
pub struct TabManager {
    /// Active tabs.
    tabs: RwLock<BTreeMap<TabId, TabProcess>>,
    /// Currently foreground tab.
    foreground_tab: Mutex<Option<TabId>>,
    /// Next tab ID.
    next_id: AtomicU64,
    /// Total memory budget.
    memory_budget: AtomicU64,
    /// Current memory usage.
    current_memory: AtomicU64,
    /// Memory pressure level.
    pressure_level: Mutex<MemoryPressure>,
    /// Compression enabled.
    compression_enabled: AtomicBool,
    /// Maximum tabs before auto-discard.
    max_tabs: u32,
}

impl TabManager {
    /// Create new tab manager.
    pub fn new(memory_budget: u64) -> Self {
        Self {
            tabs: RwLock::new(BTreeMap::new()),
            foreground_tab: Mutex::new(None),
            next_id: AtomicU64::new(1),
            memory_budget: AtomicU64::new(memory_budget),
            current_memory: AtomicU64::new(0),
            pressure_level: Mutex::new(MemoryPressure::None),
            compression_enabled: AtomicBool::new(true),
            max_tabs: 100,
        }
    }
    
    /// Create a new tab.
    pub fn create_tab(&self, url: &str, parent: Option<TabId>) -> TabId {
        let id = TabId(self.next_id.fetch_add(1, Ordering::SeqCst));
        let process_id = id.0; // In real impl, allocate WASM process
        
        let mut tab = TabProcess::new(id, process_id, url);
        tab.parent = parent;
        
        let mut tabs = self.tabs.write();
        
        // Add to parent's children
        if let Some(parent_id) = parent {
            if let Some(parent_tab) = tabs.get_mut(&parent_id) {
                parent_tab.children.push(id);
            }
        }
        
        tabs.insert(id, tab);
        
        // Check if we need to free memory
        drop(tabs);
        self.check_memory_pressure();
        
        id
    }
    
    /// Close a tab.
    pub fn close_tab(&self, id: TabId) -> bool {
        let mut tabs = self.tabs.write();
        
        if let Some(mut tab) = tabs.remove(&id) {
            tab.state = TabState::Closing;
            
            // Update memory tracking
            let freed = tab.memory.total();
            self.current_memory.fetch_sub(freed, Ordering::SeqCst);
            
            // Remove from parent's children
            if let Some(parent_id) = tab.parent {
                if let Some(parent) = tabs.get_mut(&parent_id) {
                    parent.children.retain(|&c| c != id);
                }
            }
            
            // Close child tabs
            for child_id in tab.children.clone() {
                drop(tabs);
                self.close_tab(child_id);
                tabs = self.tabs.write();
            }
            
            // Update foreground if needed
            {
                let mut fg = self.foreground_tab.lock();
                if *fg == Some(id) {
                    *fg = None;
                }
            }
            
            true
        } else {
            false
        }
    }
    
    /// Set foreground tab.
    pub fn set_foreground(&self, id: TabId) {
        let mut tabs = self.tabs.write();
        
        // Background previous foreground tab
        {
            let fg = self.foreground_tab.lock();
            if let Some(prev_id) = *fg {
                if prev_id != id {
                    if let Some(prev) = tabs.get_mut(&prev_id) {
                        prev.state = TabState::Background;
                    }
                }
            }
        }
        
        // Activate new tab
        if let Some(tab) = tabs.get_mut(&id) {
            // Restore if compressed
            if tab.state == TabState::Compressed {
                tab.state = TabState::Restoring;
                // In real impl, decompress and restore WASM state
                tab.compressed_state = None;
            } else if tab.state == TabState::Suspended {
                // Resume
            }
            
            tab.state = TabState::Active;
            tab.last_active = 0; // Update timestamp
            
            *self.foreground_tab.lock() = Some(id);
        }
    }
    
    /// Get foreground tab.
    pub fn get_foreground(&self) -> Option<TabId> {
        *self.foreground_tab.lock()
    }
    
    /// Update tab memory stats.
    pub fn update_memory(&self, id: TabId, stats: TabMemoryStats) {
        let mut tabs = self.tabs.write();
        
        if let Some(tab) = tabs.get_mut(&id) {
            let old_total = tab.memory.total();
            let new_total = stats.total();
            
            tab.memory = stats;
            
            // Update global tracking
            if new_total > old_total {
                self.current_memory.fetch_add(new_total - old_total, Ordering::SeqCst);
            } else {
                self.current_memory.fetch_sub(old_total - new_total, Ordering::SeqCst);
            }
        }
        
        drop(tabs);
        self.check_memory_pressure();
    }
    
    /// Check and handle memory pressure.
    pub fn check_memory_pressure(&self) {
        let current = self.current_memory.load(Ordering::SeqCst);
        let budget = self.memory_budget.load(Ordering::SeqCst);
        
        let ratio = current as f32 / budget as f32;
        
        let pressure = if ratio < 0.7 {
            MemoryPressure::None
        } else if ratio < 0.85 {
            MemoryPressure::Moderate
        } else if ratio < 0.95 {
            MemoryPressure::Critical
        } else {
            MemoryPressure::Emergency
        };
        
        let old_pressure = *self.pressure_level.lock();
        *self.pressure_level.lock() = pressure;
        
        if pressure > old_pressure {
            self.handle_memory_pressure(pressure);
        }
    }
    
    /// Handle memory pressure.
    fn handle_memory_pressure(&self, level: MemoryPressure) {
        match level {
            MemoryPressure::None => {}
            MemoryPressure::Moderate => {
                // Clear caches
                self.clear_caches();
            }
            MemoryPressure::Critical => {
                // Suspend background tabs
                self.suspend_background_tabs();
                // Compress suspended tabs
                if self.compression_enabled.load(Ordering::SeqCst) {
                    self.compress_suspended_tabs();
                }
            }
            MemoryPressure::Emergency => {
                // Kill lowest priority tabs
                self.kill_low_priority_tabs();
            }
        }
    }
    
    /// Clear tab caches.
    fn clear_caches(&self) {
        let mut tabs = self.tabs.write();
        
        for (_, tab) in tabs.iter_mut() {
            if tab.state == TabState::Background {
                // Clear image cache
                tab.memory.image_cache = 0;
                // Signal to tab to clear its caches
            }
        }
    }
    
    /// Suspend background tabs.
    fn suspend_background_tabs(&self) {
        let mut tabs = self.tabs.write();
        
        // Sort by priority (lowest first)
        let mut background: Vec<_> = tabs.iter()
            .filter(|(_, t)| t.can_suspend())
            .map(|(id, t)| (*id, t.effective_priority()))
            .collect();
        
        background.sort_by_key(|(_, p)| *p);
        
        // Suspend lowest priority tabs
        for (id, _) in background.iter().take(3) {
            if let Some(tab) = tabs.get_mut(id) {
                tab.state = TabState::Suspended;
                // In real impl, pause WASM execution
            }
        }
    }
    
    /// Compress suspended tabs.
    fn compress_suspended_tabs(&self) {
        let mut tabs = self.tabs.write();
        
        for (_, tab) in tabs.iter_mut() {
            if tab.can_compress() {
                tab.state = TabState::Compressing;
                
                // In real impl, serialize and compress WASM memory
                // For now, simulate compression
                let uncompressed_size = tab.memory.total();
                let compressed_size = uncompressed_size / 3; // ~33% compression
                
                tab.compressed_state = Some(Vec::with_capacity(compressed_size as usize));
                tab.memory.compressed_size = Some(compressed_size);
                
                // Free original memory
                let freed = uncompressed_size - compressed_size;
                self.current_memory.fetch_sub(freed, Ordering::SeqCst);
                
                tab.state = TabState::Compressed;
            }
        }
    }
    
    /// Kill low priority tabs.
    fn kill_low_priority_tabs(&self) {
        let tabs = self.tabs.read();
        
        // Find killable tabs sorted by priority
        let mut killable: Vec<_> = tabs.iter()
            .filter(|(_, t)| t.can_kill())
            .map(|(id, t)| (*id, t.effective_priority()))
            .collect();
        
        killable.sort_by_key(|(_, p)| *p);
        
        drop(tabs);
        
        // Kill lowest priority tabs until memory is okay
        for (id, _) in killable {
            self.close_tab(id);
            
            let current = self.current_memory.load(Ordering::SeqCst);
            let budget = self.memory_budget.load(Ordering::SeqCst);
            
            if current < budget * 8 / 10 {
                break;
            }
        }
    }
    
    /// Get tab info.
    pub fn get_tab(&self, id: TabId) -> Option<TabInfo> {
        let tabs = self.tabs.read();
        tabs.get(&id).map(|t| TabInfo {
            id: t.id,
            state: t.state,
            title: t.title.clone(),
            url: t.url.clone(),
            memory: t.memory.total(),
            pinned: t.pinned,
            priority: t.priority,
        })
    }
    
    /// List all tabs.
    pub fn list_tabs(&self) -> Vec<TabInfo> {
        let tabs = self.tabs.read();
        tabs.values().map(|t| TabInfo {
            id: t.id,
            state: t.state,
            title: t.title.clone(),
            url: t.url.clone(),
            memory: t.memory.total(),
            pinned: t.pinned,
            priority: t.priority,
        }).collect()
    }
    
    /// Get memory statistics.
    pub fn memory_stats(&self) -> MemoryStats {
        let tabs = self.tabs.read();
        
        let mut total = 0u64;
        let mut active = 0u64;
        let mut suspended = 0u64;
        let mut compressed = 0u64;
        
        for tab in tabs.values() {
            let mem = tab.memory.total();
            total += mem;
            
            match tab.state {
                TabState::Active | TabState::Loading => active += mem,
                TabState::Suspended => suspended += mem,
                TabState::Compressed => compressed += tab.memory.compressed_size.unwrap_or(0),
                _ => {}
            }
        }
        
        MemoryStats {
            total,
            active,
            suspended,
            compressed,
            budget: self.memory_budget.load(Ordering::SeqCst),
            pressure: *self.pressure_level.lock(),
        }
    }
    
    /// Pin/unpin a tab.
    pub fn set_pinned(&self, id: TabId, pinned: bool) {
        let mut tabs = self.tabs.write();
        if let Some(tab) = tabs.get_mut(&id) {
            tab.pinned = pinned;
        }
    }
}

/// Summary tab info.
#[derive(Debug, Clone)]
pub struct TabInfo {
    pub id: TabId,
    pub state: TabState,
    pub title: String,
    pub url: String,
    pub memory: u64,
    pub pinned: bool,
    pub priority: u32,
}

/// Memory statistics.
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub total: u64,
    pub active: u64,
    pub suspended: u64,
    pub compressed: u64,
    pub budget: u64,
    pub pressure: MemoryPressure,
}

/// Global tab manager instance.
static TAB_MANAGER: spin::Once<TabManager> = spin::Once::new();

/// Initialize tab manager.
pub fn init(memory_budget: u64) {
    TAB_MANAGER.call_once(|| TabManager::new(memory_budget));
}

/// Get tab manager.
pub fn manager() -> &'static TabManager {
    TAB_MANAGER.get().expect("Tab manager not initialized")
}
