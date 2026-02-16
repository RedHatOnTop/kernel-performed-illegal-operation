//! WASM linear memory management.
//!
//! This module handles allocation and access to WASM linear memory.

use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;

use crate::RuntimeError;

/// Page size in bytes (64 KB).
pub const PAGE_SIZE: usize = 65536;

/// Maximum memory size (4 GB).
pub const MAX_MEMORY_SIZE: usize = 4 * 1024 * 1024 * 1024;

/// Linear memory for a WASM instance.
pub struct LinearMemory {
    /// Memory data.
    data: Vec<u8>,

    /// Current size in pages.
    current_pages: u32,

    /// Maximum size in pages (if specified).
    max_pages: Option<u32>,
}

impl LinearMemory {
    /// Create a new linear memory.
    pub fn new(initial_pages: u32, max_pages: Option<u32>) -> Result<Self, RuntimeError> {
        let initial_size = initial_pages as usize * PAGE_SIZE;

        if initial_size > MAX_MEMORY_SIZE {
            return Err(RuntimeError::MemoryError(
                "Initial memory size exceeds maximum".into(),
            ));
        }

        if let Some(max) = max_pages {
            if (max as usize * PAGE_SIZE) > MAX_MEMORY_SIZE {
                return Err(RuntimeError::MemoryError(
                    "Maximum memory size exceeds limit".into(),
                ));
            }
            if initial_pages > max {
                return Err(RuntimeError::MemoryError(
                    "Initial pages exceeds maximum pages".into(),
                ));
            }
        }

        let mut data = Vec::new();
        data.resize(initial_size, 0);

        Ok(LinearMemory {
            data,
            current_pages: initial_pages,
            max_pages,
        })
    }

    /// Get the current size in bytes.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Get the current size in pages.
    pub fn pages(&self) -> u32 {
        self.current_pages
    }

    /// Get the maximum size in pages.
    pub fn max_pages(&self) -> Option<u32> {
        self.max_pages
    }

    /// Grow memory by the specified number of pages.
    /// Returns the previous size in pages, or an error if growth fails.
    pub fn grow(&mut self, delta_pages: u32) -> Result<u32, RuntimeError> {
        let new_pages = self
            .current_pages
            .checked_add(delta_pages)
            .ok_or_else(|| RuntimeError::MemoryError("Page count overflow".into()))?;

        if let Some(max) = self.max_pages {
            if new_pages > max {
                return Err(RuntimeError::MemoryError(
                    "Would exceed maximum memory size".into(),
                ));
            }
        }

        let new_size = new_pages as usize * PAGE_SIZE;
        if new_size > MAX_MEMORY_SIZE {
            return Err(RuntimeError::MemoryError(
                "Would exceed absolute maximum memory size".into(),
            ));
        }

        let old_pages = self.current_pages;
        self.data.resize(new_size, 0);
        self.current_pages = new_pages;

        Ok(old_pages)
    }

    /// Read a byte from memory.
    pub fn read_u8(&self, offset: usize) -> Result<u8, RuntimeError> {
        self.check_bounds(offset, 1)?;
        Ok(self.data[offset])
    }

    /// Read a u16 from memory (little-endian).
    pub fn read_u16(&self, offset: usize) -> Result<u16, RuntimeError> {
        self.check_bounds(offset, 2)?;
        Ok(u16::from_le_bytes([
            self.data[offset],
            self.data[offset + 1],
        ]))
    }

    /// Read a u32 from memory (little-endian).
    pub fn read_u32(&self, offset: usize) -> Result<u32, RuntimeError> {
        self.check_bounds(offset, 4)?;
        Ok(u32::from_le_bytes([
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ]))
    }

    /// Read a u64 from memory (little-endian).
    pub fn read_u64(&self, offset: usize) -> Result<u64, RuntimeError> {
        self.check_bounds(offset, 8)?;
        Ok(u64::from_le_bytes([
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
            self.data[offset + 4],
            self.data[offset + 5],
            self.data[offset + 6],
            self.data[offset + 7],
        ]))
    }

