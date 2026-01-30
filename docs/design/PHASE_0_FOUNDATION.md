# Phase 0: Foundation Design

For the original Korean version (full detail), see [PHASE_0_FOUNDATION.md](../ko/design/PHASE_0_FOUNDATION.md).

## Overview

Phase 0 establishes the bootable kernel skeleton and the minimum infrastructure needed for further phases: boot, early hardware initialization, memory management, and a basic test/debug loop.

---

## Prerequisites

- None (first phase)

## Completion Criteria

- Boots in QEMU and prints `"Hello, Kernel"` via serial output
- Basic page frame allocator works
- Kernel unit test framework runs

---

## Boot Process

### UEFI boot flow (high level)

1. UEFI firmware initialization
2. UEFI bootloader loads
3. Kernel ELF is loaded and parsed
4. Framebuffer (GOP) is configured
5. Memory map is collected
6. Control jumps to the kernel entry point

### Kernel entry responsibilities

- Initialize serial output
- Initialize GDT/IDT and exception handlers
- Initialize paging and virtual memory mapping using the bootloader-provided physical memory offset
- Initialize heap allocator
- Run tests when built in test mode
- Enter the halt loop when idle

Relevant code:
- `kernel/src/main.rs`
- `kernel/src/gdt.rs`
- `kernel/src/interrupts/*`
- `kernel/src/memory/*`
- `kernel/src/allocator.rs`
- `kernel/src/serial.rs`

---

## Hardware Initialization

### GDT/TSS and IDT (x86_64)

- Provide a reliable GDT/TSS setup with a dedicated stack for double faults
- Install IDT entries for exceptions and interrupts
- Ensure a safe default handler behavior and a stable halt loop

### Early device access (minimal)

- Serial output for early debugging
- Optional framebuffer output (if enabled by the bootloader configuration)

---

## Memory Management

### Physical memory

- Build a frame allocator from the UEFI/bootloader memory map
- Support allocating and freeing frames reliably

### Virtual memory

- Set up paging and map required regions for kernel execution
- Validate assumptions around physical memory offset mappings

### Heap

- Create a heap region for dynamic allocations
- Ensure allocation failures fail safely (no undefined behavior)

---

## Testing and Debugging

- Kernel test runner integrated via a custom test framework
- QEMU-based workflow as the primary validation environment for Phase 0

