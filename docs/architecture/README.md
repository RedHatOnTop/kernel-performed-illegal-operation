# System Architecture Overview

**Document Version:** 1.0.0  
**Last Updated:** 2026-01-21  
**Status:** Initial Draft

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Design Principles](#2-design-principles)
3. [System Layers](#3-system-layers)
4. [Component Interactions](#4-component-interactions)
5. [Security Model](#5-security-model)
6. [Performance Considerations](#6-performance-considerations)
7. [Related Documents](#7-related-documents)

---

## 1. Introduction

### 1.1 Purpose

This document provides a comprehensive overview of the KPIO (Kernel Performed Illegal Operation) operating system architecture. It serves as the authoritative reference for understanding how system components interact and the rationale behind key design decisions.

### 1.2 Scope

This document covers:
- High-level system architecture
- Inter-component communication patterns
- Security boundaries and isolation mechanisms
- Performance optimization strategies

This document does NOT cover:
- Implementation-specific details (see individual component documents)
- API specifications (see `docs/specifications/`)
- Build procedures (see `docs/building.md`)

### 1.3 Audience

- Kernel developers
- System programmers
- Security researchers
- Contributors evaluating the project

---

## 2. Design Principles

### 2.1 Stability Through Isolation

The fundamental architectural principle is that **no user-space failure should ever compromise kernel stability**. This is achieved through:

1. **WebAssembly Sandboxing:** All user-space applications run as WASM binaries with linear memory isolation.
2. **Capability-Based Security:** Processes receive explicit capabilities; no ambient authority exists.
3. **Fail-Safe Defaults:** When in doubt, deny access and log the attempt.

### 2.2 Minimal Kernel Surface

The kernel exposes the smallest possible attack surface:

| Kernel Responsibility | Delegated To |
|-----------------------|--------------|
| Hardware abstraction | Kernel (Ring 0) |
| Memory management | Kernel (Ring 0) |
| Process scheduling | Kernel (Ring 0) |
| IPC primitives | Kernel (Ring 0) |
| Device drivers | User-space WASM services |
| File systems | User-space WASM services |
| Graphics rendering | User-space (Mesa + Compositor) |
| Network stack | User-space (smoltcp service) |

### 2.3 Rust Memory Safety Guarantees

The kernel is written entirely in Rust with `no_std`, leveraging:

- **Ownership semantics:** Compile-time memory safety without garbage collection
- **No null pointers:** `Option<T>` for optional values
- **No buffer overflows:** Bounds-checked array access
- **No use-after-free:** Borrow checker prevents dangling references

### 2.4 Immutability by Default

The system image is read-only at runtime:

```
+---------------------------+
|     Read-Only Root        |  <-- Verified boot image
+---------------------------+
|     Overlay (tmpfs)       |  <-- Runtime modifications
+---------------------------+
|   User Data Partitions    |  <-- Mounted as needed
+---------------------------+
```

Updates occur atomically by replacing the entire root image, ensuring:
- Rollback capability on failure
- Impossible to corrupt the base system
- Consistent, reproducible system state

---

## 3. System Layers

### 3.1 Layer Diagram

```
+================================================================+
|                        USER SPACE (Ring 3)                      |
+================================================================+
|  +------------------+  +------------------+  +----------------+ |
|  |   Applications   |  |     Services     |  |   Compositor   | |
|  |     (.wasm)      |  |  (Network, FS)   |  |  (wgpu/Vello)  | |
|  +--------+---------+  +--------+---------+  +-------+--------+ |
|           |                     |                    |          |
|           +----------+----------+----------+---------+          |
|                      |                     |                    |
|              +-------v-------+     +-------v--------+           |
|              |  WASI Layer   |     | GPU Extensions |           |
|              +-------+-------+     +-------+--------+           |
|                      |                     |                    |
+================================================================+
|                      RUNTIME LAYER                              |
+================================================================+
|               +---------------------------+                      |
|               |     KPIO Runtime          |                      |
|               | (interpreter + JIT WIP)   |                      |
|               +-------------+-------------+                      |
|                             |                                    |
+================================================================+
|                        KERNEL (Ring 0)                          |
+================================================================+
|  +------------+  +------------+  +------------+  +------------+ |
|  |   Memory   |  | Scheduler  |  |    IPC     |  |  Drivers   | |
|  |  Manager   |  |            |  |            |  |  (Minimal) | |
|  +------------+  +------------+  +------------+  +------------+ |
|                            |                                    |
+================================================================+
|                        HARDWARE LAYER                           |
+================================================================+
|        CPU        |        GPU        |       Devices           |
+================================================================+
```

### 3.2 Kernel Layer (Ring 0)

**Location:** `kernel/`

The kernel provides fundamental OS services:

| Subsystem | Description | Source Location |
|-----------|-------------|-----------------|
| Boot | UEFI boot protocol, kernel entry | `kernel/src/boot/` |
| Memory | Physical/virtual memory, allocator | `kernel/src/memory/` |
| Scheduler | Async cooperative multitasking | `kernel/src/scheduler/` |
| IPC | Message passing, shared memory | `kernel/src/ipc/` |
| Arch | x86_64 specific code, interrupts | `kernel/src/arch/` |
| Drivers | Minimal hardware drivers | `kernel/src/drivers/` |

See [Kernel Design Document](kernel.md) for detailed specifications.

### 3.3 Runtime Layer

**Location:** `runtime/`

The runtime bridges kernel services and user-space applications:

| Component | Purpose |
|-----------|---------|
| KPIO Runtime (`runtime/`) | Embedded WASM execution engine (parser/module/instance/interpreter + tiered JIT scaffold) |
| WASI Preview 1 | File I/O, clocks, random, args/env via `wasi_snapshot_preview1` |
| Host Extensions | `kpio`/`kpio_gpu`/`kpio_net` namespaces (부분 구현) |

See [WebAssembly Runtime Document](wasm-runtime.md) for detailed specifications.

### 3.4 Graphics Layer

**Location:** `graphics/`

The graphics subsystem implements the "Vulkan Mandate":

```
+------------------------------------------------------------------+
|                     APPLICATION LAYER                             |
+------------------------------------------------------------------+
|  WebGPU API  |  Vector Display Lists  |  Legacy OpenGL (via Zink) |
+------------------------------------------------------------------+
                              |
+------------------------------------------------------------------+
|                     COMPOSITOR LAYER                              |
+------------------------------------------------------------------+
|                    wgpu + Vello Renderer                          |
+------------------------------------------------------------------+
                              |
+------------------------------------------------------------------+
|                     DRIVER LAYER (User Space)                     |
+------------------------------------------------------------------+
|        RADV (AMD)      |     ANV (Intel)     |    NVK (NVIDIA)   |
+------------------------------------------------------------------+
|                         Mesa 3D Core                              |
+------------------------------------------------------------------+
                              |
+------------------------------------------------------------------+
|                     KERNEL DRM/KMS                                |
+------------------------------------------------------------------+
|            Mode Setting          |        Buffer Allocation       |
+------------------------------------------------------------------+
                              |
+------------------------------------------------------------------+
|                         HARDWARE                                  |
+------------------------------------------------------------------+
|                            GPU                                    |
+------------------------------------------------------------------+
```

See [Graphics Subsystem Document](graphics.md) for detailed specifications.

### 3.5 Network Layer

**Location:** `network/`

Networking is implemented as an isolated user-space service:

| Component | Technology | Purpose |
|-----------|------------|---------|
| TCP/IP Stack | smoltcp | Protocol implementation |
| VirtIO-Net Driver | Custom | Virtualized network interface |
| E1000 Driver | Custom | Intel Gigabit Ethernet |

See [Networking Document](networking.md) for detailed specifications.

### 3.6 Storage Layer

**Location:** `storage/`

Storage provides immutable root with flexible user data access:

| Component | Purpose |
|-----------|---------|
| VFS | Virtual file system abstraction |
| InitramFS | Initial RAM-based file system |
| FUSE-WASM | User-space file system modules |
| FS Drivers | NTFS, EXT4, FAT32 support |

See [Storage Document](storage.md) for detailed specifications.

---

## 4. Component Interactions

### 4.1 Inter-Process Communication (IPC)

KPIO uses a **capability-based message passing** system:

```
+-------------+                              +-------------+
|  Process A  |                              |  Process B  |
+------+------+                              +------+------+
       |                                            ^
       | 1. Send message                            | 4. Receive message
       v                                            |
+------+------+                              +------+------+
|   Channel   |  -- 2. Kernel validates -->  |   Channel   |
|  Endpoint   |  <-- 3. Copy to receiver --  |  Endpoint   |
+-------------+                              +-------------+
```

**IPC Message Structure:**

```rust
struct IpcMessage {
    header: MessageHeader,      // Type, length, capabilities
    payload: [u8; MAX_INLINE],  // Inline data (up to 4KB)
    capabilities: [Cap; 4],     // Transferred capabilities
    shared_pages: Option<PageRange>, // Zero-copy large data
}
```

### 4.2 System Call Interface

WASM applications invoke kernel services through the WASI-compatible syscall layer:

| Syscall Category | Examples |
|------------------|----------|
| File Operations | `fd_read`, `fd_write`, `path_open` |
| Clock | `clock_time_get`, `clock_res_get` |
| Random | `random_get` |
| Process | `proc_exit`, `sched_yield` |
| **Custom: GPU** | `gpu_submit_buffer`, `gpu_create_context` |
| **Custom: IPC** | `ipc_send`, `ipc_recv`, `ipc_create_channel` |

### 4.3 Graphics Pipeline Flow

```
1. Application creates WebGPU command buffer
                    |
                    v
2. Application submits via WASI GPU extension
                    |
                    v
3. Compositor aggregates command buffers from all windows
                    |
                    v
4. Compositor submits combined work to Vulkan queue
                    |
                    v
5. Mesa Vulkan driver translates to hardware commands
                    |
                    v
6. DRM/KMS performs page flip (zero-copy to display)
```

### 4.4 Network Packet Flow

```
+----------------+     +----------------+     +----------------+
|  Application   | --> |  Network Svc   | --> |  NIC Driver    |
|   (WASM)       |     |  (smoltcp)     |     |  (VirtIO)      |
+----------------+     +----------------+     +----------------+
        ^                      |                      |
        |                      v                      v
        |              +----------------+     +----------------+
        |              |   TCP/IP       |     |   Hardware     |
        +------------- |   Processing   | <-- |   Interrupt    |
                       +----------------+     +----------------+
```

---

## 5. Security Model

### 5.1 Privilege Separation

```
+------------------------------------------------------------------+
| Ring 0 (Kernel)                                                   |
| - Full hardware access                                            |
| - Memory management                                               |
| - Interrupt handling                                              |
+------------------------------------------------------------------+
| Ring 3 (User Space)                                               |
| - WASM sandbox                                                    |
| - No direct hardware access                                       |
| - Capability-mediated resource access                             |
+------------------------------------------------------------------+
```

### 5.2 WebAssembly Sandboxing Properties

| Property | Enforcement Mechanism |
|----------|----------------------|
| Memory Isolation | WASM linear memory (bounds-checked) |
| Control Flow Integrity | WASM structured control flow |
| Type Safety | WASM type system |
| No Raw Pointers | WASM memory model |
| Deterministic Execution | WASM semantics |

### 5.3 Capability System

Capabilities are unforgeable tokens granting specific permissions:

```rust
enum Capability {
    FileRead { path: PathBuf, recursive: bool },
    FileWrite { path: PathBuf, recursive: bool },
    NetworkConnect { host: IpAddr, port_range: Range<u16> },
    NetworkListen { port: u16 },
    GpuAccess { adapter_id: u32 },
    IpcEndpoint { channel_id: u64 },
}
```

**Capability Rules:**
1. Capabilities cannot be created; only derived from parent capabilities
2. Capabilities can be attenuated (reduced scope) but never amplified
3. The kernel validates all capability uses
4. Capabilities are revocable by the granting process

### 5.4 Verified Boot Chain

```
+-------------+     +-------------+     +-------------+     +-------------+
|    UEFI     | --> |  Shim/MOK   | --> |   Kernel    | --> |   InitFS    |
| SecureBoot  |     | (Optional)  |     | Signature   |     |   Hash      |
+-------------+     +-------------+     +-------------+     +-------------+
      |                   |                   |                   |
      v                   v                   v                   v
   Verify             Verify              Verify              Verify
   Firmware           Loader              Kernel              Root FS
```

---

## 6. Performance Considerations

### 6.1 WASM Execution Performance

| Optimization | Implementation |
|--------------|----------------|
| Tiered execution | Interpreter 기본 + JIT 프레임워크 확장 중 |
| SIMD/bulk/reference types | 런타임 설정 플래그로 기능 제어 |
| Fuel/limits | 실행 연료 및 자원 제한(DoS 완화) |

성능 수치는 인터프리터 중심 현재 구현에서 JIT 완성도에 따라 크게 변동될 수 있어, 벤치마크 기반으로 지속 갱신합니다.

### 6.2 Graphics Performance

| Optimization | Mechanism |
|--------------|-----------|
| Zero-Copy Display | Direct scanout from application buffers |
| Command Buffer Batching | Compositor aggregates GPU work |
| Async Compute | Parallel graphics and compute queues |
| Render Graph | Automatic resource barrier optimization |

### 6.3 Network Performance

| Optimization | Mechanism |
|--------------|-----------|
| Zero-Copy Receive | Page flipping for large packets |
| Batch Syscalls | Vectored I/O operations |
| Poll-Based I/O | No interrupt overhead for high throughput |
| Async Processing | Non-blocking socket operations |

### 6.4 Memory Efficiency

| Technique | Benefit |
|-----------|---------|
| Demand Paging | Only allocate used memory |
| Copy-on-Write | Efficient process forking |
| Huge Pages | Reduced TLB pressure |
| Memory Deduplication | Shared identical pages across WASM instances |

---

## 7. Related Documents

| Document | Description |
|----------|-------------|
| [Kernel Design](kernel.md) | Detailed kernel architecture |
| [Graphics Subsystem](graphics.md) | Vulkan graphics stack |
| [WebAssembly Runtime](wasm-runtime.md) | WASM execution environment |
| [Networking](networking.md) | Network stack architecture |
| [Storage](storage.md) | File system design |
| [Roadmap](../roadmap.md) | Development timeline |
| [Building](../building.md) | Build instructions |

---

## Appendix A: Comparison with Existing Systems

| Aspect | KPIO | Linux | Windows | Fuchsia |
|--------|------|-------|---------|---------|
| Kernel Type | Hybrid-Microkernel | Monolithic | Hybrid | Microkernel |
| User Binary Format | WASM only | ELF | PE | ELF |
| Graphics API | Vulkan only | Multiple | Multiple | Vulkan |
| Memory Safety | Rust (compile-time) | C (manual) | C/C++ | C++ |
| Update Model | Atomic image | Package-based | Component-based | Package-based |

## Appendix B: Glossary

| Term | Definition |
|------|------------|
| WASM | WebAssembly - portable binary instruction format |
| WASI | WebAssembly System Interface - syscall standard |
| DRM | Direct Rendering Manager - kernel graphics subsystem |
| KMS | Kernel Mode Setting - display configuration |
| IPC | Inter-Process Communication |
| VFS | Virtual File System |
| Capability | Unforgeable token granting specific permissions |
