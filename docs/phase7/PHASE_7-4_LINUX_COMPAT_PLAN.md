# Phase 7-4: Linux Binary Compatibility Layer

> **Goal:** Run statically-linked Linux x86_64 ELF binaries directly on KPIO OS by implementing an ELF loader, user-space execution environment, and a practical subset of Linux syscalls (~40 core syscalls).  
> **Status:** Planning  
> **Estimated Duration:** 5-7 weeks (6 sub-phases)  
> **Dependencies:** Phase 7.1 (App Runtime Foundation) ✅  
> **Scope Limitation:** Static-linked only. No dynamic linking (glibc/ld-linux.so), no GUI (X11/Wayland), no kernel modules.

---

## Architecture Overview

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
│  │  read(0)    → kpio_sys_read()             │  │
│  │  write(1)   → kpio_sys_write()            │  │
│  │  open(2)    → kpio_sys_open()             │  │
│  │  mmap(9)    → kpio_sys_mmap()             │  │
│  │  brk(12)    → kpio_sys_brk()              │  │
│  │  ...                                       │  │
│  │  unknown(N) → return -ENOSYS + log         │  │
│  └────────────────────────────────────────────┘  │
├──────────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐   │
│  │   VFS    │  │ Memory   │  │  Process     │   │
│  │  Layer   │  │ Manager  │  │  Manager     │   │
│  └──────────┘  └──────────┘  └──────────────┘   │
│                KPIO Kernel (Ring 0)               │
└──────────────────────────────────────────────────┘
```

---

## Current Kernel Readiness Assessment

| Area | Status | Gap |
|------|--------|-----|
| **ELF parsing** | ✅ Full ELF64 parser in `kernel/src/loader/elf.rs` (566 LOC) | Need actual segment loading into user page tables |
| **UserProgram/auxv** | ✅ `loader/program.rs` builds auxv, stack layout | Need connection to real memory mapping |
| **Syscall entry** | ⚠️ MSRs configured, but `syscall_entry()` is stub | Need full naked asm: swapgs, stack swap, register save, dispatch, sysretq |
| **Syscall numbers** | ⚠️ KPIO-native (0-111), not Linux | Need Linux x86_64 syscall number routing layer |
| **Page tables** | ⚠️ Custom PTE types with USER flag exist | Need `map_user_page()` + per-process CR3 creation |
| **User mode entry** | ⚠️ `enter_userspace()` iretq exists | Need GDT user segments + integration with process lifecycle |
| **Context switch** | ⚠️ Naked asm exists | Need scheduler integration for user processes |
| **File operations** | ⚠️ open/read/write/close/lseek exist | Need per-process FD table, dup/dup2, pipe |
| **mmap** | ❌ Stub (returns OutOfMemory) | Core requirement for ELF loading |
| **brk/sbrk** | ❌ Stub (returns placeholder) | Needed for heap allocation |
| **Signals** | ❌ Not implemented | Need basic signal delivery (SIGTERM, SIGKILL) |
| **fork/exec** | ❌ Stubs | Not strictly required (static binaries), but exec needed for BusyBox |

---

## Sub-Phase 7-4.1: User-Space Execution Foundation

> **Goal:** Enable the kernel to create a user-space address space, load ELF segments into it, and transfer control to user-mode code.  
> **Duration:** 1-1.5 weeks  
> **Dependencies:** None (builds on existing `loader/`, `memory/`, `process/` modules)

### Jobs

#### Job 1: GDT User-Mode Segment Descriptors

Currently the GDT only has kernel code/data segments + TSS. User-mode execution requires Ring 3 segments.

**Tasks:**
- [ ] **T1.1** Add user code segment descriptor (CS = 0x1B = index 3, RPL 3) to `kernel/src/gdt.rs`
- [ ] **T1.2** Add user data segment descriptor (SS = 0x23 = index 4, RPL 3) to `kernel/src/gdt.rs`
- [ ] **T1.3** Verify segment selectors match `STAR` MSR configuration in `syscall/mod.rs` (already: user CS=0x1B, SS=0x23)
- [ ] **T1.4** Add TSS kernel stack pointer (RSP0) update on context switch — required for Ring 3 → Ring 0 transitions
- [ ] **T1.5** Unit test: GDT loads without triple-fault, segment selectors are valid

#### Job 2: Per-Process Page Table Creation

Each user process needs its own page table (CR3) with the kernel half-space mapped identically.

**Tasks:**
- [ ] **T2.1** Implement `create_user_page_table() → (PhysFrame, OffsetPageTable)`: allocate P4 frame, copy kernel entries (indices 256-511 from current CR3)
- [ ] **T2.2** Implement `map_user_page(page_table, virt_addr, phys_frame, flags)`: map a single 4KB page with USER_ACCESSIBLE + given flags
- [ ] **T2.3** Implement `map_user_range(page_table, virt_start, phys_start, size, flags)`: map contiguous range
- [ ] **T2.4** Implement `unmap_user_page(page_table, virt_addr)`: unmap + free physical frame
- [ ] **T2.5** Implement `destroy_user_page_table(cr3_frame)`: recursively free all user-space page table frames (indices 0-255)
- [ ] **T2.6** Unit test: create page table, map a page at 0x400000, write data, read back, unmap, destroy

#### Job 3: ELF Segment Memory Loading

Connect the existing ELF parser to actual memory mapping.

**Tasks:**
- [ ] **T3.1** Implement `load_elf_segments(elf: &LoadedProgram, page_table) → Result<()>`: for each PT_LOAD segment, allocate frames and map at segment vaddr with correct permissions (R/W/X)
- [ ] **T3.2** Handle PIE binaries: apply random base offset within the `PIE_BASE` range (0x5555_5555_0000)
- [ ] **T3.3** Implement BSS zero-initialization: for segments where `memsz > filesz`, allocate extra zeroed pages
- [ ] **T3.4** Set up user stack: allocate 8MB stack region below `USER_STACK_TOP` (0x7FFF_FFFF_F000), push argc/argv/envp/auxv per x86_64 ABI
- [ ] **T3.5** Set up initial heap break at first page after last loaded segment (`brk_start`)
- [ ] **T3.6** Integration test: load a minimal ELF (handcrafted), verify memory contents match expected segments

#### Job 4: User-Mode Entry and Return

Wire up the actual Ring 0 → Ring 3 transition.

**Tasks:**
- [ ] **T4.1** Create `LinuxProcess` struct: holds CR3, entry point, stack pointer, brk position, FD table, PID, working directory, argv/envp
- [ ] **T4.2** Implement `launch_linux_process(elf_bytes, args, env) → Result<ProcessHandle>`: parse ELF → create page table → load segments → setup stack → create LinuxProcess → add to scheduler
- [ ] **T4.3** Implement scheduler task type `LinuxProcess`: on schedule, switch CR3 to process page table, then `iretq` to user-space entry point
- [ ] **T4.4** Implement CR3 switching: save/restore CR3 on context switch between processes
- [ ] **T4.5** Verify W^X enforcement: code pages are R+X (no W), data pages are R+W (no X)
- [ ] **T4.6** Integration test: load a minimal static binary that executes `syscall(exit, 42)`, verify exit code is 42

### Quality Gate 7-4.1

| # | Criteria | Verification |
|---|----------|-------------|
| QG1.1 | GDT has Ring 3 code/data segments and loads without faults | Boot test |
| QG1.2 | Per-process page table can be created with kernel half-space | Unit test: `create_user_page_table` succeeds |
| QG1.3 | User pages can be mapped/unmapped at arbitrary addresses | Unit test: map at 0x400000, read/write, unmap |
| QG1.4 | ELF segments are loaded into correct virtual addresses | Integration test: verify memory contents |
| QG1.5 | A minimal "exit(42)" static binary runs and returns correct exit code | E2E test |
| QG1.6 | All tests pass: `cargo test -p kpio-kernel --lib` | CI gate |

---

## Sub-Phase 7-4.2: Syscall Entry & Core File I/O

> **Goal:** Implement proper Linux syscall entry/exit mechanism and the most essential file I/O syscalls.  
> **Duration:** 1-1.5 weeks  
> **Dependencies:** Sub-Phase 7-4.1 (user-space execution working)

### Jobs

#### Job 5: Linux Syscall Entry/Exit Handler

The current `syscall_entry()` is a stub. A real handler must save all registers, switch stacks, and dispatch.

**Tasks:**
- [ ] **T5.1** Implement `linux_syscall_entry` as `naked` function:
  ```
  swapgs                    # Switch to kernel GS base (for per-CPU data)
  mov [gs:rsp_scratch], rsp # Save user RSP
  mov rsp, [gs:kernel_rsp]  # Load kernel stack
  push user registers       # Save RCX (return RIP), R11 (return RFLAGS), + all GPRs
  mov rdi, rax              # syscall number
  mov rsi, rdi_orig         # arg1 (rdi), arg2 (rsi), arg3 (rdx), arg4 (r10→rcx), arg5 (r8), arg6 (r9)
  call linux_syscall_dispatch
  pop user registers
  mov rsp, [gs:rsp_scratch]
  swapgs
  sysretq
  ```
- [ ] **T5.2** Set up per-CPU data area (`GS base` via `KERNEL_GS_BASE` MSR) with kernel stack pointer
- [ ] **T5.3** Update LSTAR MSR to point to `linux_syscall_entry` (for Linux processes; keep KPIO entry for KPIO tasks)
- [ ] **T5.4** Implement `linux_syscall_dispatch(nr, arg1..arg6) → i64`: routing table from Linux x86_64 syscall numbers to handler functions
- [ ] **T5.5** Implement errno mapping: KPIO error codes → Linux negative errno values (-ENOENT, -EACCES, -ENOMEM, etc.)
- [ ] **T5.6** Unsupported syscall fallback: log syscall number + return `-ENOSYS` (38)
- [ ] **T5.7** Unit test: syscall dispatch routes known numbers correctly; unknown returns ENOSYS

#### Job 6: Per-Process File Descriptor Table

The current global FD table must be replaced with per-process tables for Linux compatibility.

**Tasks:**
- [ ] **T6.1** Create `ProcessFdTable` struct: per-process FD table (max 256 FDs), with reference-counted file descriptions
- [ ] **T6.2** Auto-open fd 0 (stdin), 1 (stdout), 2 (stderr) on process creation → serial port
- [ ] **T6.3** Implement `alloc_fd() → fd`, `get_file(fd) → &File`, `close_fd(fd)`
- [ ] **T6.4** Integrate `ProcessFdTable` into `LinuxProcess` struct
- [ ] **T6.5** Route all file syscalls through the process's own FD table (not the global one)
- [ ] **T6.6** Unit test: each process has independent FD number space

#### Job 7: Core File I/O Syscalls (Linux Numbers)

Implement 9 file I/O syscalls using the existing VFS layer:

| Linux # | Name | Mapping |
|---------|------|---------|
| 0 | `read(fd, buf, count)` | VFS `fd::read` |
| 1 | `write(fd, buf, count)` | VFS `fd::write` |
| 2 | `open(path, flags, mode)` | VFS `fd::open` |
| 3 | `close(fd)` | VFS `fd::close` |
| 4 | `stat(path, statbuf)` | VFS `stat` |
| 5 | `fstat(fd, statbuf)` | VFS `stat` via fd path |
| 8 | `lseek(fd, offset, whence)` | VFS `fd::lseek` |
| 21 | `access(path, mode)` | VFS `stat` + permission check |
| 257 | `openat(dirfd, path, flags, mode)` | VFS `open` (dirfd=AT_FDCWD only) |

**Tasks:**
- [ ] **T7.1** Implement `sys_read(fd, user_buf_ptr, count) → ssize_t`: copy from VFS to user-space buffer (validate user pointer!)
- [ ] **T7.2** Implement `sys_write(fd, user_buf_ptr, count) → ssize_t`: copy from user-space to VFS (validate user pointer!)
- [ ] **T7.3** Implement `sys_open(user_path_ptr, flags, mode) → fd`: translate Linux O_* flags to KPIO flags, allocate FD
- [ ] **T7.4** Implement `sys_close(fd) → 0`: close FD in process table
- [ ] **T7.5** Implement `sys_stat(user_path_ptr, user_statbuf_ptr)` and `sys_fstat(fd, user_statbuf_ptr)`: fill Linux `struct stat` layout
- [ ] **T7.6** Implement `sys_lseek(fd, offset, whence) → off_t`
- [ ] **T7.7** Implement `sys_access(user_path_ptr, mode) → 0/-ENOENT`: check file existence
- [ ] **T7.8** Implement `sys_openat(dirfd, user_path_ptr, flags, mode) → fd`: AT_FDCWD support; other dirfd → ENOSYS
- [ ] **T7.9** Implement user-space pointer validation: `validate_user_ptr(ptr, len)` — ensure address is in user range (< 0x0000_8000_0000_0000) and mapped
- [ ] **T7.10** Integration test: static binary opens a file, writes "hello", reads it back, verifies content

### Quality Gate 7-4.2

| # | Criteria | Verification |
|---|----------|-------------|
| QG2.1 | `syscall` instruction from user-space correctly enters kernel handler | Integration test with exit() binary |
| QG2.2 | Linux syscall numbers route to correct handlers | Unit test: dispatch table coverage |
| QG2.3 | Unknown syscalls return -ENOSYS without crashing | Test with syscall(999) |
| QG2.4 | `write(1, "hello", 5)` outputs "hello" to serial console | E2E test |
| QG2.5 | `open` + `write` + `close` + `open` + `read` round-trip works | Integration test |
| QG2.6 | Per-process FDs are isolated (fd 3 in process A ≠ fd 3 in process B) | Unit test |
| QG2.7 | Invalid user pointers are rejected (not dereferenced in kernel) | Security test |

---

## Sub-Phase 7-4.3: Memory Management Syscalls

> **Goal:** Implement `mmap`, `mprotect`, `munmap`, and `brk` — the foundation for heap allocation and dynamic memory in Linux binaries.  
> **Duration:** 1 week  
> **Dependencies:** Sub-Phase 7-4.1 (page table mapping functions)

### Jobs

#### Job 8: brk/sbrk Heap Management

The simplest memory allocation mechanism. Most static musl binaries use `brk` for malloc.

**Tasks:**
- [ ] **T8.1** Track `brk_current` per process (initialized after last ELF segment)
- [ ] **T8.2** Implement `sys_brk(new_brk) → current_brk`:
  - If `new_brk == 0`: return current brk
  - If `new_brk > current`: allocate and map new pages (R+W, NX) between old and new brk
  - If `new_brk < current`: unmap and free pages between new and old brk
  - Enforce max heap size (256 MB default)
- [ ] **T8.3** Page-align brk operations (always operate on 4KB boundaries)
- [ ] **T8.4** Unit test: brk(0) returns initial brk; brk(initial+4096) allocates one page; write/read succeeds

#### Job 9: mmap Implementation

Required for larger allocations, file mapping, and stack guard pages.

**Tasks:**
- [ ] **T9.1** Implement per-process VMA (Virtual Memory Area) list: track all mapped regions `[start, end, flags, backing]`
- [ ] **T9.2** Implement `sys_mmap(addr, length, prot, flags, fd, offset) → addr`:
  - `MAP_ANONYMOUS | MAP_PRIVATE`: allocate zeroed pages at addr (or find free range if addr=0)
  - `MAP_FIXED`: map at exact address (unmap existing if needed)
  - `MAP_ANONYMOUS` only for Phase 7-4 (no file-backed mmap)
  - `MAP_PRIVATE | MAP_FIXED` + fd: load file content into pages (stretch goal)
- [ ] **T9.3** Implement free-range finder: scan VMA list for gap of sufficient size (start from 0x7F0000000000 downward, like Linux)
- [ ] **T9.4** Implement `sys_munmap(addr, length) → 0`: unmap pages, remove VMA, free frames
- [ ] **T9.5** Implement `sys_mprotect(addr, length, prot) → 0`: change page permissions (R/W/X flags)
- [ ] **T9.6** Translate Linux prot flags: `PROT_READ=1`, `PROT_WRITE=2`, `PROT_EXEC=4` → x86_64 page table flags
- [ ] **T9.7** Translate Linux map flags: `MAP_PRIVATE=0x02`, `MAP_ANONYMOUS=0x20`, `MAP_FIXED=0x10`
- [ ] **T9.8** Integration test: mmap anonymous region, write pattern, verify; munmap; access → page fault

### Quality Gate 7-4.3

| # | Criteria | Verification |
|---|----------|-------------|
| QG3.1 | `brk(0)` returns valid address after last segment | Unit test |
| QG3.2 | `brk(current + N*4096)` allocates N pages, writable | Integration test |
| QG3.3 | `mmap(NULL, 4096, PROT_READ|PROT_WRITE, MAP_ANONYMOUS|MAP_PRIVATE, -1, 0)` returns usable address | Integration test |
| QG3.4 | `munmap` frees memory (subsequent access faults) | Page fault test |
| QG3.5 | `mprotect` can make a page read-only (write → fault) | Protection test |
| QG3.6 | musl `malloc`/`free` works in a static binary (uses brk + mmap internally) | E2E test with C binary |

---

## Sub-Phase 7-4.4: Process, Directory, and System Syscalls

> **Goal:** Implement process management, directory operations, and system information syscalls needed for common CLI tools.  
> **Duration:** 1 week  
> **Dependencies:** Sub-Phase 7-4.2 (syscall dispatch working)

### Jobs

#### Job 10: Process & System Identity Syscalls

| Linux # | Name | Implementation |
|---------|------|---------------|
| 39 | `getpid()` | Return process ID |
| 60 | `exit(status)` | Terminate process, set exit code |
| 231 | `exit_group(status)` | Same as exit (single-threaded) |
| 63 | `uname(buf)` | Fill `struct utsname` with KPIO info |
| 158 | `arch_prctl(code, addr)` | `ARCH_SET_FS` → set FS base (TLS), others → ENOSYS |
| 218 | `set_tid_address(tidptr)` | Store pointer, return tid (stub) |
| 273 | `set_robust_list(head, len)` | No-op, return 0 (stub) |
| 102 | `getuid()` | Return 0 (root) |
| 104 | `getgid()` | Return 0 (root) |
| 107 | `geteuid()` | Return 0 (root) |
| 108 | `getegid()` | Return 0 (root) |

**Tasks:**
- [ ] **T10.1** Implement `sys_getpid() → pid`: return LinuxProcess PID
- [ ] **T10.2** Implement `sys_exit(status)`: clean up process (free page table, close FDs, remove from scheduler), store exit code
- [ ] **T10.3** Implement `sys_exit_group(status)`: alias to sys_exit (no threading yet)
- [ ] **T10.4** Implement `sys_uname(user_buf)`: fill `struct utsname` fields:
  - sysname: `"KPIO"`
  - nodename: `"kpio"`  
  - release: `"2.1.0"`
  - version: `"Phase 7-4"`
  - machine: `"x86_64"`
- [ ] **T10.5** Implement `sys_arch_prctl(code, addr)`:
  - `ARCH_SET_FS (0x1002)`: write addr to FS base MSR (0xC0000100) — required for TLS in static binaries
  - `ARCH_GET_FS (0x1003)`: read FS base
  - Others: return -ENOSYS
- [ ] **T10.6** Implement `sys_set_tid_address(tidptr) → tid` and `sys_set_robust_list(head, len) → 0` as stubs
- [ ] **T10.7** Implement `sys_getuid/getgid/geteuid/getegid → 0` (single-user system, always root)
- [ ] **T10.8** Unit test: uname fills correct strings; arch_prctl ARCH_SET_FS sets MSR

#### Job 11: Directory Operations Syscalls

| Linux # | Name | Implementation |
|---------|------|---------------|
| 79 | `getcwd(buf, size)` | Return process working directory |
| 80 | `chdir(path)` | Change process working directory |
| 83 | `mkdir(path, mode)` | Create directory in VFS |
| 87 | `unlink(path)` | Delete file |
| 89 | `readlink(path, buf, size)` | Read symlink target (or /proc/self/exe → binary path) |
| 267 | `readlinkat(dirfd, path, buf, size)` | Same with dirfd (AT_FDCWD only) |
| 78 | `getdents64(fd, dirp, count)` | Read directory entries |

**Tasks:**
- [ ] **T11.1** Add `cwd: String` field to `LinuxProcess` (default: `"/"`)
- [ ] **T11.2** Implement `sys_getcwd(user_buf, size) → len`: copy cwd to user buffer
- [ ] **T11.3** Implement `sys_chdir(user_path)`: validate path exists in VFS, update process cwd
- [ ] **T11.4** Implement `sys_mkdir(user_path, mode) → 0`: create directory via VFS
- [ ] **T11.5** Implement `sys_unlink(user_path) → 0`: remove file via VFS
- [ ] **T11.6** Implement `sys_readlink(user_path, user_buf, size) → len`:
  - Special case: `/proc/self/exe` → return binary path
  - Otherwise: EINVAL (no symlink support)
- [ ] **T11.7** Implement `sys_getdents64(fd, user_dirp, count) → bytes_read`: read directory entries from VFS, fill Linux `struct linux_dirent64` structs
- [ ] **T11.8** Integration test: mkdir + chdir + getcwd round-trip; getdents64 lists directory contents

#### Job 12: Basic Signal Handling

Minimal signal support for process termination:

**Tasks:**
- [ ] **T12.1** Define signal constants: `SIGTERM=15`, `SIGKILL=9`, `SIGINT=2`, `SIGABRT=6`
- [ ] **T12.2** Implement `sys_kill(pid, sig)`:
  - `SIGKILL`/`SIGTERM` → terminate target process
  - `SIGINT` → terminate (no handler support)
  - Other signals → ignore
- [ ] **T12.3** Implement default signal actions for unhandled signals (terminate, ignore, or core dump → terminate)
- [ ] **T12.4** `sys_rt_sigaction`, `sys_rt_sigprocmask` → stubs returning 0 (musl init calls these)
- [ ] **T12.5** Unit test: kill(pid, SIGKILL) terminates process

### Quality Gate 7-4.4

| # | Criteria | Verification |
|---|----------|-------------|
| QG4.1 | `getpid()` returns unique PID per process | Unit test |
| QG4.2 | `exit(42)` terminates process with exit code 42, resources freed | Integration test |
| QG4.3 | `uname()` fills all fields correctly | Integration test |
| QG4.4 | `arch_prctl(ARCH_SET_FS, addr)` sets FS base (TLS works) | Unit test |
| QG4.5 | `getcwd` + `chdir` + `mkdir` work correctly | Integration test |
| QG4.6 | `getdents64` returns correct directory listing | Integration test |
| QG4.7 | `kill(pid, SIGKILL)` terminates target process | Unit test |

---

## Sub-Phase 7-4.5: Extended I/O, Pipes, and Time Syscalls

> **Goal:** Implement pipe, file descriptor manipulation, and time syscalls to support shell-like utilities and time-dependent operations.  
> **Duration:** 1 week  
> **Dependencies:** Sub-Phase 7-4.2 (FD table), Sub-Phase 7-4.4 (process syscalls)

### Jobs

#### Job 13: Pipe and File Descriptor Syscalls

| Linux # | Name | Implementation |
|---------|------|---------------|
| 22 | `pipe(pipefd[2])` | Create pipe, return read/write FDs |
| 32 | `dup(oldfd)` | Duplicate FD |
| 33 | `dup2(oldfd, newfd)` | Duplicate FD to specific number |
| 72 | `fcntl(fd, cmd, arg)` | FD control (F_GETFL, F_SETFL, F_DUPFD) |
| 16 | `ioctl(fd, request, ...)` | Stub: TIOCGWINSZ (terminal size) only |

**Tasks:**
- [ ] **T13.1** Implement kernel pipe: circular buffer (4KB), blocking read when empty, blocking write when full
- [ ] **T13.2** Implement `sys_pipe(user_pipefd_ptr) → 0`: create pipe, allocate 2 FDs (read + write ends), write to user array
- [ ] **T13.3** Implement `sys_dup(oldfd) → newfd`: clone file description to lowest available FD
- [ ] **T13.4** Implement `sys_dup2(oldfd, newfd) → newfd`: clone file description to specific FD (close newfd if open)
- [ ] **T13.5** Implement `sys_fcntl(fd, cmd, arg)`:
  - `F_GETFL (3)`: return file status flags
  - `F_SETFL (4)`: set file status flags (O_NONBLOCK)
  - `F_DUPFD (0)`: duplicate to >= arg
  - Others: return -ENOSYS
- [ ] **T13.6** Implement `sys_ioctl(fd, request, arg)`:
  - `TIOCGWINSZ (0x5413)`: return terminal window size (80x25 default)
  - Others: return -ENOTTY
- [ ] **T13.7** Integration test: pipe creation, write "hello" to write-end, read from read-end

#### Job 14: Time Syscalls

| Linux # | Name | Implementation |
|---------|------|---------------|
| 96 | `gettimeofday(tv, tz)` | Current time (since boot) |
| 35 | `nanosleep(req, rem)` | Sleep for duration |
| 228 | `clock_gettime(clockid, tp)` | Monotonic/realtime clock |

**Tasks:**
- [ ] **T14.1** Implement `sys_gettimeofday(user_tv, user_tz)`: fill `struct timeval` with seconds and microseconds (from boot ticks × 10ms)
- [ ] **T14.2** Implement `sys_nanosleep(user_req, user_rem)`: read `struct timespec`, sleep for specified duration using scheduler, write remaining time
- [ ] **T14.3** Implement `sys_clock_gettime(clockid, user_tp)`:
  - `CLOCK_MONOTONIC (1)`: ticks since boot
  - `CLOCK_REALTIME (0)`: same as monotonic (no RTC yet)
  - `CLOCK_PROCESS_CPUTIME_ID (2)`: return 0 (stub)
- [ ] **T14.4** Implement `struct timespec` and `struct timeval` layout matching Linux x86_64
- [ ] **T14.5** Unit test: clock_gettime returns increasing values; nanosleep delays execution

#### Job 15: Remaining Tier A Stubs

Syscalls that musl/static binaries call during init but don't need full implementation:

**Tasks:**
- [ ] **T15.1** `sys_madvise(addr, len, advice) → 0`: no-op (advisory only)
- [ ] **T15.2** `sys_futex(uaddr, op, val, ...) → -ENOSYS` or minimal `FUTEX_WAIT`/`FUTEX_WAKE` stubs
- [ ] **T15.3** `sys_prlimit64(pid, resource, new, old) → 0`: return default limits (stub)
- [ ] **T15.4** `sys_getrandom(buf, buflen, flags)`: fill buffer with random bytes from KPIO's CSPRNG
- [ ] **T15.5** `sys_writev(fd, iov, iovcnt) → bytes`: vectored write (iterate iov array, write each to fd)
- [ ] **T15.6** `sys_readv(fd, iov, iovcnt) → bytes`: vectored read
- [ ] **T15.7** Log list of all stubs called during binary execution for debugging

### Quality Gate 7-4.5

| # | Criteria | Verification |
|---|----------|-------------|
| QG5.1 | `pipe` + `write` + `read` passes data correctly | Integration test |
| QG5.2 | `dup2(fd, 1)` redirects stdout to file | Integration test |
| QG5.3 | `ioctl(TIOCGWINSZ)` returns 80x25 | Unit test |
| QG5.4 | `clock_gettime(CLOCK_MONOTONIC)` returns monotonically increasing values | Unit test |
| QG5.5 | `nanosleep({0, 100_000_000})` sleeps approximately 100ms | Timing test |
| QG5.6 | `getrandom` fills buffer with non-zero random bytes | Unit test |
| QG5.7 | `writev` with multiple iov entries writes all data | Integration test |

---

## Sub-Phase 7-4.6: Integration Testing & Compatibility Validation

> **Goal:** Validate the entire Linux compatibility layer with real-world static binaries and document the compatibility matrix.  
> **Duration:** 1-1.5 weeks  
> **Dependencies:** Sub-Phases 7-4.1 through 7-4.5 complete

### Jobs

#### Job 16: Test Binary Preparation

**Tasks:**
- [ ] **T16.1** Build BusyBox (static musl, x86_64): `make defconfig && make LDFLAGS=-static CC=musl-gcc`
- [ ] **T16.2** Build minimal C test binary (musl-gcc, static): hello world, file I/O, directory listing, pipe
- [ ] **T16.3** Build Rust test binary (`--target x86_64-unknown-linux-musl`): hello world, file I/O, argument parsing
- [ ] **T16.4** Build Go test binary (`CGO_ENABLED=0 go build`): hello world, basic I/O
- [ ] **T16.5** Load test binaries into KPIO's VFS (embed in kernel image or load via initrd-like mechanism)

#### Job 17: BusyBox Compatibility Testing

BusyBox is the primary compatibility target — a single static binary providing 100+ Unix utilities.

**Tasks:**
- [ ] **T17.1** Run `busybox echo hello` → verify "hello" output
- [ ] **T17.2** Run `busybox cat /test.txt` → verify file content output
- [ ] **T17.3** Run `busybox ls /` → verify directory listing
- [ ] **T17.4** Run `busybox wc /test.txt` → verify line/word/byte count
- [ ] **T17.5** Run `busybox head -n 3 /test.txt` → verify first 3 lines
- [ ] **T17.6** Run `busybox tail -n 3 /test.txt` → verify last 3 lines
- [ ] **T17.7** Run `busybox grep pattern /test.txt` → verify matching lines
- [ ] **T17.8** Run `busybox sort /test.txt` → verify sorted output
- [ ] **T17.9** Run `busybox mkdir /testdir && busybox ls /` → verify directory creation
- [ ] **T17.10** Run `busybox tr 'a-z' 'A-Z' < /test.txt` → verify stream transformation
- [ ] **T17.11** Document which BusyBox applets work/fail with failure reasons

#### Job 18: Cross-Language Binary Testing

**Tasks:**
- [ ] **T18.1** C binary: prints args, reads/writes files, calls getpid/getuid → all correct
- [ ] **T18.2** Rust binary: string processing, file I/O, error handling → runs correctly
- [ ] **T18.3** Go binary: goroutine-less hello world, basic I/O → runs correctly (Go may need futex/clone stubs)
- [ ] **T18.4** Test binaries with multiple syscalls in sequence (realistic usage patterns)
- [ ] **T18.5** Stress test: binary allocating large memory via brk/mmap → no kernel panic

#### Job 19: Compatibility Matrix Documentation

**Tasks:**
- [ ] **T19.1** Create `docs/architecture/linux-compat.md` documenting:
  - Supported Linux x86_64 syscalls (full list with numbers)
  - Partially supported syscalls (stub behavior documented)
  - Unsupported syscalls (with rationale)
  - Known limitations
- [ ] **T19.2** Create compatibility matrix table:

  | Binary Type | Status | Notes |
  |-------------|--------|-------|
  | C (musl static) | ✅ Supported | Tested with BusyBox |
  | Rust (musl static) | ✅ Supported | `--target x86_64-unknown-linux-musl` |
  | Go (static) | ⚠️ Partial | Needs futex stubs for goroutine runtime |
  | C (glibc dynamic) | ❌ Not supported | Requires dynamic linker |
  | Any (dynamic) | ❌ Not supported | Static linking required |

- [ ] **T19.3** Document recommended build commands for each language
- [ ] **T19.4** Add Phase 7-4 completion status to root README.md and roadmap.md

#### Job 20: Syscall Tracing and Debugging

**Tasks:**
- [ ] **T20.1** Implement syscall trace mode: log every syscall (number, args, return value) to serial console
- [ ] **T20.2** Implement unknown syscall report: on first encounter, print human-readable name (from table)
- [ ] **T20.3** Implement syscall statistics: count per-syscall invocations for profiling
- [ ] **T20.4** Add `--linux-trace` boot flag to enable/disable syscall tracing

### Quality Gate 7-4.6

| # | Criteria | Verification |
|---|----------|-------------|
| QG6.1 | BusyBox `echo`, `cat`, `ls`, `wc`, `grep` all produce correct output | E2E tests |
| QG6.2 | At least 10 BusyBox applets work correctly | Test matrix |
| QG6.3 | Static Rust binary (hello + file I/O) runs correctly | E2E test |
| QG6.4 | Static C binary (musl) runs correctly | E2E test |
| QG6.5 | No kernel panics during any binary execution | Stability test |
| QG6.6 | `linux-compat.md` documents all syscalls with status | Doc review |
| QG6.7 | Syscall trace mode logs all invocations for debugging | Manual verification |
| QG6.8 | All tests pass: `cargo test -p kpio-kernel --lib` + E2E suite | CI gate |

---

## Full Syscall Implementation Matrix

### Tier A — Required (~45 syscalls)

| # | Syscall | Sub-Phase | Category | Notes |
|---|---------|-----------|----------|-------|
| 0 | `read` | 7-4.2 | File I/O | |
| 1 | `write` | 7-4.2 | File I/O | |
| 2 | `open` | 7-4.2 | File I/O | |
| 3 | `close` | 7-4.2 | File I/O | |
| 4 | `stat` | 7-4.2 | File I/O | |
| 5 | `fstat` | 7-4.2 | File I/O | |
| 8 | `lseek` | 7-4.2 | File I/O | |
| 9 | `mmap` | 7-4.3 | Memory | Anonymous only |
| 10 | `mprotect` | 7-4.3 | Memory | |
| 11 | `munmap` | 7-4.3 | Memory | |
| 12 | `brk` | 7-4.3 | Memory | |
| 13 | `rt_sigaction` | 7-4.4 | Signal | Stub (return 0) |
| 14 | `rt_sigprocmask` | 7-4.4 | Signal | Stub (return 0) |
| 16 | `ioctl` | 7-4.5 | I/O | TIOCGWINSZ only |
| 20 | `writev` | 7-4.5 | File I/O | Vectored write |
| 21 | `access` | 7-4.2 | File I/O | |
| 22 | `pipe` | 7-4.5 | Pipe/FD | |
| 28 | `madvise` | 7-4.5 | Memory | Stub (no-op) |
| 32 | `dup` | 7-4.5 | Pipe/FD | |
| 33 | `dup2` | 7-4.5 | Pipe/FD | |
| 35 | `nanosleep` | 7-4.5 | Time | |
| 39 | `getpid` | 7-4.4 | Process | |
| 57 | `fork` | — | Process | Not impl (return -ENOSYS) |
| 59 | `execve` | — | Process | Stretch goal |
| 60 | `exit` | 7-4.4 | Process | |
| 61 | `wait4` | — | Process | Stub for single-process |
| 62 | `kill` | 7-4.4 | Signal | SIGKILL/SIGTERM only |
| 63 | `uname` | 7-4.4 | System | |
| 72 | `fcntl` | 7-4.5 | Pipe/FD | F_GETFL/F_SETFL/F_DUPFD |
| 78 | `getdents64` | 7-4.4 | Directory | |
| 79 | `getcwd` | 7-4.4 | Directory | |
| 80 | `chdir` | 7-4.4 | Directory | |
| 83 | `mkdir` | 7-4.4 | Directory | |
| 87 | `unlink` | 7-4.4 | Directory | |
| 89 | `readlink` | 7-4.4 | Directory | /proc/self/exe |
| 96 | `gettimeofday` | 7-4.5 | Time | |
| 102 | `getuid` | 7-4.4 | Identity | Returns 0 |
| 104 | `getgid` | 7-4.4 | Identity | Returns 0 |
| 107 | `geteuid` | 7-4.4 | Identity | Returns 0 |
| 108 | `getegid` | 7-4.4 | Identity | Returns 0 |
| 158 | `arch_prctl` | 7-4.4 | System | ARCH_SET_FS/GET_FS |
| 218 | `set_tid_address` | 7-4.4 | Threading | Stub |
| 228 | `clock_gettime` | 7-4.5 | Time | MONOTONIC/REALTIME |
| 231 | `exit_group` | 7-4.4 | Process | Same as exit |
| 257 | `openat` | 7-4.2 | File I/O | AT_FDCWD only |
| 267 | `readlinkat` | 7-4.4 | Directory | AT_FDCWD only |
| 273 | `set_robust_list` | 7-4.4 | Threading | Stub |
| 302 | `prlimit64` | 7-4.5 | System | Stub |
| 318 | `getrandom` | 7-4.5 | System | CSPRNG |

### Tier B — Nice to Have (future phases)

| # | Syscall | Notes |
|---|---------|-------|
| 56 | `clone` | Process/thread creation |
| 59 | `execve` | Process replacement |
| 61 | `wait4` | Wait for child process |
| 41 | `socket` | Network sockets |
| 42-49 | `connect/accept/send/recv/...` | Network I/O |
| 17 | `pread64` | Positioned read |
| 18 | `pwrite64` | Positioned write |
| 19 | `readv` | Vectored read |
| 76 | `truncate` | File truncation |
| 77 | `ftruncate` | File truncation (by fd) |
| 82 | `rename` | File rename |
| 90 | `chmod` | File permissions |
| 91 | `fchmod` | File permissions (by fd) |
| 92 | `chown` | File ownership |

---

## Implementation Timeline

```
Week     1       2       3       4       5       6       7
        ├───────┤───────┤───────┤───────┤───────┤───────┤
 7-4.1  ████████████████                                    User-Space Foundation
 7-4.2          ████████████████                            Syscall Entry & File I/O
 7-4.3                  ████████████                        Memory Syscalls
 7-4.4                          ████████████                Process & Dir Syscalls
 7-4.5                                  ████████████        Extended I/O & Time
 7-4.6                                          ████████████ Integration & Validation
