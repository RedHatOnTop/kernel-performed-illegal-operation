# KPIO Development Roadmap

**Document Version:** 7.9.0  
**Last Updated:** 2026-03-10  
**Status:** Phase 13 In Progress 🔄 (13-1 ✅, 13-2 🔄, 13-3 ⬜, 13-4 ⬜, 13-5 ⬜)

---

## Overview

This document outlines the phased development plan for the KPIO (Kernel Performed Illegal Operation) operating system. The roadmap is divided into multiple phases, each building upon the previous to create a complete, production-ready system.

**Update:** Phase 13-2 (BSD Socket Syscalls) started 2026-03-10. Implementing bind/listen/accept/connect/sendto/recvfrom/shutdown/getpeername/getsockname/setsockopt/getsockopt syscall handlers in `ring3_syscall_dispatch`. Wiring `network::socket::send()`/`recv()` to use kernel-internal loopback buffer for TCP echo testing (smoltcp loopback not available under QEMU SLIRP). Adding `accept()` and `poll()` to `network::socket`.

**Previous:** Phase 13-1 (Socket FD Infrastructure) completed 2026-03-10. Extended Ring 3 FD table (`RING3_FD_TABLE`) with `FdKind` enum supporting both VFS files and network sockets. Implemented `dispatch_sys_socket` (SYS_SOCKET=41) in `ring3_syscall_dispatch` with AF_INET/SOCK_STREAM/SOCK_DGRAM validation, delegating to `network::socket::create()`. Socket FDs wired through `dispatch_sys_read` (via `network::socket::recv`), `dispatch_sys_write` (via `network::socket::send`), and `dispatch_sys_close` (via `network::socket::close`). `dispatch_sys_lseek` returns -ESPIPE for socket FDs. QEMU serial log shows `[Socket] created fd=4` and Ring 3 ELF test writes `[IPC] Socket create` to stdout. 21/21 tests pass.

**Previous:** Phase 13 (IPC, Socket Syscalls & Kernel Threading) planning started 2026-03-10. Five sub-phases: 13-1 (Socket FD Infrastructure), 13-2 (BSD Socket Syscalls), 13-3 (Kernel Threading via clone), 13-4 (Epoll Event Multiplexing), 13-5 (Integration Test). Exposes existing TCP/IP stack to Ring 3 via socket syscalls, adds CLONE_THREAD support with shared address space, and implements epoll for event-driven I/O. See [Phase 13 Plan](../plans/PHASE_13_IPC_AND_THREADING_PLAN.md).

**Previous:** Phase 12-7 (Integration Test) completed 2026-03-09. Automated QEMU integration test (`qemu-test.ps1 -Mode userspace`) validates all Phase 12 features in a single boot: execve return path, fork child return, ProcessManager::spawn from VFS, FAT32 write support, init-from-disk boot, and userlib syscall wiring. 21/21 checks pass (11 smoke + 10 Phase 12-specific). Phase 12 is now fully complete. See [Phase 12 Plan](../plans/PHASE_12_USERSPACE_AND_WRITABLE_FS_PLAN.md).

**Previous:** Phase 12-5 (Init Process & ELF-from-Disk Boot) completed 2026-03-07. First end-to-end user-space pipeline from persistent storage: `create-test-disk.ps1` generates two minimal ELF64 binaries (INIT=173 bytes, BIN/HELLO=179 bytes) and places them on the FAT32 test disk. Kernel reads `/mnt/test/INIT` from FAT32 via `storage::vfs::open/read`, bridges to in-memory VFS, and spawns via `ProcessManager::spawn_from_vfs("/init")` (pid=3). Similarly loads `/mnt/test/BIN/HELLO` and spawns (pid=4). QEMU serial log shows `[INIT] PID 1 running` and `Hello from disk!`. No panics or triple faults. See [Phase 12 Plan](../plans/PHASE_12_USERSPACE_AND_WRITABLE_FS_PLAN.md).

