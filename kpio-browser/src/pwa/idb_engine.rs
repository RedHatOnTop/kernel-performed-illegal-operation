//! IDB Engine — B-Tree key-value store
//!
//! Low-level storage engine for IndexedDB.
//!
//! - Per-app data lives at VFS path `/apps/storage/{app_id}/idb/{db_name}/`
//! - 50 MB total quota per app across all databases
//! - BTreeMap-backed storage for ordered key access
//! - Supports secondary indices via shadow BTreeMaps
//! - Concurrent readonly transactions allowed; exclusive readwrite via external sync

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cmp::Ordering;

// ── Constants ───────────────────────────────────────────────

/// Maximum total storage per app (50 MB).
pub const MAX_DB_SIZE: usize = 50 * 1024 * 1024;

// ── Key type ────────────────────────────────────────────────

/// IndexedDB key — supports the standard key types.
#[derive(Debug, Clone)]
pub enum IDBKey {
    /// No key (used with auto-increment).
    None,
    /// Numeric key.
    Number(f64),
    /// String key.
    String(String),
    /// Date key (milliseconds since epoch).
    Date(f64),
    /// Binary key.
    Binary(Vec<u8>),
    /// Array key (compound).
    Array(Vec<IDBKey>),
}

impl IDBKey {
    /// Serialize to a canonical string for BTreeMap ordering.
    pub fn to_sort_key(&self) -> String {
        match self {
            IDBKey::None => String::from("\x00"),
            IDBKey::Number(n) => alloc::format!("N{:020.6}", n),
            IDBKey::String(s) => alloc::format!("S{}", s),
            IDBKey::Date(d) => alloc::format!("D{:020.6}", d),
            IDBKey::Binary(b) => {
                let mut s = String::from("B");
                for byte in b {
                    s.push_str(&alloc::format!("{:02x}", byte));
                }
                s
            }
            IDBKey::Array(arr) => {
                let mut s = String::from("A[");
                for (i, k) in arr.iter().enumerate() {
                    if i > 0 {
                        s.push(',');
                    }
                    s.push_str(&k.to_sort_key());
                }
                s.push(']');
                s
            }
        }
    }

    /// Check if this is a None key.
    pub fn is_none(&self) -> bool {
        matches!(self, IDBKey::None)
    }
}

impl PartialEq for IDBKey {
    fn eq(&self, other: &Self) -> bool {
        self.to_sort_key() == other.to_sort_key()
    }
}

impl Eq for IDBKey {}

impl PartialOrd for IDBKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IDBKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_sort_key().cmp(&other.to_sort_key())
    }
}

// ── Value type ──────────────────────────────────────────────

/// Stored value — simplified structured clone.
#[derive(Debug, Clone)]
pub enum IDBValue {
    /// Null.
    Null,
    /// Boolean.
    Bool(bool),
    /// Number.
    Number(f64),
    /// String.
    String(String),
    /// Binary blob.
    Blob(Vec<u8>),
    /// Object (key-value pairs).
    Object(BTreeMap<String, IDBValue>),
    /// Array of values.
    Array(Vec<IDBValue>),
}

impl IDBValue {
    /// Estimated byte size.
    pub fn size(&self) -> usize {
        match self {
            IDBValue::Null => 4,
            IDBValue::Bool(_) => 5,
            IDBValue::Number(_) => 8,
            IDBValue::String(s) => s.len(),
            IDBValue::Blob(b) => b.len(),
            IDBValue::Object(map) => {
                map.iter()
                    .map(|(k, v)| k.len() + v.size())
                    .sum::<usize>()
                    + 2
            }
            IDBValue::Array(arr) => {
                arr.iter().map(|v| v.size()).sum::<usize>() + 2
            }
        }
    }

    /// Extract a field by key path (dot notation: "a.b.c").
    pub fn get_field(&self, key_path: &str) -> Option<&IDBValue> {
        let parts: Vec<&str> = key_path.split('.').collect();
        let mut current = self;
        for part in parts {
            match current {
                IDBValue::Object(map) => {
                    current = map.get(part)?;
                }
                _ => return None,
            }
        }
        Some(current)
    }

    /// Try to convert this value to an IDBKey (for index purposes).
    pub fn to_key(&self) -> Option<IDBKey> {
        match self {
            IDBValue::Number(n) => Some(IDBKey::Number(*n)),
            IDBValue::String(s) => Some(IDBKey::String(s.clone())),
            _ => None,
        }
    }
}

// ── Engine Error ────────────────────────────────────────────

/// Low-level engine errors.
#[derive(Debug, Clone)]
pub enum EngineError {
    /// Key not found.
    NotFound,
    /// Key already exists.
    AlreadyExists,
    /// Quota exceeded.
    QuotaExceeded,
    /// VFS I/O error.
    IoError,
    /// Data corruption detected.
    CorruptedData,
}

