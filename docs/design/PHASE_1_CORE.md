# Phase 1: Core Design

For the original Korean version (full detail), see [PHASE_1_CORE.md](../ko/design/PHASE_1_CORE.md).

**Status:** ✅ Complete (2026-01-23)

## Overview

Phase 1 builds the core runtime of KPIO: a working interrupt system with APIC timer support, PCI enumeration with VirtIO detection, a minimal block device driver, and an embedded WebAssembly runtime suitable for kernel constraints.

---

## Completed Work (High Level)

### Core features

- APIC timer at 100Hz (with legacy 8259 PIC disabled)
- PCI bus enumeration (7 devices detected)
- VirtIO block device driver (64MB disk recognized)
- WebAssembly runtime integration and basic execution test (`add(2,3)=5`)

### Related commits (as recorded in the original doc)

- `cbf0c6a` - APIC timer and scheduler infrastructure
- `28b72dd` - PCI bus enumeration with VirtIO detection
- `d4560cf` - VirtIO block device driver (Stage 5)
- `5e0269f` - WASM runtime with wasmi interpreter (Phase 1 Complete)

---

## Key Design Decision: Choosing wasmi

Wasmtime requires a `std` environment, so the kernel integrates `wasmi` (a `no_std`-compatible interpreter) for Phase 1.

| Option | Description | Status |
|--------|-------------|--------|
| A | Run Wasmtime in userspace | Deferred |
| B | Fork Wasmtime for `no_std` | Too costly |
| C | Use wasmi (`no_std`) | ✅ Chosen |
| D | Hybrid (wasmi now, Wasmtime later) | Revisit in Phase 2 |

Rationale:
- Native `no_std` support
- Can run inside the kernel
- Browser-side performance can later be handled with a JS engine strategy (Phase 2)

---

## Prerequisites

- Phase 0 complete (boot, memory, serial output)

## Completion Criteria

- WASM test application runs (`add(2,3)=5`)
- APIC timer works at 100Hz
- VirtIO-Blk driver detects the disk (64MB)
- WASI core functions (e.g., `fd_write`, `clock_time_get`) deferred to Phase 2

---

## Major Subsystems (Pointers)

- Interrupts/APIC: `kernel/src/interrupts/*`
- ACPI parsing: `kernel/src/acpi/*` (if present)
- PCI enumeration: `kernel/src/pci/*` (if present)
- VirtIO: `kernel/src/drivers/virtio/*` (if present)
- Scheduling context infrastructure: `kernel/src/scheduler/*`
- WASM runtime integration: `runtime/*`

