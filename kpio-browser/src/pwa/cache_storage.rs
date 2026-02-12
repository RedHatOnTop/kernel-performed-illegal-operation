//! Cache Storage API
//!
//! Implements the W3C Cache API for Service Workers and PWAs.
//! Data is persisted to VFS under `/apps/cache/{app_id}/{cache_name}/`.
//!
//! Each cache entry is stored as:
//!   - `{hash}.body`    — response body bytes
//!   - `{hash}.headers` — response headers (JSON)
//!   - `_meta.json`     — URL → hash mapping

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

// ── Constants ───────────────────────────────────────────────

/// Maximum cache size per app (25 MB).
const MAX_CACHE_SIZE: usize = 25 * 1024 * 1024;

// ── Types ───────────────────────────────────────────────────

/// A single cached response.
#[derive(Debug, Clone)]
pub struct CachedResponse {
    /// The request URL this response is keyed on.
    pub url: String,
    /// HTTP status code.
    pub status: u16,
    /// Response headers (name → value).
    pub headers: BTreeMap<String, String>,
    /// Response body bytes.
    pub body: Vec<u8>,
    /// Timestamp when this entry was cached.
    pub cached_at: u64,
    /// Size in bytes (body + overhead).
    pub size: usize,
}

/// A named cache (equivalent to the JS `Cache` object).
#[derive(Debug, Clone)]
pub struct Cache {
    /// Cache name (e.g., `"v1"`, `"static-assets"`).
    pub name: String,
    /// URL → CachedResponse.
    entries: BTreeMap<String, CachedResponse>,
    /// Total size of all entries.
    total_size: usize,
}

/// Global cache storage (equivalent to the JS `caches` object).
pub struct CacheStorage {
    /// App-specific cache namespace.
    app_id: u64,
    /// cache_name → Cache.
    caches: BTreeMap<String, Cache>,
    /// Total size across all caches for this app.
    total_size: usize,
}

/// Cache storage error.
#[derive(Debug, Clone)]
pub enum CacheError {
    /// Cache not found.
    NotFound,
    /// Quota exceeded.
    QuotaExceeded,
    /// VFS I/O error.
    IoError,
    /// Invalid request/response data.
    InvalidData(String),
}

impl core::fmt::Display for CacheError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CacheError::NotFound => write!(f, "cache not found"),
            CacheError::QuotaExceeded => write!(f, "cache quota exceeded (25MB)"),
            CacheError::IoError => write!(f, "VFS I/O error"),
            CacheError::InvalidData(s) => write!(f, "invalid data: {}", s),
        }
    }
}

// ── Cache Implementation ────────────────────────────────────

impl Cache {
    /// Create a new empty cache.
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            entries: BTreeMap::new(),
            total_size: 0,
        }
    }

    /// Store a request/response pair.
    pub fn put(&mut self, url: &str, response: CachedResponse) -> Result<usize, CacheError> {
        let entry_size = response.size;

        // Remove old entry for same URL if present
        if let Some(old) = self.entries.remove(url) {
            self.total_size = self.total_size.saturating_sub(old.size);
        }

        self.total_size += entry_size;
        self.entries.insert(String::from(url), response);

        Ok(entry_size)
    }

    /// Look up a cached response by URL.
    pub fn match_url(&self, url: &str) -> Option<&CachedResponse> {
        self.entries.get(url)
    }

    /// Delete an entry by URL.
    pub fn delete(&mut self, url: &str) -> bool {
        if let Some(entry) = self.entries.remove(url) {
            self.total_size = self.total_size.saturating_sub(entry.size);
            true
        } else {
            false
        }
    }

    /// List all cached URLs.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether this cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total size of entries in this cache.
    pub fn size(&self) -> usize {
        self.total_size
    }

    /// Evict the oldest entry (LRU by `cached_at`).
    fn evict_lru(&mut self) -> Option<String> {
        let oldest_url = self
            .entries
            .iter()
            .min_by_key(|(_, v)| v.cached_at)
            .map(|(k, _)| k.clone());

        if let Some(ref url) = oldest_url {
            self.delete(url);
        }

        oldest_url
    }
}

