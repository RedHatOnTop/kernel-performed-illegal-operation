# Phase 10: Preemptive Kernel & User-Space Isolation

> **Status**: In Progress (10-1 Complete ✅)  
> **Predecessor**: Phase 9 (Real I/O — VirtIO Driver Completion & Stack Integration) — completed 2026-02-24  
> **Boot environment**: QEMU 10.2.0, UEFI pflash, bootloader 0.11.14, nightly-2026-01-01  
> **Goal**: Enable preemptive multitasking with real context switching, Ring 3 user-space
> isolation, and core process lifecycle syscalls (fork/exec/waitpid/mmap/signal)  
> **Milestone**: *An ELF binary executes in Ring 3, forks a child process, the child
> exec's another ELF, the parent waitpid's for it, and a CPU-bound task does not starve
> other tasks — all verified by automated QEMU test*

---

## Context

Phase 9 delivered fully functional VirtIO network and block I/O, DHCP, real WASI2
HTTP/sockets, and VFS integration. The kernel can now communicate with the outside world
and persist data.

However, the kernel's **execution model** remains fundamentally limited:

1. **No preemptive scheduling** — The scheduler has a 32-level priority queue and
   time-slice tracking, but `context_switch()` is a stub that only updates statistics.
   The APIC timer handler does not invoke the scheduler. A CPU-bound task blocks all
   other tasks indefinitely.

2. **No user-space isolation** — All code runs in Ring 0. The GDT has Ring 3 segments,
   the TSS has `RSP0` support, and `enter_userspace()` assembly exists, but no process
   actually runs in Ring 3.

3. **No process lifecycle** — `fork`, `execve`, `wait4` are not even registered in the
   syscall dispatch table (they fall through to `-ENOSYS`). The `ProcessManager::fork()`
   method returns `Err("Fork not yet implemented")`.

These gaps are **prerequisites** for every higher-level feature: browser tab isolation,
WASM sandboxing, multi-user support, and real-world application hosting.

### Current Assets (What We Already Have)

The kernel already contains significant infrastructure that Phase 10 can build upon:

| Component | File | Status |
|-----------|------|--------|
| GDT with Ring 3 segments (CS=0x23, DS=0x1B) | `kernel/src/gdt.rs` L42-53 | **Ready** |
| TSS with `set_kernel_stack()` | `kernel/src/gdt.rs` L66, L168-178 | **Ready** |
| `enter_userspace()` — iretq to Ring 3 | `kernel/src/process/context.rs` L284-320 | **Ready** |
| `ProcessContext` — 22 register struct (176B) | `kernel/src/process/context.rs` L10-40 | **Ready** |
| `context_switch()` — callee-saved asm | `kernel/src/process/context.rs` L250-271 | **Ready** |
| `SwitchContext` — scheduler asm | `kernel/src/scheduler/context.rs` | **Ready** |
| `switch_context()` — scheduler asm | `kernel/src/scheduler/context.rs` | **Ready** |
| `Scheduler` — 32-level priority queues | `kernel/src/scheduler/mod.rs` L141-170 | **Ready** |
| `timer_tick()` — time-slice countdown | `kernel/src/scheduler/mod.rs` L252-275 | **Ready** |
| APIC timer at ~100Hz periodic | `kernel/src/interrupts/mod.rs` L173-193 | **Ready** |
| `create_user_page_table()` — P4 with kernel half | `kernel/src/memory/user_page_table.rs` L71 | **Ready** |
| `map_user_page/range/unmap/destroy` | `kernel/src/memory/user_page_table.rs` | **Ready** |
| `Process` struct with `page_table_root` (CR3) | `kernel/src/process/table.rs` L164-199 | **Ready** |
| `Thread` struct with `ProcessContext` | `kernel/src/process/table.rs` L144-162 | **Ready** |
| ELF64 loader + segment mapping | `kernel/src/process/manager.rs` | **Ready** |
| `brk` syscall (complete) | `kernel/src/syscall/linux_handlers.rs` L808-890 | **Ready** |
| `mmap` (anonymous only) | `kernel/src/syscall/linux_handlers.rs` L933 | **Partial** |
| `munmap` (complete) | `kernel/src/syscall/linux_handlers.rs` L1034 | **Ready** |
| Linux syscall dispatch table | `kernel/src/syscall/linux.rs` L281-341 | **Ready** |

### Current Gaps

| # | Gap | Location | Impact |
|---|-----|----------|--------|
| 1 | ACPI misaligned pointer dereference — 14 sites | `kernel/src/hw/acpi.rs` L176-390 | Boot panic (debug builds) |
| 2 | `Scheduler::context_switch()` is a stats-only stub | `kernel/src/scheduler/mod.rs` L291-303 | No real task switching |
| 3 | APIC timer handler does not call scheduler | `kernel/src/interrupts/mod.rs` L253-267 | No preemption |
| 4 | `need_reschedule` flag set but never acted upon | `kernel/src/scheduler/mod.rs` L270 | Dead code |
| 5 | No CR3 switch on context switch | — | No address space isolation |
| 6 | `enter_userspace()` never called from scheduler | `kernel/src/process/context.rs` L284 | Ring 3 unused |
| 7 | `SYS_FORK`/`SYS_CLONE`/`SYS_EXECVE`/`SYS_WAIT4` missing from dispatch | `kernel/src/syscall/linux.rs` | Process lifecycle broken |
| 8 | `ProcessManager::fork()` returns error stub | `kernel/src/process/manager.rs` L148-162 | No fork |
| 9 | `mprotect` only updates VMA, not PTE flags | `kernel/src/syscall/linux_handlers.rs` L1118-1150 | Memory protection broken |
| 10 | `rt_sigaction`/`rt_sigprocmask` return 0 stub | `kernel/src/syscall/linux.rs` L314-315 | No signal handling |
| 11 | `futex` returns 0 for all ops | `kernel/src/syscall/linux_handlers.rs` L2294-2317 | No synchronization |
| 12 | VFS `seek(SeekFrom::End)` uses `size = 0` | `storage/src/vfs.rs` L535 | Seek-from-end broken |
| 13 | MMIO path missing `MRG_RXBUF` in feature negotiation | `kernel/src/drivers/net/virtio_net.rs` L590-592 | MMIO net headers misaligned |

