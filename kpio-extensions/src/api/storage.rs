//! chrome.storage API
//!
//! Provides persistent key-value storage for extensions.

#![allow(dead_code)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

use super::{ApiContext, ApiError, ApiResult, EventEmitter};

/// Storage key.
pub type StorageKey = String;

/// Storage value (JSON string).
pub type StorageValue = String;

/// Storage area type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageAreaType {
    /// Local storage (5MB limit).
    Local,
    /// Sync storage (100KB limit, synced across devices).
    Sync,
    /// Session storage (1MB limit, cleared on browser close).
    Session,
    /// Managed storage (read-only, set by admin).
    Managed,
}

/// Storage change.
#[derive(Debug, Clone)]
pub struct StorageChange {
    /// Old value.
    pub old_value: Option<StorageValue>,
    /// New value.
    pub new_value: Option<StorageValue>,
}

/// Storage changes map.
pub type StorageChanges = BTreeMap<StorageKey, StorageChange>;

/// Access level for session storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessLevel {
    /// Trusted contexts only.
    TrustedContexts,
    /// Trusted and untrusted contexts.
    TrustedAndUntrustedContexts,
}

/// Storage quota bytes.
pub const QUOTA_BYTES: usize = 5_242_880; // 5MB
pub const QUOTA_BYTES_PER_ITEM: usize = 8_192; // 8KB
pub const SYNC_QUOTA_BYTES: usize = 102_400; // 100KB
pub const SYNC_QUOTA_BYTES_PER_ITEM: usize = 8_192; // 8KB
pub const SYNC_MAX_ITEMS: usize = 512;
pub const SYNC_MAX_WRITE_OPERATIONS_PER_HOUR: usize = 1_800;
pub const SYNC_MAX_WRITE_OPERATIONS_PER_MINUTE: usize = 120;
pub const SESSION_QUOTA_BYTES: usize = 1_048_576; // 1MB

/// Storage area.
pub struct StorageArea {
    /// Area type.
    area_type: StorageAreaType,
    /// Data storage.
    data: RwLock<BTreeMap<String, BTreeMap<StorageKey, StorageValue>>>,
    /// On changed event.
    on_changed: RwLock<EventEmitter<(StorageChanges, StorageAreaType)>>,
    /// Access level (for session storage).
    access_level: RwLock<AccessLevel>,
}

impl StorageArea {
    /// Create a new storage area.
    pub fn new(area_type: StorageAreaType) -> Self {
        Self {
            area_type,
            data: RwLock::new(BTreeMap::new()),
            on_changed: RwLock::new(EventEmitter::new()),
            access_level: RwLock::new(AccessLevel::TrustedContexts),
        }
    }

    /// Get the quota for this area.
    pub fn get_quota(&self) -> usize {
        match self.area_type {
            StorageAreaType::Local => QUOTA_BYTES,
            StorageAreaType::Sync => SYNC_QUOTA_BYTES,
            StorageAreaType::Session => SESSION_QUOTA_BYTES,
            StorageAreaType::Managed => QUOTA_BYTES,
        }
    }

    /// Get the per-item quota for this area.
    pub fn get_per_item_quota(&self) -> usize {
        match self.area_type {
            StorageAreaType::Sync => SYNC_QUOTA_BYTES_PER_ITEM,
            _ => QUOTA_BYTES_PER_ITEM,
        }
    }

    /// Get values.
    pub fn get(
        &self,
        ctx: &ApiContext,
        keys: Option<Vec<StorageKey>>,
    ) -> ApiResult<BTreeMap<StorageKey, StorageValue>> {
        let ext_id = ctx.extension_id.as_str();
        let data = self.data.read();

        let ext_data = data.get(ext_id);

        match keys {
            Some(key_list) => {
                let mut result = BTreeMap::new();
                if let Some(ext_data) = ext_data {
                    for key in key_list {
                        if let Some(value) = ext_data.get(&key) {
                            result.insert(key, value.clone());
                        }
                    }
                }
                Ok(result)
            }
            None => {
                // Return all values
                Ok(ext_data.cloned().unwrap_or_default())
            }
        }
    }