impl core::fmt::Display for EngineError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EngineError::NotFound => write!(f, "not found"),
            EngineError::AlreadyExists => write!(f, "already exists"),
            EngineError::QuotaExceeded => write!(f, "quota exceeded"),
            EngineError::IoError => write!(f, "I/O error"),
            EngineError::CorruptedData => write!(f, "corrupted data"),
        }
    }
}

// ── Store ───────────────────────────────────────────────────

/// A single object store backed by a BTreeMap.
struct Store {
    /// Primary data: sort_key → (IDBKey, IDBValue).
    data: BTreeMap<String, (IDBKey, IDBValue)>,
    /// Secondary indices: index_name → { indexed_value_sort_key → primary_sort_key }.
    indices: BTreeMap<String, IndexStore>,
    /// Current byte size estimate.
    size: usize,
}

/// A secondary index.
struct IndexStore {
    /// key_path used for extraction.
    key_path: String,
    /// Mapping: indexed-value sort key → primary sort key.
    mapping: BTreeMap<String, String>,
}

impl Store {
    fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            indices: BTreeMap::new(),
            size: 0,
        }
    }
}

// ── IDBEngine ───────────────────────────────────────────────

/// The storage engine for one app's IndexedDB instances.
pub struct IDBEngine {
    app_id: u64,
    db_name: String,
    /// store_name → Store
    stores: BTreeMap<String, Store>,
    /// Total bytes used across all stores.
    total_size: usize,
}

impl IDBEngine {
    /// Create a new engine for a given app + database.
    pub fn new(app_id: u64, db_name: &str) -> Self {
        Self {
            app_id,
            db_name: String::from(db_name),
            stores: BTreeMap::new(),
            total_size: 0,
        }
    }

    /// VFS base path for this database.
    pub fn vfs_path(&self) -> String {
        alloc::format!(
            "/apps/storage/{}/idb/{}/",
            self.app_id,
            self.db_name
        )
    }

    /// Current total size.
    pub fn total_size(&self) -> usize {
        self.total_size
    }

    // ── Store management ────────────────────────────────────

    /// Create an object store.
    pub fn create_store(
        &mut self,
        _db_name: &str,
        store_name: &str,
    ) -> Result<(), EngineError> {
        if self.stores.contains_key(store_name) {
            return Err(EngineError::AlreadyExists);
        }
        self.stores
            .insert(String::from(store_name), Store::new());
        Ok(())
    }

    /// Delete an object store.
    pub fn delete_store(
        &mut self,
        _db_name: &str,
        store_name: &str,
    ) -> Result<(), EngineError> {
        if let Some(store) = self.stores.remove(store_name) {
            self.total_size = self.total_size.saturating_sub(store.size);
        }
        Ok(())
    }

    // ── CRUD operations ─────────────────────────────────────

    /// Put (insert or update) an entry.
    pub fn put(
        &mut self,
        _db_name: &str,
        store_name: &str,
        key: &IDBKey,
        value: IDBValue,
    ) -> Result<(), EngineError> {
        let sort_key = key.to_sort_key();
        let entry_size = sort_key.len() + key.to_sort_key().len() + value.size();

        let store = self
            .stores
            .get_mut(store_name)
            .ok_or(EngineError::NotFound)?;

        // Check quota
        let old_size = store
            .data
            .get(&sort_key)
            .map(|(_, v)| sort_key.len() + v.size())
            .unwrap_or(0);
        let new_total = self.total_size + entry_size - old_size;
        if new_total > MAX_DB_SIZE {
            return Err(EngineError::QuotaExceeded);
        }

        // Update secondary indices
        let index_names: Vec<String> = store.indices.keys().cloned().collect();
        for idx_name in &index_names {
            let key_path = store.indices[idx_name].key_path.clone();
            if let Some(idx_key) = value.get_field(&key_path).and_then(|v| v.to_key()) {
                let idx_sort = idx_key.to_sort_key();
                if let Some(idx_store) = store.indices.get_mut(idx_name) {
                    idx_store.mapping.insert(idx_sort, sort_key.clone());
                }
            }
        }

        store.size = store.size + entry_size - old_size;
        self.total_size = new_total;
        store
            .data
            .insert(sort_key, (key.clone(), value));

        Ok(())
    }

    /// Get a value by key.
    pub fn get(
        &self,
        _db_name: &str,
        store_name: &str,
        key: &IDBKey,
    ) -> Result<Option<IDBValue>, EngineError> {
        let store = self
            .stores
            .get(store_name)
            .ok_or(EngineError::NotFound)?;
        let sort_key = key.to_sort_key();
        Ok(store.data.get(&sort_key).map(|(_, v)| v.clone()))
    }