**Previous:** Phase 12-4 (FAT32 Write Support) completed 2026-03-06. Full FAT32 write support implemented in `storage/src/fs/fat32.rs`: cluster allocation (`alloc_cluster`), chain extension/freeing, FAT entry write with backup FAT mirroring, file create/write/unlink/mkdir/rmdir/truncate, `open()` with CREATE and TRUNCATE flag support. `free_clusters` changed to `AtomicU32` for interior mutability. `OpenFile` extended with `parent_dir_cluster` and `dir_entry_chain_offset` for directory entry write-back. Mount changed from `READ_ONLY` to writable. Integration test creates `/mnt/test/WRITTEN.TXT` with "Hello from KPIO", reads back and verifies content. See [Phase 12 Plan](../plans/PHASE_12_USERSPACE_AND_WRITABLE_FS_PLAN.md).

**Previous:** Phase 12-3 (ProcessManager::spawn from VFS) completed 2026-03-06. `spawn_from_vfs(path)` reads an ELF binary from the in-memory VFS, parses it with `Elf64Loader`, creates a per-process page table via `create_user_page_table()`, loads PT_LOAD segments via `load_elf_segments()`, allocates a 32 KiB kernel stack, registers the process in `PROCESS_TABLE` with `LinuxMemoryInfo` (CR3, brk, mmap base), creates a `Task::new_user_process()`, and enqueues it via `scheduler::spawn()`. Integration test: minimal 171-byte ELF64 at `/bin/spawn-test` writes "SPAWN OK\n" + SYS_EXIT(0). QEMU test: pid=2, cr3=0xd434000, 17 pages mapped. See [Phase 12 Plan](../plans/PHASE_12_USERSPACE_AND_WRITABLE_FS_PLAN.md).

**Previous:** Phase 12-2 (Fix fork Child Return) completed 2026-03-06. After `fork()`, the child now resumes from the exact instruction after the parent's `syscall` with RAX=0. Implementation saves the parent's user RIP/RSP/RFLAGS to AtomicU64 statics in `ring3_syscall_entry` assembly, `Task::new_forked_process()` creates a child whose first context-switch enters `fork_child_trampoline()` which `iretq`s to Ring 3 at the saved call-site with all GPRs zeroed (RAX=0 = child fork return). CoW page table clone via `clone_user_page_table()` (bugfix: only P4[0] deep-cloned, P4[1-255] shallow-copied to avoid cloning kernel lower-half entries). QEMU test: parent receives child PID 50, child receives 0, both print to serial. See [Phase 12 Plan](../plans/PHASE_12_USERSPACE_AND_WRITABLE_FS_PLAN.md).

**Previous:** Phase 12-1 (Fix execve Return Path) completed 2026-03-06. `sys_execve()` now correctly redirects SYSCALL return to the new ELF entry point. Implementation uses `EXECVE_PENDING` AtomicU64 statics checked in `ring3_syscall_entry` assembly epilogue — if set, `sysretq` is redirected to new RIP/RSP/RFLAGS instead of the original caller. Inline ELF64 loader in `ring3_syscall_dispatch` handles minimal PT_LOAD segment mapping with page reuse via `read_pte()`. QEMU test confirms "EXECVE OK" from target binary with exit code 42. See [Phase 12 Plan](../plans/PHASE_12_USERSPACE_AND_WRITABLE_FS_PLAN.md).

**Previous:** Phase 11 (Kernel Hardening) completed 2026-03-05. All four sub-phases are done. Sub-phase 11-1 (Copy-on-Write Fork) — `fork()` now shares user-space data frames instead of eagerly copying; writes trigger CoW page faults via bit-9 PTE marker, per-frame reference counting in `memory/refcount.rs`, `clone_user_page_table()` rewritten for CoW sharing, `handle_cow_fault()` allocates private copies on demand. Sub-phase 11-2 (Bottom-Half Work Queue) — lock-free 256-entry ring buffer (`interrupts/workqueue.rs`) eliminates ISR deadlocks; timer/keyboard/mouse callbacks dispatched outside interrupt context via `drain()` in the main loop; known issue #6 (timer callback deadlock) permanently resolved. Sub-phase 11-3 (Stack Guard Pages) — kernel stacks allocated from raw physical frames at `0xFFFF_C000_0000_0000` (P4 index 384) with unmapped guard pages; page fault handler detects guard hits and panics with task name. Sub-phase 11-4 (Integration Test) — `qemu-test.ps1 -Mode hardening` validates CoW fork sharing, CoW fault handling, work queue drain, and stack guard mapping. See [Phase 11 Plan](../plans/PHASE_11_KERNEL_HARDENING_PLAN.md).

