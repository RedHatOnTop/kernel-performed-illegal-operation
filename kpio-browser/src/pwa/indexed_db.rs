//! IndexedDB API — front-facing types
//!
//! Provides W3C IndexedDB–style interfaces:
//! - `IDBFactory` — open / delete databases
//! - `IDBDatabase` — create / delete object stores, start transactions
//! - `IDBObjectStore` — put / get / delete / clear / count / get_all / create_index
//! - `IDBTransaction` — Readonly / Readwrite / Versionchange
//! - `IDBIndex` — secondary index queries
//! - `IDBCursor` — forward iteration over store entries
//!
//! Storage is delegated to `idb_engine.rs` which provides the B-Tree backed
//! key-value store with VFS persistence.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::idb_engine::{EngineError, IDBEngine, IDBKey, IDBValue};

// ── Error ───────────────────────────────────────────────────

/// IndexedDB error types.
#[derive(Debug, Clone)]
pub enum IDBError {
    /// Database not found (for delete, transaction, etc.).
    NotFound,
    /// Object store with that name already exists.
    AlreadyExists,
    /// Store doesn't exist in this database.
    NoSuchStore,
    /// Index doesn't exist in this store.
    NoSuchIndex,
    /// Tried to do a write in a Readonly transaction.
    ReadOnly,
    /// Version requested ≤ current version.
    VersionError,
    /// Key already exists (and no overwrite).
    ConstraintError,
    /// Engine / VFS error.
    EngineError(String),
    /// Key path extraction failed.
    DataError,
    /// Quota exceeded.
    QuotaExceeded,
}

impl core::fmt::Display for IDBError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            IDBError::NotFound => write!(f, "NotFoundError"),
            IDBError::AlreadyExists => write!(f, "ConstraintError: store already exists"),
            IDBError::NoSuchStore => write!(f, "NotFoundError: no such object store"),
            IDBError::NoSuchIndex => write!(f, "NotFoundError: no such index"),
            IDBError::ReadOnly => write!(f, "ReadOnlyError"),
            IDBError::VersionError => write!(f, "VersionError"),
            IDBError::ConstraintError => write!(f, "ConstraintError: key already exists"),
            IDBError::EngineError(e) => write!(f, "EngineError: {}", e),
            IDBError::DataError => write!(f, "DataError"),
            IDBError::QuotaExceeded => write!(f, "QuotaExceededError"),
        }
    }
}

impl From<EngineError> for IDBError {
    fn from(e: EngineError) -> Self {
        match e {
            EngineError::NotFound => IDBError::NotFound,
            EngineError::AlreadyExists => IDBError::AlreadyExists,
            EngineError::QuotaExceeded => IDBError::QuotaExceeded,
            EngineError::IoError => IDBError::EngineError(String::from("I/O error")),
            EngineError::CorruptedData => IDBError::EngineError(String::from("corrupted")),
        }
    }
}

// ── Transaction Mode ────────────────────────────────────────

/// Transaction isolation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionMode {
    /// Read-only access (concurrent).
    Readonly,
    /// Read-write access (exclusive per store).
    Readwrite,
    /// Used during version-change upgrades.
    Versionchange,
}

// ── Index descriptor ────────────────────────────────────────

/// Describes a secondary index on an object store.
#[derive(Debug, Clone)]
pub struct IndexDescriptor {
    pub name: String,
    /// Key path within the stored value (e.g. "email").
    pub key_path: String,
    /// Whether the index enforces uniqueness.
    pub unique: bool,
    /// Whether the index covers array values.
    pub multi_entry: bool,
}

// ── Object Store descriptor ─────────────────────────────────

/// Describes an object store inside a database.
#[derive(Debug, Clone)]
pub struct ObjectStoreDescriptor {
    pub name: String,
    /// Optional key path for inline keys.
    pub key_path: Option<String>,
    /// Auto-increment counter.
    pub auto_increment: bool,
    /// Secondary indices.
    pub indices: BTreeMap<String, IndexDescriptor>,
    /// Current auto-inc value.
    pub next_key: u64,
}