    /// Delete a value by key.
    pub fn delete(
        &mut self,
        _db_name: &str,
        store_name: &str,
        key: &IDBKey,
    ) -> Result<(), EngineError> {
        let sort_key = key.to_sort_key();
        let store = self
            .stores
            .get_mut(store_name)
            .ok_or(EngineError::NotFound)?;

        if let Some((_, value)) = store.data.remove(&sort_key) {
            let entry_size = sort_key.len() + value.size();
            store.size = store.size.saturating_sub(entry_size);
            self.total_size = self.total_size.saturating_sub(entry_size);

            // Remove from indices
            for idx_store in store.indices.values_mut() {
                idx_store
                    .mapping
                    .retain(|_, primary| primary != &sort_key);
            }
        }
        Ok(())
    }

    /// Clear all entries in a store.
    pub fn clear_store(
        &mut self,
        _db_name: &str,
        store_name: &str,
    ) -> Result<(), EngineError> {
        let store = self
            .stores
            .get_mut(store_name)
            .ok_or(EngineError::NotFound)?;
        self.total_size = self.total_size.saturating_sub(store.size);
        store.data.clear();
        store.size = 0;
        for idx in store.indices.values_mut() {
            idx.mapping.clear();
        }
        Ok(())
    }

    /// Count entries in a store.
    pub fn count(
        &self,
        _db_name: &str,
        store_name: &str,
    ) -> Result<usize, EngineError> {
        let store = self
            .stores
            .get(store_name)
            .ok_or(EngineError::NotFound)?;
        Ok(store.data.len())
    }

    /// Get all entries (up to `limit`).
    pub fn get_all(
        &self,
        _db_name: &str,
        store_name: &str,
        limit: Option<usize>,
    ) -> Result<Vec<(IDBKey, IDBValue)>, EngineError> {
        let store = self
            .stores
            .get(store_name)
            .ok_or(EngineError::NotFound)?;
        let iter = store.data.values().map(|(k, v)| (k.clone(), v.clone()));
        match limit {
            Some(n) => Ok(iter.take(n).collect()),
            None => Ok(iter.collect()),
        }
    }

    /// Get all keys in a store (sorted).
    pub fn all_keys(
        &self,
        _db_name: &str,
        store_name: &str,
    ) -> Result<Vec<IDBKey>, EngineError> {
        let store = self
            .stores
            .get(store_name)
            .ok_or(EngineError::NotFound)?;
        Ok(store
            .data
            .values()
            .map(|(k, _)| k.clone())
            .collect())
    }

    // ── Index operations ────────────────────────────────────

    /// Create a secondary index on a store.
    pub fn create_index(
        &mut self,
        _db_name: &str,
        store_name: &str,
        index_name: &str,
        key_path: &str,
    ) -> Result<(), EngineError> {
        let store = self
            .stores
            .get_mut(store_name)
            .ok_or(EngineError::NotFound)?;

        if store.indices.contains_key(index_name) {
            return Err(EngineError::AlreadyExists);
        }

        // Build the index from existing data
        let mut mapping = BTreeMap::new();
        for (sort_key, (_, value)) in &store.data {
            if let Some(idx_key) = value.get_field(key_path).and_then(|v| v.to_key())
            {
                mapping.insert(idx_key.to_sort_key(), sort_key.clone());
            }
        }

        store.indices.insert(
            String::from(index_name),
            IndexStore {
                key_path: String::from(key_path),
                mapping,
            },
        );
        Ok(())
    }

    /// Delete a secondary index.
    pub fn delete_index(
        &mut self,
        _db_name: &str,
        store_name: &str,
        index_name: &str,
    ) -> Result<(), EngineError> {
        let store = self
            .stores
            .get_mut(store_name)
            .ok_or(EngineError::NotFound)?;
        store
            .indices
            .remove(index_name)
            .ok_or(EngineError::NotFound)?;
        Ok(())
    }

    /// Query an index: find a primary value by indexed secondary key.
    pub fn index_get(
        &self,
        _db_name: &str,
        store_name: &str,
        index_name: &str,
        indexed_value: &IDBKey,
    ) -> Result<Option<IDBValue>, EngineError> {
        let store = self
            .stores
            .get(store_name)
            .ok_or(EngineError::NotFound)?;
        let idx = store
            .indices
            .get(index_name)
            .ok_or(EngineError::NotFound)?;

        let idx_sort = indexed_value.to_sort_key();
        if let Some(primary_sort) = idx.mapping.get(&idx_sort) {
            Ok(store
                .data
                .get(primary_sort)
                .map(|(_, v)| v.clone()))
        } else {
            Ok(None)
        }
    }
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> IDBEngine {
        let mut e = IDBEngine::new(1, "testdb");
        e.create_store("testdb", "items").unwrap();
        e
    }

