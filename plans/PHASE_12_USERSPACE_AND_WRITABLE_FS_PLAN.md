# Phase 12: Real User-Space Execution & Writable Filesystem — Implementation Plan

Complete the user-space execution pipeline (execve return path, process spawn, init process)
and implement FAT32 write support so that the kernel can load programs from disk, execute
them in Ring 3, and persist data back to the block device.

**Dependencies:** Phase 11 (Kernel Hardening) ✅ complete.

**Codebase baseline (verified 2026-03-05):**

| Component | Current State |
|-----------|---------------|
| ELF loader (`kernel/src/loader/`) | Parses ELF64, loads PT_LOAD segments, sets up user stack. Works for embedded test binaries. |
| `sys_execve` (`syscall/linux_handlers.rs`) | Reads ELF from VFS, loads segments, but **does not modify the SYSCALL return frame** — process never jumps to new entry point. argv/envp parsing missing. |
| `fork()` (`syscall/linux_handlers.rs`) | CoW page table clone works, but child starts at parent's ELF entry rather than fork return point — **saved register state not copied to child**. |
| `ProcessManager::spawn()` (`process/manager.rs`) | Creates metadata only — CR3=0, kernel stack unallocated, segments not loaded via `segment_loader`. |
| VFS write path (`storage/src/vfs.rs`) | Trait fully defined; dispatch layer complete. All writes delegate to the FS impl. |
| FAT32 (`storage/src/fs/fat32.rs`) | Read-only. All write methods return `Err(ReadOnly)`. `read_only: true` hardcoded at mount. |
| VirtIO-blk write (`driver/virtio/block.rs`) | **Fully functional** — `write_sector()` with DMA address translation. |
| Pipe | Implemented (4 KiB ring buffer, non-blocking). |
| Init process | None. PID 1 is a hardcoded 16-byte `SYS_EXIT(42)` test program. |
| userlib syscall wiring | Basic I/O + process calls work; 20+ fs/env/thread wrappers are stubs. |

---

## Sub-Phases

### 12-1: Fix execve Return Path ✅ COMPLETE

- **Goal**: `sys_execve()` modifies the saved SYSCALL return frame (RCX, R11, RSP on the kernel stack) so the calling process resumes execution at the new ELF entry point with a fresh user stack.
- **Status**: COMPLETE (2026-03-06)
- **Implementation**:
  - `kernel/src/scheduler/userspace.rs` — Added `EXECVE_PENDING`, `EXECVE_NEW_RIP`, `EXECVE_NEW_RSP`, `EXECVE_NEW_RFLAGS` AtomicU64 statics with `set_execve_context()` / `clear_execve_context()` helpers.
  - `kernel/src/scheduler/userspace.rs` — Modified `ring3_syscall_entry` assembly epilogue: after `call ring3_syscall_dispatch`, a RIP-relative `lea` checks `EXECVE_PENDING`. If set, diverts to label `2:` which loads new RIP→RCX, new RSP→r14, new RFLAGS→R11, zeros all GPRs, sets RSP, swapgs+sysretq.
  - `kernel/src/scheduler/userspace.rs` — Added `handle_execve(pathname_ptr)` implementing minimal inline ELF64 loading: reads pathname from user memory, loads ELF from VFS, parses ELF header + PT_LOAD segments, reuses existing page mappings via `read_pte()` + `write_to_phys()` (avoids `destroy_user_mappings` which hangs under SFMASK IF=0), maps new stack, calls `set_execve_context()`. Added SYS_EXECVE (59) routing to `ring3_syscall_dispatch`.
  - `kernel/src/syscall/linux_handlers.rs` — Rewrote `sys_execve()` body with `read_user_string_array` helper for argv/envp parsing, CR3 fallback to hardware register. Added EXECVE_PENDING globals (library crate copy).
  - `kernel/src/main.rs` — Integration test: inline ELF target that writes "EXECVE OK\n" via SYS_WRITE + SYS_EXIT(42), caller program invokes SYS_EXECVE, runs before Phase 11.
- **Quality Gate**: ✅ PASSED — QEMU serial log contains:
  ```
  [EXECVE] Success: new_rip=0x400078 new_rsp=0x800000 (CR3=0xe9a6000)
  EXECVE OK
  [RING3] SYS_EXIT called with status=42
  ```
  `cargo build -p kpio-kernel` succeeds with no errors.

---

### 12-2: Fix fork Child Return