    /// Get bytes in use.
    pub fn get_bytes_in_use(
        &self,
        ctx: &ApiContext,
        keys: Option<Vec<StorageKey>>,
    ) -> ApiResult<usize> {
        let ext_id = ctx.extension_id.as_str();
        let data = self.data.read();

        let ext_data = match data.get(ext_id) {
            Some(d) => d,
            None => return Ok(0),
        };

        match keys {
            Some(key_list) => {
                let mut total = 0;
                for key in &key_list {
                    if let Some(value) = ext_data.get(key) {
                        total += key.len() + value.len();
                    }
                }
                Ok(total)
            }
            None => {
                let mut total = 0;
                for (k, v) in ext_data {
                    total += k.len() + v.len();
                }
                Ok(total)
            }
        }
    }

    /// Set values.
    pub fn set(
        &self,
        ctx: &ApiContext,
        items: BTreeMap<StorageKey, StorageValue>,
    ) -> ApiResult<()> {
        // Check managed storage
        if self.area_type == StorageAreaType::Managed {
            return Err(ApiError::permission_denied("Managed storage is read-only"));
        }

        let ext_id = ctx.extension_id.as_str().to_string();
        let mut data = self.data.write();

        let ext_data = data.entry(ext_id).or_insert_with(BTreeMap::new);

        // Calculate size and check quotas
        let mut new_size = 0;
        for (k, v) in ext_data.iter() {
            if !items.contains_key(k) {
                new_size += k.len() + v.len();
            }
        }

        for (k, v) in &items {
            let item_size = k.len() + v.len();
            if item_size > self.get_per_item_quota() {
                return Err(ApiError::quota_exceeded("Item too large"));
            }
            new_size += item_size;
        }

        if new_size > self.get_quota() {
            return Err(ApiError::quota_exceeded("Storage quota exceeded"));
        }

        // Check sync item count
        if self.area_type == StorageAreaType::Sync {
            let current_count = ext_data.len();
            let new_items = items.keys().filter(|k| !ext_data.contains_key(*k)).count();
            if current_count + new_items > SYNC_MAX_ITEMS {
                return Err(ApiError::quota_exceeded("Too many items"));
            }
        }

        // Build changes
        let mut changes = StorageChanges::new();

        for (key, value) in items {
            let old_value = ext_data.get(&key).cloned();

            changes.insert(
                key.clone(),
                StorageChange {
                    old_value,
                    new_value: Some(value.clone()),
                },
            );

            ext_data.insert(key, value);
        }

        // Emit change event
        if !changes.is_empty() {
            self.on_changed.read().emit(&(changes, self.area_type));
        }

        Ok(())
    }

    /// Remove values.
    pub fn remove(&self, ctx: &ApiContext, keys: Vec<StorageKey>) -> ApiResult<()> {
        if self.area_type == StorageAreaType::Managed {
            return Err(ApiError::permission_denied("Managed storage is read-only"));
        }

        let ext_id = ctx.extension_id.as_str().to_string();
        let mut data = self.data.write();

        let ext_data = match data.get_mut(&ext_id) {
            Some(d) => d,
            None => return Ok(()),
        };

        let mut changes = StorageChanges::new();

        for key in keys {
            if let Some(old_value) = ext_data.remove(&key) {
                changes.insert(
                    key,
                    StorageChange {
                        old_value: Some(old_value),
                        new_value: None,
                    },
                );
            }
        }

        if !changes.is_empty() {
            self.on_changed.read().emit(&(changes, self.area_type));
        }

        Ok(())
    }

    /// Clear all values.
    pub fn clear(&self, ctx: &ApiContext) -> ApiResult<()> {
        if self.area_type == StorageAreaType::Managed {
            return Err(ApiError::permission_denied("Managed storage is read-only"));
        }

        let ext_id = ctx.extension_id.as_str().to_string();
        let mut data = self.data.write();

        if let Some(ext_data) = data.get_mut(&ext_id) {
            let mut changes = StorageChanges::new();

            // BTreeMap doesn't have drain in no_std, collect keys first
            let keys: Vec<String> = ext_data.keys().cloned().collect();
            for key in keys {
                if let Some(value) = ext_data.remove(&key) {
                    changes.insert(
                        key,
                        StorageChange {
                            old_value: Some(value),
                            new_value: None,
                        },
                    );
                }
            }

            if !changes.is_empty() {
                self.on_changed.read().emit(&(changes, self.area_type));
            }
        }

        Ok(())
    }

