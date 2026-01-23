//! VirtQueue implementation.
//!
//! Virtqueues are the mechanism for bulk data transport between
//! the driver and the device.

use alloc::vec::Vec;
use core::sync::atomic::{fence, Ordering};

/// VirtQueue descriptor flags.
pub mod desc_flags {
    /// Buffer continues via the next field.
    pub const NEXT: u16 = 1;
    /// Buffer is write-only (device writes, driver reads).
    pub const WRITE: u16 = 2;
    /// Buffer contains a list of buffer descriptors (indirect).
    pub const INDIRECT: u16 = 4;
}

/// A virtqueue descriptor.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VirtqDesc {
    /// Physical address of buffer.
    pub addr: u64,
    /// Length of buffer.
    pub len: u32,
    /// Descriptor flags.
    pub flags: u16,
    /// Next descriptor index if NEXT flag is set.
    pub next: u16,
}

/// Available ring structure.
#[repr(C)]
#[derive(Debug)]
pub struct VirtqAvail {
    /// Flags (unused in most drivers).
    pub flags: u16,
    /// Index of next descriptor to add.
    pub idx: u16,
    /// Ring of descriptor indices.
    pub ring: [u16; 256],
    /// Used event (for event suppression).
    pub used_event: u16,
}

/// Used ring element.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct VirtqUsedElem {
    /// Index of the descriptor chain head.
    pub id: u32,
    /// Total bytes written to buffer.
    pub len: u32,
}

/// Used ring structure.
#[repr(C)]
#[derive(Debug)]
pub struct VirtqUsed {
    /// Flags (unused in most drivers).
    pub flags: u16,
    /// Index of next element device will write.
    pub idx: u16,
    /// Ring of used elements.
    pub ring: [VirtqUsedElem; 256],
    /// Available event (for event suppression).
    pub avail_event: u16,
}

/// VirtQueue state managed by the driver.
pub struct VirtQueue {
    /// Queue size (number of descriptors).
    queue_size: u16,
    /// Descriptor table.
    desc: *mut VirtqDesc,
    /// Available ring.
    avail: *mut VirtqAvail,
    /// Used ring.
    used: *mut VirtqUsed,
    /// Index of next descriptor to use.
    next_desc: u16,
    /// Last seen used index.
    last_used_idx: u16,
    /// Free descriptor list.
    free_list: Vec<u16>,
}

impl VirtQueue {
    /// Create a new virtqueue with the given memory regions.
    ///
    /// # Safety
    ///
    /// The provided pointers must be valid and properly aligned.
    /// The memory must remain valid for the lifetime of the VirtQueue.
    pub unsafe fn new(
        queue_size: u16,
        desc: *mut VirtqDesc,
        avail: *mut VirtqAvail,
        used: *mut VirtqUsed,
    ) -> Self {
        // Initialize free list with all descriptors
        let free_list = (0..queue_size).collect();
        
        VirtQueue {
            queue_size,
            desc,
            avail,
            used,
            next_desc: 0,
            last_used_idx: 0,
            free_list,
        }
    }
    
    /// Allocate a descriptor from the free list.
    pub fn alloc_desc(&mut self) -> Option<u16> {
        self.free_list.pop()
    }
    
    /// Free a descriptor back to the free list.
    pub fn free_desc(&mut self, idx: u16) {
        self.free_list.push(idx);
    }
    
    /// Add a buffer to the available ring.
    ///
    /// Returns the descriptor index used.
    pub fn add_buffer(
        &mut self,
        addr: u64,
        len: u32,
        write_only: bool,
    ) -> Option<u16> {
        let desc_idx = self.alloc_desc()?;
        
        unsafe {
            let desc = &mut *self.desc.add(desc_idx as usize);
            desc.addr = addr;
            desc.len = len;
            desc.flags = if write_only { desc_flags::WRITE } else { 0 };
            desc.next = 0;
            
            // Add to available ring
            let avail = &mut *self.avail;
            let avail_idx = avail.idx;
            avail.ring[(avail_idx % self.queue_size) as usize] = desc_idx;
            
            // Memory barrier before updating index
            fence(Ordering::SeqCst);
            
            avail.idx = avail_idx.wrapping_add(1);
        }
        
        Some(desc_idx)
    }
    
    /// Add a chained buffer (read then write).
    pub fn add_chained_buffer(
        &mut self,
        read_addr: u64,
        read_len: u32,
        write_addr: u64,
        write_len: u32,
    ) -> Option<u16> {
        let read_desc = self.alloc_desc()?;
        let write_desc = match self.alloc_desc() {
            Some(d) => d,
            None => {
                self.free_desc(read_desc);
                return None;
            }
        };
        
        unsafe {
            // Read buffer (device reads, driver provides data)
            let desc1 = &mut *self.desc.add(read_desc as usize);
            desc1.addr = read_addr;
            desc1.len = read_len;
            desc1.flags = desc_flags::NEXT;
            desc1.next = write_desc;
            
            // Write buffer (device writes, driver receives data)
            let desc2 = &mut *self.desc.add(write_desc as usize);
            desc2.addr = write_addr;
            desc2.len = write_len;
            desc2.flags = desc_flags::WRITE;
            desc2.next = 0;
            
            // Add to available ring
            let avail = &mut *self.avail;
            let avail_idx = avail.idx;
            avail.ring[(avail_idx % self.queue_size) as usize] = read_desc;
            
            fence(Ordering::SeqCst);
            
            avail.idx = avail_idx.wrapping_add(1);
        }
        
        Some(read_desc)
    }
    
    /// Pop a completed buffer from the used ring.
    ///
    /// Returns (descriptor index, bytes written) if available.
    pub fn pop_used(&mut self) -> Option<(u16, u32)> {
        unsafe {
            let used = &*self.used;
            
            fence(Ordering::SeqCst);
            
            if self.last_used_idx == used.idx {
                return None;
            }
            
            let elem = used.ring[(self.last_used_idx % self.queue_size) as usize];
            self.last_used_idx = self.last_used_idx.wrapping_add(1);
            
            // Free the descriptor chain
            self.free_desc_chain(elem.id as u16);
            
            Some((elem.id as u16, elem.len))
        }
    }
    
    /// Free a descriptor chain.
    fn free_desc_chain(&mut self, head: u16) {
        let mut idx = head;
        loop {
            let desc = unsafe { &*self.desc.add(idx as usize) };
            let next = desc.next;
            let has_next = (desc.flags & desc_flags::NEXT) != 0;
            
            self.free_desc(idx);
            
            if has_next {
                idx = next;
            } else {
                break;
            }
        }
    }
    
    /// Check if the queue has pending completions.
    pub fn has_pending(&self) -> bool {
        unsafe {
            fence(Ordering::SeqCst);
            self.last_used_idx != (*self.used).idx
        }
    }
    
    /// Get the number of free descriptors.
    pub fn free_count(&self) -> usize {
        self.free_list.len()
    }
}
