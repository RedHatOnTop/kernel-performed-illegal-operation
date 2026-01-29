//! Cache API Implementation
//!
//! Provides caching storage for service workers.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::RwLock;

use super::fetch::{Request, Response};

/// Cache ID counter
static NEXT_CACHE_ID: AtomicU64 = AtomicU64::new(1);

/// Cache ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CacheId(u64);

impl CacheId {
    fn new() -> Self {
        Self(NEXT_CACHE_ID.fetch_add(1, Ordering::SeqCst))
    }
}

/// Cache error types
#[derive(Debug, Clone)]
pub enum CacheError {
    /// Cache not found
    NotFound,
    /// Request not found in cache
    RequestNotFound,
    /// Quota exceeded
    QuotaExceeded,
    /// Storage error
    StorageError(String),
    /// Invalid request
    InvalidRequest,
}

/// Cache match options
#[derive(Debug, Clone, Default)]
pub struct CacheMatchOptions {
    /// Ignore search (query string)
    pub ignore_search: bool,
    /// Ignore method
    pub ignore_method: bool,
    /// Ignore vary header
    pub ignore_vary: bool,
}

/// Cache query options
#[derive(Debug, Clone, Default)]
pub struct CacheQueryOptions {
    /// Ignore search
    pub ignore_search: bool,
    /// Ignore method
    pub ignore_method: bool,
    /// Ignore vary
    pub ignore_vary: bool,
}

/// A cached request-response pair
#[derive(Debug, Clone)]
struct CacheEntry {
    /// The request
    request: Request,
    /// The response
    response: Response,
    /// Timestamp when cached
    cached_at: u64,
    /// Size in bytes
    size: usize,
}

impl CacheEntry {
    fn new(request: Request, response: Response) -> Self {
        let size = response.body.as_ref().map(|b| b.len()).unwrap_or(0);
        Self {
            request,
            response,
            cached_at: 0, // Would use actual timestamp
            size,
        }
    }
}

/// A cache storage
#[derive(Debug)]
pub struct Cache {
    /// Cache ID
    id: CacheId,
    /// Cache name
    name: String,
    /// Cached entries (URL -> entry)
    entries: BTreeMap<String, CacheEntry>,
    /// Total size in bytes
    total_size: usize,
}

impl Cache {
    /// Create a new cache
    fn new(name: impl Into<String>) -> Self {
        Self {
            id: CacheId::new(),
            name: name.into(),
            entries: BTreeMap::new(),
            total_size: 0,
        }
    }

    /// Get cache name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Match a request
    pub fn match_request(
        &self,
        request: &Request,
        options: &CacheMatchOptions,
    ) -> Option<Response> {
        let key = self.make_key(request, options);
        self.entries.get(&key).map(|e| e.response.clone())
    }

    /// Match all matching requests
    pub fn match_all(
        &self,
        request: Option<&Request>,
        options: &CacheQueryOptions,
    ) -> Vec<Response> {
        match request {
            Some(req) => {
                let key = self.make_key(req, &CacheMatchOptions {
                    ignore_search: options.ignore_search,
                    ignore_method: options.ignore_method,
                    ignore_vary: options.ignore_vary,
                });
                self.entries
                    .get(&key)
                    .map(|e| vec![e.response.clone()])
                    .unwrap_or_default()
            }
            None => {
                self.entries.values().map(|e| e.response.clone()).collect()
            }
        }
    }

    /// Add a request/response pair to cache
    pub fn put(&mut self, request: Request, response: Response) -> Result<(), CacheError> {
        let key = self.make_key(&request, &CacheMatchOptions::default());
        let entry = CacheEntry::new(request, response);
        let size = entry.size;

        // Remove old entry if exists
        if let Some(old) = self.entries.remove(&key) {
            self.total_size -= old.size;
        }

        self.entries.insert(key, entry);
        self.total_size += size;

        Ok(())
    }

    /// Add all from an iterator
    pub fn add_all(&mut self, requests: impl Iterator<Item = Request>) -> Result<(), CacheError> {
        for request in requests {
            // In real implementation, would fetch from network
            let response = Response::new(200);
            self.put(request, response)?;
        }
        Ok(())
    }