- **Goal**: After `fork()`, the child process resumes from the exact instruction after the `syscall` that invoked `fork`, with RAX=0 (child return value), rather than starting from the ELF entry point.
- **Tasks**:
  - `kernel/src/syscall/linux_handlers.rs` — In `sys_fork()`, capture the parent's full SYSCALL frame (all GPRs + RCX/R11/RSP) at the point of the syscall. This requires either:
    - (a) Passing the `SyscallFrame` pointer into the handler (preferred — modify the syscall dispatch path to forward the frame pointer), or
    - (b) Reading the parent's saved context from its kernel stack top.
  - `kernel/src/scheduler/task.rs` — `new_user_process()` already accepts a `user_entry` and `user_stack`. Extend it (or add `new_forked_process()`) to accept a full `SyscallFrame` that is placed on the child's kernel stack so that the first context-switch into the child returns via `sysretq` to the fork call-site with RAX=0.
  - `kernel/src/scheduler/context.rs` — Ensure `setup_initial_stack()` (or equivalent) can plant a complete SYSCALL return frame for the child, including the parent's RCX (return RIP), R11 (RFLAGS), and RSP, with RAX overridden to 0.
  - `kernel/src/memory/user_page_table.rs` — Verify `clone_user_page_table()` (CoW path from Phase 11) is used in the fork path. If the current fork path uses deep copy, switch to CoW. Log `[FORK] child PID N, CoW shared M pages`.
- **Quality Gate**: A test sequence: parent calls `fork()`. Parent receives child PID > 0. Child receives 0. Both print their PID to serial. QEMU serial log contains `[FORK] parent=N child=M` and `[FORK] child M running, fork returned 0`. No triple faults.

---

### 12-3: Complete ProcessManager::spawn() from VFS

- **Goal**: `ProcessManager::spawn(path)` loads an ELF binary from the VFS, creates a real page table, allocates a kernel stack with guard page, loads ELF segments, and enqueues the new process for scheduling — making it possible to launch programs from the filesystem.
- **Tasks**:
  - `kernel/src/process/manager.rs` — Rewrite `spawn()`:
    1. Read ELF bytes from VFS via `crate::vfs::read_all(path)`.
    2. Parse with `Elf64Loader::parse(&bytes)`.
    3. Create user page table via `create_user_page_table()` → get CR3.
    4. Load segments via `load_elf_segments(cr3, &program)`.
    5. Set up user stack via `setup_user_stack(cr3, &program, args, envp)`.
    6. Allocate kernel stack with guard page via `alloc_kernel_stack_with_guard(name)`.
    7. Create `Task` via `new_user_process(name, entry, user_rsp, cr3, kernel_stack_top)`.
    8. Register in `ProcessTable` with correct CR3, brk, and mmap base.
    9. Enqueue task in the scheduler.
    10. Return `Ok(pid)`.
  - `kernel/src/process/manager.rs` — Implement `spawn_with_args(path, argv, envp)` variant that forwards arguments to the user stack setup.
  - `kernel/src/main.rs` — Replace at least one hardcoded inline-assembly test program with a `ProcessManager::spawn("/bin/hello")` call (requires 12-5 to place the binary on disk, but the code path should be testable with an in-memory VFS entry).
- **Quality Gate**: `ProcessManager::spawn("/test/hello")` (where `/test/hello` is an ELF registered in VFS) creates a process that runs in Ring 3 and produces serial output. QEMU serial log contains `[SPAWN] loaded '/test/hello' pid=N cr3=0x...`. `cargo build -p kpio-kernel` succeeds.

---

### 12-4: FAT32 Write Support

- **Goal**: The FAT32 filesystem supports file creation, writing, deletion, and directory creation — enabling persistent state on the VirtIO block device.
- **Tasks**:
  - `storage/src/fs/fat32.rs` — Implement cluster chain management:
    - `alloc_cluster()` — Scan the FAT for the first free cluster entry (`0x00000000`), mark it as end-of-chain (`0x0FFFFFFF`), return cluster number. Update `FSInfo` free count.
    - `extend_chain(last_cluster, new_cluster)` — Write `new_cluster` into the FAT entry for `last_cluster`.
    - `free_chain(start_cluster)` — Walk the chain, zero each FAT entry, update `FSInfo` free count.
    - `flush_fat()` — Write dirty FAT sectors back to disk via `write_sector()`. If `fat_count == 2`, mirror to the backup FAT.
  - `storage/src/fs/fat32.rs` — Implement `create()`:
    1. Find the parent directory's cluster chain.
    2. Scan for a free 32-byte directory entry slot (or extend the directory cluster chain).
    3. Write a new directory entry (8.3 name, attributes, first cluster=0, size=0, timestamps).
    4. Return the new inode ID.
  - `storage/src/fs/fat32.rs` — Implement `write()`:
    1. Seek to the correct cluster/offset in the file's chain.
    2. If writing beyond the current chain, call `alloc_cluster()` + `extend_chain()`.
    3. Write data to the cluster's sectors via `write_sector()`.
    4. Update the directory entry's file size and modification timestamp.
  - `storage/src/fs/fat32.rs` — Implement `unlink()`:
    1. Remove the directory entry (mark first byte as `0xE5`).
    2. Free the file's cluster chain via `free_chain()`.
  - `storage/src/fs/fat32.rs` — Implement `mkdir()`:
    1. Allocate a cluster for the new directory.
    2. Create `.` and `..` directory entries.
    3. Add a directory entry in the parent.
  - `storage/src/fs/fat32.rs` — Implement `truncate()`:
    1. Free excess clusters if truncating to a shorter length.
    2. Update directory entry size.
  - `storage/src/fs/fat32.rs` — Implement `fsync()` / `flush()`:
    1. Flush the FAT table.
    2. Flush the FSInfo sector.
    3. Flush any cached directory entry changes.
  - `storage/src/fs/fat32.rs` — Change `read_only: true` → `read_only: false` at mount time. Add a `MountFlags::READ_ONLY` option that callers can use to force read-only mounts.
  - `storage/src/cache.rs` — If not already present, implement a simple block cache (write-back with explicit flush) to avoid writing every sector change immediately. A 64-entry LRU cache using `BTreeMap<u64, CacheEntry>` with dirty bit suffices.
