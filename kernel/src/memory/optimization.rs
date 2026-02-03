//! Memory Optimization Module
//!
//! Provides memory compression, page reclamation, and efficient memory management.

use alloc::vec::Vec;
use alloc::collections::VecDeque;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Compressed page data
#[derive(Debug, Clone)]
pub struct CompressedPage {
    /// Compressed data bytes
    pub compressed_data: Vec<u8>,
    /// Original page size before compression
    pub original_size: usize,
    /// Compression ratio (compressed / original)
    pub compression_ratio: f32,
    /// Physical frame number
    pub pfn: usize,
}

/// Memory compressor for zswap-like functionality
pub struct MemoryCompressor {
    /// Compressed pages pool
    compressed_pool: Vec<CompressedPage>,
    /// Total bytes saved
    bytes_saved: AtomicUsize,
    /// Pages compressed
    pages_compressed: AtomicUsize,
    /// Compression enabled
    enabled: bool,
}

impl MemoryCompressor {
    /// Create a new memory compressor
    pub fn new() -> Self {
        Self {
            compressed_pool: Vec::new(),
            bytes_saved: AtomicUsize::new(0),
            pages_compressed: AtomicUsize::new(0),
            enabled: true,
        }
    }

    /// Simple LZ4-like compression (simplified for no_std)
    pub fn compress(&self, page_data: &[u8]) -> CompressedPage {
        // Simple run-length encoding for demonstration
        let mut compressed = Vec::new();
        let mut i = 0;
        
        while i < page_data.len() {
            let byte = page_data[i];
            let mut run_length = 1u8;
            
            while i + (run_length as usize) < page_data.len() 
                  && page_data[i + run_length as usize] == byte 
                  && run_length < 255 {
                run_length += 1;
            }
            
            if run_length >= 4 {
                // Encode run: 0xFF, byte, length
                compressed.push(0xFF);
                compressed.push(byte);
                compressed.push(run_length);
                i += run_length as usize;
            } else {
                if byte == 0xFF {
                    compressed.push(0xFF);
                    compressed.push(0xFF);
                    compressed.push(1);
                } else {
                    compressed.push(byte);
                }
                i += 1;
            }
        }
        
        let ratio = compressed.len() as f32 / page_data.len() as f32;
        
        CompressedPage {
            compressed_data: compressed,
            original_size: page_data.len(),
            compression_ratio: ratio,
            pfn: 0,
        }
    }

    /// Decompress page data
    pub fn decompress(&self, compressed: &CompressedPage) -> Vec<u8> {
        let mut decompressed = Vec::with_capacity(compressed.original_size);
        let data = &compressed.compressed_data;
        let mut i = 0;
        
        while i < data.len() {
            if data[i] == 0xFF && i + 2 < data.len() {
                let byte = data[i + 1];
                let count = data[i + 2] as usize;
                for _ in 0..count {
                    decompressed.push(byte);
                }
                i += 3;
            } else {
                decompressed.push(data[i]);
                i += 1;
            }
        }
        
        decompressed
    }

    /// Store a compressed page
    pub fn store_compressed(&mut self, page: CompressedPage) {
        let saved = page.original_size.saturating_sub(page.compressed_data.len());
        self.bytes_saved.fetch_add(saved, Ordering::Relaxed);
        self.pages_compressed.fetch_add(1, Ordering::Relaxed);
        self.compressed_pool.push(page);
    }

    /// Get statistics
    pub fn stats(&self) -> CompressionStats {
        CompressionStats {
            pages_compressed: self.pages_compressed.load(Ordering::Relaxed),
            bytes_saved: self.bytes_saved.load(Ordering::Relaxed),
            pool_size: self.compressed_pool.len(),
        }
    }
}

impl Default for MemoryCompressor {
    fn default() -> Self {
        Self::new()
    }
}

/// Compression statistics
#[derive(Debug, Clone, Copy)]
pub struct CompressionStats {
    pub pages_compressed: usize,
    pub bytes_saved: usize,
    pub pool_size: usize,
}

/// LRU entry for page reclamation
#[derive(Debug, Clone, Copy)]
struct LruEntry {
    pfn: usize,
    last_access: u64,
    referenced: bool,
}