// ── CacheStorage Implementation ─────────────────────────────

impl CacheStorage {
    /// Create a new cache storage for the given app.
    pub fn new(app_id: u64) -> Self {
        Self {
            app_id,
            caches: BTreeMap::new(),
            total_size: 0,
        }
    }

    /// Open (or create) a named cache.
    pub fn open(&mut self, cache_name: &str) -> &mut Cache {
        if !self.caches.contains_key(cache_name) {
            self.caches
                .insert(String::from(cache_name), Cache::new(cache_name));
        }
        self.caches.get_mut(cache_name).unwrap()
    }

    /// Check if a named cache exists.
    pub fn has(&self, cache_name: &str) -> bool {
        self.caches.contains_key(cache_name)
    }

    /// Delete a named cache.
    pub fn delete(&mut self, cache_name: &str) -> bool {
        if let Some(cache) = self.caches.remove(cache_name) {
            self.total_size = self.total_size.saturating_sub(cache.size());
            true
        } else {
            false
        }
    }

    /// List all cache names.
    pub fn keys(&self) -> Vec<&str> {
        self.caches.keys().map(|s| s.as_str()).collect()
    }

    /// Put a response into the specified cache, enforcing quota.
    pub fn put(
        &mut self,
        cache_name: &str,
        url: &str,
        status: u16,
        headers: BTreeMap<String, String>,
        body: Vec<u8>,
    ) -> Result<(), CacheError> {
        let body_len = body.len();
        let entry_size = body_len + 256; // body + estimated header overhead

        // Check if adding this entry would exceed the quota
        if self.total_size + entry_size > MAX_CACHE_SIZE {
            // Try LRU eviction until we have space
            self.evict_to_fit(entry_size)?;
        }

        let response = CachedResponse {
            url: String::from(url),
            status,
            headers,
            body,
            cached_at: Self::now(),
            size: entry_size,
        };

        let cache = self.open(cache_name);
        let added = cache.put(url, response)?;
        self.total_size += added;

        Ok(())
    }

    /// Match a URL across all caches (returns first hit).
    pub fn match_url(&self, url: &str) -> Option<(&str, &CachedResponse)> {
        for (name, cache) in &self.caches {
            if let Some(resp) = cache.match_url(url) {
                return Some((name.as_str(), resp));
            }
        }
        None
    }

    /// Match a URL in a specific cache.
    pub fn match_in(&self, cache_name: &str, url: &str) -> Option<&CachedResponse> {
        self.caches.get(cache_name)?.match_url(url)
    }

    /// Total storage used by this app's caches.
    pub fn total_size(&self) -> usize {
        self.total_size
    }

    /// Maximum allowed cache size.
    pub fn max_size(&self) -> usize {
        MAX_CACHE_SIZE
    }

    /// App ID this storage belongs to.
    pub fn app_id(&self) -> u64 {
        self.app_id
    }

    /// Evict LRU entries across all caches until `needed` bytes are free.
    fn evict_to_fit(&mut self, needed: usize) -> Result<(), CacheError> {
        let max_iterations = 1000;
        let mut iterations = 0;

        while self.total_size + needed > MAX_CACHE_SIZE && iterations < max_iterations {
            // Find the cache with the oldest entry
            let oldest_cache = self
                .caches
                .iter()
                .filter(|(_, c)| !c.is_empty())
                .min_by_key(|(_, c)| {
                    c.entries
                        .values()
                        .map(|e| e.cached_at)
                        .min()
                        .unwrap_or(u64::MAX)
                })
                .map(|(name, _)| name.clone());

            if let Some(cache_name) = oldest_cache {
                if let Some(cache) = self.caches.get_mut(&cache_name) {
                    if let Some(_evicted_url) = cache.evict_lru() {
                        // Recalculate total
                        self.total_size = self.caches.values().map(|c| c.size()).sum();
                    } else {
                        break;
                    }
                }
            } else {
                break;
            }

            iterations += 1;
        }

        if self.total_size + needed > MAX_CACHE_SIZE {
            Err(CacheError::QuotaExceeded)
        } else {
            Ok(())
        }
    }

