# KPIO OS Release Notes

## Version 1.0.0

**Release Date**: 2026-XX-XX

---

## 🎉 Highlights

This is the first stable release of KPIO OS, a modern browser-based operating system.

- 🚀 **Custom Kernel** - Built from scratch in Rust for safety and performance
- 🌐 **Full Web Browser** - HTML5, CSS3, and JavaScript support
- 💻 **Desktop Environment** - Modern window management and taskbar
- 🔒 **Security-First** - Capability-based security and sandboxing
- 🌍 **Internationalization** - Multi-language support

---

## ✨ New Features

### Kernel
- Memory management with virtual memory, paging, and ASLR
- Process management with multi-tasking and scheduling
- Inter-process communication (channels, shared memory, message queues)
- Device driver framework (keyboard, mouse, storage, network)
- System call interface for applications
- **Full TCP/IP network stack** for online browsing (see below)
- **Phase 10-1: Stability fixes** — ACPI misaligned pointer deref (14 sites fixed with `read_unaligned`), VFS `seek(SeekFrom::End)` now returns real file size, VirtIO MMIO net path negotiates `MRG_RXBUF` feature and uses physical DMA addresses
- **Phase 10-2: Preemptive scheduling** — Real context switching via APIC timer interrupt. Tasks receive 10-tick time slices (~100 ms at 100 Hz); when a slice expires the scheduler saves callee-saved registers + RSP and switches to the next ready task. Per-task 16 KiB kernel stacks, `setup_initial_stack()` for first-run entry, `preempt_disable()`/`preempt_enable()` guards, `try_lock()` in interrupt context to prevent deadlocks. Verified: two CPU-bound tasks interleave output, context switch counter increments (56 switches in test run), no page faults or panics
- **Phase 10-3: Ring 3 user-space isolation** — Processes now execute in Ring 3 with full privilege separation. SYSCALL/SYSRET entry point via `IA32_LSTAR` MSR with SWAPGS-based per-CPU kernel stack switching. Per-process page tables created by copying all 512 P4 entries (kernel pages protected by clearing `USER_ACCESSIBLE` flag). CR3 switched on context switch; TSS RSP0 updated so Ring 3 interrupts land on the correct kernel stack. User-mode page faults and GPFs are caught gracefully — kernel logs the fault and kills the offending process instead of triple-faulting. Self-test: 16-byte x86_64 program at `0x400000` executes `mov rax,60; mov rdi,42; syscall` in Ring 3, the SYSCALL/SYSRET round-trip succeeds, and `SYS_EXIT(42)` is handled cleanly
- **Phase 10-4: Core process syscalls** — Full implementation of essential process lifecycle syscalls for Linux binary compatibility. `fork()` deep-copies parent address space (all user-space page tables and frames), creates child process with inherited file descriptors, signal handlers, and memory state. `execve()` replaces process image: tears down existing user mappings, loads new ELF binary from VFS, sets up fresh user stack. `wait4()` supports specific-PID and any-child waiting with WNOHANG, reaps zombie children and returns WEXITSTATUS-encoded exit codes. `mprotect()` now performs real PTE flag updates via 4-level page table walk with per-page TLB invalidation. Signal infrastructure: `rt_sigaction` installs/queries per-signal handlers (SIGKILL/SIGSTOP cannot be caught), `rt_sigprocmask` manages blocked signal masks, `kill`/`tkill`/`tgkill` deliver signals. Futex: `FUTEX_WAIT` blocks tasks on per-address wait queues with atomic value comparison, `FUTEX_WAKE` unblocks waiters via scheduler integration
- **Phase 10-5: Process lifecycle integration tests** — Automated QEMU test mode (`qemu-test.ps1 -Mode process`) validates the full preemptive scheduling + Ring 3 isolation + process lifecycle pipeline end-to-end. Three minimal x86_64 flat binaries embedded in the kernel as byte arrays: `HELLO_PROGRAM` (SYS_WRITE + SYS_EXIT), `SPIN_PROGRAM` (infinite loop proving preemption), `EXIT42_PROGRAM` (multi-process isolation with separate CR3). Each test creates a dedicated user page table, maps code + stack pages, and spawns a user-space process. A delayed summary after ~500ms prints final pass/fail status and context switch count. Assembly source files documented in `tests/e2e/userspace/`. Phase 10 is now complete: preemptive multitasking, Ring 3 isolation, process syscalls, and automated integration testing
- **Phase 11-1: Copy-on-Write fork** — `fork()` now shares user-space data frames instead of eagerly copying them. Both parent and child PTEs are marked read-only with a CoW marker (bit 9); writes trigger page faults handled by `handle_cow_fault()` which allocates a private copy on demand. Per-frame reference counting via `memory/refcount.rs` (`BTreeMap<u64, u32>`) tracks shared frames and ensures correct deallocation in `destroy_user_page_table()` / `destroy_user_mappings()`. Resolves known issue #9 (fork memory overhead)
- **Phase 11-2: Bottom-half work queue** — Lock-free 256-entry ring buffer (`interrupts/workqueue.rs`) eliminates ISR deadlocks. Timer, keyboard, and mouse ISRs push `WorkItem` events via atomic indices; the main loop drains and dispatches them outside interrupt context. Callback registration uses `AtomicPtr` instead of `Mutex<Option<fn>>`. Permanently resolves known issue #6 (timer callback deadlock)
- **Phase 11-3: Stack guard pages** — Kernel stacks now allocated from raw physical frames at `0xFFFF_C000_0000_0000` (P4 index 384) with a 4 KiB unmapped guard page below each stack. Stack overflows cause a clean page fault panic with task name instead of silent heap corruption. `is_stack_guard_page()` detects guard hits, and `[STACK] guard page at 0x...` is logged for each spawned task
- **Phase 11-4: Kernel hardening integration test** — `qemu-test.ps1 -Mode hardening` validates all Phase 11 features: CoW fork sharing, CoW fault handling, work queue drain, stack guard mapping, plus all Phase 10 process tests as regression baseline
- **Phase 12-1: Fix execve return path** — `execve()` now correctly redirects the calling process to the new ELF entry point. `EXECVE_PENDING` / `EXECVE_NEW_RIP` / `EXECVE_NEW_RSP` / `EXECVE_NEW_RFLAGS` AtomicU64 statics communicate between the Rust handler and the `ring3_syscall_entry` assembly epilogue. After `ring3_syscall_dispatch` returns, the epilogue checks `EXECVE_PENDING`: if set, it loads the new RIP→RCX, new RFLAGS→R11, new RSP→r14, zeros all GPRs, and issues `sysretq` to the new entry point. The inline `handle_execve()` in `scheduler/userspace.rs` reads the pathname from user memory, loads the ELF from VFS, parses ELF64 PT_LOAD headers, reuses existing page mappings via `read_pte()` + `write_to_phys()` (avoids `destroy_user_mappings` which hangs under SFMASK IF=0), and sets the execve context. QEMU test: execve-caller invokes `execve("/bin/exec-target")`, target writes "EXECVE OK" via SYS_WRITE and exits with code 42
- **Phase 12-2: Fix fork child return** — After `fork()`, the child now resumes from the exact instruction after the parent's `syscall` with RAX=0 (correct POSIX fork return value for child). Parent's user RIP/RSP/RFLAGS saved to AtomicU64 statics in `ring3_syscall_entry` assembly. `Task::new_forked_process()` creates a child whose first context-switch enters `fork_child_trampoline()`, which uses `iretq` to enter Ring 3 at the saved call-site with all GPRs zeroed. CoW page table clone via `clone_user_page_table()` with critical bugfix: only P4[0] (user-space range) is deep-cloned, P4[1-255] (kernel lower-half entries) are shallow-copied to avoid triple faults from cloning bootloader infrastructure. QEMU test: parent gets child PID 50, child gets RAX=0, both print confirmation to serial

