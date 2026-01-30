# Phase 4: Polish Design

For the original Korean version (full detail), see [PHASE_4_POLISH.md](../ko/design/PHASE_4_POLISH.md).

## Overview

Phase 4 focuses on production readiness and “polish”: SMP support, performance tuning, completing the window manager UX, expanding driver coverage, and consolidating documentation.

---

## Prerequisites

- Phase 3 complete (graphics, compositor, input)

## Completion Criteria

- Distribute WASM tasks across multiple CPU cores (SMP)
- Window manager UI is complete and usable
- Performance benchmarks meet targets

---

## Major Workstreams (High Level)

### SMP (multi-core)

- Boot application processors (APs) and bring them online safely
- Per-core scheduling data structures
- Inter-processor interrupts (IPIs) for coordination
- Correct synchronization primitives and memory ordering requirements

### Performance

- Profiling-driven optimization across kernel and userspace services
- Reduce context switch overhead and IPC latency
- Improve GPU submission pipeline and frame pacing

### Window manager and UX

- Window lifecycle and focus management
- Input routing, shortcuts, and accessibility basics
- Basic animations and responsiveness goals

### Driver expansion

- Broader VirtIO device support as baseline
- Incrementally introduce real hardware support (GPU/network/storage) behind isolation boundaries

### Documentation consolidation

- Ensure the canonical docs are English and navigable
- Keep detailed Korean originals archived under `docs/ko/`

