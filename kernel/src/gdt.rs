//! GDT (Global Descriptor Table) initialization
//!
//! The GDT manages segment descriptors on x86_64.
//! In 64-bit long mode, segmentation is mostly disabled,
//! but the TSS (Task State Segment) is still required.

use lazy_static::lazy_static;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

/// IST index for the Double Fault handler.
///
/// Since a Double Fault may be caused by a stack overflow,
/// a separate stack is used.
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    /// Task State Segment.
    ///
    /// The TSS contains:
    /// - Privilege Stack Table (stacks used for Ring transitions)
    /// - Interrupt Stack Table (separate stacks for specific interrupts)
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();

        // Set up a separate stack for Double Fault
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5; // 20 KiB

            // Static stack allocation (since heap is not yet initialized)
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &raw const STACK });
            let stack_end = stack_start + STACK_SIZE as u64;

            // Debug: Print stack addresses
            // Note: This runs during lazy_static initialization

            // Stack grows from high to low addresses, so return the end address
            stack_end
        };

        tss
    };
}

lazy_static! {
    /// Global Descriptor Table and segment selectors.
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();

        // Kernel code segment (Ring 0, executable)
        let code_selector = gdt.append(Descriptor::kernel_code_segment());

        // Kernel data segment (Ring 0, read/write)
        let data_selector = gdt.append(Descriptor::kernel_data_segment());

        // TSS segment
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));

        (gdt, Selectors {
            code_selector,
            data_selector,
            tss_selector,
        })
    };
}

/// Segment selectors.
struct Selectors {
    code_selector: SegmentSelector,
    #[allow(dead_code)]
    data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

/// Initialize the GDT.
///
/// This function must be called only once at kernel startup.
/// Must be called before IDT initialization for proper TSS setup.
pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS};
    use x86_64::instructions::tables::load_tss;

    // Load the GDT
    GDT.0.load();

    unsafe {
        // Set code segment register
        CS::set_reg(GDT.1.code_selector);

        // Load the TSS
        load_tss(GDT.1.tss_selector);
    }
}
