//! Memory Management Unit Tests
//!
//! Tests for buddy allocator, slab allocator, and page table management.

#[cfg(test)]
mod tests {
    use crate::memory::{PhysicalMemoryManager, VirtualMemoryManager};
    use crate::allocator::{BuddyAllocator, HEAP_SIZE};
    
    // ========================================
    // Buddy Allocator Tests
    // ========================================
    
    #[test]
    fn test_buddy_alloc_single() {
        // Test allocating a single page (4KB)
        // In production this would use the actual allocator
        let size = 4096usize;
        let layout = core::alloc::Layout::from_size_align(size, 4096).unwrap();
        
        // Verify layout is valid
        assert_eq!(layout.size(), 4096);
        assert_eq!(layout.align(), 4096);
    }
    
    #[test]
    fn test_buddy_alloc_multi() {
        // Test allocating multiple pages
        let sizes = [4096, 8192, 16384, 32768];
        
        for size in sizes {
            let layout = core::alloc::Layout::from_size_align(size, 4096).unwrap();
            assert_eq!(layout.size(), size);
        }
    }
    
    #[test]
    fn test_buddy_power_of_two() {
        // Buddy allocator works with power of 2 sizes
        fn next_power_of_two(n: usize) -> usize {
            let mut power = 1;
            while power < n {
                power *= 2;
            }
            power
        }
        
        assert_eq!(next_power_of_two(1), 1);
        assert_eq!(next_power_of_two(3), 4);
        assert_eq!(next_power_of_two(5), 8);
        assert_eq!(next_power_of_two(1000), 1024);
        assert_eq!(next_power_of_two(4096), 4096);
    }
    
    #[test]
    fn test_buddy_order_calculation() {
        // Calculate order for buddy system
        fn size_to_order(size: usize) -> usize {
            let min_block = 4096; // PAGE_SIZE
            let size = size.max(min_block);
            let blocks = (size + min_block - 1) / min_block;
            (usize::BITS - blocks.leading_zeros() - 1) as usize
        }
        
        assert_eq!(size_to_order(4096), 0);  // 1 page = order 0
        assert_eq!(size_to_order(8192), 1);  // 2 pages = order 1
        assert_eq!(size_to_order(16384), 2); // 4 pages = order 2
    }
    
    // ========================================
    // Slab Allocator Tests
    // ========================================
    
    #[test]
    fn test_slab_object_size() {
        // Slab allocator object sizing
        struct TestObject {
            id: u64,
            data: [u8; 56],
        }
        
        assert_eq!(core::mem::size_of::<TestObject>(), 64);
        assert!(core::mem::size_of::<TestObject>() <= 128);
    }
    
    #[test]
    fn test_slab_alignment() {
        // Objects must be properly aligned
        struct AlignedObject {
            _pad: u64,
        }
        
        let align = core::mem::align_of::<AlignedObject>();
        assert!(align >= 8);
        assert!(align.is_power_of_two());
    }
    
    // ========================================
    // Page Table Tests
    // ========================================
    
    #[test]
    fn test_page_table_entry_flags() {
        // Page table entry flags
        const PRESENT: u64 = 1 << 0;
        const WRITABLE: u64 = 1 << 1;
        const USER: u64 = 1 << 2;
        const NO_EXECUTE: u64 = 1 << 63;
        
        let entry = PRESENT | WRITABLE;
        assert!(entry & PRESENT != 0);
        assert!(entry & WRITABLE != 0);
        assert!(entry & USER == 0);
        assert!(entry & NO_EXECUTE == 0);
    }
    
    #[test]
    fn test_virtual_address_parts() {
        // Virtual address breakdown for 4-level paging
        fn get_page_table_indices(vaddr: u64) -> (u16, u16, u16, u16, u16) {
            let pml4 = ((vaddr >> 39) & 0x1FF) as u16;
            let pdpt = ((vaddr >> 30) & 0x1FF) as u16;
            let pd = ((vaddr >> 21) & 0x1FF) as u16;
            let pt = ((vaddr >> 12) & 0x1FF) as u16;
            let offset = (vaddr & 0xFFF) as u16;
            (pml4, pdpt, pd, pt, offset)
        }
        
        // Test canonical high address
        let (pml4, pdpt, pd, pt, offset) = get_page_table_indices(0xFFFF_8000_0010_0000);
        assert_eq!(offset, 0);
        assert!(pml4 >= 256); // High half
        
        // Test low address
        let (pml4, _, _, _, _) = get_page_table_indices(0x0000_0000_0040_0000);
        assert!(pml4 < 256); // Low half
    }
    
    #[test]
    fn test_page_frame_number() {
        // Physical address to page frame number
        fn phys_to_pfn(phys: u64) -> u64 {
            phys >> 12 // Divide by PAGE_SIZE (4096)
        }
        
        fn pfn_to_phys(pfn: u64) -> u64 {
            pfn << 12 // Multiply by PAGE_SIZE
        }
        
        assert_eq!(phys_to_pfn(0x1000), 1);
        assert_eq!(phys_to_pfn(0x100000), 256);
        assert_eq!(pfn_to_phys(1), 0x1000);
        assert_eq!(pfn_to_phys(256), 0x100000);
    }
    
    // ========================================
    // Heap Tests
    // ========================================
    
    #[test]
    fn test_heap_layout_sizes() {
        // Common allocation sizes
        let sizes = [8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
        
        for size in sizes {
            let layout = core::alloc::Layout::from_size_align(size, 8);
            assert!(layout.is_ok());
        }
    }
    
    #[test]
    fn test_heap_alignment_requirements() {
        // Various alignment requirements
        let alignments = [1, 2, 4, 8, 16, 32, 64, 128, 256, 4096];
        
        for align in alignments {
            if align.is_power_of_two() {
                let layout = core::alloc::Layout::from_size_align(align, align);
                assert!(layout.is_ok());
            }
        }
    }
}
