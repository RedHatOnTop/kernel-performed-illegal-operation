//! Page table management for virtual memory.
//!
//! Implements x86_64 4-level paging with support for 4KB and 2MB pages.

use crate::config::{PAGE_SIZE, PHYS_MAP_BASE};
use bitflags::bitflags;

bitflags! {
    /// Page table entry flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PageTableFlags: u64 {
        /// Page is present in memory.
        const PRESENT = 1 << 0;
        /// Page is writable.
        const WRITABLE = 1 << 1;
        /// Page is accessible from user mode.
        const USER_ACCESSIBLE = 1 << 2;
        /// Write-through caching.
        const WRITE_THROUGH = 1 << 3;
        /// Disable caching.
        const NO_CACHE = 1 << 4;
        /// Page has been accessed.
        const ACCESSED = 1 << 5;
        /// Page has been written to.
        const DIRTY = 1 << 6;
        /// Use huge pages (2MB or 1GB).
        const HUGE_PAGE = 1 << 7;
        /// Page is global (not flushed on context switch).
        const GLOBAL = 1 << 8;
        /// Disable execution (NX bit).
        const NO_EXECUTE = 1 << 63;
    }
}

/// A page table with 512 entries.
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

/// A single page table entry.
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTable {
    /// Create a new empty page table.
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry::empty(); 512],
        }
    }
    
    /// Get a reference to an entry.
    pub fn entry(&self, index: usize) -> &PageTableEntry {
        &self.entries[index]
    }
    
    /// Get a mutable reference to an entry.
    pub fn entry_mut(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.entries[index]
    }
    
    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = &PageTableEntry> {
        self.entries.iter()
    }
    
    /// Iterate mutably over all entries.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut PageTableEntry> {
        self.entries.iter_mut()
    }
    
    /// Clear all entries.
    pub fn clear(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.set_unused();
        }
    }
}

impl PageTableEntry {
    /// Create an empty (not present) entry.
    pub const fn empty() -> Self {
        Self(0)
    }
    
    /// Check if the entry is unused (not present).
    pub fn is_unused(&self) -> bool {
        self.0 == 0
    }
    
    /// Set the entry as unused.
    pub fn set_unused(&mut self) {
        self.0 = 0;
    }
    
    /// Get the flags of this entry.
    pub fn flags(&self) -> PageTableFlags {
        PageTableFlags::from_bits_truncate(self.0)
    }
    
    /// Get the physical address this entry points to.
    pub fn addr(&self) -> Option<u64> {
        if self.flags().contains(PageTableFlags::PRESENT) {
            Some(self.0 & 0x000F_FFFF_FFFF_F000)
        } else {
            None
        }
    }
    
    /// Set the physical address and flags.
    pub fn set(&mut self, addr: u64, flags: PageTableFlags) {
        debug_assert!(addr & 0xFFF == 0, "Address must be page-aligned");
        self.0 = addr | flags.bits();
    }
    
    /// Get the next level page table.
    ///
    /// # Safety
    ///
    /// The caller must ensure the physical memory offset is correct.
    pub unsafe fn next_table(&self) -> Option<&'static PageTable> {
        if !self.flags().contains(PageTableFlags::PRESENT) {
            return None;
        }
        if self.flags().contains(PageTableFlags::HUGE_PAGE) {
            return None;
        }
        
        let phys = self.addr()?;
        let virt = phys + PHYS_MAP_BASE;
        Some(unsafe { &*(virt as *const PageTable) })
    }
    
    /// Get the next level page table mutably.
    ///
    /// # Safety
    ///
    /// The caller must ensure the physical memory offset is correct and
    /// exclusive access is properly synchronized.
    pub unsafe fn next_table_mut(&mut self) -> Option<&'static mut PageTable> {
        if !self.flags().contains(PageTableFlags::PRESENT) {
            return None;
        }
        if self.flags().contains(PageTableFlags::HUGE_PAGE) {
            return None;
        }
        
        let phys = self.addr()?;
        let virt = phys + PHYS_MAP_BASE;
        Some(unsafe { &mut *(virt as *mut PageTable) })
    }
}

/// Virtual address structure for 4-level paging.
#[derive(Debug, Clone, Copy)]
pub struct VirtAddr(u64);

impl VirtAddr {
    /// Create a new virtual address.
    pub fn new(addr: u64) -> Self {
        // Ensure canonical address
        Self(addr)
    }
    
    /// Get the raw address value.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
    
    /// Get the page offset (bits 0-11).
    pub fn page_offset(&self) -> u16 {
        (self.0 & 0xFFF) as u16
    }
    
    /// Get the P1 (page table) index (bits 12-20).
    pub fn p1_index(&self) -> usize {
        ((self.0 >> 12) & 0x1FF) as usize
    }
    
    /// Get the P2 (page directory) index (bits 21-29).
    pub fn p2_index(&self) -> usize {
        ((self.0 >> 21) & 0x1FF) as usize
    }
    
    /// Get the P3 (PDPT) index (bits 30-38).
    pub fn p3_index(&self) -> usize {
        ((self.0 >> 30) & 0x1FF) as usize
    }
    
    /// Get the P4 (PML4) index (bits 39-47).
    pub fn p4_index(&self) -> usize {
        ((self.0 >> 39) & 0x1FF) as usize
    }
}

/// Physical address structure.
#[derive(Debug, Clone, Copy)]
pub struct PhysAddr(u64);

impl PhysAddr {
    /// Create a new physical address.
    pub fn new(addr: u64) -> Self {
        debug_assert!(addr <= 0x000F_FFFF_FFFF_FFFF, "Physical address too large");
        Self(addr)
    }
    
    /// Get the raw address value.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
    
    /// Check if the address is page-aligned.
    pub fn is_aligned(&self) -> bool {
        self.0 & (PAGE_SIZE as u64 - 1) == 0
    }
}
