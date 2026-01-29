//! Open Tabs Synchronization
//!
//! Cross-device tab sync and send-to-device functionality.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{SyncItem, SyncItemType, SyncError};

/// Synced tab
#[derive(Debug, Clone)]
pub struct SyncedTab {
    /// Tab ID
    pub id: String,
    /// URL
    pub url: String,
    /// Title
    pub title: String,
    /// Favicon URL
    pub favicon: Option<String>,
    /// Last accessed timestamp
    pub last_accessed: u64,
    /// Scroll position
    pub scroll_position: u32,
}

impl SyncedTab {
    /// Create new synced tab
    pub fn new(id: String, url: String, title: String) -> Self {
        Self {
            id,
            url,
            title,
            favicon: None,
            last_accessed: 0,
            scroll_position: 0,
        }
    }

    /// Set favicon
    pub fn with_favicon(mut self, favicon: String) -> Self {
        self.favicon = Some(favicon);
        self
    }
}

/// Device with synced tabs
#[derive(Debug, Clone)]
pub struct DeviceTabs {
    /// Device ID
    pub device_id: String,
    /// Device name
    pub device_name: String,
    /// Device type
    pub device_type: DeviceType,
    /// Tabs on this device
    pub tabs: Vec<SyncedTab>,
    /// Last sync timestamp
    pub last_sync: u64,
}

impl DeviceTabs {
    /// Create new device tabs
    pub fn new(device_id: String, device_name: String, device_type: DeviceType) -> Self {
        Self {
            device_id,
            device_name,
            device_type,
            tabs: Vec::new(),
            last_sync: 0,
        }
    }

    /// Add tab
    pub fn add_tab(&mut self, tab: SyncedTab) {
        // Update existing or add new
        if let Some(existing) = self.tabs.iter_mut().find(|t| t.id == tab.id) {
            *existing = tab;
        } else {
            self.tabs.push(tab);
        }
    }

    /// Remove tab
    pub fn remove_tab(&mut self, tab_id: &str) {
        self.tabs.retain(|t| t.id != tab_id);
    }

    /// Get tab by ID
    pub fn get_tab(&self, tab_id: &str) -> Option<&SyncedTab> {
        self.tabs.iter().find(|t| t.id == tab_id)
    }

    /// Tab count
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }
}

/// Device type for tabs display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// Desktop computer
    Desktop,
    /// Laptop
    Laptop,
    /// Tablet
    Tablet,
    /// Phone
    Phone,
    /// Unknown
    Unknown,
}

impl Default for DeviceType {
    fn default() -> Self {
        Self::Unknown
    }
}

impl DeviceType {
    /// Get icon name
    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Laptop => "laptop",
            Self::Tablet => "tablet",
            Self::Phone => "phone",
            Self::Unknown => "device",
        }
    }
}

/// Sent tab (send-to-device)
#[derive(Debug, Clone)]
pub struct SentTab {
    /// Unique ID
    pub id: String,
    /// URL
    pub url: String,
    /// Title
    pub title: String,
    /// Sender device ID
    pub sender_device_id: String,
    /// Sender device name
    pub sender_device_name: String,
    /// Target device ID
    pub target_device_id: String,
    /// Sent timestamp
    pub sent_at: u64,
    /// Received/opened
    pub received: bool,
}

impl SentTab {
    /// Create new sent tab
    pub fn new(
        url: String,
        title: String,
        sender_id: String,
        sender_name: String,
        target_id: String,
    ) -> Self {
        let id = generate_id();
        Self {
            id,
            url,
            title,
            sender_device_id: sender_id,
            sender_device_name: sender_name,
            target_device_id: target_id,
            sent_at: 0,
            received: false,
        }
    }

    /// Mark as received
    pub fn mark_received(&mut self) {
        self.received = true;
    }
}

/// Tabs sync handler
pub struct TabsSync {
    /// Current device ID
    device_id: String,
    /// Current device name
    device_name: String,
    /// Current device type
    device_type: DeviceType,
    /// Local tabs
    local_tabs: Vec<SyncedTab>,
    /// Remote devices with tabs
    remote_devices: BTreeMap<String, DeviceTabs>,
    /// Pending sent tabs (to send)
    outgoing_tabs: Vec<SentTab>,
    /// Received tabs (from other devices)
    incoming_tabs: Vec<SentTab>,
    /// Last sync timestamp
    last_sync: u64,
}

impl TabsSync {
    /// Create new tabs sync
    pub fn new(device_id: String, device_name: String, device_type: DeviceType) -> Self {
        Self {
            device_id,
            device_name,
            device_type,
            local_tabs: Vec::new(),
            remote_devices: BTreeMap::new(),
            outgoing_tabs: Vec::new(),
            incoming_tabs: Vec::new(),
            last_sync: 0,
        }
    }

    /// Get device ID
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Get device name
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Update local tab
    pub fn update_local_tab(&mut self, tab: SyncedTab) {
        if let Some(existing) = self.local_tabs.iter_mut().find(|t| t.id == tab.id) {
            *existing = tab;
        } else {
            self.local_tabs.push(tab);
        }
    }

    /// Remove local tab
    pub fn remove_local_tab(&mut self, tab_id: &str) {
        self.local_tabs.retain(|t| t.id != tab_id);
    }

    /// Get local tabs
    pub fn local_tabs(&self) -> &[SyncedTab] {
        &self.local_tabs
    }

    /// Get remote devices
    pub fn remote_devices(&self) -> Vec<&DeviceTabs> {
        self.remote_devices.values().collect()
    }

    /// Get tabs from device
    pub fn tabs_from_device(&self, device_id: &str) -> Option<&[SyncedTab]> {
        self.remote_devices.get(device_id).map(|d| d.tabs.as_slice())
    }