**Previous:** Phase 10 (Preemptive Kernel & User-Space Isolation) completed 2026-02-26. All five sub-phases are done. Sub-phase 10-1 (stability fixes) — ACPI misaligned pointer derefs fixed, VFS `seek(SeekFrom::End)` bug fixed, VirtIO MMIO `MRG_RXBUF` feature negotiation corrected. Sub-phase 10-2 (preemptive scheduling) — real context switching via APIC timer, per-task kernel stacks, `setup_initial_stack()`, preemption guards, interrupt-safe `try_lock()`. Sub-phase 10-3 (Ring 3 user-space isolation) — SYSCALL/SYSRET via IA32_LSTAR with SWAPGS per-CPU stack switching, per-process CR3 page tables, TSS RSP0 update on context switch, graceful user-mode fault handling. Sub-phase 10-4 (core process syscalls) — `fork()` with full address-space copy, `execve()` with ELF reload and user-mapping teardown, `wait4()` with zombie reaping, `mprotect()` with real PTE flag updates and TLB flush, `rt_sigaction`/`rt_sigprocmask` signal handling, futex WAIT/WAKE with per-address wait queues. Sub-phase 10-5 (integration & validation) — automated QEMU process lifecycle tests (`qemu-test.ps1 -Mode process`), embedded test programs (hello Ring 3, spin preemption, multi-process isolation), Phase 10 documentation. See [Phase 10 Plan](../plans/PHASE_10_PREEMPTIVE_USERSPACE_PLAN.md).

**Previous:** Phase 9 (Real I/O — VirtIO Driver Completion & Stack Integration) completed 2026-02-24. All five sub-phases (9-1 through 9-5) have been implemented and verified. The kernel now has fully functional VirtIO network and block I/O, DHCP-acquired addressing, real WASI2 HTTP/sockets, and an automated E2E integration test (`qemu-test.ps1 -Mode io`). See [Phase 9 Plan](../plans/PHASE_9_REAL_IO_PLAN.md).

**Previous:** Phase 8 (Technical Debt Resolution) completed 2026-02-23. See [Phase 8 Plan](../plans/PHASE_8_BUGFIX_PLAN.md).

**Previous:** Phase 7-4 (Linux Binary Compatibility Layer) has been completed as of 2026-02-19. This includes ELF64 loading, 47 Linux syscalls, per-process page tables, syscall tracing, and integration tests. For full details, see [Phase 7-4 Plan](../docs/phase7/PHASE_7-4_LINUX_COMPAT_PLAN.md) and [Linux Compatibility](../docs/architecture/linux-compat.md).

**Previous:** Phase 7-3 (WASM/WASI App Runtime) was completed as of 2026-02-17. This includes WASI Preview 2, Component Model, JIT compiler, app packaging, and developer documentation. For full implementation details, see [Phase 7-3 Plan](../docs/phase7/PHASE_7-3_WASI_APP_RUNTIME_PLAN.md) and [Phase 7 Master Plan](../plans/PHASE_7_APP_EXECUTION_LAYER_PLAN.md).

### Key Strategic Decision: Servo-Based Browser Integration

KPIO will include a deeply integrated browser engine based on **Mozilla Servo**, providing:
- Full web standards support (HTML5, CSS3, JavaScript)
- OS-level optimizations impossible in traditional browsers
- Shared WASM runtime between browser and native apps
- Kernel-level privacy features (ad blocking, tracking protection)

---

## Phase 1: Core Foundation ✅ COMPLETE

**Duration:** 3-4 months  
**Status:** ✅ Complete (2026-01-23)
**Commits:** `cbf0c6a`, `28b72dd`, `d4560cf`, `5e0269f`

### Objectives

Establish the fundamental kernel infrastructure required for all subsequent development.

### Deliverables

