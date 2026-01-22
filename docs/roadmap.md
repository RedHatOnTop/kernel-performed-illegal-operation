# KPIO Development Roadmap

**Document Version:** 1.0.0  
**Last Updated:** 2026-01-21  
**Status:** Initial Draft

---

## Overview

This document outlines the phased development plan for the KPIO (Kernel Performed Illegal Operation) operating system. The roadmap is divided into six major phases, each building upon the previous to create a complete, production-ready system.

---

## Phase 1: Core Foundation

**Duration:** 3-4 months  
**Status:** Not Started

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

- [ ] Kernel boots to a command prompt (serial output)
- [ ] Page allocator can allocate and free frames reliably
- [ ] Scheduler can switch between 3+ kernel tasks
- [ ] Page fault handler recovers gracefully from invalid access
- [ ] All code passes `cargo clippy` with no warnings

---

## Phase 2: WebAssembly Runtime

**Duration:** 3-4 months  
**Status:** Not Started  
**Dependencies:** Phase 1 complete

### Objectives

Integrate Wasmtime as the primary execution environment for user-space applications.

### Deliverables

| Component | Description | Priority |
|-----------|-------------|----------|
| Wasmtime Integration | Cranelift JIT in kernel context | Critical |
| WASI Preview 2 | Basic filesystem, clock, random | Critical |
| Module Loader | WASM module loading from memory | Critical |
| Syscall Interface | Kernel syscalls for WASI implementation | High |
| Basic IPC | Channel-based inter-process communication | High |

### Technical Milestones

1. **Week 1-3:** Wasmtime Port
   - no_std compatibility patches
   - Custom memory allocator integration
   - Cranelift backend configuration

2. **Week 4-6:** WASI Implementation
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
- [ ] Wasmtime spec tests pass (subset applicable to no_std)

---

## Phase 3: Graphics Foundation

**Duration:** 4-5 months  
**Status:** Not Started  
**Dependencies:** Phase 2 complete

### Objectives

Implement the Vulkan-exclusive graphics stack with GPU driver support.

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
   - EXT4 driver (WASM module)
   - FAT32 driver (WASM module)
   - Mount management

3. **Week 9-12:** Input and Init
   - PS/2 keyboard driver
   - Input event dispatch
   - Init process
   - Service supervision

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
| Web Browser | Basic HTML/CSS rendering | Medium |

### Technical Milestones

1. **Week 1-4:** Desktop Shell
   - Window manager (tiling/floating)
   - Taskbar and system tray
   - Application launcher
   - Desktop background

2. **Week 5-8:** Core Applications
   - Terminal emulator with shell
   - File manager with navigation
   - Basic text editor

3. **Week 9-12:** Extended Applications
   - Settings panel
   - Theme support
   - Notification system

4. **Week 13-16:** Web Browser
   - HTML parser
   - CSS layout engine
   - Network integration
   - JavaScript (via wasm)

5. **Week 17-20:** Polish
   - UI consistency
   - Performance optimization
   - Accessibility basics

### Success Criteria

- [ ] Desktop boots to usable shell
- [ ] Terminal can run interactive programs
- [ ] Can browse local files and open them
- [ ] Can edit and save text files
- [ ] Can view simple web pages

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
| Mesa port complexity | Start early, engage upstream |
| Wasmtime no_std challenges | Maintain close fork, contribute upstream |
| Performance goals not met | Continuous profiling, early optimization |

### Medium Risk

| Risk | Mitigation |
|------|------------|
| Driver compatibility | Focus on VirtIO first |
| Security vulnerabilities | Continuous fuzzing, external audits |
| Scope creep | Strict phase gating |

### Low Risk

| Risk | Mitigation |
|------|------------|
| Build system issues | Use established tooling |
| Documentation lag | Document as you go |

---

## Change Log

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-21 | Initial roadmap |