---

## Sub-phase 10-1: Stability Fixes (ACPI, VFS, VirtIO MMIO)

### Goal

Resolve all known bugs and technical debt items that could destabilize the kernel during
the more complex work in 10-2 through 10-5.

### Tasks

#### 1. Fix ACPI Misaligned Pointer Dereference (14 sites)

The root cause is `#[repr(C, packed)]` structs being dereferenced via raw pointer casts
like `&*(addr as *const Rsdp)`. On addresses that are not naturally aligned, Rust's debug
builds panic with "misaligned pointer dereference".

**Fix strategy**: Replace all `&*(addr as *const T)` with `ptr::read_unaligned(addr as *const T)`
and work with stack-local copies. For structure field access through pointers, use
`core::ptr::addr_of!((*ptr).field).read_unaligned()`.

All 14 sites to fix:

| Line | Current Code | Fix |
|------|-------------|-----|
| L176 | `&*(rsdp_virt as *const Rsdp)` | `ptr::read_unaligned(rsdp_virt as *const Rsdp)` |
| L212 | `&*(rsdt_virt as *const AcpiTableHeader)` | `ptr::read_unaligned(...)` |
| L221 | `*entries.add(i)` (u32) | `entries.add(i).read_unaligned()` |
| L232 | `&*(xsdt_virt as *const AcpiTableHeader)` | `ptr::read_unaligned(...)` |
| L242 | `*entries.add(i)` (u64) — **primary crash site** | `entries.add(i).read_unaligned()` |
| L252 | `&*(table_virt as *const AcpiTableHeader)` | `ptr::read_unaligned(...)` |
| L340 | `&*(madt_virt as *const AcpiTableHeader)` | `ptr::read_unaligned(...)` |
| L349 | `*local_apic_addr_ptr` (u32) | `local_apic_addr_ptr.read_unaligned()` |
| L352 | `*((addr + 4) as *const u32)` | `((addr + 4) as *const u32).read_unaligned()` |
| L367 | `&*(current as *const MadtEntryHeader)` | `ptr::read_unaligned(...)` |
| L372 | `&*(current as *const MadtLocalApic)` | `ptr::read_unaligned(...)` |
| L381 | `&*(current as *const MadtIoApic)` | `ptr::read_unaligned(...)` |
| L390 | `&*(current as *const MadtInterruptOverride)` | `ptr::read_unaligned(...)` |

After fix, ACPI init should complete without panic and correctly parse MADT to discover
Local APIC and I/O APIC entries.

#### 2. Fix VFS `seek(SeekFrom::End)` Bug

**File**: `storage/src/vfs.rs` L535

**Current code**:
```rust
SeekFrom::End(offset) => {
    // TODO: Get file size
    let size: u64 = 0;  // ← always 0
```

**Fix**: Retrieve the `VfsHandle`'s mount index and filesystem handle, call
`Filesystem::lookup()` or add a `Filesystem::file_size(handle: u64)` method to obtain
the real file size:

```rust
SeekFrom::End(offset) => {
    let size = get_file_size(handle.mount_idx, handle.fs_handle)?;
    if offset < 0 {
        size.checked_sub((-offset) as u64)
            .ok_or(StorageError::InvalidArgument)?
    } else {
        size + offset as u64
    }
}
```

Where `get_file_size()` calls through the mounted filesystem's `stat()` or `lookup()`.

#### 3. Fix VirtIO Net MMIO `MRG_RXBUF` Feature Negotiation

**File**: `kernel/src/drivers/net/virtio_net.rs` L590-592

**Current code** (MMIO path):
```rust
self.features = device_features
    & (features::MAC | features::STATUS | features::CSUM | features::GUEST_CSUM);
```

**Fix**: Add `features::MRG_RXBUF` to match the PIO path:
```rust
self.features = device_features
    & (features::MAC | features::STATUS | features::MRG_RXBUF
       | features::CSUM | features::GUEST_CSUM);
```

Also add `virt_to_phys()` translations to the MMIO queue init path (`init_virtqueue()`
at L762-770) for when MMIO mode is used on real hardware.

### QG (Quality Gate)

- [ ] `cargo build --target x86_64-kpio.json` succeeds
- [ ] QEMU boot completes without ACPI panic — serial shows `[ACPI] MADT parsed: N local APICs, M I/O APICs`
- [ ] `vfs::seek(fd, SeekFrom::End(0))` returns the actual file size for a mounted FAT32 file
- [ ] VirtIO MMIO net path includes `MRG_RXBUF` in feature negotiation
- [ ] All existing qemu-test.ps1 modes (boot, io) still pass

---

## Sub-phase 10-2: Preemptive Scheduling

### Goal

Connect the existing scheduler infrastructure to actually perform context switches,
driven by the APIC timer interrupt. After this sub-phase, a CPU-bound task cannot
starve other tasks.