/// Page reclaimer using LRU algorithm
pub struct PageReclaimer {
    /// LRU list of pages
    lru_list: VecDeque<LruEntry>,
    /// High watermark - start reclaiming
    watermark_high: usize,
    /// Low watermark - stop reclaiming
    watermark_low: usize,
    /// Current free pages
    free_pages: AtomicUsize,
    /// Pages reclaimed total
    pages_reclaimed: AtomicUsize,
}

impl PageReclaimer {
    /// Create a new page reclaimer
    pub fn new(watermark_low: usize, watermark_high: usize) -> Self {
        Self {
            lru_list: VecDeque::new(),
            watermark_high,
            watermark_low,
            free_pages: AtomicUsize::new(watermark_high),
            pages_reclaimed: AtomicUsize::new(0),
        }
    }

    /// Check if reclamation is needed
    pub fn should_reclaim(&self) -> bool {
        self.free_pages.load(Ordering::Relaxed) < self.watermark_low
    }

    /// Mark page as accessed
    pub fn access_page(&mut self, pfn: usize) {
        // Find and update or add new entry
        if let Some(entry) = self.lru_list.iter_mut().find(|e| e.pfn == pfn) {
            entry.referenced = true;
            entry.last_access = current_timestamp();
        } else {
            self.lru_list.push_back(LruEntry {
                pfn,
                last_access: current_timestamp(),
                referenced: true,
            });
        }
    }

    /// Reclaim inactive pages to reach target
    pub fn reclaim(&mut self, target_pages: usize) -> usize {
        let mut reclaimed = 0;
        
        while reclaimed < target_pages && !self.lru_list.is_empty() {
            // Second chance algorithm
            if let Some(mut entry) = self.lru_list.pop_front() {
                if entry.referenced {
                    // Give second chance
                    entry.referenced = false;
                    self.lru_list.push_back(entry);
                } else {
                    // Reclaim this page
                    reclaimed += 1;
                    self.pages_reclaimed.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        
        reclaimed
    }

    /// Get reclamation statistics
    pub fn stats(&self) -> ReclaimStats {
        ReclaimStats {
            lru_size: self.lru_list.len(),
            free_pages: self.free_pages.load(Ordering::Relaxed),
            pages_reclaimed: self.pages_reclaimed.load(Ordering::Relaxed),
        }
    }
}

/// Reclamation statistics
#[derive(Debug, Clone, Copy)]
pub struct ReclaimStats {
    pub lru_size: usize,
    pub free_pages: usize,
    pub pages_reclaimed: usize,
}

/// String interning pool
pub struct StringInterner {
    /// Interned strings
    strings: Vec<&'static str>,
    /// Common strings pre-interned
    common: CommonStrings,
}

/// Pre-interned common strings
struct CommonStrings {
    empty: &'static str,
    true_str: &'static str,
    false_str: &'static str,
    null_str: &'static str,
}

impl StringInterner {
    /// Create new string interner
    pub fn new() -> Self {
        Self {
            strings: Vec::new(),
            common: CommonStrings {
                empty: "",
                true_str: "true",
                false_str: "false",
                null_str: "null",
            },
        }
    }

    /// Get interned version of common string
    pub fn get_common(&self, s: &str) -> Option<&'static str> {
        match s {
            "" => Some(self.common.empty),
            "true" => Some(self.common.true_str),
            "false" => Some(self.common.false_str),
            "null" => Some(self.common.null_str),
            _ => None,
        }
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current timestamp (mock for no_std)
fn current_timestamp() -> u64 {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression() {
        let compressor = MemoryCompressor::new();
        
        // Test with repeating data (compressible)
        let data = [0u8; 4096];
        let compressed = compressor.compress(&data);
        
        assert!(compressed.compression_ratio < 1.0);
        
        // Test decompression
        let decompressed = compressor.decompress(&compressed);
        assert_eq!(decompressed.len(), data.len());
    }

    #[test]
    fn test_page_reclaimer() {
        let mut reclaimer = PageReclaimer::new(100, 200);
        
        // Add some pages
        for i in 0..10 {
            reclaimer.access_page(i);
        }
        
        assert_eq!(reclaimer.lru_list.len(), 10);
    }
}
