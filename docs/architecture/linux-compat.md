# Linux Binary Compatibility Layer

> **Phase:** 7-4 — Linux Binary Compatibility  
> **Status:** ✅ Complete  
> **Last Updated:** 2026-02-19

---

## Overview

KPIO OS provides a Linux binary compatibility layer that enables running **statically-linked Linux x86_64 ELF binaries** directly on the KPIO kernel. This is achieved through:

1. **ELF Loader** — Full ELF64 parser with segment loading, BSS initialization, PIE support
2. **User-Space Execution** — Ring 3 execution with per-process page tables, CR3 switching
3. **Linux Syscall Router** — Translates Linux x86_64 syscall numbers to KPIO handler functions
4. **Per-Process State** — Isolated FD tables, memory management (brk/mmap), working directories

### Architecture

```
┌─────────────────────────────────────────────────┐
│          Linux ELF Binary (static, musl)         │
│            User-space (Ring 3)                    │
└─────────────────────┬───────────────────────────┘
                      │ syscall instruction
┌─────────────────────┴───────────────────────────┐
│          Linux Syscall Router                    │
│  ┌────────────────────────────────────────────┐  │
│  │  Linux syscall # → KPIO handler function   │  │
│  │  read(0)    → sys_read()                  │  │
│  │  write(1)   → sys_write()                 │  │
│  │  mmap(9)    → sys_mmap()                  │  │
│  │  brk(12)    → sys_brk()                   │  │
│  │  unknown(N) → return -ENOSYS + log         │  │
│  └────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────┐ │
│  │  Syscall Tracing & Statistics (Phase 7-4.6) │ │
│  └─────────────────────────────────────────────┘ │
├──────────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐   │
│  │   VFS    │  │ Memory   │  │  Process     │   │
│  │  Layer   │  │ Manager  │  │  Manager     │   │
│  └──────────┘  └──────────┘  └──────────────┘   │
│                KPIO Kernel (Ring 0)               │
└──────────────────────────────────────────────────┘
```

---

## Supported Linux x86_64 Syscalls

### Tier A — Fully Implemented (~47 syscalls)

| # | Syscall | Category | Status | Notes |
|---|---------|----------|--------|-------|
| 0 | `read` | File I/O | ✅ Full | Per-process FD table, user pointer validation |
| 1 | `write` | File I/O | ✅ Full | Stdout/stderr → serial console |
| 2 | `open` | File I/O | ✅ Full | Linux O_* flag translation |
| 3 | `close` | File I/O | ✅ Full | Per-process FD cleanup |
| 4 | `stat` | File I/O | ✅ Full | Linux `struct stat` layout |
| 5 | `fstat` | File I/O | ✅ Full | Via FD → path resolution |
| 8 | `lseek` | File I/O | ✅ Full | SEEK_SET/CUR/END |
| 9 | `mmap` | Memory | ✅ Partial | MAP_ANONYMOUS only; no file-backed mmap |
| 10 | `mprotect` | Memory | ✅ Partial | VMA protection update; PTE in-place TODO |
| 11 | `munmap` | Memory | ✅ Full | VMA split/remove, frame deallocation |
| 12 | `brk` | Memory | ✅ Full | Page-aligned, 256 MB limit |
| 13 | `rt_sigaction` | Signal | ⚠️ Stub | Returns 0 (musl init compatibility) |
| 14 | `rt_sigprocmask` | Signal | ⚠️ Stub | Returns 0 (musl init compatibility) |
| 16 | `ioctl` | I/O | ✅ Partial | TIOCGWINSZ only (80×25) |
| 19 | `readv` | File I/O | ✅ Full | Scatter/gather read |
| 20 | `writev` | File I/O | ✅ Full | Scatter/gather write |
| 21 | `access` | File I/O | ✅ Full | File existence check |
| 22 | `pipe` | Pipe/FD | ✅ Full | 4 KB circular buffer |
| 28 | `madvise` | Memory | ⚠️ No-op | Advisory only |
| 32 | `dup` | Pipe/FD | ✅ Full | Lowest available FD |
| 33 | `dup2` | Pipe/FD | ✅ Full | Specific FD target |
| 35 | `nanosleep` | Time | ✅ Full | TSC-based busy wait |
| 39 | `getpid` | Process | ✅ Full | Per-process PID |
| 60 | `exit` | Process | ✅ Full | Exit code, resource cleanup |
| 62 | `kill` | Signal | ✅ Partial | SIGKILL/SIGTERM only |
| 63 | `uname` | System | ✅ Full | sysname="Linux", release="6.1.0-kpio" |
| 72 | `fcntl` | Pipe/FD | ✅ Full | F_DUPFD, F_GETFD/SETFD, F_GETFL/SETFL |
| 79 | `getcwd` | Directory | ✅ Full | Per-process working directory |
| 80 | `chdir` | Directory | ✅ Full | VFS path validation |
| 83 | `mkdir` | Directory | ✅ Full | VFS directory creation |
| 87 | `unlink` | Directory | ✅ Full | VFS file removal |
| 89 | `readlink` | Directory | ✅ Full | /proc/self/exe → binary path |
| 96 | `gettimeofday` | Time | ✅ Full | TSC-based |
| 102 | `getuid` | Identity | ✅ Full | Returns 0 (root) |
| 104 | `getgid` | Identity | ✅ Full | Returns 0 (root) |
| 107 | `geteuid` | Identity | ✅ Full | Returns 0 (root) |
| 108 | `getegid` | Identity | ✅ Full | Returns 0 (root) |
| 158 | `arch_prctl` | System | ✅ Full | ARCH_SET_FS/GET_FS for TLS |
| 202 | `futex` | Threading | ⚠️ Stub | Minimal FUTEX_WAIT/WAKE stubs |
| 217 | `getdents64` | Directory | ✅ Full | Linux `struct linux_dirent64` |
| 218 | `set_tid_address` | Threading | ✅ Full | Returns tid |
| 228 | `clock_gettime` | Time | ✅ Full | CLOCK_MONOTONIC/REALTIME |
| 231 | `exit_group` | Process | ✅ Full | Alias to exit (single-threaded) |
| 257 | `openat` | File I/O | ✅ Full | AT_FDCWD support |
| 267 | `readlinkat` | Directory | ✅ Full | AT_FDCWD support |
| 273 | `set_robust_list` | Threading | ⚠️ Stub | Returns 0 |
| 293 | `pipe2` | Pipe/FD | ✅ Full | Flags support |
| 302 | `prlimit64` | System | ✅ Full | Default resource limits |
| 318 | `getrandom` | System | ✅ Full | RDRAND-based CSPRNG |

