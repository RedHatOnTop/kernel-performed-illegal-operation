# Kernel Design Document

**Document Version:** 1.0.0  
**Last Updated:** 2026-01-21  
**Status:** Initial Draft

---

## Table of Contents

1. [Overview](#1-overview)
2. [Design Goals](#2-design-goals)
3. [Kernel Type and Rationale](#3-kernel-type-and-rationale)
4. [Boot Process](#4-boot-process)
5. [Memory Management](#5-memory-management)
6. [Process and Thread Model](#6-process-and-thread-model)
7. [Scheduler Design](#7-scheduler-design)
8. [Interrupt and Exception Handling](#8-interrupt-and-exception-handling)
9. [Inter-Process Communication](#9-inter-process-communication)
10. [Device Driver Model](#10-device-driver-model)
11. [Kernel API Surface](#11-kernel-api-surface)
12. [Error Handling Strategy](#12-error-handling-strategy)
13. [Testing Strategy](#13-testing-strategy)

---

## 1. Overview

### 1.1 Purpose

This document specifies the design of the KPIO kernel, the Ring 0 component responsible for hardware abstraction, memory management, scheduling, and providing a stable foundation for the WASM runtime.

### 1.2 Scope

This document covers:
- Kernel architecture and subsystem design
- Memory management algorithms and data structures
- Scheduling policies and implementation
- Interrupt handling and device driver interface
- Internal kernel APIs

This document does NOT cover:
- WASM runtime implementation (see `wasm-runtime.md`)
- Graphics subsystem details (see `graphics.md`)
- User-space service design

### 1.3 Source Location

```
kernel/
    Cargo.toml
    src/
        lib.rs              # Kernel library root
        main.rs             # Kernel entry point
        arch/
            mod.rs
            x86_64/
                mod.rs
                boot.rs     # x86_64 boot sequence
                gdt.rs      # Global Descriptor Table
                idt.rs      # Interrupt Descriptor Table
                paging.rs   # Page table management
                apic.rs     # Advanced PIC
                cpu.rs      # CPU feature detection
        boot/
            mod.rs
            uefi.rs         # UEFI protocol handling
            handoff.rs      # Boot info handoff
        memory/
            mod.rs
            pmm.rs          # Physical memory manager
            vmm.rs          # Virtual memory manager
            heap.rs         # Kernel heap allocator
            frame.rs        # Frame allocator
            address.rs      # Address type definitions
        scheduler/
            mod.rs
            task.rs         # Task structure
            executor.rs     # Async executor
            waker.rs        # Waker implementation
        ipc/
            mod.rs
            channel.rs      # IPC channels
            message.rs      # Message types
            capability.rs   # Capability system
        drivers/
            mod.rs
            pci.rs          # PCI enumeration
            uart.rs         # Serial console
            timer.rs        # System timer
        sync/
            mod.rs
            spinlock.rs     # Spinlock primitives
            mutex.rs        # Kernel mutex
            rwlock.rs       # Reader-writer lock
        panic.rs            # Panic handler
        logger.rs           # Kernel logging
```

---

## 2. Design Goals

### 2.1 Primary Goals

| Goal | Priority | Description |
|------|----------|-------------|
| Stability | Critical | Kernel must never crash due to user-space failures |
| Memory Safety | Critical | No buffer overflows, use-after-free, or null dereferences |
| Minimal Attack Surface | High | Expose only essential syscalls |
| Performance | High | Competitive with Linux for target workloads |
| Maintainability | Medium | Clean, documented, testable code |

### 2.2 Non-Goals

| Non-Goal | Rationale |
|----------|-----------|
| POSIX Compatibility | WASI provides sufficient abstraction |
| Native Binary Support | WASM-only policy simplifies security |
| Legacy Hardware | Focus on modern UEFI systems |
| Real-time Guarantees | Cooperative scheduling prioritizes throughput |

---

## 3. Kernel Type and Rationale

### 3.1 Hybrid-Microkernel Classification

KPIO is classified as a **hybrid-microkernel** because:

**Microkernel aspects:**
- Device drivers run in user space (as WASM services)
- File systems run in user space
- Network stack runs in user space
- IPC is a fundamental primitive

**Monolithic aspects:**
- WASM runtime has privileged access for performance
- Memory management is fully in-kernel
- Some critical drivers (timer, interrupt controller) are in-kernel

### 3.2 Comparison Matrix

| Component | Traditional Microkernel | KPIO | Monolithic |
|-----------|------------------------|------|------------|
| Scheduler | Kernel | Kernel | Kernel |
| Memory Manager | Kernel | Kernel | Kernel |
| IPC | Kernel | Kernel | Kernel |
| File System | User | User | Kernel |
| Network Stack | User | User | Kernel |
| Device Drivers | User | Mostly User | Kernel |
| WASM Runtime | N/A | Privileged User | N/A |

### 3.3 Rationale for Hybrid Approach

1. **Security:** User-space drivers cannot corrupt kernel memory
2. **Performance:** Critical paths (memory, scheduling) avoid IPC overhead
3. **Reliability:** Driver crashes are recoverable
4. **WASM Optimization:** Runtime needs fast syscall path

---

## 4. Boot Process

### 4.1 Boot Sequence Overview

```
+-----------------------------------------------------------------------------+
|                              BOOT SEQUENCE                                   |
+-----------------------------------------------------------------------------+

Phase 1: UEFI (Firmware)
    |
    +---> Load kernel EFI application
    +---> Parse kernel command line
    +---> Obtain memory map
    +---> Set graphics mode (GOP)
    +---> Exit boot services
    |
    v
Phase 2: Early Boot (kernel/src/boot/)
    |
    +---> Validate boot info
    +---> Initialize serial console (debug output)
    +---> Parse ACPI tables (RSDP, MADT)
    |        Note: All ACPI physical addresses must be translated
    |        to virtual addresses using phys_mem_offset before access.
    +---> Initialize physical memory manager
    +---> Create initial page tables
    +---> Enable paging with new tables
    |
    v
Phase 3: Architecture Init (kernel/src/arch/x86_64/)
    |
    +---> Load GDT
    +---> Load IDT
    +---> Initialize APIC (disable legacy PIC)
    +---> Calibrate timers (TSC, APIC timer)
    +---> Enable SSE/AVX if available
    |
    v
Phase 4: Kernel Init (kernel/src/main.rs)
    |
    +---> Initialize kernel heap
    +---> Initialize scheduler
    +---> Initialize IPC subsystem
    +---> Enumerate PCI devices
    +---> Start essential drivers (timer, keyboard)
    |
    v
Phase 5: Runtime Init
    |
    +---> Initialize KPIO runtime engine
    +---> Load init.wasm from initramfs
    +---> Transfer control to init process
    |
    v
Phase 6: User Space
    |
    +---> Init process starts services
    +---> Compositor starts
    +---> Shell available
```

### 4.2 UEFI Boot Protocol

The kernel is compiled as an EFI application:

```rust
// kernel/src/boot/uefi.rs

#[repr(C)]
pub struct BootInfo {
    pub memory_map: MemoryMap,
    pub framebuffer: FramebufferInfo,
    pub acpi_rsdp: PhysAddr,
    pub kernel_start: VirtAddr,
    pub kernel_end: VirtAddr,
    pub initramfs_start: PhysAddr,
    pub initramfs_size: usize,
}

#[repr(C)]
pub struct MemoryMap {
    pub entries: *const MemoryDescriptor,
    pub entry_count: usize,
    pub entry_size: usize,
}

#[repr(C)]
pub struct MemoryDescriptor {
    pub memory_type: MemoryType,
    pub physical_start: PhysAddr,
    pub virtual_start: VirtAddr,
    pub page_count: u64,
    pub attribute: u64,
}

#[repr(u32)]
pub enum MemoryType {
    Reserved = 0,
    LoaderCode = 1,
    LoaderData = 2,
    BootServicesCode = 3,
    BootServicesData = 4,
    RuntimeServicesCode = 5,
    RuntimeServicesData = 6,
    Conventional = 7,
    Unusable = 8,
    ACPIReclaimable = 9,
    ACPIMemoryNVS = 10,
    MemoryMappedIO = 11,
    MemoryMappedIOPortSpace = 12,
    PalCode = 13,
    PersistentMemory = 14,
}
```

### 4.3 Memory Map Processing

After exiting UEFI boot services, the kernel processes the memory map:

```rust
// kernel/src/memory/pmm.rs

pub fn init_from_uefi_memory_map(memory_map: &MemoryMap) {
    for descriptor in memory_map.iter() {
        match descriptor.memory_type {
            MemoryType::Conventional |
            MemoryType::BootServicesCode |
            MemoryType::BootServicesData |
            MemoryType::LoaderCode |
            MemoryType::LoaderData => {
                // Mark as usable
                FRAME_ALLOCATOR.add_region(
                    descriptor.physical_start,
                    descriptor.page_count as usize,
                );
            }
            _ => {
                // Mark as reserved
            }
        }
    }
}
```

---

## 5. Memory Management

### 5.1 Address Space Layout

```
Virtual Address Space (x86_64 canonical addresses)
+------------------------------------------------------------------+
| 0xFFFF_FFFF_FFFF_FFFF |                                          |
|         ...           |  Kernel Higher Half                      |
| 0xFFFF_8000_0000_0000 |  (-128 TB to -1)                        |
+------------------------------------------------------------------+
| 0xFFFF_7FFF_FFFF_FFFF |                                          |
|         ...           |  Non-canonical (hole)                    |
| 0x0000_8000_0000_0000 |                                          |
+------------------------------------------------------------------+
| 0x0000_7FFF_FFFF_FFFF |                                          |
|         ...           |  User Space (per-process)                |
| 0x0000_0000_0000_0000 |  (0 to +128 TB)                         |
+------------------------------------------------------------------+

Kernel Address Space Detail:
+------------------------------------------------------------------+
| 0xFFFF_FFFF_FFFF_FFFF | Guard page (unmapped)                    |
+------------------------------------------------------------------+
| 0xFFFF_FFFF_8000_0000 | Kernel stacks region                     |
|         ...           | (2 MB per CPU, guard pages between)      |
| 0xFFFF_FFFF_0000_0000 |                                          |
+------------------------------------------------------------------+
| 0xFFFF_FFFE_FFFF_FFFF | Kernel heap                              |
|         ...           | (grows upward, demand-paged)             |
| 0xFFFF_FF00_0000_0000 |                                          |
+------------------------------------------------------------------+
| 0xFFFF_FEFF_FFFF_FFFF | Direct physical memory mapping           |
|         ...           | (identity map of all physical RAM)       |
| 0xFFFF_8800_0000_0000 |                                          |
+------------------------------------------------------------------+
| 0xFFFF_87FF_FFFF_FFFF | Kernel code/data (loaded from EFI)       |
|         ...           |                                          |
| 0xFFFF_8000_0000_0000 |                                          |
+------------------------------------------------------------------+
```

### 5.2 Physical Memory Manager

**Algorithm:** Buddy allocator with bitmap backing

```rust
// kernel/src/memory/pmm.rs

pub struct PhysicalMemoryManager {
    /// Bitmap tracking allocated 4KB frames
    bitmap: Bitmap,
    
    /// Free lists for each order (0 = 4KB, 1 = 8KB, ..., 10 = 4MB)
    free_lists: [LinkedList<FreeBlock>; MAX_ORDER + 1],
    
    /// Statistics
    stats: PmmStats,
}

pub struct PmmStats {
    pub total_frames: usize,
    pub free_frames: usize,
    pub allocated_frames: usize,
}

impl PhysicalMemoryManager {
    /// Allocate 2^order contiguous pages
    pub fn allocate(&mut self, order: usize) -> Option<PhysAddr> {
        // Try to find a block of the requested size
        if let Some(block) = self.free_lists[order].pop() {
            self.stats.free_frames -= 1 << order;
            self.stats.allocated_frames += 1 << order;
            return Some(block.addr);
        }
        
        // Split a larger block
        for larger_order in (order + 1)..=MAX_ORDER {
            if let Some(block) = self.free_lists[larger_order].pop() {
                // Split down to requested size
                self.split_block(block, larger_order, order);
                return self.allocate(order);
            }
        }
        
        None // Out of memory
    }
    
    /// Free previously allocated pages
    pub fn free(&mut self, addr: PhysAddr, order: usize) {
        debug_assert!(addr.is_aligned(PAGE_SIZE << order));
        
        self.stats.free_frames += 1 << order;
        self.stats.allocated_frames -= 1 << order;
        
        // Try to coalesce with buddy
        let buddy_addr = self.buddy_of(addr, order);
        if self.is_free(buddy_addr, order) {
            self.remove_from_free_list(buddy_addr, order);
            let combined = core::cmp::min(addr, buddy_addr);
            self.free(combined, order + 1);
        } else {
            self.free_lists[order].push(FreeBlock { addr });
        }
    }
}
```

### 5.3 Virtual Memory Manager

**Page Table Structure (4-level, x86_64):**

```
CR3 --> PML4 (Page Map Level 4)
           |
           +--> PDPT (Page Directory Pointer Table)
                   |
                   +--> PD (Page Directory)
                           |
                           +--> PT (Page Table)
                                   |
                                   +--> Physical Frame
```

```rust
// kernel/src/memory/vmm.rs

#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    // Bit flags
    pub const PRESENT: u64 = 1 << 0;
    pub const WRITABLE: u64 = 1 << 1;
    pub const USER_ACCESSIBLE: u64 = 1 << 2;
    pub const WRITE_THROUGH: u64 = 1 << 3;
    pub const NO_CACHE: u64 = 1 << 4;
    pub const ACCESSED: u64 = 1 << 5;
    pub const DIRTY: u64 = 1 << 6;
    pub const HUGE_PAGE: u64 = 1 << 7;
    pub const GLOBAL: u64 = 1 << 8;
    pub const NO_EXECUTE: u64 = 1 << 63;
    
    pub fn frame_addr(&self) -> PhysAddr {
        PhysAddr::new(self.0 & 0x000F_FFFF_FFFF_F000)
    }
    
    pub fn set_frame(&mut self, addr: PhysAddr, flags: u64) {
        self.0 = addr.as_u64() | flags;
    }
}

pub struct AddressSpace {
    /// Root page table (PML4)
    root: PhysAddr,
    
    /// Reference count
    refcount: AtomicUsize,
}

impl AddressSpace {
    /// Map a virtual address to a physical address
    pub fn map(
        &mut self,
        virt: VirtAddr,
        phys: PhysAddr,
        flags: PageFlags,
    ) -> Result<(), MapError> {
        let pml4_index = (virt.as_u64() >> 39) & 0x1FF;
        let pdpt_index = (virt.as_u64() >> 30) & 0x1FF;
        let pd_index = (virt.as_u64() >> 21) & 0x1FF;
        let pt_index = (virt.as_u64() >> 12) & 0x1FF;
        
        // Walk/create page tables
        let pml4 = self.get_table_mut(self.root)?;
        let pdpt = self.get_or_create_table(&mut pml4[pml4_index])?;
        let pd = self.get_or_create_table(&mut pdpt[pdpt_index])?;
        let pt = self.get_or_create_table(&mut pd[pd_index])?;
        
        // Set the mapping
        if pt[pt_index].is_present() {
            return Err(MapError::AlreadyMapped);
        }
        pt[pt_index].set_frame(phys, flags.bits());
        
        // Invalidate TLB for this address
        unsafe { invalidate_page(virt); }
        
        Ok(())
    }
    
    /// Unmap a virtual address
    pub fn unmap(&mut self, virt: VirtAddr) -> Result<PhysAddr, UnmapError> {
        // Similar walk, but clear the entry and return the frame
        // ...
    }
}
```

### 5.4 Kernel Heap Allocator

**Algorithm:** Slab allocator for small objects, buddy for large

```rust
// kernel/src/memory/heap.rs

pub struct KernelHeap {
    /// Slab caches for common sizes
    slabs: [SlabCache; SLAB_COUNT],
    
    /// Fallback for large allocations
    large_allocator: BuddyAllocator,
}

/// Slab sizes: 16, 32, 64, 128, 256, 512, 1024, 2048 bytes
const SLAB_SIZES: [usize; SLAB_COUNT] = [16, 32, 64, 128, 256, 512, 1024, 2048];

pub struct SlabCache {
    object_size: usize,
    objects_per_slab: usize,
    partial_slabs: LinkedList<Slab>,
    full_slabs: LinkedList<Slab>,
    empty_slabs: LinkedList<Slab>,
}

unsafe impl GlobalAlloc for KernelHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(layout.align());
        
        // Use slab for small allocations
        for (i, &slab_size) in SLAB_SIZES.iter().enumerate() {
            if size <= slab_size {
                return self.slabs[i].allocate();
            }
        }
        
        // Large allocation
        self.large_allocator.allocate(layout)
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size().max(layout.align());
        
        for (i, &slab_size) in SLAB_SIZES.iter().enumerate() {
            if size <= slab_size {
                return self.slabs[i].deallocate(ptr);
            }
        }
        
        self.large_allocator.deallocate(ptr, layout)
    }
}
```

---

## 6. Process and Thread Model

### 6.1 Task Abstraction

In KPIO, the fundamental unit of execution is a **Task**, which corresponds to a WASM instance:

```rust
// kernel/src/scheduler/task.rs

pub struct Task {
    /// Unique task identifier
    pub id: TaskId,
    
    /// Task state
    pub state: TaskState,
    
    /// Address space (page tables)
    pub address_space: Arc<AddressSpace>,
    
    /// CPU context for context switching
    pub context: CpuContext,
    
    /// WASM instance handle
    pub wasm_instance: WasmInstance,
    
    /// Capabilities granted to this task
    pub capabilities: CapabilitySet,
    
    /// Parent task (for capability inheritance)
    pub parent: Option<TaskId>,
    
    /// Exit code (set when task exits)
    pub exit_code: Option<i32>,
    
    /// IPC endpoints owned by this task
    pub ipc_endpoints: Vec<IpcEndpoint>,
    
    /// Statistics
    pub stats: TaskStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Ready to run
    Ready,
    
    /// Currently executing on a CPU
    Running,
    
    /// Waiting for I/O or IPC
    Blocked(BlockReason),
    
    /// Task has exited but not yet reaped
    Zombie,
}

#[derive(Debug, Clone, Copy)]
pub enum BlockReason {
    IpcReceive { channel: ChannelId },
    IpcSend { channel: ChannelId },
    Timer { deadline: Instant },
    IoCompletion { request_id: u64 },
}

#[repr(C)]
pub struct CpuContext {
    // General purpose registers
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    
    // Instruction pointer and flags
    pub rip: u64,
    pub rflags: u64,
    
    // Segment registers (for user/kernel mode)
    pub cs: u64,
    pub ss: u64,
    
    // FPU/SSE state pointer
    pub fpu_state: *mut FpuState,
}

pub struct TaskStats {
    pub created_at: Instant,
    pub cpu_time: Duration,
    pub memory_pages: usize,
    pub syscall_count: u64,
}
```

### 6.2 Task Lifecycle

```
                    +-------------------+
                    |      Created      |
                    +--------+----------+
                             |
                             | schedule()
                             v
          +--------------> Ready <--------------+
          |                  |                  |
          |                  | dispatch()       |
          |                  v                  |
          |              Running                |
          |                  |                  |
          +------------------+------------------+
          |   yield()/       |        |         |
          |   preempt()      |        |         |
          |                  |        |         |
          |           block()|        |exit()   |
          |                  v        |         |
          |              Blocked      |         |
          |                  |        |         |
          |     unblock()    |        |         |
          +------------------+        |         |
                                      v         |
                                   Zombie       |
                                      |         |
                                      | reap()  |
                                      v         |
                                  Destroyed ----+
```

---

## 7. Scheduler Design

### 7.1 Cooperative Async Scheduling

KPIO uses **cooperative multitasking** based on Rust's async/await:

```rust
// kernel/src/scheduler/executor.rs

pub struct Executor {
    /// Ready queue of tasks
    ready_queue: SegQueue<TaskId>,
    
    /// All tasks in the system
    tasks: RwLock<BTreeMap<TaskId, Arc<Mutex<Task>>>>,
    
    /// Waker registry
    wakers: RwLock<BTreeMap<TaskId, Waker>>,
    
    /// Timer wheel for sleeping tasks
    timer_wheel: TimerWheel,
}

impl Executor {
    pub fn run(&self) -> ! {
        loop {
            // Process expired timers
            self.timer_wheel.tick();
            
            // Try to get next ready task
            if let Some(task_id) = self.ready_queue.pop() {
                let task = self.tasks.read().get(&task_id).cloned();
                
                if let Some(task) = task {
                    let mut task = task.lock();
                    
                    if task.state == TaskState::Ready {
                        task.state = TaskState::Running;
                        
                        // Create waker for this task
                        let waker = self.create_waker(task_id);
                        let mut cx = Context::from_waker(&waker);
                        
                        // Poll the task's future
                        match task.poll(&mut cx) {
                            Poll::Ready(exit_code) => {
                                task.state = TaskState::Zombie;
                                task.exit_code = Some(exit_code);
                            }
                            Poll::Pending => {
                                // Task yielded, will be re-queued when woken
                                task.state = TaskState::Blocked(BlockReason::Pending);
                            }
                        }
                    }
                }
            } else {
                // No ready tasks, halt until interrupt
                unsafe { halt_until_interrupt(); }
            }
        }
    }
    
    pub fn spawn(&self, wasm_module: &[u8], caps: CapabilitySet) -> TaskId {
        let task_id = TaskId::new();
        let task = Task::new(task_id, wasm_module, caps);
        
        self.tasks.write().insert(task_id, Arc::new(Mutex::new(task)));
        self.ready_queue.push(task_id);
        
        task_id
    }
    
    pub fn wake(&self, task_id: TaskId) {
        if let Some(task) = self.tasks.read().get(&task_id) {
            let mut task = task.lock();
            if matches!(task.state, TaskState::Blocked(_)) {
                task.state = TaskState::Ready;
                self.ready_queue.push(task_id);
            }
        }
    }
}
```

### 7.2 Timer Management

```rust
// kernel/src/scheduler/timer.rs

pub struct TimerWheel {
    /// Current tick count
    current_tick: AtomicU64,
    
    /// Timer slots (hierarchical)
    wheels: [TimerLevel; 4],
    
    /// Tick duration
    tick_duration: Duration,
}

struct TimerLevel {
    slots: [LinkedList<TimerEntry>; 256],
    current_slot: usize,
}

struct TimerEntry {
    deadline: u64,
    task_id: TaskId,
    callback: Option<Box<dyn FnOnce() + Send>>,
}

impl TimerWheel {
    pub fn schedule(&self, task_id: TaskId, delay: Duration) {
        let deadline = self.current_tick.load(Ordering::SeqCst) 
            + (delay.as_nanos() / self.tick_duration.as_nanos()) as u64;
        
        let entry = TimerEntry {
            deadline,
            task_id,
            callback: None,
        };
        
        self.insert(entry);
    }
    
    pub fn tick(&self) {
        let current = self.current_tick.fetch_add(1, Ordering::SeqCst);
        
        // Process expired timers at each level
        for expired in self.collect_expired(current) {
            if let Some(callback) = expired.callback {
                callback();
            } else {
                EXECUTOR.wake(expired.task_id);
            }
        }
    }
}
```

### 7.3 Priority Considerations

While the base scheduler is cooperative, priority hints can be provided:

```rust
pub enum TaskPriority {
    /// System services (compositor, network stack)
    System = 0,
    
    /// Interactive applications
    Interactive = 1,
    
    /// Background tasks
    Background = 2,
    
    /// Idle tasks
    Idle = 3,
}
```

The scheduler uses separate queues per priority level, draining higher priority queues first.

---

## 8. Interrupt and Exception Handling

### 8.1 Interrupt Descriptor Table

```rust
// kernel/src/arch/x86_64/idt.rs

pub static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    
    // Exceptions (0-31)
    idt.divide_error.set_handler(divide_error_handler);
    idt.debug.set_handler(debug_handler);
    idt.non_maskable_interrupt.set_handler(nmi_handler);
    idt.breakpoint.set_handler(breakpoint_handler);
    idt.overflow.set_handler(overflow_handler);
    idt.bound_range_exceeded.set_handler(bound_range_handler);
    idt.invalid_opcode.set_handler(invalid_opcode_handler);
    idt.device_not_available.set_handler(device_not_available_handler);
    idt.double_fault.set_handler(double_fault_handler)
        .set_stack_index(DOUBLE_FAULT_IST_INDEX);
    idt.invalid_tss.set_handler(invalid_tss_handler);
    idt.segment_not_present.set_handler(segment_not_present_handler);
    idt.stack_segment_fault.set_handler(stack_segment_handler);
    idt.general_protection_fault.set_handler(general_protection_handler);
    idt.page_fault.set_handler(page_fault_handler);
    idt.x87_floating_point.set_handler(x87_fp_handler);
    idt.alignment_check.set_handler(alignment_check_handler);
    idt.machine_check.set_handler(machine_check_handler);
    idt.simd_floating_point.set_handler(simd_fp_handler);
    idt.virtualization.set_handler(virtualization_handler);
    
    // Hardware interrupts (32+)
    idt[32].set_handler(timer_interrupt_handler);  // APIC Timer
    idt[33].set_handler(keyboard_interrupt_handler);
    idt[34..48].iter_mut().for_each(|e| {
        e.set_handler(spurious_interrupt_handler);
    });
    
    // Syscall (128)
    idt[128].set_handler(syscall_handler)
        .set_privilege_level(Ring::Ring3);
    
    idt
});
```

### 8.2 Page Fault Handler

```rust
// kernel/src/arch/x86_64/idt.rs

extern "x86-interrupt" fn page_fault_handler(
    frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let fault_addr = Cr2::read();
    
    // Check if this is a valid fault we can handle
    let result = handle_page_fault(fault_addr, error_code, &frame);
    
    match result {
        Ok(()) => {
            // Fault handled (e.g., demand paging, COW)
            return;
        }
        Err(PageFaultError::InvalidAccess) => {
            // User-space fault: terminate the task
            if frame.code_segment.rpl() == 3 {
                log::warn!(
                    "Task {} segfault at {:?}, error: {:?}",
                    current_task_id(),
                    fault_addr,
                    error_code
                );
                terminate_current_task(ExitCode::SEGFAULT);
            } else {
                // Kernel fault: this is a bug
                panic!(
                    "Kernel page fault at {:?}, error: {:?}\n{:#?}",
                    fault_addr, error_code, frame
                );
            }
        }
        Err(PageFaultError::OutOfMemory) => {
            // OOM: terminate task or trigger OOM killer
            handle_oom();
        }
    }
}

fn handle_page_fault(
    addr: VirtAddr,
    error: PageFaultErrorCode,
    _frame: &InterruptStackFrame,
) -> Result<(), PageFaultError> {
    let current = current_task();
    let mut address_space = current.address_space.lock();
    
    // Check VMAs (Virtual Memory Areas)
    if let Some(vma) = address_space.find_vma(addr) {
        match vma.fault_type(addr, error) {
            FaultType::DemandZero => {
                // Allocate a zero page
                let frame = PHYSICAL_MEMORY.allocate(0)?;
                unsafe { zero_page(frame); }
                address_space.map(addr.page_align_down(), frame, vma.flags)?;
                Ok(())
            }
            FaultType::CopyOnWrite => {
                // Copy the page and map writable
                let old_frame = address_space.get_physical(addr)?;
                let new_frame = PHYSICAL_MEMORY.allocate(0)?;
                unsafe { copy_page(old_frame, new_frame); }
                address_space.remap(addr.page_align_down(), new_frame, vma.flags | WRITABLE)?;
                Ok(())
            }
            FaultType::SwapIn => {
                // Unimplemented: no swap support initially
                Err(PageFaultError::InvalidAccess)
            }
        }
    } else {
        Err(PageFaultError::InvalidAccess)
    }
}
```

### 8.3 APIC Configuration

```rust
// kernel/src/arch/x86_64/apic.rs

pub struct LocalApic {
    base_addr: VirtAddr,
}

impl LocalApic {
    pub fn init(&mut self) {
        // Disable legacy 8259 PIC
        unsafe {
            outb(0x21, 0xFF);
            outb(0xA1, 0xFF);
        }
        
        // Enable APIC via MSR
        let apic_base = rdmsr(IA32_APIC_BASE);
        wrmsr(IA32_APIC_BASE, apic_base | APIC_ENABLE);
        
        // Set spurious interrupt vector and enable APIC
        self.write(APIC_SPURIOUS, 0xFF | APIC_SW_ENABLE);
        
        // Configure timer
        self.write(APIC_TIMER_DIV, 0x3); // Divide by 16
        self.write(APIC_TIMER_INIT, 0xFFFFFFFF);
        
        // Calibrate against PIT
        let ticks_per_ms = self.calibrate_timer();
        
        // Set periodic timer
        self.write(APIC_LVT_TIMER, 32 | TIMER_PERIODIC);
        self.write(APIC_TIMER_INIT, ticks_per_ms * TICK_INTERVAL_MS);
    }
    
    pub fn eoi(&self) {
        self.write(APIC_EOI, 0);
    }
}
```

---

## 9. Inter-Process Communication

### 9.1 Channel-Based IPC

```rust
// kernel/src/ipc/channel.rs

pub struct Channel {
    id: ChannelId,
    
    /// Message queue
    messages: ArrayQueue<IpcMessage>,
    
    /// Tasks waiting to receive
    receivers: SegQueue<TaskId>,
    
    /// Tasks waiting to send (if queue full)
    senders: SegQueue<TaskId>,
    
    /// Channel capacity
    capacity: usize,
    
    /// Is the channel still open?
    open: AtomicBool,
}

#[repr(C)]
pub struct IpcMessage {
    /// Message header
    pub header: MessageHeader,
    
    /// Inline payload (up to 4KB)
    pub payload: [u8; MAX_INLINE_SIZE],
    
    /// Transferred capabilities
    pub capabilities: [Option<Capability>; MAX_CAPS],
    
    /// Shared memory pages (for zero-copy large transfers)
    pub shared_pages: Option<SharedPageRange>,
}

#[repr(C)]
pub struct MessageHeader {
    /// Message type/opcode
    pub msg_type: u32,
    
    /// Payload length
    pub payload_len: u32,
    
    /// Number of capabilities
    pub cap_count: u8,
    
    /// Flags
    pub flags: u8,
    
    /// Reserved
    pub reserved: [u8; 6],
}

impl Channel {
    pub fn send(&self, msg: IpcMessage) -> Result<(), IpcError> {
        if !self.open.load(Ordering::SeqCst) {
            return Err(IpcError::ChannelClosed);
        }
        
        // Validate capabilities
        for cap in msg.capabilities.iter().flatten() {
            if !current_task().capabilities.contains(cap) {
                return Err(IpcError::InvalidCapability);
            }
        }
        
        // Try to enqueue
        match self.messages.push(msg) {
            Ok(()) => {
                // Wake a waiting receiver
                if let Some(receiver) = self.receivers.pop() {
                    EXECUTOR.wake(receiver);
                }
                Ok(())
            }
            Err(msg) => {
                // Queue full, block sender
                self.senders.push(current_task_id());
                block_current(BlockReason::IpcSend { channel: self.id });
                
                // Retry after wake
                self.send(msg)
            }
        }
    }
    
    pub fn receive(&self) -> Result<IpcMessage, IpcError> {
        loop {
            if let Some(msg) = self.messages.pop() {
                // Wake a waiting sender
                if let Some(sender) = self.senders.pop() {
                    EXECUTOR.wake(sender);
                }
                return Ok(msg);
            }
            
            if !self.open.load(Ordering::SeqCst) {
                return Err(IpcError::ChannelClosed);
            }
            
            // Block until message available
            self.receivers.push(current_task_id());
            block_current(BlockReason::IpcReceive { channel: self.id });
        }
    }
}
```

### 9.2 Shared Memory

For large data transfers, shared memory avoids copying:

```rust
// kernel/src/ipc/shared_memory.rs

pub struct SharedMemoryRegion {
    id: SharedMemoryId,
    
    /// Physical frames backing this region
    frames: Vec<PhysAddr>,
    
    /// Size in pages
    page_count: usize,
    
    /// Processes that have mapped this region
    mappings: RwLock<Vec<(TaskId, VirtAddr)>>,
}

impl SharedMemoryRegion {
    pub fn create(page_count: usize) -> Result<Self, MemoryError> {
        let mut frames = Vec::with_capacity(page_count);
        
        for _ in 0..page_count {
            let frame = PHYSICAL_MEMORY.allocate(0)
                .ok_or(MemoryError::OutOfMemory)?;
            frames.push(frame);
        }
        
        Ok(Self {
            id: SharedMemoryId::new(),
            frames,
            page_count,
            mappings: RwLock::new(Vec::new()),
        })
    }
    
    pub fn map_into(&self, task: &mut Task, virt: VirtAddr) -> Result<(), MemoryError> {
        for (i, &frame) in self.frames.iter().enumerate() {
            let addr = virt + (i * PAGE_SIZE);
            task.address_space.map(addr, frame, PageFlags::USER_DATA)?;
        }
        
        self.mappings.write().push((task.id, virt));
        Ok(())
    }
}
```

---

## 10. Device Driver Model

### 10.1 Driver Architecture

KPIO uses a **user-space driver model** for most devices:

```
+------------------------------------------------------------------+
|                         USER SPACE                                |
+------------------------------------------------------------------+
|  +------------------+    +------------------+                     |
|  |  Device Driver   |    |  Device Driver   |                     |
|  |  (WASM Service)  |    |  (WASM Service)  |                     |
|  +--------+---------+    +--------+---------+                     |
|           |                       |                               |
|           | DeviceIO Capability   | DeviceIO Capability           |
|           v                       v                               |
+------------------------------------------------------------------+
|                       KERNEL (Ring 0)                             |
+------------------------------------------------------------------+
|  +-----------------------------------------------------------+   |
|  |                    Device Manager                          |   |
|  |  - PCI enumeration                                         |   |
|  |  - Interrupt routing                                       |   |
|  |  - MMIO mapping                                            |   |
|  +-----------------------------------------------------------+   |
+------------------------------------------------------------------+
|                         HARDWARE                                  |
+------------------------------------------------------------------+
```

### 10.2 In-Kernel Drivers

Only essential drivers remain in the kernel:

| Driver | Justification |
|--------|---------------|
| APIC Timer | Required for scheduling |
| UART | Early boot debugging |
| Framebuffer | Fallback display |

### 10.3 Device Capability System

```rust
// kernel/src/ipc/capability.rs

#[derive(Debug, Clone)]
pub enum DeviceCapability {
    /// Access to MMIO region
    MmioAccess {
        base: PhysAddr,
        size: usize,
        writable: bool,
    },
    
    /// Receive interrupts
    IrqHandler {
        irq: u8,
    },
    
    /// DMA buffer allocation
    DmaAllocate {
        max_size: usize,
    },
    
    /// Port I/O (x86 specific)
    PortIo {
        base: u16,
        count: u16,
    },
}
```

---

## 11. Kernel API Surface

### 11.1 Syscall Table

| Number | Name | Parameters | Returns |
|--------|------|------------|---------|
| 0 | `syscall_exit` | exit_code: i32 | never |
| 1 | `syscall_yield` | - | - |
| 2 | `syscall_spawn` | wasm_ptr, wasm_len, caps_ptr | task_id |
| 3 | `syscall_wait` | task_id | exit_code |
| 10 | `syscall_ipc_create` | capacity | channel_id |
| 11 | `syscall_ipc_send` | channel_id, msg_ptr | result |
| 12 | `syscall_ipc_recv` | channel_id, buf_ptr | msg_len |
| 13 | `syscall_ipc_close` | channel_id | result |
| 20 | `syscall_mem_alloc` | size, flags | virt_addr |
| 21 | `syscall_mem_free` | virt_addr, size | result |
| 22 | `syscall_mem_share` | virt_addr, size | shm_id |
| 23 | `syscall_mem_map` | shm_id, virt_addr | result |
| 30 | `syscall_time_now` | clock_id | timestamp |
| 31 | `syscall_time_sleep` | duration_ns | - |
| 40 | `syscall_log` | level, msg_ptr, msg_len | result |

### 11.2 WASI Integration

The WASI layer translates standard WASI calls to kernel syscalls:

```rust
// runtime/src/wasi/mod.rs

pub fn fd_read(fd: Fd, iovs: &[IoVec]) -> Result<usize, Errno> {
    // Translate to IPC message to filesystem service
    let channel = get_fs_channel()?;
    
    let msg = IpcMessage::new(FsRequest::Read {
        fd: fd.as_raw(),
        len: iovs.iter().map(|v| v.len).sum(),
    });
    
    channel.send(msg)?;
    let response = channel.receive()?;
    
    // Copy data to iovecs
    // ...
}
```

---

## 12. Error Handling Strategy

### 12.1 Error Types

```rust
// kernel/src/error.rs

#[derive(Debug)]
pub enum KernelError {
    /// Memory allocation failed
    OutOfMemory,
    
    /// Invalid virtual address
    InvalidAddress(VirtAddr),
    
    /// Address already mapped
    AlreadyMapped(VirtAddr),
    
    /// Permission denied
    PermissionDenied,
    
    /// Resource not found
    NotFound,
    
    /// Invalid capability
    InvalidCapability,
    
    /// IPC channel closed
    ChannelClosed,
    
    /// Invalid WASM module
    InvalidWasm,
}

impl KernelError {
    pub fn to_errno(&self) -> i32 {
        match self {
            Self::OutOfMemory => ENOMEM,
            Self::InvalidAddress(_) => EFAULT,
            Self::AlreadyMapped(_) => EEXIST,
            Self::PermissionDenied => EPERM,
            Self::NotFound => ENOENT,
            Self::InvalidCapability => EACCES,
            Self::ChannelClosed => EPIPE,
            Self::InvalidWasm => ENOEXEC,
        }
    }
}
```

### 12.2 Panic Handling

```rust
// kernel/src/panic.rs

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable interrupts
    unsafe { cli(); }
    
    // Log to serial
    log::error!("KERNEL PANIC: {}", info);
    
    // Print backtrace if available
    if let Some(backtrace) = backtrace::capture() {
        log::error!("Backtrace:\n{}", backtrace);
    }
    
    // Display on framebuffer
    if let Some(fb) = FRAMEBUFFER.get() {
        fb.draw_panic_screen(info);
    }
    
    // Halt all CPUs
    halt_all_cpus();
}
```

---

## 13. Testing Strategy

### 13.1 Test Levels

| Level | Location | Description |
|-------|----------|-------------|
| Unit | `kernel/src/**/tests.rs` | Individual function tests |
| Integration | `tests/integration/` | Subsystem interaction tests |
| System | `tests/system/` | Full boot tests in QEMU |

### 13.2 Unit Testing

```rust
// kernel/src/memory/pmm.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_buddy_allocate_single() {
        let mut pmm = PhysicalMemoryManager::new_test(1024);
        
        let frame = pmm.allocate(0).expect("allocation failed");
        assert!(frame.is_aligned(PAGE_SIZE));
        
        pmm.free(frame, 0);
        assert_eq!(pmm.stats.free_frames, 1024);
    }
    
    #[test]
    fn test_buddy_coalesce() {
        let mut pmm = PhysicalMemoryManager::new_test(4);
        
        let f1 = pmm.allocate(0).unwrap();
        let f2 = pmm.allocate(0).unwrap();
        
        pmm.free(f1, 0);
        pmm.free(f2, 0);
        
        // Should coalesce to order 1
        let f3 = pmm.allocate(1).unwrap();
        assert!(f3.is_aligned(PAGE_SIZE * 2));
    }
}
```

### 13.3 Integration Testing

```rust
// tests/integration/ipc_test.rs

#[test]
fn test_ipc_ping_pong() {
    let kernel = TestKernel::boot();
    
    let channel = kernel.create_channel(16);
    
    // Spawn echo server
    let server = kernel.spawn_wasm(include_bytes!("echo_server.wasm"), 
        CapabilitySet::new().with_ipc(channel));
    
    // Send message
    kernel.ipc_send(channel, b"hello");
    
    // Receive reply
    let reply = kernel.ipc_recv(channel);
    assert_eq!(&reply, b"hello");
    
    kernel.shutdown();
}
```

### 13.4 System Testing

```rust
// tests/system/boot_test.rs

#[test]
fn test_boot_to_init() {
    let qemu = QemuInstance::new()
        .kernel("target/x86_64-unknown-none/release/kernel")
        .initramfs("tests/initramfs/basic.cpio")
        .timeout(Duration::from_secs(30))
        .spawn();
    
    // Wait for init message
    qemu.expect_serial("init: started");
    
    // Verify basic services
    qemu.expect_serial("compositor: initialized");
    qemu.expect_serial("network: stack ready");
    
    qemu.shutdown();
}
```

---

## Appendix A: Register Conventions

### Syscall ABI (x86_64)

| Register | Purpose |
|----------|---------|
| RAX | Syscall number |
| RDI | Argument 1 |
| RSI | Argument 2 |
| RDX | Argument 3 |
| R10 | Argument 4 |
| R8 | Argument 5 |
| R9 | Argument 6 |
| RAX | Return value |

### Preserved Registers

| Caller-saved | Callee-saved |
|--------------|--------------|
| RAX, RCX, RDX | RBX, RBP, RSP |
| RSI, RDI | R12, R13, R14, R15 |
| R8, R9, R10, R11 | |

---

## Appendix B: Memory Layout Constants

```rust
// kernel/src/memory/constants.rs

pub const PAGE_SIZE: usize = 4096;
pub const HUGE_PAGE_SIZE: usize = 2 * 1024 * 1024; // 2MB

pub const KERNEL_BASE: VirtAddr = VirtAddr::new(0xFFFF_8000_0000_0000);
pub const KERNEL_PHYS_MAP: VirtAddr = VirtAddr::new(0xFFFF_8800_0000_0000);
pub const KERNEL_HEAP_START: VirtAddr = VirtAddr::new(0xFFFF_FF00_0000_0000);
pub const KERNEL_HEAP_END: VirtAddr = VirtAddr::new(0xFFFF_FFFE_FFFF_FFFF);
pub const KERNEL_STACK_START: VirtAddr = VirtAddr::new(0xFFFF_FFFF_0000_0000);

pub const USER_STACK_TOP: VirtAddr = VirtAddr::new(0x0000_7FFF_FFFF_0000);
pub const USER_HEAP_START: VirtAddr = VirtAddr::new(0x0000_0000_4000_0000);
```