    /// Delete a cached request
    pub fn delete(
        &mut self,
        request: &Request,
        options: &CacheQueryOptions,
    ) -> Result<bool, CacheError> {
        let key = self.make_key(request, &CacheMatchOptions {
            ignore_search: options.ignore_search,
            ignore_method: options.ignore_method,
            ignore_vary: options.ignore_vary,
        });

        if let Some(entry) = self.entries.remove(&key) {
            self.total_size -= entry.size;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get all cached request keys
    pub fn keys(
        &self,
        request: Option<&Request>,
        options: &CacheQueryOptions,
    ) -> Vec<Request> {
        match request {
            Some(req) => {
                let key = self.make_key(req, &CacheMatchOptions {
                    ignore_search: options.ignore_search,
                    ignore_method: options.ignore_method,
                    ignore_vary: options.ignore_vary,
                });
                self.entries
                    .get(&key)
                    .map(|e| vec![e.request.clone()])
                    .unwrap_or_default()
            }
            None => {
                self.entries.values().map(|e| e.request.clone()).collect()
            }
        }
    }

    /// Make a cache key from a request
    fn make_key(&self, request: &Request, options: &CacheMatchOptions) -> String {
        let mut key = request.url.clone();

        // Handle ignore_search option
        if options.ignore_search {
            if let Some(pos) = key.find('?') {
                key.truncate(pos);
            }
        }

        // Handle ignore_method option
        if !options.ignore_method {
            key = alloc::format!("{}:{}", request.method.as_str(), key);
        }

        key
    }

    /// Get total size
    pub fn size(&self) -> usize {
        self.total_size
    }
}

/// Cache storage (manages multiple caches)
pub struct CacheStorage {
    /// Origin
    origin: String,
    /// Caches by name
    caches: BTreeMap<String, Cache>,
    /// Quota (bytes)
    quota: usize,
    /// Usage (bytes)
    usage: usize,
}

impl CacheStorage {
    /// Create new cache storage
    pub fn new(origin: impl Into<String>) -> Self {
        Self {
            origin: origin.into(),
            caches: BTreeMap::new(),
            quota: 50 * 1024 * 1024, // 50 MB default quota
            usage: 0,
        }
    }

    /// Open or create a cache
    pub fn open(&mut self, name: &str) -> Result<&mut Cache, CacheError> {
        if !self.caches.contains_key(name) {
            self.caches.insert(name.to_string(), Cache::new(name));
        }
        Ok(self.caches.get_mut(name).unwrap())
    }

    /// Check if a cache exists
    pub fn has(&self, name: &str) -> bool {
        self.caches.contains_key(name)
    }

    /// Delete a cache
    pub fn delete(&mut self, name: &str) -> Result<bool, CacheError> {
        if let Some(cache) = self.caches.remove(name) {
            self.usage -= cache.size();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get all cache names
    pub fn keys(&self) -> Vec<String> {
        self.caches.keys().cloned().collect()
    }

    /// Match across all caches
    pub fn match_request(
        &self,
        request: &Request,
        options: &CacheMatchOptions,
    ) -> Option<Response> {
        for cache in self.caches.values() {
            if let Some(response) = cache.match_request(request, options) {
                return Some(response);
            }
        }
        None
    }

    /// Get quota
    pub fn quota(&self) -> usize {
        self.quota
    }

    /// Get usage
    pub fn usage(&self) -> usize {
        self.caches.values().map(|c| c.size()).sum()
    }

    /// Get origin
    pub fn origin(&self) -> &str {
        &self.origin
    }
}

/// Global cache storage manager
pub struct CacheStorageManager {
    /// Storages by origin
    storages: BTreeMap<String, CacheStorage>,
}

impl CacheStorageManager {
    /// Create new manager
    pub const fn new() -> Self {
        Self {
            storages: BTreeMap::new(),
        }
    }

    /// Get or create storage for origin
    pub fn storage(&mut self, origin: &str) -> &mut CacheStorage {
        if !self.storages.contains_key(origin) {
            self.storages.insert(origin.to_string(), CacheStorage::new(origin));
        }
        self.storages.get_mut(origin).unwrap()
    }

    /// Get storage (immutable)
    pub fn get_storage(&self, origin: &str) -> Option<&CacheStorage> {
        self.storages.get(origin)
    }

    /// Clear all caches for an origin
    pub fn clear(&mut self, origin: &str) {
        self.storages.remove(origin);
    }
}

impl Default for CacheStorageManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global cache manager
pub static CACHE_MANAGER: RwLock<CacheStorageManager> = RwLock::new(CacheStorageManager::new());

/// Cache-first strategy
pub fn cache_first(
    storage: &CacheStorage,
    request: &Request,
) -> Option<Response> {
    storage.match_request(request, &CacheMatchOptions::default())
}

/// Network-first strategy
pub fn network_first(
    storage: &mut CacheStorage,
    request: &Request,
    cache_name: &str,
) -> Option<Response> {
    // Try network first (would be async in real implementation)
    // Fall back to cache
    cache_first(storage, request)
}

/// Stale-while-revalidate strategy
pub fn stale_while_revalidate(
    storage: &CacheStorage,
    request: &Request,
) -> Option<Response> {
    // Return cached response immediately, update in background
    storage.match_request(request, &CacheMatchOptions::default())
}

/// Cache only strategy
pub fn cache_only(
    storage: &CacheStorage,
    request: &Request,
) -> Option<Response> {
    storage.match_request(request, &CacheMatchOptions::default())
}

/// Network only strategy (no caching)
pub fn network_only(_request: &Request) -> Option<Response> {
    // Would fetch from network
    None
}
