//! Code Cache for JIT Compiled Functions
//!
//! This module implements a cache for storing compiled native code,
//! with LRU eviction when the cache is full.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;

use super::codegen::NativeCode;
use super::{CompilationTier, FunctionId};

/// Entry in the code cache.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Compiled native code.
    pub code: Arc<NativeCode>,
    /// Compilation tier.
    pub tier: CompilationTier,
    /// Time taken to compile (microseconds).
    pub compilation_time_us: u64,
}

/// Code cache with LRU eviction.
pub struct CodeCache {
    /// Cached entries by function ID.
    entries: BTreeMap<FunctionId, CacheEntry>,
    /// Total size of cached code.
    total_size: usize,
    /// Maximum cache size.
    max_size: usize,
    /// Access order for LRU (most recent last).
    access_order: Vec<FunctionId>,
}

impl CodeCache {
    /// Create a new code cache with maximum size.
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            total_size: 0,
            max_size,
            access_order: Vec::new(),
        }
    }

    /// Get cached code for a function.
    pub fn get(&self, func_id: &FunctionId) -> Option<&CacheEntry> {
        self.entries.get(func_id)
    }

    /// Insert compiled code into the cache.
    pub fn insert(&mut self, func_id: FunctionId, entry: CacheEntry) {
        let code_size = entry.code.size();

        // Evict entries if necessary
        while self.total_size + code_size > self.max_size && !self.entries.is_empty() {
            self.evict_lru();
        }

        // Remove from access order if already present
        self.access_order.retain(|&id| id != func_id);

        // Add to access order (most recent last)
        self.access_order.push(func_id);

        // Update total size
        if let Some(old) = self.entries.get(&func_id) {
            self.total_size -= old.code.size();
        }
        self.total_size += code_size;

        // Insert entry
        self.entries.insert(func_id, entry);
    }

    /// Remove cached code for a function.
    pub fn remove(&mut self, func_id: &FunctionId) -> Option<CacheEntry> {
        if let Some(entry) = self.entries.remove(func_id) {
            self.total_size -= entry.code.size();
            self.access_order.retain(|&id| id != *func_id);
            Some(entry)
        } else {
            None
        }
    }

    /// Evict the least recently used entry.
    fn evict_lru(&mut self) {
        if let Some(func_id) = self.access_order.first().copied() {
            self.remove(&func_id);
        }
    }

    /// Mark a function as recently accessed.
    pub fn touch(&mut self, func_id: FunctionId) {
        // Move to end of access order
        self.access_order.retain(|&id| id != func_id);
        self.access_order.push(func_id);
    }

    /// Get number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get total size of cached code.
    pub fn total_size(&self) -> usize {
        self.total_size
    }

    /// Get maximum cache size.
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_order.clear();
        self.total_size = 0;
    }

    /// Iterate over all cached functions.
    pub fn iter(&self) -> impl Iterator<Item = (&FunctionId, &CacheEntry)> {
        self.entries.iter()
    }

    /// Get statistics about the cache.
    pub fn stats(&self) -> CacheStats {
        let mut baseline_count = 0;
        let mut optimized_count = 0;
        let mut baseline_size = 0;
        let mut optimized_size = 0;

        for entry in self.entries.values() {
            match entry.tier {
                CompilationTier::Baseline => {
                    baseline_count += 1;
                    baseline_size += entry.code.size();
                }
                CompilationTier::Optimized => {
                    optimized_count += 1;
                    optimized_size += entry.code.size();
                }
                _ => {}
            }
        }

        CacheStats {
            total_entries: self.entries.len(),
            baseline_entries: baseline_count,
            optimized_entries: optimized_count,
            total_size: self.total_size,
            baseline_size,
            optimized_size,
            max_size: self.max_size,
            utilization: self.total_size as f32 / self.max_size as f32,
        }
    }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total number of cached entries.
    pub total_entries: usize,
    /// Number of baseline-compiled entries.
    pub baseline_entries: usize,
    /// Number of optimized entries.
    pub optimized_entries: usize,
    /// Total size of cached code.
    pub total_size: usize,
    /// Size of baseline code.
    pub baseline_size: usize,
    /// Size of optimized code.
    pub optimized_size: usize,
    /// Maximum cache size.
    pub max_size: usize,
    /// Cache utilization (0.0 - 1.0).
    pub utilization: f32,
}

/// Persistent code cache for AOT compiled code.
///
/// In a full kernel environment this would persist compiled code to
/// the block device. In the current `no_std` configuration we keep
/// an in-memory store keyed by `(module_id, hash)` so that modules
/// reloaded within the same boot cycle can reuse prior compilations.
pub struct PersistentCache {
    /// Path to cache directory (for future on-disk storage).
    cache_path: alloc::string::String,
    /// Index of cached modules.
    index: BTreeMap<u64, CachedModuleInfo>,
    /// In-memory store: module_id → Vec<(func_index, NativeCode)>
    store: BTreeMap<u64, Vec<(u32, Arc<NativeCode>)>>,
}

/// Information about a cached module.
#[derive(Debug, Clone)]
pub struct CachedModuleInfo {
    /// Module hash (for invalidation).
    pub hash: u64,
    /// Number of cached functions.
    pub function_count: usize,
    /// Total code size.
    pub total_size: usize,
    /// Cache timestamp.
    pub timestamp: u64,
}

impl PersistentCache {
    /// Create a new persistent cache.
    pub fn new(cache_path: &str) -> Self {
        Self {
            cache_path: cache_path.into(),
            index: BTreeMap::new(),
            store: BTreeMap::new(),
        }
    }

    /// Load a cached module. Returns `Some(functions)` if the module
    /// was previously compiled and its hash still matches.
    pub fn load_module(
        &self,
        module_id: u64,
        expected_hash: u64,
    ) -> Option<Vec<(u32, Arc<NativeCode>)>> {
        // Check if the module is in the index and the hash matches
        let info = self.index.get(&module_id)?;
        if info.hash != expected_hash {
            return None; // stale cache entry
        }
        self.store.get(&module_id).cloned()
    }

    /// Save a compiled module to cache.
    pub fn save_module(
        &mut self,
        module_id: u64,
        hash: u64,
        functions: &[(u32, Arc<NativeCode>)],
    ) {
        let total_size: usize = functions.iter().map(|(_, code)| code.size()).sum();
        let info = CachedModuleInfo {
            hash,
            function_count: functions.len(),
            total_size,
            timestamp: 0, // No real clock in no_std; kernel would use system time
        };
        self.index.insert(module_id, info);
        self.store.insert(module_id, functions.to_vec());
    }

    /// Check if a module is cached.
    pub fn has_module(&self, module_id: u64) -> bool {
        self.index.contains_key(&module_id)
    }

    /// Get cache info for a module.
    pub fn module_info(&self, module_id: u64) -> Option<&CachedModuleInfo> {
        self.index.get(&module_id)
    }

    /// Invalidate cached module.
    pub fn invalidate(&mut self, module_id: u64) {
        self.index.remove(&module_id);
        self.store.remove(&module_id);
    }

    /// Clear all cached modules.
    pub fn clear(&mut self) {
        self.index.clear();
        self.store.clear();
    }

    /// Get path to cache directory.
    pub fn cache_path(&self) -> &str {
        &self.cache_path
    }

    /// Total number of cached modules.
    pub fn module_count(&self) -> usize {
        self.index.len()
    }

    /// Total cached code size across all modules.
    pub fn total_cached_size(&self) -> usize {
        self.index.values().map(|info| info.total_size).sum()
    }
}