### Root Cause

Three disconnections prevent preemptive scheduling:

1. **Timer → Scheduler**: `timer_interrupt_handler()` does not call
   `Scheduler::timer_tick()`. The time-slice countdown never runs.

2. **Scheduler → Context Switch**: `Scheduler::context_switch()` is a stub that only
   updates statistics. It does not call `switch_context()` or
   `process::context::context_switch()`.

3. **Context Storage**: `Task` structs do not store a `SwitchContext` for the scheduler
   to save/restore.

### Design

```
APIC Timer IRQ (Vector 32, ~100Hz)
        │
        ▼
timer_interrupt_handler()
        │
        ├─ TIMER_TICKS += 1
        ├─ SCHEDULER.lock().timer_tick()
        │       │
        │       ├─ Wake sleeping tasks
        │       ├─ time_slice_remaining -= 1
        │       └─ if 0 → need_reschedule = true
        │
        ├─ if need_reschedule:
        │       SCHEDULER.lock().schedule()
        │              │
        │              ├─ Save current task context (SwitchContext)
        │              ├─ Select next task from priority queue
        │              ├─ Restore next task context
        │              └─ switch_context(old_ctx_ptr, new_ctx_ptr)
        │                      │
        │                      └─ [ASM] push callee-saved → swap RSP → pop callee-saved → ret
        │
        └─ apic::end_of_interrupt()
```

### Tasks

1. **Add `SwitchContext` field to `Task`** — Each task needs its own saved register state:
   ```rust
   // kernel/src/scheduler/mod.rs
   pub struct Task {
       // ... existing fields ...
       pub switch_ctx: SwitchContext,  // callee-saved registers + rsp + rip
   }
   ```

2. **Implement real `context_switch()`** — Replace the stats-only stub with actual
   register save/restore:
   ```rust
   fn context_switch(&mut self, prev: &mut Task, next: &mut Task) {
       // Switch kernel stacks; switch_context is naked asm
       unsafe {
           crate::scheduler::context::switch_context(
               &mut prev.switch_ctx as *mut SwitchContext,
               &next.switch_ctx as *const SwitchContext,
           );
       }
   }
   ```

3. **Connect timer interrupt to scheduler** — In `timer_interrupt_handler()`, after
   incrementing ticks:
   ```rust
   extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
       let ticks = TIMER_TICKS.fetch_add(1, Ordering::Relaxed);

       // Drive scheduler time-slice
       {
           let mut sched = SCHEDULER.lock();
           sched.timer_tick();
           if sched.needs_reschedule() {
               sched.schedule();
           }
       }

       // Existing timer callback (GUI, etc.)
       if let Some(cb) = *TIMER_CALLBACK.lock() {
           cb();
       }

       apic::end_of_interrupt();
   }
   ```

   **Deadlock prevention**: The scheduler lock must not be held when `switch_context()`
   executes (the switched-to task might try to acquire it). Solution: extract the old/new
   context pointers under the lock, drop the lock, then call `switch_context()` outside
   the lock.

4. **Initialize task contexts with proper entry points** — When creating a new task,
   set `SwitchContext.rip` to the task's entry function and `SwitchContext.rsp` to the
   top of its kernel stack:
   ```rust
   pub fn create_task(entry: fn(), stack: &[u8]) -> Task {
       let stack_top = stack.as_ptr() as u64 + stack.len() as u64;
       Task {
           switch_ctx: SwitchContext {
               rip: entry as u64,
               rsp: stack_top,
               rbp: 0, rbx: 0, r12: 0, r13: 0, r14: 0, r15: 0,
           },
           // ...
       }
   }
   ```

5. **Add per-task kernel stacks** — Each task needs its own kernel stack (at least 16KiB)
   for interrupt handling and context saving. Allocate via `alloc::alloc::alloc()` with
   page alignment.

6. **Preemption guard for critical sections** — Add `preempt_disable()` /
   `preempt_enable()` to protect critical sections where context switching must not occur
   (e.g., while holding spinlocks):
   ```rust
   pub fn preempt_disable() {
       PREEMPT_COUNT.fetch_add(1, Ordering::SeqCst);
   }
   pub fn preempt_enable() {
       if PREEMPT_COUNT.fetch_sub(1, Ordering::SeqCst) == 1 {
           // Was last disable — check if reschedule needed
           if SCHEDULER.lock().needs_reschedule() {
               SCHEDULER.lock().schedule();
           }
       }
   }
   ```

### QG (Quality Gate)

- [x] `cargo build` succeeds
- [x] A test with two tasks (A prints "A", B prints "B") interleaves output on serial
- [x] A CPU-bound infinite loop task does not prevent a second task from running
- [x] Timer tick counter keeps incrementing during task switching
- [x] `CONTEXT_SWITCHES` counter increments (visible via `[TIMER] tick=N ctx_switches=M` log)
- [x] No deadlocks or double panics during scheduling

---

## Sub-phase 10-3: User-Space Isolation (Ring 3)

### Goal

Enable processes to run in Ring 3 with per-process page tables. A user-space page fault
is caught by the kernel (not a triple fault), and syscalls go through `SYSCALL`/`SYSRET`.

### Design