### Network Stack (NEW)
- **VirtIO-net driver** — real packet TX/RX over both MMIO virtqueues and PIO (legacy PCI transport)
- **DHCP** — automatic IP acquisition from QEMU SLIRP (`10.0.2.15/24`, gw `10.0.2.2`, dns `10.0.2.3`)
- **DMA** — `virt_to_phys()` 4-level page table walk for correct VirtIO descriptor addresses (net + block)
- **WASI2 Real Network** — WASI Preview 2 HTTP and TCP sockets backed by the kernel's real TCP/IP stack (via `kernel::net::wasi_bridge`); DNS resolution uses wire-format UDP queries instead of hardcoded stubs; mock fallback preserved for unit testing without kernel

### Storage (NEW)
- **VirtIO-blk driver** — page-aligned queue, `virt_to_phys()` DMA translation, read/write sectors
- **FAT32** — read-only filesystem: BPB parse, FAT chain traversal, directory listing, file read
- **VFS** — mount table, file handle table, open/read/close/readdir/stat dispatch through `Filesystem` trait
- Boot-time self-test: mounts FAT32 disk, reads HELLO.TXT, lists directory, validates error paths

### End-to-End I/O Integration Test (NEW)
- **`qemu-test.ps1 -Mode io`** — automated QEMU test validates full I/O path: NIC init → DHCP → packet TX/RX → VFS mount → disk read
- Boot-time E2E self-test checks NIC state, DHCP lease, VFS mount, file read, TX counter
- Logs `[E2E] Integration test PASSED` on success
- Host-side HTTP test server (`tests/e2e/http-server.py`) for advanced integration testing
- **Ethernet** — IEEE 802.3 frame parse/build
- **ARP** — address resolution with cache (RFC 826)
- **IPv4** — packet construction, internet checksum, ICMP echo, subnet routing
- **UDP** — datagram socket table for DNS queries
- **TCP** — full 3-way handshake, sequence/ack, retransmission, FIN teardown
- **DNS** — RFC 1035 wire-format queries over UDP, host table + TTL cache
- **TLS 1.2** — SHA-256, HMAC, AES-128-CBC, RSA key exchange (demo cert validation)
- **HTTP/1.1** — client with chunked decoding; serves built-in kpio:// pages
- **HTTPS** — TLS-wrapped HTTP for port-443 connections
- Works out-of-the-box with QEMU `-netdev user` (IP 10.0.2.15, GW 10.0.2.2, DNS 10.0.2.3)