### Tier B — Not Implemented (Future Phases)

| # | Syscall | Category | Rationale |
|---|---------|----------|-----------|
| 56 | `clone` | Process | Thread/process creation — requires full process model |
| 57 | `fork` | Process | Process duplication — not required for static binaries |
| 59 | `execve` | Process | Process replacement — stretch goal for BusyBox shell |
| 61 | `wait4` | Process | Wait for child — requires fork/exec |
| 41-49 | `socket/*` | Network | Network sockets — separate network phase |
| 17 | `pread64` | File I/O | Positioned read — future enhancement |
| 18 | `pwrite64` | File I/O | Positioned write — future enhancement |
| 76 | `truncate` | File I/O | File truncation — future enhancement |
| 77 | `ftruncate` | File I/O | FD truncation — future enhancement |
| 82 | `rename` | Directory | File rename — future enhancement |
| 90 | `chmod` | Permission | File permissions — single-user system |
| 91 | `fchmod` | Permission | FD permissions — single-user system |
| 92 | `chown` | Permission | File ownership — single-user system |

### Unsupported Syscalls

Any syscall not listed above returns `-ENOSYS` (38) and is logged to the serial console with its human-readable name when first encountered.

---

## Binary Compatibility Matrix

| Binary Type | Status | Build Command | Notes |
|-------------|--------|---------------|-------|
| **C (musl static)** | ✅ Supported | `musl-gcc -static -o hello hello.c` | Primary target; tested with BusyBox |
| **Rust (musl static)** | ✅ Supported | `cargo build --target x86_64-unknown-linux-musl --release` | String processing, file I/O work |
| **Go (static)** | ⚠️ Partial | `CGO_ENABLED=0 go build -o hello hello.go` | Needs futex stubs; limit `GOMAXPROCS=1` |
| **C (glibc dynamic)** | ❌ Not supported | — | Requires dynamic linker (ld-linux.so) |
| **Any (dynamic)** | ❌ Not supported | — | Static linking required |
| **C++ (musl static)** | ⚠️ Partial | `x86_64-linux-musl-g++ -static -o hello hello.cpp` | Basic I/O works; exceptions need signal support |

### Recommended Build Commands

#### C (musl)
```bash
# Install musl-tools: apt install musl-tools
musl-gcc -static -O2 -o myapp myapp.c
```

#### Rust
```bash
# Install target: rustup target add x86_64-unknown-linux-musl
cargo build --target x86_64-unknown-linux-musl --release
```

#### Go
```bash
CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -o myapp myapp.go
```

#### BusyBox
```bash
make defconfig
make LDFLAGS=-static CC=musl-gcc -j$(nproc)
```

---

## Known Limitations

1. **Static linking only** — No dynamic linking support (no glibc, no ld-linux.so)
2. **No threading** — `clone()` returns ENOSYS; applications must be single-threaded
3. **No signal handlers** — `rt_sigaction` is a stub; no user-space signal delivery
4. **No file-backed mmap** — Only `MAP_ANONYMOUS` is supported
5. **No network sockets** — Socket syscalls are not implemented
6. **No GUI** — No X11, Wayland, or framebuffer support for Linux binaries
7. **No kernel modules** — Linux .ko files cannot be loaded
8. **Single user** — All processes run as root (uid=0, gid=0)
9. **mprotect PTE** — VMA protection is tracked but PTE in-place modification is pending
10. **Go goroutines** — Go runtime requires `clone` for goroutines; limited to `GOMAXPROCS=1`

