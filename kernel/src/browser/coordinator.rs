//! Browser Coordinator
//!
//! The coordinator manages all browser tabs and routes messages
//! between tabs and kernel services.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use spin::{Mutex, RwLock};

use crate::ipc::{ChannelId, create_channel};

/// Tab identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TabId(pub u32);

impl TabId {
    /// Coordinator ID (always 0).
    pub const COORDINATOR: TabId = TabId(0);
}

/// Tab type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TabType {
    /// Main renderer process.
    Renderer = 0,
    /// Network/fetch process.
    Network = 1,
    /// Web worker.
    Worker = 2,
    /// Service worker.
    ServiceWorker = 3,
    /// GPU process.
    Gpu = 4,
}

impl From<u32> for TabType {
    fn from(val: u32) -> Self {
        match val {
            0 => TabType::Renderer,
            1 => TabType::Network,
            2 => TabType::Worker,
            3 => TabType::ServiceWorker,
            4 => TabType::Gpu,
            _ => TabType::Renderer,
        }
    }
}

/// Tab state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabState {
    /// Tab is initializing.
    Initializing,
    /// Tab is loading content.
    Loading,
    /// Tab is ready/idle.
    Ready,
    /// Tab is actively processing.
    Active,
    /// Tab is suspended (background).
    Suspended,
    /// Tab has crashed.
    Crashed,
    /// Tab is shutting down.
    ShuttingDown,
}

/// Tab memory statistics.
#[derive(Debug, Clone, Default)]
pub struct TabMemoryStats {
    /// Heap memory used.
    pub heap_bytes: u64,
    /// GPU memory used.
    pub gpu_bytes: u64,
    /// Shared memory mapped.
    pub shm_bytes: u64,
    /// Peak memory usage.
    pub peak_bytes: u64,
}

/// Tab channels for communication.
#[derive(Debug, Clone)]
pub struct TabChannels {
    /// Channel to coordinator (tab's end).
    pub to_coordinator: ChannelId,
    /// Channel from coordinator (coordinator's end).
    pub from_coordinator: ChannelId,
    /// GPU command channel.
    pub gpu_channel: Option<ChannelId>,
    /// Network channel.
    pub net_channel: Option<ChannelId>,
    /// Input event channel.
    pub input_channel: Option<ChannelId>,
}

/// Tab information.
pub struct TabInfo {
    /// Tab ID.
    pub id: TabId,
    /// Tab type.
    pub tab_type: TabType,
    /// Tab name/title.
    pub name: String,
    /// Process ID.
    pub pid: u64,
    /// Current state.
    pub state: TabState,
    /// Communication channels.
    pub channels: TabChannels,
    /// Memory statistics.
    pub memory: TabMemoryStats,
    /// Priority (for scheduling).
    pub priority: u32,
    /// Creation timestamp.
    pub created_at: u64,
    /// Last activity timestamp.
    pub last_active: u64,
}

impl TabInfo {
    /// Create new tab info.
    pub fn new(id: TabId, tab_type: TabType, name: &str, pid: u64, channels: TabChannels) -> Self {
        TabInfo {
            id,
            tab_type,
            name: String::from(name),
            pid,
            state: TabState::Initializing,
            channels,
            memory: TabMemoryStats::default(),
            priority: 16, // Normal priority
            created_at: 0, // TODO: Get timestamp
            last_active: 0,
        }
    }
    
    /// Update memory stats.
    pub fn update_memory(&mut self, heap: u64, gpu: u64, shm: u64) {
        self.memory.heap_bytes = heap;
        self.memory.gpu_bytes = gpu;
        self.memory.shm_bytes = shm;
        
        let total = heap + gpu + shm;
        if total > self.memory.peak_bytes {
            self.memory.peak_bytes = total;
        }
    }
    
    /// Total memory usage.
    pub fn total_memory(&self) -> u64 {
        self.memory.heap_bytes + self.memory.gpu_bytes + self.memory.shm_bytes
    }
}

