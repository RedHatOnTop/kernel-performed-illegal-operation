# Phase 2: Browser Integration Design

For the original Korean version (full detail), see [PHASE_2_USERSPACE.md](../ko/design/PHASE_2_USERSPACE.md).

**Status:** 🔄 Planning

## Overview

Phase 2 integrates a Servo-based browser deeply into KPIO to unlock kernel-level optimizations that conventional OS/browser boundaries cannot provide.

Key supporting docs:
- [Phase 2 Servo Integration Architecture](../architecture/phase2-servo-integration.md)
- [Phase 2 Implementation Checklist](../architecture/phase2-implementation-checklist.md)

---

## Core Strategy: OS-Level Browser Integration

### What typical browsers cannot optimize

In a traditional setup, the browser duplicates OS responsibilities (GPU scheduling, memory policy, process model), while the OS has limited visibility into browser intent and workload patterns.

### KPIO approach

Servo runs as a first-class userspace workload with explicit kernel integration points:

- **Per-tab GPU scheduling:** prioritize foreground interactivity and frame deadlines.
- **Memory pressure integration:** background tab compression/hibernation coordinated by the kernel.
- **Shared WASM runtime strategy:** reuse runtime components and caching mechanisms across apps and browser.
- **Zero-copy network → GPU pipeline:** reduce data movement and latency for media and rendering.

---

## High-Level Architecture

### Process model

- A top-level browser process coordinates UI, compositing, and kernel IPC.
- Tabs/scripts are isolated as separate sandboxed processes (or compartments) to contain failures and enforce security boundaries.

### IPC and shared memory

- Control-plane messages via channels
- Data-plane buffers (network, GPU command buffers, WASM cache) via shared memory for zero-copy performance

---

## Key Deliverables

- Userspace runtime capable of hosting Servo (`std`-level functionality in userspace)
- Minimal syscall surface for Servo bootstrapping (memory, threads, file I/O, networking)
- Kernel-side browser support layer:
  - GPU scheduler hooks
  - tab memory manager
  - WASM AOT cache integration points
  - zero-copy networking primitives

---

## Success Criteria

- Minimal Servo build compiles and runs in KPIO userspace
- about:blank renders via Vulkan/WebRender path
- Basic input (click/scroll) flows through end-to-end
- Memory and GPU policies show measurable wins vs baseline targets