---

## Syscall Tracing & Debugging

### Enabling Trace Mode

Syscall tracing can be enabled through several methods:

1. **Boot flag**: Add `--linux-trace` to boot arguments
2. **Runtime**: Call `trace::enable_trace()` from kernel code
3. **Compile-time feature**: Build with `--features trace-syscalls`

### Trace Output

When enabled, every syscall invocation is logged to the serial console:

```
[TRACE] pid=2 write(1) args=(0x1, 0x400100, 0x5, 0x0, 0x0, 0x0)
[TRACE] pid=2 write(1) → 0x5 (5)
[TRACE] pid=2 exit(60) args=(0x0, 0x0, 0x0, 0x0, 0x0, 0x0)
[TRACE] pid=2 exit(60) → 0x0 (0)
```

Error returns show the errno name:

```
[TRACE] pid=2 open(2) args=(0x400200, 0x0, 0x0, 0x0, 0x0, 0x0)
[TRACE] pid=2 open(2) → -2 (ENOENT)
```

### Syscall Statistics

Statistics are collected by default and can be dumped via `trace::dump_stats()`:

```
╔══════════════════════════════════════════════╗
║      Linux Syscall Statistics Summary        ║
╠══════════════════════════════════════════════╣
║ Total invocations: 1247                      ║
║ Unknown syscalls:  3                         ║
╠══════════════════════════════════════════════╣
║  # │ Name                │ Count            ║
╠══════════════════════════════════════════════╣
║   1 │ write               │ 523              ║
║   0 │ read                │ 312              ║
║  12 │ brk                 │ 89               ║
║   9 │ mmap                │ 45               ║
║  ... │ ...                │ ...              ║
╚══════════════════════════════════════════════╝
```

---

## Implementation Details

### Module Structure

| File | Purpose | LOC |
|------|---------|-----|
| `kernel/src/loader/elf.rs` | ELF64 parser | ~566 |
| `kernel/src/loader/program.rs` | UserProgram, auxv builder | ~487 |
| `kernel/src/loader/segment_loader.rs` | Segment loading, stack setup | ~531 |
| `kernel/src/syscall/linux.rs` | Naked entry point, dispatch table, errno | ~510 |
| `kernel/src/syscall/linux_handlers.rs` | All syscall handler implementations | ~2,435 |
| `kernel/src/syscall/percpu.rs` | Per-CPU data for GS base | ~175 |
| `kernel/src/syscall/trace.rs` | Syscall tracing and statistics | ~400 |
| `kernel/src/process/linux.rs` | LinuxProcess, launch_linux_process() | ~350 |
| `kernel/src/process/table.rs` | Process table, FD table, VMA | ~486 |
| `kernel/src/process/context.rs` | ProcessContext, iretq entry | ~356 |
| **Total** | | **~6,300** |

### Syscall Entry Path

1. User code executes `syscall` instruction
2. CPU loads LSTAR → `linux_syscall_entry` (naked function)
3. `swapgs` — switch to kernel GS base
4. Save user RSP, load kernel stack from per-CPU data
5. Push all registers (SyscallFrame)
6. Call `linux_syscall_dispatch_inner(nr, a1..a6)`
7. Trace entry + record statistics
8. Match syscall number → handler function
9. Handler executes (with user pointer validation)
10. Trace exit
11. Return value in RAX
12. Pop registers, restore user RSP
13. `swapgs` + `sysretq` — return to user-space

### Security Measures

- **User pointer validation**: All user-space pointers checked against `USER_ADDR_MAX` (0x8000_0000_0000)
- **W^X enforcement**: Code pages are R+X (no W), data pages are R+W (no X)
- **Per-process page tables**: Each process has isolated address space
- **FD isolation**: Per-process file descriptor tables
- **Stack guard pages**: Guard pages below user stack
- **Kernel half-space**: Indices 256-511 of P4 table are kernel-mapped (user cannot access)

---

## Testing

### Unit Tests

```bash
cargo test -p kpio-kernel --lib
```

Covers:
- Syscall dispatch routing (all Tier A syscalls)
- errno values
- User pointer validation
- Process table operations
- VMA overlap detection
- ELF parsing
- Trace module (enable/disable, statistics, name lookup)

### Integration Tests

The `linux_compat_tests` module validates:
- BusyBox applet syscall sequences (echo, cat, ls, wc, grep, head, tail, sort, mkdir, rm, pwd, uname)
- Static Rust binary init sequence (arch_prctl → set_tid_address → brk → mmap)
- Static C/musl binary init sequence (musl __init_libc flow)
- Go binary init stubs
- Stress testing (no kernel panics under rapid syscall dispatch)
- Complete Tier A syscall coverage
- Syscall trace statistics recording

### E2E Tests (QEMU)

```bash
# Build and run in QEMU with syscall tracing
./scripts/run-qemu.ps1 --linux-trace
```

---

*This document was generated as part of Phase 7-4.6 (Integration Testing & Compatibility Validation).*