```
┌──────────────────────────────────────────────────┐
│            User Space (Ring 3)                    │
│  Virtual Address Space: 0x0000_0000_0000 -       │
│                         0x0000_7FFF_FFFF_FFFF    │
│  ├── .text (RX)  — ELF code segments             │
│  ├── .data (RW)  — ELF data segments             │
│  ├── heap         — grows up via brk/mmap        │
│  └── stack        — top at 0x7FFF_FFFF_F000      │
├──────────────────────────────────────────────────┤
│  SYSCALL instruction → MSR_LSTAR handler         │
├──────────────────────────────────────────────────┤
│            Kernel Space (Ring 0)                  │
│  Virtual Address Space: 0xFFFF_8000_0000_0000 -   │
│                         0xFFFF_FFFF_FFFF_FFFF    │
│  Shared across all page tables (P4[256..511])     │
└──────────────────────────────────────────────────┘
```

### Tasks

1. **Set up SYSCALL/SYSRET MSRs** — Write the Model-Specific Registers to enable the
   `SYSCALL` instruction:
   ```rust
   // kernel/src/syscall/mod.rs
   pub fn init_syscall_msrs() {
       use x86_64::registers::model_specific::Msr;
       unsafe {
           // STAR: kernel CS/SS in bits 47:32, user CS/SS in bits 63:48
           // Kernel: CS=0x08, SS=0x10 → STAR[47:32] = 0x08
           // User:   CS=0x23, SS=0x1B → STAR[63:48] = 0x1B (SYSRET adds offsets)
           //   SYSRET loads CS from STAR[63:48]+16=0x23, SS from STAR[63:48]+8=0x1B ✓
           let star = (0x001B_u64 << 48) | (0x0008_u64 << 32);
           Msr::new(0xC000_0081).write(star);  // IA32_STAR

           // LSTAR: syscall entry point (kernel handler address)
           Msr::new(0xC000_0082).write(syscall_entry as u64);  // IA32_LSTAR

           // SFMASK: clear IF (bit 9) on syscall entry → disable interrupts
           Msr::new(0xC000_0084).write(0x200);  // IA32_FMASK

           // Enable SCE (System Call Extensions) in IA32_EFER
           let efer = Msr::new(0xC000_0080).read();
           Msr::new(0xC000_0080).write(efer | 1);  // SCE = bit 0
       }
   }
   ```

2. **Implement `syscall_entry` in assembly** — The naked function that the CPU jumps to
   on `SYSCALL`:
   ```rust
   #[naked]
   unsafe extern "C" fn syscall_entry() {
       // On SYSCALL entry:
       //   RCX = user RIP (return address)
       //   R11 = user RFLAGS
       //   RAX = syscall number
       //   RDI, RSI, RDX, R10, R8, R9 = args 1-6
       core::arch::naked_asm!(
           // 1. Switch to kernel stack (from TSS RSP0)
           "swapgs",                    // GS base → kernel per-CPU data
           "mov gs:[0], rsp",           // save user RSP to per-CPU scratch
           "mov rsp, gs:[8]",           // load kernel stack from per-CPU area
           // 2. Save user context
           "push rcx",                  // user RIP
           "push r11",                  // user RFLAGS
           "push gs:[0]",              // user RSP
           // 3. Save callee-saved registers
           "push rbp",
           "push rbx",
           "push r12",
           "push r13",
           "push r14",
           "push r15",
           // 4. Re-enable interrupts, call Rust handler
           "sti",
           "mov rcx, r10",             // Linux convention: arg4 in R10, SysV wants RCX
           "call syscall_dispatch",
           // 5. Restore and return to user
           "cli",
           "pop r15",
           "pop r14",
           "pop r13",
           "pop r12",
           "pop rbp",
           "pop rsp",                  // user RSP
           "pop r11",                  // user RFLAGS
           "pop rcx",                  // user RIP
           "swapgs",                   // restore user GS
           "sysretq",                  // return to Ring 3
       );
   }
   ```

3. **Allocate per-CPU data for kernel stack pointer** — The `SWAPGS` pattern requires
   a per-CPU area (usually via `GS` base MSR) storing:
   - Offset 0: scratch space for user RSP
   - Offset 8: kernel stack top (RSP0)
   ```rust
   #[repr(C)]
   struct PerCpuData {
       user_rsp_scratch: u64,
       kernel_stack_top: u64,
   }
   ```
   Set `IA32_KERNEL_GS_BASE` (MSR 0xC000_0102) to point to this struct.

4. **CR3 switching on context switch** — When switching between processes with different
   page tables:
   ```rust
   fn switch_address_space(new_cr3: u64) {
       unsafe {
           core::arch::asm!("mov cr3, {}", in(reg) new_cr3);
       }
   }
   ```
   In the scheduler's `context_switch()`, before swapping register state:
   ```rust
   if prev.page_table_root != next.page_table_root {
       switch_address_space(next.page_table_root);
   }
   // Also update TSS RSP0 to next task's kernel stack
   gdt::set_kernel_stack(VirtAddr::new(next.kernel_stack_top));
   ```

5. **First user-space process launch** — Create a minimal process that:
   - Gets a new page table via `create_user_page_table()`
   - Maps ELF segments into user space
   - Allocates a user stack at `0x7FFF_FFFF_F000` (top, growing down)
   - Fills `ProcessContext` with entry point, user stack, CS=0x23, DS=0x1B, RFLAGS=0x202
   - Calls `enter_userspace()` via `iretq`

6. **User-space page fault handler** — Update the existing page fault handler to
   distinguish user vs kernel faults:
   ```rust
   extern "x86-interrupt" fn page_fault_handler(
       stack_frame: InterruptStackFrame,
       error_code: PageFaultErrorCode,
   ) {
       let fault_addr = Cr2::read();
       if error_code.contains(PageFaultErrorCode::USER_MODE) {
           // User-space fault → kill process, don't panic kernel
           serial_println!("[FAULT] Process {} page fault at {:#x}", current_pid(), fault_addr);
           process::kill_current(SIGSEGV);
       } else {
           // Kernel fault → existing panic behavior
           panic!("Kernel page fault at {:#x}", fault_addr);
       }
   }
   ```