impl ObjectStoreDescriptor {
    pub fn new(name: &str, key_path: Option<&str>, auto_increment: bool) -> Self {
        Self {
            name: String::from(name),
            key_path: key_path.map(String::from),
            auto_increment,
            indices: BTreeMap::new(),
            next_key: 1,
        }
    }
}

// ── Database descriptor ─────────────────────────────────────

/// Describes an IndexedDB database.
#[derive(Debug, Clone)]
pub struct DatabaseDescriptor {
    pub name: String,
    pub version: u64,
    pub stores: BTreeMap<String, ObjectStoreDescriptor>,
}

impl DatabaseDescriptor {
    pub fn new(name: &str, version: u64) -> Self {
        Self {
            name: String::from(name),
            version,
            stores: BTreeMap::new(),
        }
    }
}

// ── Cursor ──────────────────────────────────────────────────

/// A forward-only cursor over store entries.
pub struct IDBCursor {
    /// Snapshot of keys at cursor-creation time.
    keys: Vec<IDBKey>,
    /// Current position.
    position: usize,
}

impl IDBCursor {
    pub(crate) fn new(keys: Vec<IDBKey>) -> Self {
        Self { keys, position: 0 }
    }

    /// Current key, or `None` if exhausted.
    pub fn key(&self) -> Option<&IDBKey> {
        self.keys.get(self.position)
    }

    /// Advance to the next entry.  Returns `true` if there is a next entry.
    pub fn advance(&mut self) -> bool {
        if self.position < self.keys.len() {
            self.position += 1;
            self.position < self.keys.len()
        } else {
            false
        }
    }

    /// Reset to the beginning.
    pub fn reset(&mut self) {
        self.position = 0;
    }
}

// ── IDBObjectStore (transactional view) ─────────────────────

/// Transactional view of an object store.
pub struct IDBObjectStore<'a> {
    db_name: String,
    store_name: String,
    descriptor: &'a mut ObjectStoreDescriptor,
    engine: &'a mut IDBEngine,
    mode: TransactionMode,
}

impl<'a> IDBObjectStore<'a> {
    pub(crate) fn new(
        db_name: &str,
        store_name: &str,
        descriptor: &'a mut ObjectStoreDescriptor,
        engine: &'a mut IDBEngine,
        mode: TransactionMode,
    ) -> Self {
        Self {
            db_name: String::from(db_name),
            store_name: String::from(store_name),
            descriptor,
            engine,
            mode,
        }
    }

    /// Put (insert or overwrite) a value.
    pub fn put(&mut self, key: IDBKey, value: IDBValue) -> Result<IDBKey, IDBError> {
        if self.mode == TransactionMode::Readonly {
            return Err(IDBError::ReadOnly);
        }
        let actual_key = if matches!(key, IDBKey::None) && self.descriptor.auto_increment {
            let k = IDBKey::Number(self.descriptor.next_key as f64);
            self.descriptor.next_key += 1;
            k
        } else {
            key
        };

        self.engine
            .put(&self.db_name, &self.store_name, &actual_key, value)?;
        Ok(actual_key)
    }

    /// Get a value by key.
    pub fn get(&self, key: &IDBKey) -> Result<Option<IDBValue>, IDBError> {
        Ok(self.engine.get(&self.db_name, &self.store_name, key)?)
    }

    /// Delete a value by key.
    pub fn delete(&mut self, key: &IDBKey) -> Result<(), IDBError> {
        if self.mode == TransactionMode::Readonly {
            return Err(IDBError::ReadOnly);
        }
        self.engine.delete(&self.db_name, &self.store_name, key)?;
        Ok(())
    }

    /// Clear all entries.
    pub fn clear(&mut self) -> Result<(), IDBError> {
        if self.mode == TransactionMode::Readonly {
            return Err(IDBError::ReadOnly);
        }
        self.engine.clear_store(&self.db_name, &self.store_name)?;
        Ok(())
    }

    /// Count entries.
    pub fn count(&self) -> Result<usize, IDBError> {
        Ok(self.engine.count(&self.db_name, &self.store_name)?)
    }

    /// Get all entries (up to `limit`).
    pub fn get_all(&self, limit: Option<usize>) -> Result<Vec<(IDBKey, IDBValue)>, IDBError> {
        Ok(self
            .engine
            .get_all(&self.db_name, &self.store_name, limit)?)
    }