/// Browser coordinator manages all tabs.
pub struct BrowserCoordinator {
    /// All registered tabs.
    tabs: BTreeMap<TabId, Arc<Mutex<TabInfo>>>,
    
    /// Next tab ID.
    next_tab_id: AtomicU32,
    
    /// Maximum tabs allowed.
    max_tabs: usize,
    
    /// Total GPU memory budget.
    gpu_memory_budget: u64,
    
    /// Currently used GPU memory.
    gpu_memory_used: AtomicU64,
    
    /// Active/focused tab.
    active_tab: Option<TabId>,
}

impl BrowserCoordinator {
    /// Create a new coordinator.
    pub fn new() -> Self {
        BrowserCoordinator {
            tabs: BTreeMap::new(),
            next_tab_id: AtomicU32::new(1),
            max_tabs: 64,
            gpu_memory_budget: 512 * 1024 * 1024, // 512MB
            gpu_memory_used: AtomicU64::new(0),
            active_tab: None,
        }
    }
    
    /// Register a new tab.
    pub fn register_tab(
        &mut self,
        tab_type: TabType,
        name: &str,
        pid: u64,
    ) -> Result<TabId, CoordinatorError> {
        // Check limits
        if self.tabs.len() >= self.max_tabs {
            return Err(CoordinatorError::TooManyTabs);
        }
        
        // Generate tab ID
        let tab_id = TabId(self.next_tab_id.fetch_add(1, Ordering::Relaxed));
        
        // Create channels
        let (coord_end, tab_end) = create_channel()
            .ok_or(CoordinatorError::ChannelCreationFailed)?;
        
        let channels = TabChannels {
            to_coordinator: tab_end,
            from_coordinator: coord_end,
            gpu_channel: None,
            net_channel: None,
            input_channel: None,
        };
        
        // Create tab info
        let tab = TabInfo::new(tab_id, tab_type, name, pid, channels);
        
        self.tabs.insert(tab_id, Arc::new(Mutex::new(tab)));
        
        // Set as active if first tab
        if self.active_tab.is_none() {
            self.active_tab = Some(tab_id);
        }
        
        crate::serial_println!("[Browser] Registered tab {} ({:?}): {}", tab_id.0, tab_type, name);
        
        Ok(tab_id)
    }
    
    /// Unregister a tab.
    pub fn unregister_tab(&mut self, tab_id: TabId) -> Result<(), CoordinatorError> {
        let tab = self.tabs.remove(&tab_id).ok_or(CoordinatorError::TabNotFound)?;
        
        // Update GPU memory tracking
        let tab = tab.lock();
        self.gpu_memory_used.fetch_sub(tab.memory.gpu_bytes, Ordering::Relaxed);
        
        // Clear active tab if needed
        if self.active_tab == Some(tab_id) {
            self.active_tab = self.tabs.keys().next().copied();
        }
        
        crate::serial_println!("[Browser] Unregistered tab {}", tab_id.0);
        
        Ok(())
    }
    
    /// Get tab info.
    pub fn get_tab(&self, tab_id: TabId) -> Option<Arc<Mutex<TabInfo>>> {
        self.tabs.get(&tab_id).cloned()
    }
    
    /// Update tab state.
    pub fn set_tab_state(&self, tab_id: TabId, state: TabState) -> Result<(), CoordinatorError> {
        let tab = self.tabs.get(&tab_id).ok_or(CoordinatorError::TabNotFound)?;
        tab.lock().state = state;
        Ok(())
    }
    
    /// Set active tab.
    pub fn set_active_tab(&mut self, tab_id: TabId) -> Result<(), CoordinatorError> {
        if !self.tabs.contains_key(&tab_id) {
            return Err(CoordinatorError::TabNotFound);
        }
        
        // Suspend previous active tab
        if let Some(prev) = self.active_tab {
            if prev != tab_id {
                if let Some(tab) = self.tabs.get(&prev) {
                    tab.lock().state = TabState::Suspended;
                }
            }
        }
        
        // Activate new tab
        if let Some(tab) = self.tabs.get(&tab_id) {
            tab.lock().state = TabState::Active;
        }
        
        self.active_tab = Some(tab_id);
        
        Ok(())
    }
    
