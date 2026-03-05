# Phase 11: Kernel Hardening — Implementation Plan

Resolve the documented technical debts (CoW fork, ISR deadlock, stack overflow protection) left by Phase 10, strengthening the kernel's reliability before any new features are added.

## Proposed Changes

---

### 11-1: Copy-on-Write Fork ✅ COMPLETE

- **Goal**: `fork()` shares user-space data frames instead of eagerly copying them; writes trigger CoW page faults.
- **Tasks**: New `memory/refcount.rs`, modify `user_page_table.rs` and `interrupts/mod.rs`.
- **Quality Gate**: QEMU serial log shows `[CoW] fork shared N pages` and `[CoW] fault handled`, `process` mode tests still pass.

#### Implementation Summary

- `kernel/src/memory/refcount.rs` — Global frame reference counter using `BTreeMap<u64, u32>`.
  - `increment(phys)` — bump refcount (default implicit = 1).
  - `decrement(phys) -> u32` — decrement and return new count. Caller frees when 0.
  - `get(phys) -> u32` — query current refcount.
- `kernel/src/memory/user_page_table.rs` — `COW_BIT` constant (bit 9), `clone_user_page_table()` rewritten for CoW sharing, `read_pte()` and `handle_cow_fault()` added, `destroy_user_page_table()` / `destroy_user_mappings()` use refcount-aware deallocation.
- `kernel/src/interrupts/mod.rs` — Page fault handler checks `PROTECTION_VIOLATION | CAUSED_BY_WRITE | USER_MODE` + `COW_BIT`, delegates to `handle_cow_fault()`.

---

### 11-2: Bottom-Half / Deferred Work Queue ✅ COMPLETE

- **Goal**: ISR deadlock eliminated; keyboard/mouse/timer events dispatched outside interrupt context via lock-free work queue.
- **Tasks**: New `interrupts/workqueue.rs`, modify `interrupts/mod.rs` and `main.rs`.
- **Quality Gate**: QEMU serial log shows `[WorkQueue] drained N items`. Timer callback deadlock (known issue #6) is resolved — boot animation proceeds without hangs.

#### Implementation Summary

- `kernel/src/interrupts/workqueue.rs` — Lock-free 256-entry ring buffer with `AtomicUsize` head/tail. `WorkItem` variants: `TimerTick`, `KeyEvent(...)`, `MouseByte(u8)`. ISR calls `push()`, main loop calls `drain()`.
- `kernel/src/interrupts/mod.rs` — Keyboard, timer, mouse callbacks replaced with `AtomicPtr`-based lock-free function pointers. ISRs push work items instead of calling callbacks inline.
- `kernel/src/main.rs` — Main loop calls `interrupts::workqueue::drain()` each iteration.

---

### 11-3: Stack Guard Pages ✅ COMPLETE

- **Goal**: Kernel stack overflows cause a clean page fault panic with task name instead of silent heap corruption.
- **Tasks**: Modify `scheduler/task.rs`.
- **Quality Gate**: QEMU serial log shows `[STACK] guard page at 0x...` for each spawned task. No triple-faults during normal boot.

#### Implementation Summary

- `kernel/src/scheduler/task.rs` — Kernel stacks allocated from raw physical frames (4 pages = 16 KiB) via `allocate_frame()`. Mapped at `0xFFFF_C000_0000_0000` (P4 index 384) with a guard page (1 unmapped page below stack). `is_stack_guard_page()` checks guard region. Page fault handler detects guard page hits and panics with clear message.

---

### 11-4: QEMU Integration Test ✅ COMPLETE

- **Goal**: Automated test verifies all Phase 11 features work and Phase 10 process tests don't regress.
- **Tasks**: Modify `qemu-test.ps1` and `main.rs`.
- **Quality Gate**: `.\scripts\qemu-test.ps1 -Mode hardening` passes ALL checks with 0 failures.

#### Implementation Summary

- `kernel/src/main.rs` — Added Phase 11 hardening test block: creates parent/child page tables with CoW fork, spawns cow-writer process to trigger CoW fault, logs refcount verification and guard page addresses.
- `scripts/qemu-test.ps1` — Added `-Mode hardening` with checks:

| Check | Pattern |
|-------|---------|
| CoW fork sharing | `CoW.*fork shared.*pages` (regex) |
| CoW fault handled | `CoW.*fault handled` (regex) |
| Work queue drain | `WorkQueue.*drained` (regex) |
| Stack guard mapped | `STACK.*guard page` (regex) |
| Process tests pass | `PROC.*All process tests PASSED` (regex) |
| No panics | absence of `panicked at` |

---

## Completion Protocol (per AGENTS.md §4)

After each sub-phase:
1. ✅ Quality Gate verified via QEMU serial log
2. ✅ `docs/roadmap.md`, `RELEASE_NOTES.md`, `docs/known-issues.md` updated
3. ✅ Changes committed with descriptive English commit message
4. ✅ Sub-phase marked complete

## Verification Plan

### Automated Tests

```powershell
.\scripts\qemu-test.ps1 -Mode hardening -Verbose
```

### Regression Tests

```powershell
.\scripts\qemu-test.ps1 -Mode process -Verbose   # Phase 10
.\scripts\qemu-test.ps1 -Mode boot                # Basic boot
```

### Build Verification

```powershell
cargo build -p kpio-kernel
```