- **Quality Gate**: After boot, the kernel creates a file `/mnt/test/WRITTEN.TXT` with contents `"Hello from KPIO"`, reads it back, and verifies the content matches. QEMU serial log contains `[FAT32] created WRITTEN.TXT` and `[FAT32] write 15 bytes` and `[VFS] readback verified: "Hello from KPIO"`. The file persists across a graceful flush (FAT table written to disk).

---

### 12-5: Init Process & ELF-from-Disk Boot

- **Goal**: The kernel launches PID 1 (`/init` or `/bin/init`) from the mounted FAT32 filesystem, replacing the hardcoded test programs. This is the first end-to-end demonstration of the full user-space pipeline: VFS → ELF load → Ring 3 execution → syscall → serial output.
- **Tasks**:
  - `kernel/src/main.rs` — After VFS mount and FAT32 initialization, attempt to spawn `/init` (or `/bin/init`) via `ProcessManager::spawn("/init")`. If not found, fall back to the hardcoded test suite with a warning log.
  - `scripts/create-test-disk.ps1` — Modify (or create) a script that builds a FAT32 disk image containing:
    - `/init` — A minimal static ELF64 binary that prints `"[INIT] PID 1 running\n"` to serial and enters an infinite loop (or calls `wait4(-1, ...)` to reap children).
    - `/bin/hello` — A static ELF64 binary that prints `"Hello from disk!\n"` and exits.
    - `/HELLO.TXT` — Existing test file.
  - `kernel/src/main.rs` — After init spawns, call `ProcessManager::spawn("/bin/hello")` as a child process. Init reaps it via `wait4()`.
  - Tools: Create a minimal `no_std` ELF binary in `tools/init/` (or `examples/init/`) that uses raw `syscall` instructions to `SYS_WRITE` and `SYS_EXIT`. Cross-compile to `x86_64-unknown-none` static ELF. Include a build step in the disk image script.
- **Quality Gate**: QEMU boots, mounts FAT32, loads `/init` from disk, and serial log contains `[INIT] PID 1 running` and `Hello from disk!`. Both processes appear in the process table log. No panics or triple faults.

---

### 12-6: userlib Syscall Wiring

- **Goal**: Wire the userlib filesystem, environment, and process syscall stubs to real kernel syscalls so that user-space Rust programs linked against `userlib` can perform file I/O and query process state.
- **Tasks**:
  - `userlib/src/syscall.rs` — Replace stub implementations with real `syscall!()` invocations for:
    - `fs_seek()` → `SYS_LSEEK` (syscall 8)
    - `fs_stat()` → `SYS_STAT` (syscall 4) — marshal `StatBuf` struct
    - `fs_stat_fd()` → `SYS_FSTAT` (syscall 5)
    - `fs_readdir()` → `SYS_GETDENTS64` (syscall 217) — parse dirent buffer
    - `fs_mkdir()` → `SYS_MKDIR` (syscall 83) or `SYS_MKDIRAT` (258)
    - `fs_unlink()` → `SYS_UNLINK` (syscall 87) or `SYS_UNLINKAT` (263)
    - `fs_rmdir()` → `SYS_RMDIR` (syscall 84)
    - `fs_rename()` → `SYS_RENAME` (syscall 82) or `SYS_RENAMEAT2` (316)
    - `fs_sync()` → `SYS_FSYNC` (syscall 74)
    - `getcwd()` → `SYS_GETCWD` (syscall 79)
    - `chdir()` → `SYS_CHDIR` (syscall 80)
    - `get_args()` → Read from initial stack (argc/argv placed by `setup_user_stack`)
    - `env_get()` → Read from initial stack (envp placed by `setup_user_stack`)
  - `userlib/src/io.rs` — `File::open()`, `File::create()`, `File::read()`, `File::write()`, `File::seek()` should use the real syscall wrappers.
  - `userlib/src/process.rs` — `fork()` and `waitpid()` should return proper typed results with PID and exit status.