    /// Set access level (session storage only).
    pub fn set_access_level(&self, level: AccessLevel) -> ApiResult<()> {
        if self.area_type != StorageAreaType::Session {
            return Err(ApiError::invalid_argument(
                "Access level only for session storage",
            ));
        }
        *self.access_level.write() = level;
        Ok(())
    }
}

/// Storage API.
pub struct StorageApi {
    /// Local storage.
    pub local: StorageArea,
    /// Sync storage.
    pub sync: StorageArea,
    /// Session storage.
    pub session: StorageArea,
    /// Managed storage.
    pub managed: StorageArea,
    /// On changed event (global).
    pub on_changed: RwLock<EventEmitter<(StorageChanges, StorageAreaType)>>,
}

impl StorageApi {
    /// Create a new Storage API.
    pub fn new() -> Self {
        Self {
            local: StorageArea::new(StorageAreaType::Local),
            sync: StorageArea::new(StorageAreaType::Sync),
            session: StorageArea::new(StorageAreaType::Session),
            managed: StorageArea::new(StorageAreaType::Managed),
            on_changed: RwLock::new(EventEmitter::new()),
        }
    }

    /// Get storage area by type.
    pub fn get_area(&self, area_type: StorageAreaType) -> &StorageArea {
        match area_type {
            StorageAreaType::Local => &self.local,
            StorageAreaType::Sync => &self.sync,
            StorageAreaType::Session => &self.session,
            StorageAreaType::Managed => &self.managed,
        }
    }
}

impl Default for StorageApi {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for JSON serialization.
pub mod json {
    use super::*;

    /// Serialize a value to JSON string (simplified).
    pub fn to_string<T: core::fmt::Debug>(value: &T) -> String {
        // Would use actual JSON serialization
        alloc::format!("{:?}", value)
    }

    /// Parse a JSON string (simplified).
    pub fn from_str<T: Default>(_s: &str) -> Result<T, &'static str> {
        // Would use actual JSON parsing
        Ok(T::default())
    }

    /// Null value.
    pub fn null() -> StorageValue {
        "null".to_string()
    }

    /// Boolean value.
    pub fn boolean(v: bool) -> StorageValue {
        if v { "true" } else { "false" }.to_string()
    }

    /// Number value.
    pub fn number(v: f64) -> StorageValue {
        alloc::format!("{}", v)
    }

    /// String value.
    pub fn string(v: &str) -> StorageValue {
        alloc::format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExtensionId;

    #[test]
    fn test_storage_api() {
        let api = StorageApi::new();
        let ctx = ApiContext::new(ExtensionId::new("test"));

        // Set values
        let mut items = BTreeMap::new();
        items.insert("key1".to_string(), "\"value1\"".to_string());
        items.insert("key2".to_string(), "42".to_string());
        api.local.set(&ctx, items).unwrap();

        // Get values
        let result = api.local.get(&ctx, Some(vec!["key1".to_string()])).unwrap();
        assert_eq!(result.get("key1"), Some(&"\"value1\"".to_string()));

        // Get all values
        let result = api.local.get(&ctx, None).unwrap();
        assert_eq!(result.len(), 2);

        // Get bytes in use
        let bytes = api.local.get_bytes_in_use(&ctx, None).unwrap();
        assert!(bytes > 0);

        // Remove values
        api.local.remove(&ctx, vec!["key1".to_string()]).unwrap();
        let result = api.local.get(&ctx, None).unwrap();
        assert_eq!(result.len(), 1);

        // Clear
        api.local.clear(&ctx).unwrap();
        let result = api.local.get(&ctx, None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_managed_readonly() {
        let api = StorageApi::new();
        let ctx = ApiContext::new(ExtensionId::new("test"));

        // Should fail to set managed storage
        let mut items = BTreeMap::new();
        items.insert("key".to_string(), "value".to_string());
        let result = api.managed.set(&ctx, items);
        assert!(result.is_err());
    }

    #[test]
    fn test_quota_limits() {
        let api = StorageApi::new();
        let ctx = ApiContext::new(ExtensionId::new("test"));

        // Create oversized item
        let large_value = "x".repeat(QUOTA_BYTES_PER_ITEM + 1);
        let mut items = BTreeMap::new();
        items.insert("key".to_string(), large_value);

        let result = api.local.set(&ctx, items);
        assert!(result.is_err());
    }
}