    /// Open a forward cursor over all keys.
    pub fn open_cursor(&self) -> Result<IDBCursor, IDBError> {
        let keys = self.engine.all_keys(&self.db_name, &self.store_name)?;
        Ok(IDBCursor::new(keys))
    }

    /// Create a secondary index (only in Versionchange).
    pub fn create_index(
        &mut self,
        name: &str,
        key_path: &str,
        unique: bool,
        multi_entry: bool,
    ) -> Result<(), IDBError> {
        if self.mode != TransactionMode::Versionchange {
            return Err(IDBError::ReadOnly);
        }
        if self.descriptor.indices.contains_key(name) {
            return Err(IDBError::AlreadyExists);
        }
        self.descriptor.indices.insert(
            String::from(name),
            IndexDescriptor {
                name: String::from(name),
                key_path: String::from(key_path),
                unique,
                multi_entry,
            },
        );
        // Create the index table in the engine
        self.engine
            .create_index(&self.db_name, &self.store_name, name, key_path)?;
        Ok(())
    }

    /// Delete a secondary index (only in Versionchange).
    pub fn delete_index(&mut self, name: &str) -> Result<(), IDBError> {
        if self.mode != TransactionMode::Versionchange {
            return Err(IDBError::ReadOnly);
        }
        self.descriptor
            .indices
            .remove(name)
            .ok_or(IDBError::NoSuchIndex)?;
        self.engine
            .delete_index(&self.db_name, &self.store_name, name)?;
        Ok(())
    }

    /// Query an index by value.
    pub fn index_get(
        &self,
        index_name: &str,
        value: &IDBKey,
    ) -> Result<Option<IDBValue>, IDBError> {
        if !self.descriptor.indices.contains_key(index_name) {
            return Err(IDBError::NoSuchIndex);
        }
        Ok(self
            .engine
            .index_get(&self.db_name, &self.store_name, index_name, value)?)
    }

    /// Store name.
    pub fn name(&self) -> &str {
        &self.store_name
    }
}

// ── IDBDatabase ─────────────────────────────────────────────

/// An opened database.
pub struct IDBDatabase {
    descriptor: DatabaseDescriptor,
    engine: IDBEngine,
}

impl IDBDatabase {
    pub(crate) fn new(descriptor: DatabaseDescriptor, engine: IDBEngine) -> Self {
        Self { descriptor, engine }
    }

    /// Database name.
    pub fn name(&self) -> &str {
        &self.descriptor.name
    }

    /// Current version.
    pub fn version(&self) -> u64 {
        self.descriptor.version
    }

    /// List object store names.
    pub fn object_store_names(&self) -> Vec<&str> {
        self.descriptor.stores.keys().map(|s| s.as_str()).collect()
    }

    /// Create an object store (only during version-change).
    pub fn create_object_store(
        &mut self,
        name: &str,
        key_path: Option<&str>,
        auto_increment: bool,
    ) -> Result<(), IDBError> {
        if self.descriptor.stores.contains_key(name) {
            return Err(IDBError::AlreadyExists);
        }
        let store = ObjectStoreDescriptor::new(name, key_path, auto_increment);
        self.engine.create_store(&self.descriptor.name, name)?;
        self.descriptor.stores.insert(String::from(name), store);
        Ok(())
    }

    /// Delete an object store (only during version-change).
    pub fn delete_object_store(&mut self, name: &str) -> Result<(), IDBError> {
        self.descriptor
            .stores
            .remove(name)
            .ok_or(IDBError::NoSuchStore)?;
        self.engine.delete_store(&self.descriptor.name, name)?;
        Ok(())
    }

    /// Start a transaction on the given stores.
    pub fn transaction_on(
        &mut self,
        store_name: &str,
        mode: TransactionMode,
    ) -> Result<IDBObjectStore<'_>, IDBError> {
        let desc = self
            .descriptor
            .stores
            .get_mut(store_name)
            .ok_or(IDBError::NoSuchStore)?;

