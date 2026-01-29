//! Memory management subsystem.
//!
//! This module provides physical and virtual memory management for the kernel.
//!
//! # Components
//!
//! - **BootInfoFrameAllocator**: Physical frame allocator using bootloader memory map
//! - **Page Table Mapper**: Virtual memory management
//! - **Heap**: Dynamic memory allocation (in allocator module)
//! - **Slab**: Fixed-size object caching
//! - **Buddy**: Power-of-two block allocator

pub mod slab;
pub mod buddy;

use bootloader_api::info::MemoryRegionKind;
use spin::Mutex;
use x86_64::{
    structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB},
    PhysAddr, VirtAddr,
};

/// Global frame allocator for slab and buddy systems.
static GLOBAL_FRAME_ALLOCATOR: Mutex<Option<GlobalFrameAllocator>> = Mutex::new(None);

/// Simple global frame allocator.
struct GlobalFrameAllocator {
    next_frame: u64,
    end_frame: u64,
}

impl GlobalFrameAllocator {
    fn allocate(&mut self) -> Option<u64> {
        if self.next_frame >= self.end_frame {
            return None;
        }
        let frame = self.next_frame;
        self.next_frame += 4096;
        Some(frame)
    }
}

/// Initialize the global frame allocator for slab/buddy.
pub fn init_frame_allocator(start: u64, end: u64) {
    *GLOBAL_FRAME_ALLOCATOR.lock() = Some(GlobalFrameAllocator {
        next_frame: start,
        end_frame: end,
    });
}

/// Allocate a physical frame for slab allocator.
pub fn allocate_frame() -> Option<usize> {
    GLOBAL_FRAME_ALLOCATOR.lock().as_mut()?.allocate().map(|f| f as usize)
}

/// Free a physical frame.
pub fn free_frame(_addr: usize) {
    // In a real implementation, this would return the frame to the pool
    // For now, we don't reclaim frames
}

/// 물리 메모리 오프셋 검증.
///
/// 부트로더가 제공한 물리 메모리 오프셋이 유효한지 확인합니다.
/// 잘못된 오프셋은 메모리 접근 오류를 유발할 수 있습니다.
///
/// # Panics
///
/// 다음 조건 중 하나라도 실패하면 패닉:
/// - 페이지 정렬 검사 (4KiB)
/// - 정규 주소 검사 (canonical address)
/// - 커널 공간 검사 (>= 0xFFFF_8000_0000_0000)
/// - 읽기 테스트 (실제 접근 가능 여부)
pub fn validate_physical_memory_offset(offset: u64) {
    const PAGE_SIZE: u64 = 4096;
    const KERNEL_SPACE_START: u64 = 0xFFFF_8000_0000_0000;

    // 1. 페이지 정렬 검사
    if offset % PAGE_SIZE != 0 {
        panic!(
            "Physical memory offset {:#x} is not page-aligned (must be aligned to {:#x})",
            offset, PAGE_SIZE
        );
    }

    // 2. 정규 주소 검사 (canonical address)
    // x86_64에서 가상 주소는 47비트 또는 57비트 (LA57) 주소 공간 사용
    // 비트 47(또는 57)부터 63까지는 모두 같은 값이어야 함
    let sign_extension = if offset & (1 << 47) != 0 {
        // 음수 범위: 상위 비트가 모두 1이어야 함
        offset & 0xFFFF_0000_0000_0000 == 0xFFFF_0000_0000_0000
    } else {
        // 양수 범위: 상위 비트가 모두 0이어야 함
        offset & 0xFFFF_0000_0000_0000 == 0
    };

    if !sign_extension {
        panic!(
            "Physical memory offset {:#x} is not a canonical address",
            offset
        );
    }

    // 3. 커널 공간 검사 (경고만, Dynamic 매핑에서는 lower half도 유효)
    // bootloader 0.11의 Dynamic 매핑은 higher half를 보장하지 않음
    if offset < KERNEL_SPACE_START {
        crate::serial_println!(
            "[KPIO] Warning: Physical memory offset {:#x} is in lower half (< {:#x})",
            offset,
            KERNEL_SPACE_START
        );
        crate::serial_println!("[KPIO] This is valid with bootloader Dynamic mapping");
    }

    // 4. 읽기 테스트
    // 오프셋 + 0 (물리 주소 0) 위치를 읽어서 접근 가능한지 확인
    // 물리 주소 0은 보통 리얼 모드 IVT가 있거나 비어있음
    let test_ptr = offset as *const u8;
    let _test_read = unsafe { core::ptr::read_volatile(test_ptr) };

    crate::serial_println!("[KPIO] Physical memory offset validated: {:#x}", offset);
}

/// 페이지 테이블 매퍼 초기화.
///
/// # Safety
///
/// 호출자는 다음을 보장해야 합니다:
/// - 전체 물리 메모리가 `physical_memory_offset`에 매핑되어 있음
/// - 이 함수는 한 번만 호출됨
pub unsafe fn init(physical_memory_offset: u64) -> OffsetPageTable<'static> {
    let phys_offset = VirtAddr::new(physical_memory_offset);
    let level_4_table = unsafe { active_level_4_table(phys_offset) };
    unsafe { OffsetPageTable::new(level_4_table, phys_offset) }
}

/// 활성 레벨 4 페이지 테이블 반환.
///
/// # Safety
///
/// 호출자는 전체 물리 메모리가 `physical_memory_offset`에 매핑되어 있음을 보장해야 합니다.
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}

/// 부트로더 메모리 맵 기반 프레임 할당자.
///
/// 부트로더가 제공한 메모리 맵에서 사용 가능한 프레임을 할당합니다.
pub struct BootInfoFrameAllocator<I>
where
    I: Iterator<Item = &'static bootloader_api::info::MemoryRegion>,
{
    memory_regions: I,
    current_region: Option<&'static bootloader_api::info::MemoryRegion>,
    next_frame: u64,
}

impl<I> BootInfoFrameAllocator<I>
where
    I: Iterator<Item = &'static bootloader_api::info::MemoryRegion>,
{
    /// 새 프레임 할당자 생성.
    ///
    /// # Safety
    ///
    /// 호출자는 전달된 메모리 영역이 유효하고 실제로 사용 가능함을 보장해야 합니다.
    pub unsafe fn new(memory_regions: I) -> Self {
        let mut allocator = Self {
            memory_regions,
            current_region: None,
            next_frame: 0,
        };
        allocator.advance_to_usable_region();
        allocator
    }

    /// 다음 사용 가능한 메모리 영역으로 이동.
    fn advance_to_usable_region(&mut self) {
        while let Some(region) = self.memory_regions.next() {
            if region.kind == MemoryRegionKind::Usable {
                self.current_region = Some(region);
                self.next_frame = region.start;
                return;
            }
        }
        self.current_region = None;
    }
}

unsafe impl<I> FrameAllocator<Size4KiB> for BootInfoFrameAllocator<I>
where
    I: Iterator<Item = &'static bootloader_api::info::MemoryRegion>,
{
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        const PAGE_SIZE: u64 = 4096;

        loop {
            let region = self.current_region?;

            if self.next_frame < region.end {
                let frame_addr = self.next_frame;
                self.next_frame += PAGE_SIZE;

                let frame = PhysFrame::containing_address(PhysAddr::new(frame_addr));
                return Some(frame);
            }

            // 현재 영역 소진, 다음 영역으로 이동
            self.advance_to_usable_region();
        }
    }
}
