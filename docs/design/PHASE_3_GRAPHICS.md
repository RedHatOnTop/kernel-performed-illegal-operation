# Phase 3: Graphics Design

For the original Korean version (full detail), see [PHASE_3_GRAPHICS.md](../ko/design/PHASE_3_GRAPHICS.md).

## Overview

Phase 3 adds a graphical user interface to KPIO: a driver isolation architecture, a unified GPU API built around wgpu concepts, a Vello-based renderer for vector content, and a minimal compositor/window system.

---

## Prerequisites

- Phase 2 complete (IPC, filesystem, networking foundations)

## Completion Criteria

- Render a triangle via wgpu-style pipeline
- Create a basic window
- Handle keyboard/mouse input

---

## Architecture (High Level)

### GPU driver isolation

GPU drivers run as **userspace processes**. The kernel exposes only minimal primitives:

- device discovery and resource enumeration
- MMIO and DMA mediation
- buffer submission primitives and synchronization

This minimizes the kernel attack surface while allowing richer driver logic and rapid iteration in userspace.

### Kernel GPU HAL (minimal)

The kernel-side GPU HAL should focus on:

- safely mapping hardware resources
- enforcing access control/capabilities
- exposing a stable ABI for userspace driver services

### Userspace driver services

- Translate high-level rendering requests into Vulkan command buffers
- Manage GPU memory and scheduling cooperation with the kernel
- Provide stable rendering services to the compositor

---

## Rendering and UI Stack

- **wgpu-compatible abstraction:** a stable API surface for apps and UI components
- **Vello renderer:** vector display lists, text, and UI rendering
- **Compositor:** window management, composition, and presentation timing (VSync-aware)
- **Input:** device events flow from kernel to compositor to focused surfaces

---

## Success Criteria (Performance and Stability)

- No kernel crashes on malformed GPU requests (isolation works)
- Predictable frame scheduling for foreground UI
- Clear separation between kernel primitives and userspace driver logic

