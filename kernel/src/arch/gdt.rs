//! Global Descriptor Table (GDT) setup.
//!
//! This module sets up the GDT for 64-bit long mode operation.

use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use spin::Lazy;

/// Stack size for interrupt handlers.
pub const INTERRUPT_STACK_SIZE: usize = 4096 * 5;

/// Double fault stack index in TSS.
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

/// Page fault stack index in TSS.
pub const PAGE_FAULT_IST_INDEX: u16 = 1;

/// General protection fault stack index in TSS.
pub const GP_FAULT_IST_INDEX: u16 = 2;

/// Stack for double fault handler.
static mut DOUBLE_FAULT_STACK: [u8; INTERRUPT_STACK_SIZE] = [0; INTERRUPT_STACK_SIZE];

/// Stack for page fault handler.
static mut PAGE_FAULT_STACK: [u8; INTERRUPT_STACK_SIZE] = [0; INTERRUPT_STACK_SIZE];

/// Stack for general protection fault handler.
static mut GP_FAULT_STACK: [u8; INTERRUPT_STACK_SIZE] = [0; INTERRUPT_STACK_SIZE];

/// The Task State Segment.
static TSS: Lazy<TaskStateSegment> = Lazy::new(|| {
    let mut tss = TaskStateSegment::new();
    
    // Set up interrupt stack table
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        let stack_start = VirtAddr::from_ptr(unsafe { &DOUBLE_FAULT_STACK });
        stack_start + INTERRUPT_STACK_SIZE as u64
    };
    
    tss.interrupt_stack_table[PAGE_FAULT_IST_INDEX as usize] = {
        let stack_start = VirtAddr::from_ptr(unsafe { &PAGE_FAULT_STACK });
        stack_start + INTERRUPT_STACK_SIZE as u64
    };
    
    tss.interrupt_stack_table[GP_FAULT_IST_INDEX as usize] = {
        let stack_start = VirtAddr::from_ptr(unsafe { &GP_FAULT_STACK });
        stack_start + INTERRUPT_STACK_SIZE as u64
    };
    
    tss
});

/// GDT with segment selectors.
static GDT: Lazy<(GlobalDescriptorTable, Selectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    
    // Kernel code segment (selector 0x08)
    let kernel_code = gdt.append(Descriptor::kernel_code_segment());
    
    // Kernel data segment (selector 0x10)
    let kernel_data = gdt.append(Descriptor::kernel_data_segment());
    
    // User data segment (selector 0x18, with ring 3 RPL = 0x1B)
    let user_data = gdt.append(Descriptor::user_data_segment());
    
    // User code segment (selector 0x20, with ring 3 RPL = 0x23)
    let user_code = gdt.append(Descriptor::user_code_segment());
    
    // TSS segment
    let tss = gdt.append(Descriptor::tss_segment(&TSS));
    
    (gdt, Selectors {
        kernel_code,
        kernel_data,
        user_code,
        user_data,
        tss,
    })
});

/// Segment selectors.
pub struct Selectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub user_data: SegmentSelector,
    pub tss: SegmentSelector,
}

/// Initialize the GDT.
pub fn init() {
    use x86_64::instructions::segmentation::{CS, DS, ES, SS, Segment};
    use x86_64::instructions::tables::load_tss;
    
    GDT.0.load();
    
    unsafe {
        // Reload code segment
        CS::set_reg(GDT.1.kernel_code);
        
        // Load data segments
        DS::set_reg(GDT.1.kernel_data);
        ES::set_reg(GDT.1.kernel_data);
        SS::set_reg(GDT.1.kernel_data);
        
        // Load TSS
        load_tss(GDT.1.tss);
    }
}

/// Get the kernel code selector.
pub fn kernel_code_selector() -> SegmentSelector {
    GDT.1.kernel_code
}

/// Get the kernel data selector.
pub fn kernel_data_selector() -> SegmentSelector {
    GDT.1.kernel_data
}

/// Get the user code selector.
pub fn user_code_selector() -> SegmentSelector {
    GDT.1.user_code
}

/// Get the user data selector.
pub fn user_data_selector() -> SegmentSelector {
    GDT.1.user_data
}

/// Get the TSS selector.
pub fn tss_selector() -> SegmentSelector {
    GDT.1.tss
}