| Component | Description | Priority |
|-----------|-------------|----------|
| UEFI Boot | x86_64 UEFI bootloader with handoff | Critical |
| Memory Manager | Physical and virtual memory management | Critical |
| Interrupt Handling | IDT setup, exception handlers | Critical |
| Basic Scheduler | Cooperative task switching | Critical |
| Serial Console | Debug output capability | High |
| Kernel Panic Handler | Graceful failure with diagnostics | High |

### Technical Milestones

1. **Week 1-2:** UEFI application skeleton
   - Build system setup (Cargo, cross-compilation)
   - UEFI entry point
   - Framebuffer initialization
   - Memory map retrieval

2. **Week 3-4:** Memory Management
   - Physical frame allocator (buddy system)
   - Page table management
   - Kernel heap allocator

3. **Week 5-6:** Interrupt Infrastructure
   - GDT/IDT setup
   - Exception handlers (page fault, GPF, etc.)
   - PIC/APIC initialization

4. **Week 7-8:** Basic Scheduler
   - Task control block structure
   - Context switching
   - Kernel-mode cooperative scheduling

5. **Week 9-12:** Integration and Testing
   - Memory subsystem stress tests
   - Interrupt handling verification
   - Documentation updates

### Success Criteria

- [x] Kernel boots to a command prompt (serial output)
- [x] Page allocator can allocate and free frames reliably
- [x] Scheduler infrastructure ready for task switching
- [x] APIC timer running at 100Hz
- [x] PCI bus enumeration working (7 devices)
- [x] VirtIO-Blk driver initialized (64MB disk)
- [x] WASM runtime executing test module
- [x] All code passes `cargo clippy` with no errors

---

## Phase 2: Browser Integration (Servo-Based)

**Duration:** 4-6 months  
**Status:** 🔄 In Planning  
**Dependencies:** Phase 1 complete

### Objectives

Integrate Servo browser components with deep OS-level optimizations for unprecedented web performance.

### Strategic Rationale

Traditional browsers run as isolated applications, duplicating OS functionality:
- Own GPU scheduler (conflicts with OS)
- Own memory management (overhead)
- Own process model (inefficient IPC)

KPIO's approach: Browser components are first-class OS citizens with kernel integration.

### Deliverables

| Component | Description | Priority |
|-----------|-------------|----------|
| html5ever | Servo HTML parser | Critical |
| Stylo | Parallel CSS engine (from Firefox) | Critical |
| WebRender | GPU-accelerated renderer | Critical |
| SpiderMonkey | JavaScript engine | Critical |
| WASI Integration | Browser WASM uses kernel runtime | High |
| GPU Scheduler | Kernel-integrated rendering | High |
| Tab Isolation | Native process per site | High |
| Ad Blocking | DNS + kernel-level blocking | Medium |

### Technical Milestones

1. **Week 1-4:** Servo Foundation
   - Port html5ever to KPIO
   - Stylo CSS engine integration
   - Basic DOM tree construction

2. **Week 5-8:** Rendering Pipeline
   - WebRender integration with Vulkan
   - Kernel GPU scheduler hooks
   - Zero-copy texture upload

3. **Week 9-12:** JavaScript Engine
   - SpiderMonkey port
   - WASM via shared kernel runtime
   - Event loop integration

4. **Week 13-16:** OS Integration
   - Tab = OS process model
   - Memory pressure handling
   - Background tab suspension

5. **Week 17-20:** Privacy & Polish
   - Kernel-level ad blocking
   - Site isolation verification
   - Performance benchmarking

### OS-Level Optimizations

```
┌─────────────────────────────────────────────────────┐
│                 KPIO Browser                         │
├─────────────────────────────────────────────────────┤
│  Servo Components (Rust)                            │
│  ├─ html5ever (HTML parser)                         │
│  ├─ Stylo (CSS engine) ← Firefox-proven             │
│  ├─ WebRender (GPU renderer) ← Firefox-proven       │
│  └─ SpiderMonkey (JavaScript)                       │
├─────────────────────────────────────────────────────┤
│  KPIO Kernel Integration (Unique)                   │
│  ├─ GPU scheduler: Priority by foreground tab       │
│  ├─ Memory: OS-level tab compression/swap           │
│  ├─ WASM: Shared runtime, AOT cache                 │
│  ├─ Network: Zero-copy to GPU                       │
│  └─ Security: Native process isolation              │
└─────────────────────────────────────────────────────┘
```

