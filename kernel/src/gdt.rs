//! GDT (Global Descriptor Table) 초기화
//!
//! GDT는 x86_64에서 세그먼트 디스크립터를 관리합니다.
//! 64비트 롱 모드에서는 세그멘테이션이 대부분 비활성화되지만,
//! TSS(Task State Segment)는 여전히 필요합니다.

use lazy_static::lazy_static;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

/// Double Fault 핸들러를 위한 IST 인덱스.
///
/// Double Fault 발생 시 스택 오버플로우 가능성이 있으므로
/// 별도의 스택을 사용합니다.
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    /// Task State Segment.
    ///
    /// TSS는 다음을 포함합니다:
    /// - Privilege Stack Table (Ring 전환 시 사용할 스택)
    /// - Interrupt Stack Table (특정 인터럽트용 별도 스택)
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();

        // Double Fault용 별도 스택 설정
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5; // 20 KiB

            // 정적 스택 할당 (힙 초기화 전이므로)
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &raw const STACK });
            // 스택은 높은 주소에서 낮은 주소로 자라므로 끝 주소 반환
            stack_start + STACK_SIZE as u64
        };

        tss
    };
}

lazy_static! {
    /// Global Descriptor Table과 세그먼트 셀렉터들.
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();

        // 커널 코드 세그먼트 (Ring 0, 실행 가능)
        let code_selector = gdt.append(Descriptor::kernel_code_segment());

        // 커널 데이터 세그먼트 (Ring 0, 읽기/쓰기)
        let data_selector = gdt.append(Descriptor::kernel_data_segment());

        // TSS 세그먼트
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));

        (gdt, Selectors {
            code_selector,
            data_selector,
            tss_selector,
        })
    };
}

/// 세그먼트 셀렉터들.
struct Selectors {
    code_selector: SegmentSelector,
    #[allow(dead_code)]
    data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

/// GDT 초기화.
///
/// 이 함수는 커널 시작 시 한 번만 호출되어야 합니다.
/// IDT 초기화 전에 호출되어야 TSS가 올바르게 설정됩니다.
pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS};
    use x86_64::instructions::tables::load_tss;

    // GDT 로드
    GDT.0.load();

    unsafe {
        // 코드 세그먼트 레지스터 설정
        CS::set_reg(GDT.1.code_selector);

        // TSS 로드
        load_tss(GDT.1.tss_selector);
    }
}
