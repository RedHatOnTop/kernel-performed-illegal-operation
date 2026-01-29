//! Settings Synchronization
//!
//! Browser settings sync across devices.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{SyncItem, SyncItemType, SyncError};

/// Setting value types
#[derive(Debug, Clone)]
pub enum SettingValue {
    /// Boolean
    Bool(bool),
    /// Integer
    Int(i64),
    /// Float
    Float(f64),
    /// String
    String(String),
    /// String list
    StringList(Vec<String>),
}

impl SettingValue {
    /// Get as bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as int
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as string
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(v) => Some(v),
            _ => None,
        }
    }

    /// Serialize
    fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        match self {
            Self::Bool(v) => {
                data.push(0);
                data.push(if *v { 1 } else { 0 });
            }
            Self::Int(v) => {
                data.push(1);
                data.extend_from_slice(&v.to_le_bytes());
            }
            Self::Float(v) => {
                data.push(2);
                data.extend_from_slice(&v.to_le_bytes());
            }
            Self::String(v) => {
                data.push(3);
                let bytes = v.as_bytes();
                data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                data.extend_from_slice(bytes);
            }
            Self::StringList(list) => {
                data.push(4);
                data.extend_from_slice(&(list.len() as u32).to_le_bytes());
                for s in list {
                    let bytes = s.as_bytes();
                    data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                    data.extend_from_slice(bytes);
                }
            }
        }
        data
    }

    /// Deserialize
    fn deserialize(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        match data[0] {
            0 => {
                if data.len() < 2 { return None; }
                Some(Self::Bool(data[1] != 0))
            }
            1 => {
                if data.len() < 9 { return None; }
                let v = i64::from_le_bytes(data[1..9].try_into().ok()?);
                Some(Self::Int(v))
            }
            2 => {
                if data.len() < 9 { return None; }
                let v = f64::from_le_bytes(data[1..9].try_into().ok()?);
                Some(Self::Float(v))
            }
            3 => {
                if data.len() < 5 { return None; }
                let len = u32::from_le_bytes(data[1..5].try_into().ok()?) as usize;
                if data.len() < 5 + len { return None; }
                let s = core::str::from_utf8(&data[5..5+len]).ok()?;
                Some(Self::String(s.to_string()))
            }
            4 => {
                if data.len() < 5 { return None; }
                let count = u32::from_le_bytes(data[1..5].try_into().ok()?) as usize;
                let mut cursor = 5;
                let mut list = Vec::with_capacity(count);
                
                for _ in 0..count {
                    if cursor + 4 > data.len() { return None; }
                    let len = u32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?) as usize;
                    cursor += 4;
                    if cursor + len > data.len() { return None; }
                    let s = core::str::from_utf8(&data[cursor..cursor+len]).ok()?;
                    list.push(s.to_string());
                    cursor += len;
                }
                
                Some(Self::StringList(list))
            }
            _ => None,
        }
    }
}

/// Syncable setting
#[derive(Debug, Clone)]
pub struct SyncableSetting {
    /// Setting key
    pub key: String,
    /// Value
    pub value: SettingValue,
    /// Last modified timestamp
    pub modified_at: u64,
    /// Sync enabled for this setting
    pub sync_enabled: bool,
}

impl SyncableSetting {
    /// Create new setting
    pub fn new(key: String, value: SettingValue) -> Self {
        Self {
            key,
            value,
            modified_at: 0,
            sync_enabled: true,
        }
    }

    /// Convert to sync item
    pub fn to_sync_item(&self) -> SyncItem {
        let mut data = Vec::new();
        
        // Key
        let key_bytes = self.key.as_bytes();
        data.extend_from_slice(&(key_bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(key_bytes);
        
        // Value
        let value_data = self.value.serialize();
        data.extend_from_slice(&(value_data.len() as u32).to_le_bytes());
        data.extend_from_slice(&value_data);
        
        // Modified at
        data.extend_from_slice(&self.modified_at.to_le_bytes());
        
        SyncItem::new(self.key.clone(), SyncItemType::Setting, data)
    }

    /// Create from sync item
    pub fn from_sync_item(item: &SyncItem) -> Option<Self> {
        if item.item_type != SyncItemType::Setting {
            return None;
        }

        let data = &item.data;
        let mut cursor = 0;

        // Key
        if cursor + 4 > data.len() { return None; }
        let key_len = u32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?) as usize;
        cursor += 4;
        if cursor + key_len > data.len() { return None; }
        let key = core::str::from_utf8(&data[cursor..cursor+key_len]).ok()?;
        cursor += key_len;

        // Value
        if cursor + 4 > data.len() { return None; }
        let value_len = u32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?) as usize;
        cursor += 4;
        if cursor + value_len > data.len() { return None; }
        let value = SettingValue::deserialize(&data[cursor..cursor+value_len])?;
        cursor += value_len;

        // Modified at
        if cursor + 8 > data.len() { return None; }
        let modified_at = u64::from_le_bytes(data[cursor..cursor+8].try_into().ok()?);

        Some(Self {
            key: key.to_string(),
            value,
            modified_at,
            sync_enabled: true,
        })
    }
}

/// Settings categories
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingCategory {
    /// General settings
    General,
    /// Privacy settings
    Privacy,
    /// Appearance settings
    Appearance,
    /// Search settings
    Search,
    /// Downloads settings
    Downloads,
    /// Language settings
    Language,
    /// Accessibility settings
    Accessibility,
}

