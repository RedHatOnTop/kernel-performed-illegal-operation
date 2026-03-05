# Known Issues

This document tracks known issues, limitations, and workarounds for KPIO OS.

---

## 1. BIOS Boot — FAT Parser Integer Overflow

| Field       | Detail                                                                 |
|-------------|------------------------------------------------------------------------|
| Severity    | Medium                                                                 |
| Component   | `bootloader` crate v0.11.14 (external dependency)                     |
| Affects     | BIOS boot path only; UEFI boot is **not** affected                    |
| Status      | **Won't fix** — external crate bug; UEFI pflash is the recommended workaround |
| Discovered  | Phase 7-4 QEMU boot verification (commit `1efe2d8`)                  |

### Symptom

When booting via BIOS in **debug** mode, the bootloader panics during FAT filesystem parsing:

```
panicked at 'attempt to multiply with overflow'
bootloader-x86_64-bios-0.11.14\...\fat.rs
```

### Root Cause

The `bootloader` crate v0.11.14 contains an arithmetic overflow in its internal FAT
filesystem parser. In debug builds, Rust's overflow checks detect the bug and panic.
In release builds, overflow checks are disabled so the multiplication silently wraps;
this may allow booting to proceed but can cause subtle data corruption.

Because this code lives inside the external `bootloader` crate, it cannot be patched in
the KPIO kernel source tree.

### Upstream Reference