    #[test]
    fn put_and_get() {
        let mut e = make_engine();
        let key = IDBKey::String(String::from("hello"));
        let val = IDBValue::String(String::from("world"));
        e.put("testdb", "items", &key, val).unwrap();

        let result = e.get("testdb", "items", &key).unwrap();
        assert!(matches!(result, Some(IDBValue::String(s)) if s == "world"));
    }

    #[test]
    fn delete_entry() {
        let mut e = make_engine();
        let key = IDBKey::Number(42.0);
        e.put("testdb", "items", &key, IDBValue::Null).unwrap();
        e.delete("testdb", "items", &key).unwrap();
        assert!(e.get("testdb", "items", &key).unwrap().is_none());
    }

    #[test]
    fn clear_store() {
        let mut e = make_engine();
        for i in 0..5 {
            e.put(
                "testdb",
                "items",
                &IDBKey::Number(i as f64),
                IDBValue::Null,
            )
            .unwrap();
        }
        assert_eq!(e.count("testdb", "items").unwrap(), 5);
        e.clear_store("testdb", "items").unwrap();
        assert_eq!(e.count("testdb", "items").unwrap(), 0);
    }

    #[test]
    fn get_all_with_limit() {
        let mut e = make_engine();
        for i in 0..10 {
            e.put(
                "testdb",
                "items",
                &IDBKey::Number(i as f64),
                IDBValue::Number(i as f64),
            )
            .unwrap();
        }
        let all = e.get_all("testdb", "items", Some(3)).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn all_keys_sorted() {
        let mut e = make_engine();
        e.put(
            "testdb",
            "items",
            &IDBKey::String(String::from("banana")),
            IDBValue::Null,
        )
        .unwrap();
        e.put(
            "testdb",
            "items",
            &IDBKey::String(String::from("apple")),
            IDBValue::Null,
        )
        .unwrap();
        let keys = e.all_keys("testdb", "items").unwrap();
        assert_eq!(keys.len(), 2);
        // BTreeMap sorts by sort_key, so "apple" < "banana"
        assert!(matches!(&keys[0], IDBKey::String(s) if s == "apple"));
    }

    #[test]
    fn secondary_index() {
        let mut e = make_engine();

        // Insert an object with an "email" field
        let mut obj = BTreeMap::new();
        obj.insert(
            String::from("email"),
            IDBValue::String(String::from("test@kpio.os")),
        );
        obj.insert(
            String::from("name"),
            IDBValue::String(String::from("Test User")),
        );

        // Create index BEFORE inserting (typical versionchange flow)
        e.create_index("testdb", "items", "by_email", "email")
            .unwrap();

        e.put(
            "testdb",
            "items",
            &IDBKey::Number(1.0),
            IDBValue::Object(obj),
        )
        .unwrap();

        // Query by indexed email
        let result = e
            .index_get(
                "testdb",
                "items",
                "by_email",
                &IDBKey::String(String::from("test@kpio.os")),
            )
            .unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn key_ordering() {
        let k1 = IDBKey::Number(1.0);
        let k2 = IDBKey::Number(2.0);
        let k3 = IDBKey::String(String::from("abc"));
        assert!(k1 < k2);
        // Numbers come before strings in sort key (N < S)
        assert!(k1 < k3);
    }

    #[test]
    fn value_size_estimate() {
        let v = IDBValue::String(String::from("hello"));
        assert_eq!(v.size(), 5);

        let v2 = IDBValue::Null;
        assert_eq!(v2.size(), 4);
    }

    #[test]
    fn value_get_field() {
        let mut inner = BTreeMap::new();
        inner.insert(
            String::from("city"),
            IDBValue::String(String::from("Seoul")),
        );
        let mut obj = BTreeMap::new();
        obj.insert(String::from("address"), IDBValue::Object(inner));

        let val = IDBValue::Object(obj);
        let field = val.get_field("address.city");
        assert!(matches!(field, Some(IDBValue::String(s)) if s == "Seoul"));
    }

    #[test]
    fn quota_enforcement() {
        let mut e = make_engine();
        // Create a value that's close to 50 MB
        let big_data = IDBValue::Blob(alloc::vec![0u8; MAX_DB_SIZE - 100]);
        e.put(
            "testdb",
            "items",
            &IDBKey::String(String::from("big")),
            big_data,
        )
        .unwrap();

        // Trying to add more should fail
        let result = e.put(
            "testdb",
            "items",
            &IDBKey::String(String::from("overflow")),
            IDBValue::Blob(alloc::vec![0u8; 1000]),
        );
        assert!(matches!(result, Err(EngineError::QuotaExceeded)));
    }
}