    /// Get all remote tabs
    pub fn all_remote_tabs(&self) -> Vec<(&DeviceTabs, &SyncedTab)> {
        self.remote_devices
            .values()
            .flat_map(|d| d.tabs.iter().map(move |t| (d, t)))
            .collect()
    }

    /// Send tab to device
    pub fn send_to_device(&mut self, url: String, title: String, target_device_id: String) {
        let sent = SentTab::new(
            url,
            title,
            self.device_id.clone(),
            self.device_name.clone(),
            target_device_id,
        );
        self.outgoing_tabs.push(sent);
    }

    /// Get outgoing tabs
    pub fn outgoing_tabs(&self) -> &[SentTab] {
        &self.outgoing_tabs
    }

    /// Clear sent outgoing tabs
    pub fn clear_outgoing(&mut self) {
        self.outgoing_tabs.clear();
    }

    /// Get incoming tabs
    pub fn incoming_tabs(&self) -> &[SentTab] {
        &self.incoming_tabs
    }

    /// Get unread incoming tabs
    pub fn unread_incoming(&self) -> Vec<&SentTab> {
        self.incoming_tabs.iter().filter(|t| !t.received).collect()
    }

    /// Mark incoming tab as received
    pub fn mark_incoming_received(&mut self, tab_id: &str) {
        if let Some(tab) = self.incoming_tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.mark_received();
        }
    }

    /// Clear old incoming tabs
    pub fn clear_old_incoming(&mut self, older_than: u64) {
        self.incoming_tabs.retain(|t| t.sent_at >= older_than || !t.received);
    }

    /// Create sync item for local tabs
    pub fn to_sync_item(&self) -> SyncItem {
        let mut data = Vec::new();
        
        // Device info
        serialize_string(&mut data, &self.device_id);
        serialize_string(&mut data, &self.device_name);
        data.push(self.device_type as u8);
        
        // Tab count
        data.extend_from_slice(&(self.local_tabs.len() as u32).to_le_bytes());
        
        // Tabs
        for tab in &self.local_tabs {
            serialize_string(&mut data, &tab.id);
            serialize_string(&mut data, &tab.url);
            serialize_string(&mut data, &tab.title);
            serialize_option_string(&mut data, &tab.favicon);
            data.extend_from_slice(&tab.last_accessed.to_le_bytes());
            data.extend_from_slice(&tab.scroll_position.to_le_bytes());
        }
        
        SyncItem::new(self.device_id.clone(), SyncItemType::Tab, data)
    }

    /// Apply remote device tabs
    pub fn apply_remote(&mut self, item: &SyncItem) -> Result<(), SyncError> {
        if item.item_type != SyncItemType::Tab {
            return Err(SyncError::StorageError("Invalid item type".into()));
        }

        // Skip our own device
        if item.id == self.device_id {
            return Ok(());
        }

        let data = &item.data;
        let mut cursor = 0;

        let device_id = deserialize_string(data, &mut cursor)
            .ok_or_else(|| SyncError::StorageError("Invalid data".into()))?;
        let device_name = deserialize_string(data, &mut cursor)
            .ok_or_else(|| SyncError::StorageError("Invalid data".into()))?;
        
        if cursor >= data.len() {
            return Err(SyncError::StorageError("Invalid data".into()));
        }
        let device_type = match data[cursor] {
            0 => DeviceType::Desktop,
            1 => DeviceType::Laptop,
            2 => DeviceType::Tablet,
            3 => DeviceType::Phone,
            _ => DeviceType::Unknown,
        };
        cursor += 1;

        if cursor + 4 > data.len() {
            return Err(SyncError::StorageError("Invalid data".into()));
        }
        let tab_count = u32::from_le_bytes(data[cursor..cursor+4].try_into().unwrap()) as usize;
        cursor += 4;

        let mut tabs = Vec::with_capacity(tab_count);
        for _ in 0..tab_count {
            let id = deserialize_string(data, &mut cursor)
                .ok_or_else(|| SyncError::StorageError("Invalid data".into()))?;
            let url = deserialize_string(data, &mut cursor)
                .ok_or_else(|| SyncError::StorageError("Invalid data".into()))?;
            let title = deserialize_string(data, &mut cursor)
                .ok_or_else(|| SyncError::StorageError("Invalid data".into()))?;
            let favicon = deserialize_option_string(data, &mut cursor)
                .ok_or_else(|| SyncError::StorageError("Invalid data".into()))?;
            
            if cursor + 8 > data.len() {
                return Err(SyncError::StorageError("Invalid data".into()));
            }
            let last_accessed = u64::from_le_bytes(data[cursor..cursor+8].try_into().unwrap());
            cursor += 8;
            
            if cursor + 4 > data.len() {
                return Err(SyncError::StorageError("Invalid data".into()));
            }
            let scroll_position = u32::from_le_bytes(data[cursor..cursor+4].try_into().unwrap());
            cursor += 4;

            tabs.push(SyncedTab {
                id,
                url,
                title,
                favicon,
                last_accessed,
                scroll_position,
            });
        }

        let device_tabs = DeviceTabs {
            device_id: device_id.clone(),
            device_name,
            device_type,
            tabs,
            last_sync: item.modified_at,
        };

        self.remote_devices.insert(device_id, device_tabs);

        Ok(())
    }

    /// Remove stale devices
    pub fn remove_stale_devices(&mut self, older_than: u64) {
        self.remote_devices.retain(|_, d| d.last_sync >= older_than);
    }
}

impl Default for TabsSync {
    fn default() -> Self {
        Self::new(
            "default_device".to_string(),
            "This Device".to_string(),
            DeviceType::Desktop,
        )
    }
}

// Helper functions

fn generate_id() -> String {
    // Would generate UUID
    "sent_tab_12345".to_string()
}

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