```

> Sub-phases are sequential — each builds on the previous.  
> 7-4.3 and 7-4.4 could partially overlap since they're independent domain areas.

---

## New Module Structure

```
kernel/src/
  elf/                          # NEW — Phase 7-4
    mod.rs                      # Module root, re-exports
    linux_process.rs            # LinuxProcess struct, per-process state
    user_memory.rs              # Per-process page table, VMA list, map/unmap
    syscall_entry.rs            # naked asm: swapgs, register save, dispatch, sysretq
    syscall_router.rs           # Linux syscall number → handler function dispatch table
    syscall_file.rs             # read, write, open, close, stat, fstat, lseek, access, openat
    syscall_memory.rs           # mmap, munmap, mprotect, brk
    syscall_process.rs          # getpid, exit, exit_group, uname, arch_prctl, kill
    syscall_dir.rs              # getcwd, chdir, mkdir, unlink, readlink, getdents64
    syscall_fd.rs               # pipe, dup, dup2, fcntl, ioctl
    syscall_time.rs             # gettimeofday, nanosleep, clock_gettime
    syscall_stubs.rs            # All stub syscalls (madvise, futex, prlimit64, etc.)
    errno.rs                    # Linux errno constants and error mapping
    linux_types.rs              # struct stat, struct timespec, struct timeval, etc.
    fd_table.rs                 # Per-process file descriptor table
    pipe.rs                     # Kernel pipe implementation
    signal.rs                   # Basic signal handling
    trace.rs                    # Syscall tracing and debugging