### QG (Quality Gate)

- [ ] `cargo build` succeeds
- [ ] `SYSCALL`/`SYSRET` round-trip works — a user-mode `syscall` enters kernel and returns
- [ ] An ELF binary loaded at user addresses runs in Ring 3 (CS & 3 == 3 visible in fault logs)
- [ ] User-space page fault is caught gracefully — kernel logs error and kills process (no triple fault)
- [ ] Two processes with different page tables can run concurrently
- [ ] TSS RSP0 is updated on every context switch
- [ ] Kernel memory (upper half) is not accessible from Ring 3 (page fault on access)

---

## Sub-phase 10-4: Core Process Syscalls

### Goal

Implement the essential process lifecycle syscalls: `fork`, `execve`, `wait4`/`waitpid`,
`mprotect` (real PTE update), basic signal handling, and `futex` (WAIT/WAKE).

### Tasks

#### 1. Register Missing Syscalls in Dispatch Table

Add the following to `kernel/src/syscall/linux.rs` `linux_syscall_dispatch()`:

```rust
const SYS_CLONE: u64 = 56;
const SYS_FORK: u64 = 57;
const SYS_VFORK: u64 = 58;
const SYS_EXECVE: u64 = 59;
const SYS_EXIT: u64 = 60;     // may already exist
const SYS_WAIT4: u64 = 61;
const SYS_KILL: u64 = 62;
```

#### 2. Implement `fork()` (SYS_FORK = 57)

Fork creates a child process that is a copy of the parent. The child's address space
uses Copy-on-Write (CoW) to avoid immediately duplicating all pages.

```rust
pub fn sys_fork(parent_ctx: &ProcessContext) -> i64 {
    // 1. Allocate new PID
    let child_pid = PROCESS_TABLE.alloc_pid();

    // 2. Create new page table (CoW copy of parent)
    let parent_cr3 = current_process().page_table_root;
    let child_cr3 = cow_clone_page_table(parent_cr3);

    // 3. Copy parent's ProcessContext → child (RIP, RSP, registers)
    //    Set child RAX = 0 (fork return value for child)
    let mut child_ctx = parent_ctx.clone();
    child_ctx.rax = 0;

    // 4. Copy file descriptors
    let child_fds = current_process().file_descriptors.clone();

    // 5. Create child Process + Thread
    let child = Process {
        pid: child_pid,
        parent: current_pid(),
        page_table_root: child_cr3,
        // ... copy other fields
    };

    // 6. Add child to scheduler as Ready
    SCHEDULER.lock().add_task(child_task);

    // 7. Return child PID to parent
    child_pid.0 as i64
}
```

**CoW page table clone** (`cow_clone_page_table`):
- Walk parent P4 entries [0..255] (user half)
- For each present PTE at leaf level: clear WRITABLE bit in **both** parent and child,
  set a custom CoW flag (e.g., available bit 9), increment a refcount for the physical frame
- On subsequent write fault: allocate new frame, copy 4KiB, map writable in faulting
  process, decrement refcount on original frame

#### 3. Implement `execve()` (SYS_EXECVE = 59)

Replaces the current process's address space with a new ELF binary:

```rust
pub fn sys_execve(path: *const u8, argv: *const *const u8, envp: *const *const u8) -> i64 {
    // 1. Read path string from user memory (validate pointer!)
    let path_str = read_user_string(path);

    // 2. Open and read ELF file from VFS
    let elf_data = vfs::read_file(&path_str)?;

    // 3. Destroy old user mappings (keep kernel half)
    destroy_user_mappings(current_cr3());

    // 4. Parse ELF, map segments into fresh address space
    let entry_point = load_elf_segments(current_cr3(), &elf_data)?;

    // 5. Set up new user stack with argv/envp
    let stack_top = setup_user_stack(current_cr3(), argv, envp)?;

    // 6. Reset signal handlers to default
    reset_signal_handlers();

    // 7. Jump to new entry point (modify return context)
    set_user_entry(entry_point, stack_top);
    0 // does not return to caller
}
```

#### 4. Implement `wait4()`/`waitpid()` (SYS_WAIT4 = 61)

```rust
pub fn sys_wait4(pid: i64, wstatus: *mut i32, options: i32, _rusage: u64) -> i64 {
    loop {
        // Check for zombie children
        if let Some(child) = find_zombie_child(current_pid(), pid) {
            let exit_code = child.exit_code.unwrap_or(0);
            if !wstatus.is_null() {
                write_user_memory(wstatus, (exit_code << 8) as i32);  // WEXITSTATUS encoding
            }
            reap_process(child.pid);
            return child.pid.0 as i64;
        }

        if options & WNOHANG != 0 {
            return 0;  // No zombie yet, don't block
        }

        // Block current process until a child exits
        block_current_on_child_exit();
        schedule();
    }
}
```

#### 5. Fix `mprotect()` — Real PTE Updates

**File**: `kernel/src/syscall/linux_handlers.rs` L1118-1150

Current implementation only updates VMA metadata but does not modify actual page table
entries. Fix by walking the page table and setting/clearing PTE flags:

```rust
pub fn sys_mprotect(addr: u64, len: u64, prot: u64) -> i64 {
    let cr3 = current_process().page_table_root;
    let flags = prot_to_page_flags(prot);  // PROT_READ|PROT_WRITE|PROT_EXEC → PTE flags

    for page_addr in (addr..addr + len).step_by(4096) {
        if let Some(pte) = get_pte_mut(cr3, page_addr) {
            // Clear existing permission bits, set new ones
            pte.set_flags(flags);
        }
    }

    // Also update VMA metadata (existing code)
    update_vma_protection(addr, len, prot);

    // Flush TLB for modified range
    for page_addr in (addr..addr + len).step_by(4096) {
        unsafe { core::arch::asm!("invlpg [{}]", in(reg) page_addr); }
    }

    0
}
```

#### 6. Basic Signal Infrastructure

Implement a minimal signal subsystem supporting `SIGTERM`, `SIGKILL`, `SIGCHLD`,
`SIGSEGV`, and `SIGINT`:

```rust
// kernel/src/process/signal.rs (new file)

pub const SIGKILL: u8 = 9;
pub const SIGSEGV: u8 = 11;
pub const SIGTERM: u8 = 15;
pub const SIGCHLD: u8 = 17;
pub const SIGINT: u8 = 2;

pub struct SignalState {
    pub pending: u64,                    // bitmask of pending signals
    pub blocked: u64,                    // bitmask of blocked signals (sigprocmask)
    pub handlers: [SignalAction; 64],    // per-signal disposition
}

pub enum SignalAction {
    Default,                    // SIG_DFL — terminate, ignore, etc.
    Ignore,                     // SIG_IGN
    Handler(u64),               // user-space handler address
}
```

Replace `rt_sigaction` and `rt_sigprocmask` stubs with real implementations:

```rust
// SYS_RT_SIGACTION (13)
pub fn sys_rt_sigaction(signum: u64, act: *const SigAction, oldact: *mut SigAction) -> i64 {
    if signum == SIGKILL as u64 || signum == SIGSTOP as u64 {
        return -EINVAL;  // Cannot catch or ignore SIGKILL/SIGSTOP
    }
    let process = current_process_mut();
    if !oldact.is_null() {
        // Write current handler to oldact
        write_user_sigaction(oldact, &process.signals.handlers[signum as usize]);
    }
    if !act.is_null() {
        // Read new handler from act
        process.signals.handlers[signum as usize] = read_user_sigaction(act);
    }
    0
}

// SYS_RT_SIGPROCMASK (14)
pub fn sys_rt_sigprocmask(how: i32, set: *const u64, oldset: *mut u64) -> i64 {
    let process = current_process_mut();
    if !oldset.is_null() {
        write_user_u64(oldset, process.signals.blocked);
    }
    if !set.is_null() {
        let mask = read_user_u64(set);
        match how {
            SIG_BLOCK   => process.signals.blocked |= mask,
            SIG_UNBLOCK => process.signals.blocked &= !mask,
            SIG_SETMASK => process.signals.blocked = mask,
            _ => return -EINVAL,
        }
        // SIGKILL and SIGSTOP can never be blocked
        process.signals.blocked &= !(1 << SIGKILL) & !(1 << SIGSTOP);
    }
    0
}
```

Signal delivery: check pending signals on return to user-space (in `syscall_entry`'s
return path or in `schedule()` before `sysretq`).

#### 7. Basic `futex()` — WAIT and WAKE

Replace the stub with a real wait queue implementation:

```rust
// kernel/src/sync/futex.rs (new file)

static FUTEX_TABLE: Mutex<BTreeMap<u64, VecDeque<TaskId>>> = ...;

pub fn sys_futex(uaddr: u64, op: i32, val: u32) -> i64 {
    let cmd = op & FUTEX_CMD_MASK;
    match cmd {
        FUTEX_WAIT => {
            // Atomically check *uaddr == val, if so block
            let phys_addr = virt_to_phys_user(uaddr);
            let current_val = read_user_u32(uaddr);
            if current_val != val {
                return -EAGAIN;
            }
            // Add current task to wait queue keyed by physical address
            FUTEX_TABLE.lock().entry(phys_addr).or_default().push_back(current_task_id());
            block_current_task();
            schedule();
            0
        }
        FUTEX_WAKE => {
            let phys_addr = virt_to_phys_user(uaddr);
            let mut table = FUTEX_TABLE.lock();
            let mut woken = 0u32;
            if let Some(waiters) = table.get_mut(&phys_addr) {
                while woken < val && !waiters.is_empty() {
                    if let Some(tid) = waiters.pop_front() {
                        wake_task(tid);
                        woken += 1;
                    }
                }
            }
            woken as i64
        }
        _ => -ENOSYS,
    }
}
```

### QG (Quality Gate)

- [ ] `cargo build` succeeds
- [ ] `fork()` creates a child process that returns 0, parent gets child PID
- [ ] `execve()` replaces process image — child runs a different ELF binary
- [ ] `wait4(child_pid, &status, 0, NULL)` blocks until child exits, returns exit code
- [ ] `fork() → execve() → waitpid()` cycle completes without crash
- [ ] `mprotect(addr, 4096, PROT_READ)` followed by a write to `addr` triggers SIGSEGV
- [ ] `rt_sigaction(SIGUSR1, handler)` installs handler; `kill(pid, SIGUSR1)` invokes it
- [ ] `futex(FUTEX_WAIT)` blocks, `futex(FUTEX_WAKE)` unblocks the waiter
- [ ] SIGKILL and SIGSTOP cannot be caught or ignored

---