        Ok(IDBObjectStore::new(
            &self.descriptor.name,
            store_name,
            desc,
            &mut self.engine,
            mode,
        ))
    }

    /// Close the database (consume self).
    pub fn close(self) {
        // Engine is dropped, releasing resources.
    }
}

// ── IDBFactory ──────────────────────────────────────────────

/// Top-level factory — analogous to `window.indexedDB`.
pub struct IDBFactory {
    app_id: u64,
    /// Catalogues of opened databases: name → descriptor.
    databases: BTreeMap<String, DatabaseDescriptor>,
}

impl IDBFactory {
    /// Create a factory for a given app.
    pub fn new(app_id: u64) -> Self {
        Self {
            app_id,
            databases: BTreeMap::new(),
        }
    }

    /// Open (or create) a database.
    ///
    /// If `version` > existing version, the caller should perform an upgrade
    /// by creating/deleting stores on the returned `IDBDatabase`.
    pub fn open(&mut self, name: &str, version: u64) -> Result<IDBDatabase, IDBError> {
        let version = if version == 0 { 1 } else { version };

        let descriptor = if let Some(existing) = self.databases.get(name) {
            if version < existing.version {
                return Err(IDBError::VersionError);
            }
            let mut d = existing.clone();
            d.version = version;
            d
        } else {
            DatabaseDescriptor::new(name, version)
        };

        let engine = IDBEngine::new(self.app_id, name);

        // Update catalogue
        self.databases
            .insert(String::from(name), descriptor.clone());

        Ok(IDBDatabase::new(descriptor, engine))
    }

    /// Delete a database.
    pub fn delete_database(&mut self, name: &str) -> Result<(), IDBError> {
        self.databases.remove(name).ok_or(IDBError::NotFound)?;
        // TODO: engine.delete_all(name) — VFS cleanup
        Ok(())
    }

    /// List all database names.
    pub fn databases(&self) -> Vec<&str> {
        self.databases.keys().map(|s| s.as_str()).collect()
    }

    /// App ID.
    pub fn app_id(&self) -> u64 {
        self.app_id
    }
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_create_store() {
        let mut factory = IDBFactory::new(1);
        let mut db = factory.open("test_db", 1).unwrap();
        db.create_object_store("notes", Some("id"), true).unwrap();
        assert_eq!(db.object_store_names(), vec!["notes"]);
    }

    #[test]
    fn put_get_delete() {
        let mut factory = IDBFactory::new(1);
        let mut db = factory.open("test_db", 1).unwrap();
        db.create_object_store("items", None, false).unwrap();

        {
            let mut store = db
                .transaction_on("items", TransactionMode::Readwrite)
                .unwrap();
            store
                .put(
                    IDBKey::String(String::from("k1")),
                    IDBValue::String(String::from("hello")),
                )
                .unwrap();
        }

        {
            let store = db
                .transaction_on("items", TransactionMode::Readonly)
                .unwrap();
            let val = store.get(&IDBKey::String(String::from("k1"))).unwrap();
            assert!(val.is_some());
        }

        {
            let mut store = db
                .transaction_on("items", TransactionMode::Readwrite)
                .unwrap();
            store.delete(&IDBKey::String(String::from("k1"))).unwrap();
        }

        {
            let store = db
                .transaction_on("items", TransactionMode::Readonly)
                .unwrap();
            let val = store.get(&IDBKey::String(String::from("k1"))).unwrap();
            assert!(val.is_none());
        }
    }

    #[test]
    fn auto_increment() {
        let mut factory = IDBFactory::new(1);
        let mut db = factory.open("test_db", 1).unwrap();
        db.create_object_store("auto", None, true).unwrap();

        let mut store = db
            .transaction_on("auto", TransactionMode::Readwrite)
            .unwrap();
        let k1 = store
            .put(IDBKey::None, IDBValue::String(String::from("first")))
            .unwrap();
        let k2 = store
            .put(IDBKey::None, IDBValue::String(String::from("second")))
            .unwrap();

        assert!(matches!(k1, IDBKey::Number(n) if n == 1.0));
        assert!(matches!(k2, IDBKey::Number(n) if n == 2.0));
    }