- **Quality Gate**: A test ELF (embedded or disk-loaded) linked against `userlib` calls `fs_open("/mnt/test/HELLO.TXT")`, `fs_read()`, and prints the file contents to serial. QEMU serial log contains the file contents (`Hello from KPIO test disk!`). `cargo build -p userlib` succeeds.

---

### 12-7: Integration Test

- **Goal**: Automated QEMU test validates all Phase 12 features in a single boot.
- **Tasks**:
  - `kernel/src/main.rs` — Add a Phase 12 integration test block (gated by a feature flag or unconditional during development):
    1. Log `[P12] Phase 12 integration test start`.
    2. Exercise execve: spawn a process that execves into a "hello" binary → verify serial output.
    3. Exercise fork: fork a process, verify parent gets child PID, child gets 0.
    4. Exercise FAT32 write: create `/mnt/test/P12TEST.TXT`, write data, read back, verify.
    5. Exercise spawn-from-disk: `ProcessManager::spawn("/bin/hello")` → verify output.
    6. Log `[P12] Phase 12 integration test PASSED`.
  - `scripts/qemu-test.ps1` — Add `-Mode userspace` with checks:

    | Check | Pattern |
    |-------|---------|
    | Phase 12 test start | `P12.*Phase 12` (regex) |
    | execve works | `EXECVE OK` (literal) |
    | fork child returns 0 | `FORK.*child.*fork returned 0` (regex) |
    | FAT32 file created | `FAT32.*created` (regex) |
    | FAT32 write | `FAT32.*write.*bytes` (regex) |
    | VFS readback verified | `VFS.*readback verified` (regex) |
    | Init from disk | `INIT.*PID 1 running` (regex) |
    | Hello from disk | `Hello from disk` (literal) |
    | All passed | `P12.*PASSED` (regex) |
    | No panics | absence of `panicked at` |

  - `scripts/qemu-test.ps1` — Add `"userspace"` to the `ValidateSet` for `-Mode`.
- **Quality Gate**: `.\scripts\qemu-test.ps1 -Mode userspace` passes ALL checks with 0 failures.

---

## Completion Protocol (per AGENTS.md §4)

After each sub-phase:
1. ✅ Quality Gate verified via QEMU serial log and `cargo build`.
2. ✅ `docs/roadmap.md`, `RELEASE_NOTES.md`, `docs/known-issues.md` updated.
3. ✅ Changes committed with descriptive English commit message.
4. ✅ Sub-phase marked complete in this plan and the roadmap.

## Verification Plan

### Per-Sub-Phase Build Check

```powershell
cargo build -p kpio-kernel
```

### Automated Integration Test

```powershell
.\scripts\qemu-test.ps1 -Mode userspace -Verbose
```

### Regression Tests

```powershell
.\scripts\qemu-test.ps1 -Mode hardening -Verbose   # Phase 11
.\scripts\qemu-test.ps1 -Mode process -Verbose      # Phase 10
.\scripts\qemu-test.ps1 -Mode boot                   # Basic boot
```

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| SYSCALL return frame layout differs from expected | execve/fork produce triple fault | Add `debug_assert!` on frame offsets; test with single-step QEMU before multi-process |
| FAT32 write corrupts filesystem metadata | Disk image unreadable | Implement write-ahead logging for FAT sectors; always flush before QEMU shutdown; verify with host `fsck.fat` |
| Test ELF binaries fail to build for `x86_64-unknown-none` | Cannot test spawn-from-disk | Use minimal inline assembly binaries (< 100 bytes) as fallback; avoid linker complexity |
| CoW + execve interaction causes page table corruption | Child process crash after parent execve | Test CoW fork + execve sequence explicitly; add refcount assertions |

## Dependency Graph

```
12-1 (execve return) ──┐
                       ├──→ 12-3 (spawn from VFS) ──→ 12-5 (init from disk) ──┐
12-2 (fork child)  ────┘                                                       ├──→ 12-7 (integration test)
                                                                               │
12-4 (FAT32 write) ───────────────────────────────────────────────────────────┤
                                                                               │
12-6 (userlib wiring) ────────────────────────────────────────────────────────┘
```

**Critical path:** 12-1 → 12-3 → 12-5 → 12-7.
**Parallelizable:** 12-4 (FAT32 write) is independent of 12-1/12-2/12-3 and can proceed in parallel. 12-6 can proceed once 12-3 provides the spawn mechanism.