### Expected Performance Gains

| Metric | Chrome/Windows | KPIO Browser | Improvement |
|--------|----------------|--------------|-------------|
| Tab cold start | 2-3s | 0.5s | **4-6x** |
| Memory per tab | 100-300MB | 30-80MB | **3-4x** |
| GPU latency | 16-32ms | 4-8ms | **2-4x** |
| WASM startup | JIT compile | AOT cached | **2-3x** |

### Success Criteria

- [ ] html5ever parses major websites correctly
- [ ] Stylo renders CSS3 layouts
- [ ] WebRender displays pages via Vulkan
- [ ] JavaScript runs (SpiderMonkey integrated)
- [ ] Memory usage 50% less than Chrome
- [ ] Page load time competitive with Firefox

---

## Phase 3: Graphics Foundation
   - `wasi:filesystem` stubs
   - `wasi:clocks` implementation
   - `wasi:random` using RDRAND

3. **Week 7-9:** Module Loading
   - WASM binary parser
   - Module validation
   - Instance creation and memory setup

4. **Week 10-12:** IPC and Testing
   - Channel implementation
   - Message passing protocol
   - WASM test suite execution

### Success Criteria

- [ ] Simple WASM module (hello world) executes successfully
- [ ] WASI clock functions return correct time
- [ ] WASM can allocate and use linear memory
- [ ] Two WASM modules can communicate via IPC
- [ ] WASM spec tests pass (subset applicable to current runtime)

---

## Phase 3: Graphics Foundation

**Duration:** 4-5 months  
**Status:** Not Started  
**Dependencies:** Phase 2 browser foundation (WebRender)

### Objectives

Implement the Vulkan-exclusive graphics stack with GPU driver support, shared with browser rendering.

### Deliverables

| Component | Description | Priority |
|-----------|-------------|----------|
| DRM/KMS Port | Display mode setting, framebuffer | Critical |
| Mesa Integration | RADV driver (AMD) | Critical |
| Basic Compositor | wgpu-based window compositor | Critical |
| GPU WASI Extension | WebGPU-like API for WASM | High |
| VirtIO-GPU | Virtualization support | High |

### Technical Milestones

1. **Week 1-4:** DRM Subsystem
   - Mode setting structures
   - Framebuffer management
   - Scanout configuration
   - VirtIO-GPU driver

2. **Week 5-8:** Mesa Port
   - Build system integration
   - RADV driver bring-up
   - WSI implementation for KPIO
   - Shader compilation testing

3. **Week 9-12:** Compositor Core
   - wgpu initialization
   - Surface management
   - Basic window rendering
   - Double buffering

4. **Week 13-16:** GPU WASI Extension
   - WebGPU-like interface design
   - Resource creation/destruction
   - Command buffer submission
   - Basic rendering test

5. **Week 17-20:** Integration
   - End-to-end rendering pipeline
   - Multiple window support
   - Performance profiling

### Success Criteria

- [ ] VirtIO-GPU displays framebuffer in QEMU
- [ ] RADV can create a Vulkan instance (on AMD hardware)
- [ ] Compositor can render a colored rectangle
- [ ] WASM application can draw via GPU extension
- [ ] 60 FPS achievable with simple scene

---

## Phase 4: User-Space Services

**Duration:** 3-4 months  
**Status:** Not Started  
**Dependencies:** Phase 3 complete

### Objectives

Implement essential system services as WASM modules.

### Deliverables

| Component | Description | Priority |
|-----------|-------------|----------|
| Network Stack | smoltcp-based TCP/IP | Critical |
| Filesystem Service | VFS and FUSE-like modules | Critical |
| Input Service | Keyboard/mouse handling | Critical |
| Audio Service | Basic audio output | Medium |
| Init System | Service management | High |

### Technical Milestones

1. **Week 1-4:** Network Stack
   - smoltcp integration
   - VirtIO-Net driver
   - Socket API for WASM
   - DNS resolver