    /// Get all tabs.
    pub fn list_tabs(&self) -> Vec<TabId> {
        self.tabs.keys().copied().collect()
    }
    
    /// Get tab count.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }
    
    /// Allocate GPU memory for a tab.
    pub fn alloc_gpu_memory(&self, tab_id: TabId, size: u64) -> Result<(), CoordinatorError> {
        let current = self.gpu_memory_used.load(Ordering::Relaxed);
        
        if current + size > self.gpu_memory_budget {
            return Err(CoordinatorError::GpuMemoryExhausted);
        }
        
        let tab = self.tabs.get(&tab_id).ok_or(CoordinatorError::TabNotFound)?;
        
        self.gpu_memory_used.fetch_add(size, Ordering::Relaxed);
        tab.lock().memory.gpu_bytes += size;
        
        Ok(())
    }
    
    /// Free GPU memory from a tab.
    pub fn free_gpu_memory(&self, tab_id: TabId, size: u64) -> Result<(), CoordinatorError> {
        let tab = self.tabs.get(&tab_id).ok_or(CoordinatorError::TabNotFound)?;
        
        self.gpu_memory_used.fetch_sub(size, Ordering::Relaxed);
        let mut tab = tab.lock();
        tab.memory.gpu_bytes = tab.memory.gpu_bytes.saturating_sub(size);
        
        Ok(())
    }
    
    /// Get total memory usage.
    pub fn total_memory_usage(&self) -> u64 {
        self.tabs.values()
            .map(|t| t.lock().total_memory())
            .sum()
    }
}

/// Coordinator error types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinatorError {
    /// Too many tabs.
    TooManyTabs,
    /// Tab not found.
    TabNotFound,
    /// Failed to create channel.
    ChannelCreationFailed,
    /// GPU memory exhausted.
    GpuMemoryExhausted,
    /// Invalid operation.
    InvalidOperation,
}

/// Global browser coordinator.
static BROWSER_COORDINATOR: RwLock<Option<BrowserCoordinator>> = RwLock::new(None);

/// Initialize the browser coordinator.
pub fn init() {
    let mut coord = BROWSER_COORDINATOR.write();
    *coord = Some(BrowserCoordinator::new());
    crate::serial_println!("[Browser] Coordinator initialized");
}

/// Register a tab.
pub fn register_tab(tab_type: u32, name: &str, pid: u64) -> Result<TabId, CoordinatorError> {
    BROWSER_COORDINATOR.write()
        .as_mut()
        .ok_or(CoordinatorError::InvalidOperation)?
        .register_tab(TabType::from(tab_type), name, pid)
}

/// Unregister a tab.
pub fn unregister_tab(tab_id: TabId) -> Result<(), CoordinatorError> {
    BROWSER_COORDINATOR.write()
        .as_mut()
        .ok_or(CoordinatorError::InvalidOperation)?
        .unregister_tab(tab_id)
}

/// Get tab info.
pub fn get_tab(tab_id: TabId) -> Option<Arc<Mutex<TabInfo>>> {
    BROWSER_COORDINATOR.read().as_ref()?.get_tab(tab_id)
}

/// Set tab state.
pub fn set_tab_state(tab_id: TabId, state: TabState) -> Result<(), CoordinatorError> {
    BROWSER_COORDINATOR.read()
        .as_ref()
        .ok_or(CoordinatorError::InvalidOperation)?
        .set_tab_state(tab_id, state)
}

/// List all tabs.
pub fn list_tabs() -> Vec<TabId> {
    BROWSER_COORDINATOR.read()
        .as_ref()
        .map(|c| c.list_tabs())
        .unwrap_or_default()
}