## Sub-phase 10-5: Integration & Validation

### Goal

Create automated tests validating the full preemptive + user-space + process lifecycle
path, and document the changes.

### Tasks

1. **Create test ELF binaries** — Minimal static-linked ELF64 programs:

   - `tests/e2e/userspace/hello.S` — Writes "Hello from Ring 3\n" via `write(1, ...)`,
     then `exit(0)`:
     ```asm
     .global _start
     _start:
         mov rax, 1          ; SYS_WRITE
         mov rdi, 1          ; stdout
         lea rsi, [rip+msg]  ; buffer
         mov rdx, 18         ; length
         syscall
         mov rax, 60         ; SYS_EXIT
         xor rdi, rdi        ; exit code 0
         syscall
     msg: .ascii "Hello from Ring 3\n"
     ```

   - `tests/e2e/userspace/fork_test.S` — Forks, child prints "child", parent waitpid's
     and prints "parent done":
     ```asm
     _start:
         mov rax, 57         ; SYS_FORK
         syscall
         test rax, rax
         jz .child
         ; parent: waitpid(child, &status, 0)
         mov rdi, rax
         lea rsi, [rsp-8]
         xor rdx, rdx
         xor r10, r10
         mov rax, 61         ; SYS_WAIT4
         syscall
         ; write "parent done\n"
         ...
         mov rax, 60
         xor rdi, rdi
         syscall
     .child:
         ; write "child\n"
         ...
         mov rax, 60
         mov rdi, 42         ; exit code 42
         syscall
     ```

   - `tests/e2e/userspace/spin.S` — Infinite loop (tests preemption):
     ```asm
     _start:
         jmp _start
     ```

2. **Embed test ELFs in kernel or test disk** — Either link them as `include_bytes!()` or
   place them on the FAT32 test disk image.

3. **Boot-time process tests** — Add to `kernel/src/main.rs` after Phase 10 init:
   ```rust
   // Phase 10-5: Process lifecycle self-test
   serial_println!("[PROC] Starting Ring 3 self-test...");

   // Test 1: Launch hello.elf in Ring 3
   let pid1 = process::spawn_elf("/mnt/test/hello.elf")?;
   let status1 = process::waitpid(pid1)?;
   assert_eq!(status1, 0, "hello.elf should exit with 0");
   serial_println!("[PROC] Test 1 PASSED: Ring 3 hello");

   // Test 2: fork + waitpid
   let pid2 = process::spawn_elf("/mnt/test/fork_test.elf")?;
   let status2 = process::waitpid(pid2)?;
   assert_eq!(status2, 0, "fork_test should exit with 0");
   serial_println!("[PROC] Test 2 PASSED: fork + waitpid");

   // Test 3: Preemption — spin.elf should not block kernel
   let spin_pid = process::spawn_elf("/mnt/test/spin.elf")?;
   // Wait 100ms, then kill it
   sleep_ms(100);
   process::kill(spin_pid, SIGKILL);
   serial_println!("[PROC] Test 3 PASSED: preemption works");

   serial_println!("[PROC] All process tests PASSED");
   ```

4. **Extend `qemu-test.ps1` with `process` mode** — Add new check patterns:
   ```powershell
   $ProcessChecks = $SmokeChecks + @(
       @{ Pattern = "ACPI.*MADT parsed";              Label = "ACPI fix" },
       @{ Pattern = "PROC.*Ring 3 hello";              Label = "Ring 3 exec" },
       @{ Pattern = "PROC.*fork \+ waitpid";          Label = "fork/waitpid" },
       @{ Pattern = "PROC.*preemption works";          Label = "Preemption" },
       @{ Pattern = "PROC.*All process tests PASSED";  Label = "Process E2E" },
       @{ Pattern = "SCHED.*context switches";         Label = "Context switches"; IsRegex = $true },
   )
   ```

5. **Update documentation**:
   - `docs/known-issues.md` — Mark ACPI issue as RESOLVED
   - `docs/roadmap.md` — Add Phase 10 entry
   - `docs/architecture/kernel.md` — Document preemptive scheduler design
   - `RELEASE_NOTES.md` — Add preemptive scheduling + Ring 3 section

### QG (Quality Gate)

- [ ] `.\scripts\qemu-test.ps1 -Mode process` passes all checks
- [ ] All check patterns found in serial output
- [ ] Test completes in under 60 seconds
- [ ] No panics, triple faults, or deadlocks during test run
- [ ] `qemu-test.ps1 -Mode boot` and `-Mode io` still pass (regression-free)

---

## Execution Order & Dependencies

```
10-1 (ACPI + VFS + MMIO fixes) ──→ 10-2 (Preemptive Scheduling)
                                            │
                                            ▼
                                    10-3 (Ring 3 Isolation)
                                            │
                                            ▼
                                    10-4 (Process Syscalls)
                                            │
                                            ▼
                                    10-5 (Integration Test)
```

- **10-1 → 10-2**: ACPI fix ensures stable boot before adding context switching complexity
- **10-2 → 10-3**: Preemptive scheduling must work (kernel tasks) before adding Ring 3
- **10-3 → 10-4**: Ring 3 + SYSCALL/SYSRET must work before implementing fork/exec
- **10-4 → 10-5**: All syscalls must be implemented before integration testing

---

## Commit Plan