- Crate: [`bootloader` on crates.io](https://crates.io/crates/bootloader) (v0.11.14)
- The `bootloader` 0.11.x series is in maintenance mode. The successor
  [`bootloader` v0.12+](https://github.com/rust-osdev/bootloader) has a rewritten boot
  flow that may resolve this issue.

### Workaround: Use UEFI pflash Boot

UEFI pflash boot bypasses the BIOS FAT parser entirely and is the **recommended** boot
method for KPIO OS. All QEMU run scripts default to UEFI mode.

```powershell
# Default (UEFI pflash) — recommended
.\scripts\run-qemu.ps1

# Explicitly avoid BIOS mode
.\scripts\run-qemu.ps1          # do NOT pass -Bios

# Automated testing — always uses UEFI pflash
.\scripts\qemu-test.ps1 -Mode boot
```

If you must test BIOS boot, use a **release** build to skip Rust's overflow checks:

```powershell
.\scripts\run-qemu.ps1 -Bios    # will likely fail in debug mode
```

> **Note:** Even with a release build, BIOS boot may exhibit other issues due to the
> wrapped arithmetic. UEFI pflash is strongly preferred.

---

## 2. ~~VirtIO Network — PIO Mode Not Fully Implemented~~ ✅ RESOLVED

| Field       | Detail                                                                 |
|-------------|------------------------------------------------------------------------|
| Severity    | ~~Low~~ → Resolved                                                    |
| Component   | `kernel/src/drivers/net/virtio_net.rs`                                 |
| Status      | **Fixed** in Phase 9-1 (2026-02-23)                                   |

### Resolution

Phase 9-1 implemented real PIO register access using `x86_64::instructions::port::Port`.
The VirtIO net driver now supports the full legacy PCI init sequence: device reset,
feature negotiation, MAC address read, virtqueue allocation (descriptor table, available
ring, used ring), and DRIVER_OK. `probe()` enables PCI bus mastering and calls
`init_pio()` for each discovered NIC.

---

## 3. ~~DHCP Timeout Delay on Boot~~ ✅ RESOLVED

| Field       | Detail                                                         |
|-------------|----------------------------------------------------------------|
| Severity    | ~~Low~~ → Resolved                                             |
| Component   | `kernel/src/net/dhcp.rs`                                       |
| Status      | **Fixed** in Phase 9-2 (2026-02-24)                            |

### Resolution

Phase 9-2 wired the VirtIO NIC into `NETWORK_MANAGER`, implemented `virt_to_phys()` for
correct DMA address translation, fixed the virtqueue size to match the device's read-only
`QUEUE_SIZE` register (256 entries), and negotiated the `MRG_RXBUF` feature for correct
12-byte VirtIO net headers. DHCP now completes on the first attempt in under 1 second:

```
[DHCP] Lease acquired: 10.0.2.15 (gw 10.0.2.2, dns 10.0.2.3, mask 255.255.255.0, lease=86400s)
[VirtIO Net] TX: 2 packets (684 bytes), RX: 2 packets (1180 bytes)
```

---

## 4. ~~Phase 9-3 Blocker — VirtIO-Blk Read Timeout During FAT Mount~~ ✅ RESOLVED

| Field       | Detail                                                                 |
|-------------|------------------------------------------------------------------------|
| Severity    | ~~High~~ → Resolved                                                     |
| Component   | `kernel/src/driver/virtio/block.rs` + storage mount path              |
| Status      | **Fixed** in Phase 9-3 DMA fix (2026-02-24)                            |

### Resolution

The VirtIO block driver was using **virtual** addresses where **physical** DMA
addresses are required. Three fixes applied (mirroring the 9-2 net driver fix):

1. Queue memory allocated with page-aligned `Layout` (`alloc_zeroed` with 4096 alignment)
2. Queue PFN written to `QUEUE_ADDRESS` register using `virt_to_phys()` translation
3. All descriptor buffer addresses (header, data, status) translated via `virt_to_phys()`

Boot now shows:
```
[VirtIO-Blk] Queue mem virt=0x444444447000 phys=0xa000 pfn=0xa
[VFS] Mounted FAT filesystem on virtio-blk0 at /mnt/test
[VFS] Self-test: read 36 bytes from HELLO.TXT
[VFS] readdir /mnt/test/: 1 entries
  - HELLO.TXT (Regular)
[VFS] Self-test: NOFILE.TXT correctly not found
```

---

## 5. ~~ACPI Misaligned Pointer Panic~~ ✅ RESOLVED

| Field       | Detail                                                                 |
|-------------|------------------------------------------------------------------------|
| Severity    | ~~Low~~ → Resolved                                                    |
| Component   | `kernel/src/hw/acpi.rs`                                                |
| Status      | **Fixed** in Phase 10-1 (2026-02-25)                                   |

### Resolution

All 14 sites in `kernel/src/hw/acpi.rs` that used `&*(addr as *const T)` on
`#[repr(C, packed)]` structs were replaced with `core::ptr::read_unaligned()`.
This eliminates undefined behavior from misaligned pointer dereferences in both
debug and release builds. The RSDP, RSDT/XSDT, and MADT parsing now use
stack-local copies of packed structures, ensuring correct field access regardless
of the physical memory alignment provided by the firmware.

Boot now shows:
```
[ACPI] MADT parsed: 1 local APICs, 1 I/O APICs, 5 overrides
[ACPI] Parsed 6 ACPI table(s)
```

---

## 6. ~~Timer Callback Not Executed in Interrupt Context~~ ✅ RESOLVED

| Field       | Detail                                                                 |
|-------------|------------------------------------------------------------------------|
| Severity    | ~~Low~~ → Resolved                                                    |
| Component   | `kernel/src/interrupts/mod.rs`, `kernel/src/main.rs`                   |
| Status      | **Fixed** in Phase 11-2 (2026-03-05)                                   |
| Discovered  | Phase 10-2 preemptive scheduling (2026-02-25)                         |

### Resolution

Phase 11-2 implemented a lock-free bottom-half work queue
(`kernel/src/interrupts/workqueue.rs`). Timer, keyboard, and mouse ISRs
now push `WorkItem` events into a 256-entry ring buffer using atomic
indices — no Mutex is ever acquired inside interrupt context. The main
kernel loop calls `workqueue::drain()` each iteration to dispatch
pending events with interrupts enabled, completely eliminating the
interrupt-context deadlock.

Boot now shows:
```
[WorkQueue] drained N items so far
```

---

## 7. User Page Table Only Copied Upper-Half P4 Entries

| Field       | Detail                                                                 |
|-------------|------------------------------------------------------------------------|
| Severity    | Critical                                                               |
| Component   | `kernel/src/memory/user_page_table.rs`                                 |
| Affects     | Ring 3 process execution — CR3 switch caused triple fault              |
| Status      | **RESOLVED** (Phase 10-3, 2026-02-25)                                  |
| Discovered  | Phase 10-3 Ring 3 user-space isolation (2026-02-25)                    |

### Symptom

Switching CR3 to a newly-created user page table caused an immediate triple
fault. The Ring 3 test process never reached its entry point.

### Root Cause

`create_user_page_table()` copied only P4 entries 256–511 (the "upper half"
of the virtual address space), assuming the kernel lived in the upper half.
However, the bootloader (v0.11.14) maps its physical memory offset and heap
in the **lower half** — the physical memory identity map lands at P4 index ~5
and the heap at P4 index ~136. After CR3 switch, kernel code, heap, and
stacks were all unmapped.

### Fix

Changed `create_user_page_table()` to copy **all 512 P4 entries** from the
current kernel page table. Security is maintained because kernel pages lack
the `USER_ACCESSIBLE` page flag — the CPU's MMU enforces this in hardware,
so Ring 3 code still cannot access kernel memory despite the P4 entries
being present.

---

## 8. ~~mprotect / rt_sigaction / rt_sigprocmask / futex Were Stubs~~ ✅ RESOLVED

| Field       | Detail                                                                 |
|-------------|------------------------------------------------------------------------|
| Severity    | ~~Medium~~ → Resolved                                                  |
| Component   | `kernel/src/syscall/linux_handlers.rs`                                 |
| Status      | **Fixed** in Phase 10-4 (2026-02-26)                                   |

### Resolution

Phase 10-4 replaced all stub implementations with real functionality:

- **mprotect** — now walks the 4-level page table via `update_pte_flags()` and
  updates the leaf PTE flags in-place, followed by `invlpg` TLB invalidation
  for each modified page.
- **rt_sigaction** — reads/writes the 32-byte `struct sigaction` from/to user
  space, stores per-signal handlers in `SignalState`, and rejects SIGKILL/SIGSTOP.
- **rt_sigprocmask** — implements `SIG_BLOCK`, `SIG_UNBLOCK`, and `SIG_SETMASK`
  operations on the per-process blocked signal mask.
- **futex** — `FUTEX_WAIT` atomically compares `*uaddr` with `expected` and blocks
  the task on a per-address wait queue; `FUTEX_WAKE` wakes up to `val` waiters.

---

## 9. ~~fork() Does Immediate Full Page Copy (Not CoW)~~ ✅ RESOLVED

| Field       | Detail                                                                 |
|-------------|------------------------------------------------------------------------|
| Severity    | ~~Low~~ → Resolved                                                    |
| Component   | `kernel/src/memory/user_page_table.rs` — `clone_user_page_table()`     |
| Status      | **Fixed** in Phase 11-1 (2026-03-05)                                   |
| Discovered  | Phase 10-4 (2026-02-26)                                               |

### Resolution

Phase 11-1 implemented full Copy-on-Write for `fork()`. When
`clone_user_page_table()` is called, user-space data frames are shared
between parent and child instead of being eagerly copied. Both PTEs are
marked read-only with a CoW marker (bit 9 of the PTE, an OS-available
bit). Writes to shared pages trigger a page fault, which the CoW
handler resolves by:
1. Allocating a new physical frame
2. Copying the 4 KiB data from the shared frame
3. Remapping the PTE with WRITABLE, clearing the CoW bit
4. Decrementing the old frame’s reference count (freeing it when
   the count reaches 0)
5. Invalidating the TLB entry

Per-frame reference counting is tracked in `memory/refcount.rs` via a
`BTreeMap<u64, u32>`. The `destroy_user_page_table()` and
`destroy_user_mappings()` functions are refcount-aware — shared frames
are only freed when no process references them.

Boot now shows:
```
[CoW] fork shared N pages (refcounted)
[CoW] fault handled at 0x... (copied frame, old refcount=2)
```