    /// Read bytes from memory.
    pub fn read_bytes(&self, offset: usize, len: usize) -> Result<&[u8], RuntimeError> {
        self.check_bounds(offset, len)?;
        Ok(&self.data[offset..offset + len])
    }

    /// Write a byte to memory.
    pub fn write_u8(&mut self, offset: usize, value: u8) -> Result<(), RuntimeError> {
        self.check_bounds(offset, 1)?;
        self.data[offset] = value;
        Ok(())
    }

    /// Write a u16 to memory (little-endian).
    pub fn write_u16(&mut self, offset: usize, value: u16) -> Result<(), RuntimeError> {
        self.check_bounds(offset, 2)?;
        let bytes = value.to_le_bytes();
        self.data[offset] = bytes[0];
        self.data[offset + 1] = bytes[1];
        Ok(())
    }

    /// Write a u32 to memory (little-endian).
    pub fn write_u32(&mut self, offset: usize, value: u32) -> Result<(), RuntimeError> {
        self.check_bounds(offset, 4)?;
        let bytes = value.to_le_bytes();
        self.data[offset..offset + 4].copy_from_slice(&bytes);
        Ok(())
    }

    /// Write a u64 to memory (little-endian).
    pub fn write_u64(&mut self, offset: usize, value: u64) -> Result<(), RuntimeError> {
        self.check_bounds(offset, 8)?;
        let bytes = value.to_le_bytes();
        self.data[offset..offset + 8].copy_from_slice(&bytes);
        Ok(())
    }

    /// Write bytes to memory.
    pub fn write_bytes(&mut self, offset: usize, bytes: &[u8]) -> Result<(), RuntimeError> {
        self.check_bounds(offset, bytes.len())?;
        self.data[offset..offset + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    /// Fill a memory range with a value.
    pub fn fill(&mut self, offset: usize, len: usize, value: u8) -> Result<(), RuntimeError> {
        self.check_bounds(offset, len)?;
        self.data[offset..offset + len].fill(value);
        Ok(())
    }

    /// Copy within memory.
    pub fn copy_within(&mut self, src: usize, dst: usize, len: usize) -> Result<(), RuntimeError> {
        self.check_bounds(src, len)?;
        self.check_bounds(dst, len)?;
        self.data.copy_within(src..src + len, dst);
        Ok(())
    }

    /// Get a slice of memory.
    pub fn slice(&self, range: Range<usize>) -> Result<&[u8], RuntimeError> {
        if range.end > self.data.len() {
            return Err(RuntimeError::MemoryError(
                "Memory access out of bounds".into(),
            ));
        }
        Ok(&self.data[range])
    }

    /// Get a mutable slice of memory.
    pub fn slice_mut(&mut self, range: Range<usize>) -> Result<&mut [u8], RuntimeError> {
        if range.end > self.data.len() {
            return Err(RuntimeError::MemoryError(
                "Memory access out of bounds".into(),
            ));
        }
        Ok(&mut self.data[range])
    }

    /// Check if an access is within bounds.
    fn check_bounds(&self, offset: usize, len: usize) -> Result<(), RuntimeError> {
        let end = offset
            .checked_add(len)
            .ok_or_else(|| RuntimeError::MemoryError("Address overflow".into()))?;

        if end > self.data.len() {
            return Err(RuntimeError::MemoryError(alloc::format!(
                "Memory access out of bounds: {} + {} > {}",
                offset,
                len,
                self.data.len()
            )));
        }

        Ok(())
    }
}

impl Clone for LinearMemory {
    fn clone(&self) -> Self {
        LinearMemory {
            data: self.data.clone(),
            current_pages: self.current_pages,
            max_pages: self.max_pages,
        }
    }
}
