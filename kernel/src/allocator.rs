//! 힙 할당자 초기화
//!
//! 이 모듈은 커널 힙을 초기화하고 전역 할당자를 설정합니다.
//! linked_list_allocator를 사용하여 동적 메모리 할당을 지원합니다.

use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

/// 힙 시작 가상 주소.
///
/// 커널 공간 상위에 위치하여 다른 매핑과 충돌하지 않도록 함.
pub const HEAP_START: usize = 0x_4444_4444_0000;

/// 힙 크기 (16 MiB).
pub const HEAP_SIZE: usize = 16 * 1024 * 1024;

/// 전역 힙 할당자.
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// 힙 초기화.
///
/// 이 함수는 힙 영역에 대한 페이지를 할당하고 매핑한 후,
/// 전역 할당자를 초기화합니다.
///
/// # Arguments
///
/// * `mapper` - 페이지 테이블 매퍼
/// * `frame_allocator` - 물리 프레임 할당자
///
/// # Errors
///
/// 페이지 매핑 실패 시 `MapToError` 반환.
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    // 힙 영역의 페이지 범위 계산
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    // 각 페이지에 대해 물리 프레임 할당 및 매핑
    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    // 할당자 초기화
    unsafe {
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    Ok(())
}

/// 할당 실패 핸들러.
///
/// 메모리 할당 실패 시 호출됩니다.
#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