### Browser
- HTML5 parsing and rendering
- CSS3 styling with cascade and inheritance
- JavaScript execution environment
- Content Security Policy (CSP) support
- Cookie management with SameSite support
- Private browsing mode
- Tab management
- Extension support framework

### Desktop
- Window management with drag, resize, minimize, maximize
- Taskbar with app launcher and system tray
- Start menu with application search
- File manager with tree navigation
- Settings application

### System Applications
- Terminal emulator
- Text editor with syntax highlighting
- Calculator
- Settings panel

### Security
- Capability-based security model
- Process sandboxing
- Syscall filtering
- Security audit logging

### Internationalization
- English (en)
- Korean (ko)
- Japanese (ja)
- Chinese Simplified (zh-CN)
- Spanish (es)
- German (de)

---

## 📋 System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | x86_64 | 2+ cores, 64-bit |
| RAM | 256 MB | 1 GB |
| Storage | 2 GB | 8 GB |
| Display | 1024x768 | 1920x1080 |
| Graphics | VGA | VESA compatible |

---

## 🐛 Known Issues

- [ ] **#001** - High DPI scaling may have visual artifacts
- [ ] **#002** - Some complex CSS layouts may not render correctly
- [ ] **#003** - JavaScript performance on complex pages needs optimization
- [ ] **#004** - Network driver may require manual configuration on some hardware
- [x] **#005** - ~~ACPI page fault crash when parsing RSDP/XSDT/MADT (physical address used without virtual translation)~~ — Fixed in Phase 8-1
- [x] **#006** - ~~ACPI `tables()` returns dangling `&'static` reference via unsound `MutexGuard` → raw pointer cast~~ — Fixed in Phase 8-2
- [x] **#007** - ~~Network stack initialized before PCI/VirtIO enumeration, causing DHCP to fail due to missing NIC~~ — Fixed in Phase 8-3
- [x] **#008** - ~~VirtIO network probe was a no-op stub; QEMU launched without NIC device, so no NIC was ever discovered~~ — Fixed in Phase 8-4
- [x] **#009** - ~~`free_frame()` was a no-op; physical frames were never returned to the allocator, causing memory leak under sustained allocation~~ — Fixed in Phase 8-5
- [x] **#010** - ~~`acpi` and `aml` crates declared in `kernel/Cargo.toml` but never imported; `[features] acpi` name collided with crate name~~ — Fixed in Phase 8-6
- [x] **#011** - ~~~869 build warnings (dead_code, unused_imports, unused_variables, etc.) obscuring real issues; no workspace-level lint policy~~ — Fixed in Phase 8-7
- [x] **#012** - ~~VirtIO Net PIO `read8`/`write8`/`read32`/`write32` returned 0 / no-op — NIC initialization impossible via PIO transport~~ — Fixed in Phase 9-1
- [x] **#013** - ~~`probe()` discovered VirtIO NIC but did not call `init_pio()` — NIC was never initialized~~ — Fixed in Phase 9-1
- [x] **#014** - ~~Phase 9-3 storage integration blocker: `VirtIO-Blk` read timeout on sector 0 during FAT mount~~ — Fixed in Phase 9-3 DMA fix

---

## 📥 Installation

### From ISO

1. Download `kpio-os-1.0.0.iso`
2. Verify checksum: `sha256sum -c kpio-os-1.0.0.iso.sha256`
3. Create bootable USB: `dd if=kpio-os-1.0.0.iso of=/dev/sdX bs=4M`
4. Boot from USB

### On Virtual Machine

**QEMU:**
```bash
qemu-system-x86_64 -m 1G -cdrom kpio-os-1.0.0.iso -enable-kvm
```

**VirtualBox:**
1. Create new VM (Type: Other, Version: Other/Unknown 64-bit)
2. Allocate 1GB RAM
3. Attach ISO as optical drive
4. Start VM

---

## 🔧 Upgrade Instructions

This is the initial release. Future upgrade instructions will be provided here.

---

## 👥 Contributors

Thank you to all contributors who made this release possible!

- Core kernel development
- Browser engine implementation
- Desktop environment design
- Testing and quality assurance
- Documentation and translations

---

## 📜 License

KPIO OS is released under the MIT License.

```
MIT License

Copyright (c) 2026 KPIO Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

---

## 📮 Feedback

We'd love to hear from you!

- **Bug Reports**: [GitHub Issues](https://github.com/kpio/kpio/issues)
- **Feature Requests**: [GitHub Discussions](https://github.com/kpio/kpio/discussions)
- **General Questions**: [Community Forum](https://forum.kpio.local)

---

*KPIO OS - The browser is the operating system.*