/// Settings sync handler
pub struct SettingsSync {
    /// Settings by key
    settings: BTreeMap<String, SyncableSetting>,
    /// Non-syncable keys
    non_syncable: Vec<String>,
    /// Last sync timestamp
    last_sync: u64,
}

impl SettingsSync {
    /// Create new settings sync
    pub fn new() -> Self {
        Self {
            settings: BTreeMap::new(),
            non_syncable: alloc::vec![
                "privacy.do_not_sync".to_string(),
                "security.master_password".to_string(),
                "downloads.default_directory".to_string(),
            ],
            last_sync: 0,
        }
    }

    /// Get setting
    pub fn get(&self, key: &str) -> Option<&SettingValue> {
        self.settings.get(key).map(|s| &s.value)
    }

    /// Set setting
    pub fn set(&mut self, key: String, value: SettingValue, timestamp: u64) {
        let sync_enabled = !self.non_syncable.contains(&key);
        
        if let Some(setting) = self.settings.get_mut(&key) {
            setting.value = value;
            setting.modified_at = timestamp;
        } else {
            self.settings.insert(key.clone(), SyncableSetting {
                key,
                value,
                modified_at: timestamp,
                sync_enabled,
            });
        }
    }

    /// Remove setting
    pub fn remove(&mut self, key: &str) {
        self.settings.remove(key);
    }

    /// Get bool setting
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key)?.as_bool()
    }

    /// Get int setting
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key)?.as_int()
    }

    /// Get string setting
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_string()
    }

    /// Set bool setting
    pub fn set_bool(&mut self, key: &str, value: bool, timestamp: u64) {
        self.set(key.to_string(), SettingValue::Bool(value), timestamp);
    }

    /// Set int setting
    pub fn set_int(&mut self, key: &str, value: i64, timestamp: u64) {
        self.set(key.to_string(), SettingValue::Int(value), timestamp);
    }

    /// Set string setting
    pub fn set_string(&mut self, key: &str, value: String, timestamp: u64) {
        self.set(key.to_string(), SettingValue::String(value), timestamp);
    }

    /// Get all settings
    pub fn all(&self) -> Vec<&SyncableSetting> {
        self.settings.values().collect()
    }

    /// Get settings by prefix
    pub fn by_prefix(&self, prefix: &str) -> Vec<&SyncableSetting> {
        self.settings
            .values()
            .filter(|s| s.key.starts_with(prefix))
            .collect()
    }

    /// Get syncable settings
    pub fn syncable(&self) -> Vec<&SyncableSetting> {
        self.settings
            .values()
            .filter(|s| s.sync_enabled)
            .collect()
    }

    /// Get settings modified since
    pub fn modified_since(&self, timestamp: u64) -> Vec<&SyncableSetting> {
        self.settings
            .values()
            .filter(|s| s.modified_at > timestamp && s.sync_enabled)
            .collect()
    }

    /// Get sync items
    pub fn to_sync_items(&self) -> Vec<SyncItem> {
        self.syncable()
            .iter()
            .map(|s| s.to_sync_item())
            .collect()
    }

    /// Apply remote setting
    pub fn apply_remote(&mut self, item: &SyncItem) -> Result<(), SyncError> {
        if let Some(remote) = SyncableSetting::from_sync_item(item) {
            if self.non_syncable.contains(&remote.key) {
                return Ok(());
            }

            if let Some(local) = self.settings.get(&remote.key) {
                // Take newer value
                if remote.modified_at > local.modified_at {
                    self.settings.insert(remote.key.clone(), remote);
                }
            } else {
                self.settings.insert(remote.key.clone(), remote);
            }
        }
        Ok(())
    }

    /// Reset to defaults
    pub fn reset_to_defaults(&mut self) {
        self.settings.clear();
        
        // Add default settings
        let defaults = [
            ("general.homepage", SettingValue::String("about:home".to_string())),
            ("general.new_tab", SettingValue::String("about:newtab".to_string())),
            ("privacy.do_not_track", SettingValue::Bool(true)),
            ("privacy.block_third_party_cookies", SettingValue::Bool(true)),
            ("appearance.theme", SettingValue::String("system".to_string())),
            ("appearance.font_size", SettingValue::Int(16)),
            ("search.default_engine", SettingValue::String("duckduckgo".to_string())),
            ("downloads.ask_location", SettingValue::Bool(true)),
        ];

        for (key, value) in defaults {
            self.settings.insert(key.to_string(), SyncableSetting::new(key.to_string(), value));
        }
    }

    /// Export settings as key-value pairs
    pub fn export(&self) -> BTreeMap<String, String> {
        self.settings
            .iter()
            .map(|(k, v)| {
                let value_str = match &v.value {
                    SettingValue::Bool(b) => b.to_string(),
                    SettingValue::Int(i) => i.to_string(),
                    SettingValue::Float(f) => alloc::format!("{}", f),
                    SettingValue::String(s) => s.clone(),
                    SettingValue::StringList(l) => l.join(","),
                };
                (k.clone(), value_str)
            })
            .collect()
    }
}

impl Default for SettingsSync {
    fn default() -> Self {
        let mut sync = Self::new();
        sync.reset_to_defaults();
        sync
    }
}