```

---

## Risk Assessment

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| ELF loader memory safety (malicious ELF) | 🔴 High | Medium | Strict segment bounds checking, ASLR, W^X enforcement, user pointer validation |
| Syscall number coverage gap (binary needs unsupported syscall) | 🟡 Medium | High | ENOSYS fallback + trace logging; iteratively add syscalls as needed |
| musl libc init sequence complexity | 🟡 Medium | Medium | Trace musl startup, add stubs for init-only syscalls (arch_prctl, set_tid_address, etc.) |
| Go runtime requiring clone/futex (goroutines) | 🟡 Medium | High | Limit Go to `GOMAXPROCS=1` + minimal futex stubs; full support deferred |
| Page table / CR3 switch bugs causing triple-fault | 🔴 High | Medium | Test in QEMU with `-d int`, always map kernel half-space, extensive unit tests |
| Performance overhead from syscall translation layer | 🟢 Low | Low | Direct dispatch (no interpretation), same as native KPIO syscall overhead |

---

## Success Criteria Summary

### Must Achieve (Phase 7-4 is considered complete when all are met)
- [ ] Static musl-linked ELF binary loads and executes in user-space (Ring 3)
- [ ] write(1, "hello", 5) outputs to serial console
- [ ] File I/O round-trip (open → write → close → open → read → verify)
- [ ] brk + mmap memory allocation works (musl malloc functions)
- [ ] BusyBox `echo`, `cat`, `ls` produce correct output (minimum 3 applets)
- [ ] Process isolation: crash in one Linux process doesn't affect kernel or other processes
- [ ] All existing kernel tests continue to pass

### Should Achieve
- [ ] 10+ BusyBox applets working
- [ ] Static Rust binary runs correctly
- [ ] Syscall trace mode for debugging
- [ ] Compatibility matrix documentation complete

### Nice to Have (Phase 7-5+)
- [ ] Static Go binary runs (with futex stubs)
- [ ] execve support (one binary launching another)
- [ ] Pipe-based inter-process communication (shell pipes)
- [ ] fork/exec for simple shell operations

---

*This document is the detailed implementation plan for Phase 7-4 of KPIO OS. Each sub-phase should be committed as a unit with associated tests passing all quality gates before proceeding to the next.*