2. **Week 5-8:** Filesystem
   - VFS layer
   - FAT32 driver (for boot/recovery)
   - ext4 read support (WASM module)
   - Mount management

3. **Week 9-12:** Input and Browser Services
   - PS/2 keyboard driver
   - Input event dispatch to browser
   - Init process
   - Tab management service

4. **Week 13-16:** Audio and Polish
   - Basic audio driver (Intel HDA or VirtIO)
   - Audio mixing
   - Service integration testing

### Success Criteria

- [ ] wget-like tool can download a file over HTTP
- [ ] Can mount and read files from FAT32 USB image
- [ ] Keyboard input reaches WASM applications
- [ ] Init can start and restart services
- [ ] Basic audio playback works

---

## Phase 5: User Experience

**Duration:** 4-5 months  
**Status:** Not Started  
**Dependencies:** Phase 4 complete

### Objectives

Create a usable desktop environment with essential applications.

### Deliverables

| Component | Description | Priority |
|-----------|-------------|----------|
| Desktop Shell | Window management, taskbar | Critical |
| Terminal Emulator | VT100-compatible terminal | Critical |
| File Manager | Basic file browser | High |
| Text Editor | Simple text editing | High |
| Settings Application | System configuration | Medium |
| Browser Polish | Bookmarks, history, tabs UI | Critical |

### Technical Milestones

1. **Week 1-4:** Desktop Shell
   - Window manager (tiling/floating)
   - Taskbar and system tray
   - Application launcher
   - Browser as default "desktop"

2. **Week 5-8:** Core Applications
   - Terminal emulator with shell
   - File manager with navigation
   - Basic text editor

3. **Week 9-12:** Browser Features
   - Tab bar UI
   - Bookmarks and history
   - Download manager
   - Settings page

4. **Week 13-16:** Extended Apps
   - Settings panel
   - Theme support
   - Notification system
   - PWA installation

5. **Week 17-20:** Polish
   - UI consistency
   - Performance optimization
   - Accessibility basics

### Success Criteria

- [ ] Desktop boots to usable shell with browser
- [ ] Terminal can run interactive programs
- [ ] Can browse local files and open them
- [ ] Can edit and save text files
- [ ] Browser renders major websites (Google, GitHub, Wikipedia)
- [ ] PWAs can be installed as "apps"

---

## Phase 6: Production Hardening

**Duration:** 3-4 months  
**Status:** Not Started  
**Dependencies:** Phase 5 complete

### Objectives

Prepare the system for production use with security, stability, and performance improvements.

### Deliverables

| Component | Description | Priority |
|-----------|-------------|----------|
| Security Audit | Full codebase review | Critical |
| Performance Tuning | Profiling and optimization | Critical |
| Documentation | User and developer docs | Critical |
| Installation System | Disk partitioning, setup | High |
| Update Mechanism | A/B partition updates | High |
| Hardware Support | Additional drivers | Medium |

### Technical Milestones

1. **Week 1-4:** Security
   - Capability system audit
   - WASM sandbox verification
   - Fuzzing campaign
   - Vulnerability assessment

2. **Week 5-8:** Performance
   - CPU profiling
   - Memory usage optimization
   - I/O throughput tuning
   - Boot time reduction

3. **Week 9-12:** Polish and Docs
   - API documentation
   - User manual
   - Developer guide
   - Troubleshooting guide

4. **Week 13-16:** Installation and Updates
   - Installer application
   - Partition management
   - A/B update implementation
   - Recovery mode

### Success Criteria

- [ ] No critical security vulnerabilities
- [ ] Boot time under 5 seconds
- [ ] Memory usage under 256MB idle
- [ ] Complete documentation published
- [ ] Clean installation works on test hardware

---

## Hardware Targets

### Primary (Virtualized)

| Environment | Version | Notes |
|-------------|---------|-------|
| QEMU | 7.0+ | Primary development |
| VMware Workstation | 17+ | Secondary testing |
| VirtualBox | 7.0+ | Community testing |

### Secondary (Physical)

| Hardware | Notes |
|----------|-------|
| AMD Ryzen + Radeon | Primary GPU target |
| Intel Core + Intel Graphics | Secondary GPU target |
| NVIDIA + NVK | Experimental GPU target |

