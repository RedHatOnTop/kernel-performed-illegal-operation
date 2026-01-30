# Phase 1: Core Execution Plan

For the original Korean version (full detail), see [PHASE_1_EXECUTION_PLAN.md](../ko/design/PHASE_1_EXECUTION_PLAN.md).

## Overview

This document describes a practical implementation order for Phase 1 based on [PHASE_1_CORE.md](PHASE_1_CORE.md).

**Start Date:** 2026-01-22  
**Goal:** Run a WASM “Hello World”, basic WASI behavior, VirtIO-Blk read/write

---

## Baseline (What Already Exists)

| Module | Location | Status |
|--------|----------|--------|
| GDT | `kernel/src/gdt.rs` | done |
| IDT | `kernel/src/interrupts/mod.rs`, `idt.rs` | base structure done |
| PIC | `kernel/src/interrupts/pic.rs` | present (not used) |
| Memory management | `kernel/src/memory/mod.rs` | done |
| Heap allocator | `kernel/src/allocator.rs` | done |
| Serial output | `kernel/src/serial.rs` | done |
| Scheduler | `kernel/src/scheduler/` | skeleton exists |
| Runtime | `runtime/src/` | skeleton exists |

## Missing Pieces (Priority)

| Module | Description | Priority |
|--------|-------------|----------|
| APIC | Local APIC / I/O APIC init | P0 |
| Timer | APIC timer interrupt | P0 |
| Context switch | Assembly implementation | P0 |
| ACPI parsing | Extract APIC info from MADT | P1 |
| PCI enumeration | Discover VirtIO devices | P1 |
| VirtIO common | VirtQueue implementation | P1 |
| VirtIO-Blk | Block device driver | P1 |
| WASM runtime | Integrate wasmi (`no_std`) | P1 |
| WASI basics | `fd_write`, `clock_time_get` | P2 |

---

## Execution Stages

### Stage 1: Interrupt infrastructure (1-2 days)

- Implement Local APIC init and EOI
- Implement I/O APIC register access and IRQ routing
- Configure the APIC timer (periodic mode)
- Add an interrupt handler for the timer vector and minimal instrumentation (counters/flags)

Primary locations:
- `kernel/src/interrupts/apic.rs`
- `kernel/src/interrupts/ioapic.rs`
- `kernel/src/interrupts/mod.rs`

### Stage 2: Finish the scheduler (2-3 days)

- Add assembly context switching
- Strengthen task management (stack allocation, task lifecycle, wake/sleep)

Primary locations:
- `kernel/src/scheduler/*`

### Stage 3: ACPI + PCI + VirtIO (3-5 days)

- Parse MADT to discover APIC topology and override addresses
- Enumerate PCI and detect VirtIO devices
- Implement VirtQueue and a minimal VirtIO transport layer
- Implement VirtIO-Blk read/write

Primary locations:
- `kernel/src/acpi/*` (if present)
- `kernel/src/pci/*` (if present)
- `kernel/src/drivers/virtio/*` (if present)

### Stage 4: WASM runtime integration (2-4 days)

- Integrate wasmi for `no_std` execution
- Add a minimal module loader and test harness
- Run the basic WASM smoke test (`add(2,3)=5`)

Primary locations:
- `runtime/*`

### Stage 5: Minimal WASI surface (optional for Phase 1)

- Implement a minimal subset required by early userspace tooling
- Defer broader WASI to Phase 2 where it aligns with browser integration

---

## Validation Checklist

- APIC timer runs at 100Hz (stable over time)
- Scheduler can switch tasks without corrupting context
- PCI enumeration lists expected devices
- VirtIO-Blk can read/write a known sector range reliably
- WASM runtime can instantiate and run a test module