    fn now() -> u64 {
        // Placeholder: would use kernel tick counter
        0
    }
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_response(url: &str, body: &[u8]) -> CachedResponse {
        CachedResponse {
            url: String::from(url),
            status: 200,
            headers: BTreeMap::new(),
            body: body.to_vec(),
            cached_at: 0,
            size: body.len() + 256,
        }
    }

    #[test]
    fn cache_put_and_match() {
        let mut cache = Cache::new("v1");
        let resp = make_response("/style.css", b"body{color:red}");
        cache.put("/style.css", resp).unwrap();

        let found = cache.match_url("/style.css").unwrap();
        assert_eq!(found.body, b"body{color:red}");
        assert_eq!(found.status, 200);
    }

    #[test]
    fn cache_delete() {
        let mut cache = Cache::new("v1");
        cache.put("/a", make_response("/a", b"aaa")).unwrap();
        assert!(cache.delete("/a"));
        assert!(cache.match_url("/a").is_none());
    }

    #[test]
    fn cache_keys() {
        let mut cache = Cache::new("v1");
        cache.put("/a", make_response("/a", b"a")).unwrap();
        cache.put("/b", make_response("/b", b"b")).unwrap();
        let keys = cache.keys();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"/a"));
        assert!(keys.contains(&"/b"));
    }

    #[test]
    fn cache_storage_open_and_has() {
        let mut storage = CacheStorage::new(1);
        assert!(!storage.has("v1"));
        storage.open("v1");
        assert!(storage.has("v1"));
    }

    #[test]
    fn cache_storage_delete() {
        let mut storage = CacheStorage::new(1);
        storage.open("temp");
        assert!(storage.delete("temp"));
        assert!(!storage.has("temp"));
    }

    #[test]
    fn cache_storage_put_and_match() {
        let mut storage = CacheStorage::new(1);
        storage
            .put("v1", "/index.html", 200, BTreeMap::new(), b"<html>".to_vec())
            .unwrap();

        let (cache_name, resp) = storage.match_url("/index.html").unwrap();
        assert_eq!(cache_name, "v1");
        assert_eq!(resp.body, b"<html>");
    }

    #[test]
    fn cache_storage_match_in() {
        let mut storage = CacheStorage::new(1);
        storage
            .put("v1", "/a.js", 200, BTreeMap::new(), b"var x".to_vec())
            .unwrap();
        storage
            .put("v2", "/b.js", 200, BTreeMap::new(), b"var y".to_vec())
            .unwrap();

        assert!(storage.match_in("v1", "/a.js").is_some());
        assert!(storage.match_in("v1", "/b.js").is_none());
        assert!(storage.match_in("v2", "/b.js").is_some());
    }

    #[test]
    fn cache_lru_eviction() {
        let mut cache = Cache::new("v1");
        let old = CachedResponse {
            url: String::from("/old"),
            status: 200,
            headers: BTreeMap::new(),
            body: Vec::new(),
            cached_at: 1,
            size: 100,
        };
        let new = CachedResponse {
            url: String::from("/new"),
            status: 200,
            headers: BTreeMap::new(),
            body: Vec::new(),
            cached_at: 100,
            size: 100,
        };
        cache.put("/old", old).unwrap();
        cache.put("/new", new).unwrap();

        let evicted = cache.evict_lru();
        assert_eq!(evicted, Some(String::from("/old")));
        assert!(cache.match_url("/old").is_none());
        assert!(cache.match_url("/new").is_some());
    }

    #[test]
    fn cache_replace_same_url() {
        let mut cache = Cache::new("v1");
        cache
            .put("/file", make_response("/file", b"version1"))
            .unwrap();
        cache
            .put("/file", make_response("/file", b"version2"))
            .unwrap();

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.match_url("/file").unwrap().body, b"version2");
    }
}
