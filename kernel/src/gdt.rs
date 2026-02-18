//! GDT (Global Descriptor Table) initialization
//!
//! The GDT manages segment descriptors on x86_64.
//! In 64-bit long mode, segmentation is mostly disabled,
//! but the TSS (Task State Segment) is still required for:
//! - Ring 3 → Ring 0 stack switching (RSP0)
//! - Interrupt Stack Table (IST) for critical exceptions
//!
//! ## GDT Layout
//!
//! | Index | Byte Offset | Selector (RPL) | Description          |
//! |-------|-------------|----------------|----------------------|
//! |   0   |    0x00     |      —         | Null descriptor      |
//! |   1   |    0x08     |    0x08        | Kernel Code (Ring 0) |
//! |   2   |    0x10     |    0x10        | Kernel Data (Ring 0) |
//! |   3   |    0x18     |    0x1B        | User Data (Ring 3)   |
//! |   4   |    0x20     |    0x23        | User Code (Ring 3)   |
//! |  5-6  |    0x28     |    0x28        | TSS (System, 16B)    |
//!
//! ## SYSRET Compatibility
//!
//! STAR MSR bits[63:48] = 0x0010 (kernel data base).
//! - SYSRET SS = 0x10 + 8 = 0x18 → RPL 3 = 0x1B (User Data)
//! - SYSRET CS = 0x10 + 16 = 0x20 → RPL 3 = 0x23 (User Code)

use lazy_static::lazy_static;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

/// IST index for the Double Fault handler.
///
/// Since a Double Fault may be caused by a stack overflow,
/// a separate stack is used.
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

/// User code segment selector (Ring 3).
/// GDT index 4, byte offset 0x20, RPL 3 → selector value 0x23.
pub const USER_CS: u16 = 0x23;

/// User data segment selector (Ring 3).
/// GDT index 3, byte offset 0x18, RPL 3 → selector value 0x1B.
pub const USER_DS: u16 = 0x1B;

/// Kernel code segment selector (Ring 0).
pub const KERNEL_CS: u16 = 0x08;

/// Kernel data segment selector (Ring 0).
pub const KERNEL_DS: u16 = 0x10;

/// Double fault stack size (20 KiB).
const DOUBLE_FAULT_STACK_SIZE: usize = 4096 * 5;

/// Static stack for double fault handler (allocated before heap init).
static mut DOUBLE_FAULT_STACK: [u8; DOUBLE_FAULT_STACK_SIZE] = [0; DOUBLE_FAULT_STACK_SIZE];

/// Task State Segment.
///
/// Mutable static so we can update RSP0 (privilege stack) at runtime
/// when switching to/from user-mode processes.
static mut TSS: TaskStateSegment = TaskStateSegment::new();

lazy_static! {
    /// Global Descriptor Table and segment selectors.
    ///
    /// Order matters for SYSRET: user data must be at index 3,
    /// user code at index 4. TSS follows at index 5-6.
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();

        // Index 1: Kernel code segment (Ring 0, executable)
        let code_selector = gdt.append(Descriptor::kernel_code_segment());

        // Index 2: Kernel data segment (Ring 0, read/write)
        let data_selector = gdt.append(Descriptor::kernel_data_segment());

        // Index 3: User data segment (Ring 3, read/write)
        let user_data_selector = gdt.append(Descriptor::user_data_segment());

        // Index 4: User code segment (Ring 3, executable, 64-bit)
        let user_code_selector = gdt.append(Descriptor::user_code_segment());

        // Index 5-6: TSS segment (system descriptor, occupies 2 entries)
        // SAFETY: TSS is initialized in init() before GDT is first accessed.
        let tss_selector = gdt.append(Descriptor::tss_segment(
            unsafe { &*core::ptr::addr_of!(TSS) }
        ));

        (gdt, Selectors {
            code_selector,
            data_selector,
            user_data_selector,
            user_code_selector,
            tss_selector,
        })
    };
}

/// Segment selectors stored alongside the GDT.
struct Selectors {
    code_selector: SegmentSelector,
    #[allow(dead_code)]
    data_selector: SegmentSelector,
    #[allow(dead_code)]
    user_data_selector: SegmentSelector,
    #[allow(dead_code)]
    user_code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

/// Initialize the GDT.
///
/// This function must be called only once at kernel startup.
/// Must be called before IDT initialization for proper TSS setup.
///
/// Initialization order:
/// 1. Set up TSS IST entries (double fault stack)
/// 2. Load GDT (triggers lazy_static init which references TSS)
/// 3. Set CS register and load TSS
pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS};
    use x86_64::instructions::tables::load_tss;

    // Step 1: Initialize TSS before GDT references it
    unsafe {
        let stack_start = VirtAddr::from_ptr(core::ptr::addr_of!(DOUBLE_FAULT_STACK));
        let stack_end = stack_start + DOUBLE_FAULT_STACK_SIZE as u64;
        (*core::ptr::addr_of_mut!(TSS)).interrupt_stack_table
            [DOUBLE_FAULT_IST_INDEX as usize] = stack_end;
    }

    // Step 2: Load the GDT (triggers lazy_static initialization)
    GDT.0.load();

    // Step 3: Set segment registers and load TSS
    unsafe {
        // Set code segment register
        CS::set_reg(GDT.1.code_selector);

        // Load the TSS
        load_tss(GDT.1.tss_selector);
    }
}

/// Set the kernel stack pointer in TSS (RSP0).
///
/// This must be called before returning to Ring 3 userspace.
/// When a Ring 3 → Ring 0 transition occurs (interrupt or exception),
/// the CPU loads RSP from TSS.privilege_stack_table[0].
///
/// # Safety
///
/// The `stack_top` must point to a valid, mapped kernel stack.
/// Caller must ensure this is called with interrupts disabled or
/// from a context where TSS modification is safe.
pub fn set_kernel_stack(stack_top: VirtAddr) {
    unsafe {
        (*core::ptr::addr_of_mut!(TSS)).privilege_stack_table[0] = stack_top;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_segment_selectors() {
        // User data: GDT index 3, RPL 3 → (3 << 3) | 3 = 0x1B
        assert_eq!(USER_DS, 0x1B);
        // User code: GDT index 4, RPL 3 → (4 << 3) | 3 = 0x23
        assert_eq!(USER_CS, 0x23);
    }

    #[test]
    fn test_kernel_segment_selectors() {
        assert_eq!(KERNEL_CS, 0x08);
        assert_eq!(KERNEL_DS, 0x10);
    }

    #[test]
    fn test_sysret_compatibility() {
        // STAR MSR bits[63:48] = 0x0010 (KERNEL_DS)
        // SYSRET SS = STAR[63:48] + 8 = 0x18, RPL forced to 3 → 0x1B
        let sysret_ss = (KERNEL_DS as u16 + 8) | 3;
        assert_eq!(sysret_ss, USER_DS);

        // SYSRET CS = STAR[63:48] + 16 = 0x20, RPL forced to 3 → 0x23
        let sysret_cs = (KERNEL_DS as u16 + 16) | 3;
        assert_eq!(sysret_cs, USER_CS);
    }

    #[test]
    fn test_segment_rpl() {
        // User segments must have RPL 3
        assert_eq!(USER_CS & 0x3, 3);
        assert_eq!(USER_DS & 0x3, 3);
        // Kernel segments must have RPL 0
        assert_eq!(KERNEL_CS & 0x3, 0);
        assert_eq!(KERNEL_DS & 0x3, 0);
    }
}