    #[test]
    fn readonly_rejects_write() {
        let mut factory = IDBFactory::new(1);
        let mut db = factory.open("test_db", 1).unwrap();
        db.create_object_store("items", None, false).unwrap();

        let mut store = db
            .transaction_on("items", TransactionMode::Readonly)
            .unwrap();
        let result = store.put(
            IDBKey::String(String::from("k")),
            IDBValue::String(String::from("v")),
        );
        assert!(matches!(result, Err(IDBError::ReadOnly)));
    }

    #[test]
    fn count_and_clear() {
        let mut factory = IDBFactory::new(1);
        let mut db = factory.open("test_db", 1).unwrap();
        db.create_object_store("items", None, false).unwrap();

        {
            let mut store = db
                .transaction_on("items", TransactionMode::Readwrite)
                .unwrap();
            store
                .put(
                    IDBKey::String(String::from("a")),
                    IDBValue::String(String::from("1")),
                )
                .unwrap();
            store
                .put(
                    IDBKey::String(String::from("b")),
                    IDBValue::String(String::from("2")),
                )
                .unwrap();
            assert_eq!(store.count().unwrap(), 2);
            store.clear().unwrap();
            assert_eq!(store.count().unwrap(), 0);
        }
    }

    #[test]
    fn cursor_iteration() {
        let mut factory = IDBFactory::new(1);
        let mut db = factory.open("test_db", 1).unwrap();
        db.create_object_store("items", None, false).unwrap();

        {
            let mut store = db
                .transaction_on("items", TransactionMode::Readwrite)
                .unwrap();
            store
                .put(
                    IDBKey::String(String::from("a")),
                    IDBValue::String(String::from("1")),
                )
                .unwrap();
            store
                .put(
                    IDBKey::String(String::from("b")),
                    IDBValue::String(String::from("2")),
                )
                .unwrap();
        }

        {
            let store = db
                .transaction_on("items", TransactionMode::Readonly)
                .unwrap();
            let mut cursor = store.open_cursor().unwrap();
            assert!(cursor.key().is_some());
            cursor.advance();
            assert!(cursor.key().is_some());
            let has_more = cursor.advance();
            assert!(!has_more);
        }
    }

    #[test]
    fn get_all_with_limit() {
        let mut factory = IDBFactory::new(1);
        let mut db = factory.open("test_db", 1).unwrap();
        db.create_object_store("items", None, false).unwrap();

        {
            let mut store = db
                .transaction_on("items", TransactionMode::Readwrite)
                .unwrap();
            for i in 0..5 {
                store
                    .put(
                        IDBKey::Number(i as f64),
                        IDBValue::String(alloc::format!("val{}", i)),
                    )
                    .unwrap();
            }
        }

        {
            let store = db
                .transaction_on("items", TransactionMode::Readonly)
                .unwrap();
            let all = store.get_all(Some(3)).unwrap();
            assert_eq!(all.len(), 3);
        }
    }

    #[test]
    fn version_error() {
        let mut factory = IDBFactory::new(1);
        let _db = factory.open("test_db", 2).unwrap();

        // Opening with lower version should error
        let result = factory.open("test_db", 1);
        assert!(matches!(result, Err(IDBError::VersionError)));
    }

    #[test]
    fn delete_database() {
        let mut factory = IDBFactory::new(1);
        let _db = factory.open("test_db", 1).unwrap();
        factory.delete_database("test_db").unwrap();
        assert!(factory.databases().is_empty());
    }

    #[test]
    fn duplicate_store_rejected() {
        let mut factory = IDBFactory::new(1);
        let mut db = factory.open("test_db", 1).unwrap();
        db.create_object_store("items", None, false).unwrap();
        let result = db.create_object_store("items", None, false);
        assert!(matches!(result, Err(IDBError::AlreadyExists)));
    }

    #[test]
    fn delete_store() {
        let mut factory = IDBFactory::new(1);
        let mut db = factory.open("test_db", 1).unwrap();
        db.create_object_store("items", None, false).unwrap();
        assert_eq!(db.object_store_names().len(), 1);
        db.delete_object_store("items").unwrap();
        assert!(db.object_store_names().is_empty());
    }
}