| Sub-phase | Commit message |
|-----------|---------------|
| 10-1 | `fix(kernel): resolve ACPI misaligned pointer, VFS seek, MMIO MRG_RXBUF` |
| 10-2 | `feat(scheduler): implement preemptive context switching via APIC timer` |
| 10-3 | `feat(kernel): Ring 3 user-space isolation with SYSCALL/SYSRET and CR3 switch` |
| 10-4 | `feat(syscall): implement fork, execve, wait4, mprotect, signals, futex` |
| 10-5 | `test(e2e): add QEMU process lifecycle integration test mode` |

---

## Key Design Decisions

### 1. SYSCALL/SYSRET vs INT 0x80

**Decision**: SYSCALL/SYSRET (64-bit fast path)

**Rationale**: The GDT already has the SYSRET-compatible segment layout (kernel CS at
0x08, user DS at 0x1B, user CS at 0x23). SYSCALL/SYSRET is ~5x faster than `INT`-based
syscalls and is the standard on x86-64 Linux. We keep `INT 0x80` as a fallback for
32-bit compatibility if needed later.

### 2. CoW Strategy for `fork()`

**Decision**: Lazy CoW with reference-counted physical frames

**Rationale**: Immediate copy is O(n) in process memory size, making `fork()` of a
large process extremely slow. CoW makes `fork()` O(page table entries) and only copies
pages on first write. This is the industry-standard approach (Linux, FreeBSD, etc.).

**Implementation**: Use PTE available bit 9 as the CoW flag. Maintain a global
`FRAME_REFCOUNT: BTreeMap<PhysFrame, u32>` to track shared frames. On write fault to a
CoW page: if refcount > 1, copy to new frame, decrement old refcount; if refcount == 1,
just mark writable.

### 3. Per-CPU Data for SWAPGS

**Decision**: Use `IA32_KERNEL_GS_BASE` with a small per-CPU struct

**Rationale**: The `SWAPGS` instruction is required for safe kernel entry on SYSCALL
(need to find kernel stack without corrupting user registers). A per-CPU area indexed
by GS is the standard x86-64 technique. Since KPIO currently runs on a single CPU,
one instance suffices, but the design scales to SMP.

### 4. Signal Delivery Model

**Decision**: Check signals on return-to-userspace path

**Rationale**: Linux checks signals at `exit_to_usermode_loop()` which runs on the
return path from every syscall and every interrupt that returns to user mode. This is
the simplest correct approach — it avoids complex in-kernel preemption for signal delivery.

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Triple fault on SYSCALL entry (wrong MSR config) | Medium | Critical | Test with minimal `syscall; ret` first; verify STAR/LSTAR/FMASK values |
| Deadlock in scheduler lock during context switch | High | Critical | Drop lock before `switch_context()`; use preempt_disable in critical sections |
| CoW page fault handler race condition | Medium | High | Hold per-process lock during CoW resolution; frame refcount ops are atomic |
| Stack overflow in kernel task stacks | Medium | High | Allocate 16KiB+ stacks; guard page below each stack |
| `fork()` exhausts physical memory (many CoW pages) | Low | Medium | Track frame refcounts; defer fork if memory pressure high |
| SYSRET with non-canonical RCX → GPF | Low | Critical | Validate RCX (user RIP) before SYSRET; fall back to IRETQ if suspicious |
| Timer interrupt during context switch | Medium | High | CLI/STI around switch; disable preemption with PREEMPT_COUNT |

---

## Reference Materials

- [AMD64 Architecture Programmer's Manual, Vol. 2](https://www.amd.com/content/dam/amd/en/documents/processor-tech-docs/programmer-references/24593.pdf) — §6.1 (SYSCALL/SYSRET), §5 (Page Translation)
- [Intel SDM Vol. 3, Chapter 5](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) — Protection, SYSENTER/SYSEXIT, Task State Segment
- [OSDev Wiki — SYSCALL/SYSRET](https://wiki.osdev.org/SYSENTER) — MSR setup, segment requirements
- [OSDev Wiki — Context Switching](https://wiki.osdev.org/Context_Switching) — practical implementation patterns
- [Linux kernel — entry_64.S](https://github.com/torvalds/linux/blob/master/arch/x86/entry/entry_64.S) — reference SYSCALL entry implementation
- [Linux kernel — futex.c](https://github.com/torvalds/linux/blob/master/kernel/futex/core.c) — futex hash table design
- [Writing an OS in Rust — Async/Await (Philipp Oppermann)](https://os.phil-opp.com/) — cooperative scheduler reference
- Existing implementation reference: `kernel/src/gdt.rs` (GDT/TSS), `kernel/src/process/context.rs` (context switch asm), `kernel/src/memory/user_page_table.rs` (page table mgmt)

---

## Expected Outcomes

After Phase 10 completion:

1. **ACPI works** — MADT parsed without misaligned pointer panic; I/O APIC and Local APIC
   entries discovered
2. **Preemptive multitasking** — APIC timer drives real context switches; CPU-bound tasks
   cannot starve others; `CONTEXT_SWITCHES` counter increments
3. **Ring 3 isolation** — Processes run in user mode with per-process page tables; kernel
   memory inaccessible from user space; page faults handled gracefully
4. **Process lifecycle** — `fork()`, `execve()`, `waitpid()` work end-to-end; parent can
   spawn child, child can exec new binary, parent can wait for exit
5. **Memory protection** — `mprotect()` actually changes PTE flags; write to read-only
   page triggers SIGSEGV
6. **Basic signals** — SIGKILL terminates process; custom signal handlers can be installed
7. **Futex synchronization** — `FUTEX_WAIT` blocks, `FUTEX_WAKE` unblocks
8. **Automated testing** — `qemu-test.ps1 -Mode process` validates the entire path