---

## Resource Requirements

### Development Team

| Role | Count | Phases |
|------|-------|--------|
| Kernel Developer | 2 | 1-6 |
| Graphics Developer | 1-2 | 3-6 |
| WASM/Runtime Developer | 1 | 2-6 |
| Application Developer | 1-2 | 4-6 |
| Documentation/QA | 1 | 4-6 |

### Infrastructure

| Resource | Purpose |
|----------|---------|
| CI Server | Automated builds and tests |
| GPU Test Machines | Hardware testing |
| Documentation Server | Docs hosting |
| Issue Tracker | Project management |

---

## Risk Assessment

### High Risk

| Risk | Mitigation |
|------|------------|
| Servo port complexity | Use well-tested components (Stylo, WebRender from Firefox) |
| SpiderMonkey integration | Start with minimal JS, expand gradually |
| Performance goals not met | Continuous profiling, kernel-level optimizations |

### Medium Risk

| Risk | Mitigation |
|------|------------|
| Driver compatibility | Focus on VirtIO first, expand to real hardware |
| Web compatibility | Use WPT (Web Platform Tests) for validation |
| Security vulnerabilities | Continuous fuzzing, external audits |

### Low Risk

| Risk | Mitigation |
|------|------------|
| Build system issues | Use established tooling |
| Documentation lag | Document as you go |

---

## Change Log

| Version | Date | Changes |
|---------|------|---------|
| 7.6.0 | 2026-03-08 | Phase 12-6 complete — userlib syscall wiring (fs/process/thread stubs → real Linux syscalls, inline FS dispatch in ring3, FD table, QEMU quality gate passed) |
| 7.5.0 | 2026-03-07 | Phase 12-5 complete — Init process & ELF-from-disk boot (FAT32 → VFS → spawn, INIT + hello binaries on disk) |
| 7.4.0 | 2026-03-06 | Phase 12-4 complete — FAT32 write support (create, write, unlink, mkdir, rmdir, truncate, cluster management, backup FAT mirroring) |
| 7.3.0 | 2026-03-06 | Phase 12-3 complete — ProcessManager::spawn_from_vfs() loads ELF from VFS, creates page table, loads segments, spawns process |
| 7.2.0 | 2026-03-06 | Phase 12-2 complete — fork child return fixed (iretq trampoline, CoW clone bugfix) |
| 7.1.0 | 2026-03-06 | Phase 12-1 complete — execve return path fixed (EXECVE_PENDING assembly epilogue, inline ELF loader) |
| 7.0.0 | 2026-03-05 | Phase 11 complete — kernel hardening (CoW fork, work queue, stack guards, integration test) |
| 6.8.0 | 2026-02-26 | Phase 10 complete — 10-5 integration tests and documentation |
| 6.7.0 | 2026-02-26 | Phase 10-4 complete — core process syscalls (fork, execve, wait4, mprotect, signals, futex) |
| 6.6.0 | 2026-02-25 | Phase 10-3 complete — Ring 3 user-space isolation with SYSCALL/SYSRET |
| 6.5.0 | 2026-02-25 | Phase 10-2 complete — preemptive scheduling with real context switching |
| 6.4.0 | 2026-02-25 | Phase 10 started — 10-1 complete (ACPI, VFS, MMIO fixes) |
| 6.3.0 | 2026-02-24 | Phase 9 complete (9-5: E2E integration test) |
| 6.2.0 | 2026-02-24 | Phase 9-3, 9-4 complete (VFS integration + WASI2 real network) |
| 6.0.0 | 2026-02-23 | Phase 9 started — 9-1 complete (VirtIO Net PIO driver) |
| 5.0.0 | 2026-02-20 | Phase 8 started (8-1: ACPI address translation fix) |
| 4.0.0 | 2026-02-19 | Phase 7-4 complete (Linux binary compatibility layer) |
| 3.0.0 | 2026-02-17 | Phase 7-3 complete, roadmap updated through Phase 7 |
| 2.0.0 | 2026-01-23 | Phase 1 complete, Servo browser strategy added |
| 1.0.0 | 2026-01-21 | Initial roadmap |
