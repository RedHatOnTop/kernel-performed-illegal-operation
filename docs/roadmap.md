# KPIO Development Roadmap

**Document Version:** 5.0.0  
**Last Updated:** 2026-02-20  
**Status:** Phase 8 In Progress (8-1 Complete) ✅

---

## Overview

This document outlines the phased development plan for the KPIO (Kernel Performed Illegal Operation) operating system. The roadmap is divided into multiple phases, each building upon the previous to create a complete, production-ready system.

**Update:** Phase 8 (Technical Debt Resolution) is in progress as of 2026-02-20. Sub-phase 8-1 (ACPI physical-to-virtual address translation fix) has been completed, resolving the critical page fault crash during ACPI initialization. See [Phase 8 Plan](../plans/PHASE_8_BUGFIX_PLAN.md).

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
| 5.0.0 | 2026-02-20 | Phase 8 started (8-1: ACPI address translation fix) |
| 4.0.0 | 2026-02-19 | Phase 7-4 complete (Linux binary compatibility layer) |
| 3.0.0 | 2026-02-17 | Phase 7-3 complete, roadmap updated through Phase 7 |
| 2.0.0 | 2026-01-23 | Phase 1 complete, Servo browser strategy added |
| 1.0.0 | 2026-01-21 | Initial roadmap |
