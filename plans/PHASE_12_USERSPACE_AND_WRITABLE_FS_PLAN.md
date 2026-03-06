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

### 12-2: Fix fork Child Return ✅ COMPLETE

- **Goal**: After `fork()`, the child process resumes from the exact instruction after the `syscall` that invoked `fork`, with RAX=0 (child return value), rather than starting from the ELF entry point.
- **Status**: COMPLETE (2026-03-06)
- **Implementation**:
  - `kernel/src/scheduler/userspace.rs` — Added `SYSCALL_SAVED_USER_RIP`, `SYSCALL_SAVED_USER_RSP`, `SYSCALL_SAVED_USER_RFLAGS` AtomicU64 statics. Assembly in `ring3_syscall_entry` saves the parent's user state (RCX→RIP, R11→RFLAGS, gs:[8]→RSP) before dispatch. Added `SYS_FORK` (57) routing to `handle_fork()`. `handle_fork()` reads saved frame, calls `clone_user_page_table()` for CoW clone, allocates kernel stack with guard, creates child via `Task::new_forked_process()`, spawns it with `NEXT_FORK_PID` starting at 50.
  - `kernel/src/scheduler/task.rs` — Added `ForkChildContext` struct (rip, cs, rflags, rsp, ss) and `Task::new_forked_process()` (NEXT_ID starts at 200). `fork_child_trampoline()` reads `ForkChildContext` from R12, fixes SWAPGS state via KERNEL_GS_BASE MSR check, then enters Ring 3 via `iretq` with all GPRs zeroed (RAX=0 = child fork return value).
  - `kernel/src/syscall/linux_handlers.rs` — Updated `sys_fork()` to read saved frame from `SYSCALL_SAVED_USER_RIP/RSP/RFLAGS` statics and use `Task::new_forked_process()` when frame is available.
  - `kernel/src/memory/user_page_table.rs` — **BUGFIX**: `clone_user_page_table()` now only deep-clones P4[0] (user-space range 0x0-0x7F_FFFF_FFFF). P4[1-255] are shallow-copied to avoid deep-cloning bootloader/kernel infrastructure entries (P4[2]=ELF, P4[5]=phys offset, P4[7]=boot info) which caused triple faults.
  - `kernel/src/main.rs` — Integration test: 127-byte x86_64 fork test program at 0x400000 calls SYS_FORK, parent checks RAX>0 and writes "FORK PARENT OK", child checks RAX=0 and writes "FORK CHILD OK", both exit via SYS_EXIT(0).
- **Quality Gate**: ✅ PASSED — QEMU serial log contains:
  ```
  [FORK] parent=current child=50 (CoW CR3=0xd4db000)
  FORK PARENT OK
  [FORK] child 50 running, fork returned 0 (RIP=0x400009 RSP=0x800000)
  FORK CHILD OK
  ```
  Hardening regression: 28/29 pass (1 pre-existing "Work queue drain" failure). No triple faults.

---

### 12-3: Complete ProcessManager::spawn() from VFS ✅ COMPLETE

- **Goal**: `ProcessManager::spawn(path)` loads an ELF binary from the VFS, creates a real page table, allocates a kernel stack with guard page, loads ELF segments, and enqueues the new process for scheduling — making it possible to launch programs from the filesystem.
- **Status**: COMPLETE (2026-03-06)
- **Implementation**:
  - `kernel/src/process/manager.rs` — Added `spawn_from_vfs(path)` and `spawn_from_vfs_with_args(path, argv, envp)`. Full pipeline: (1) read ELF bytes from VFS via `crate::vfs::read_all(path)`, (2) parse with `Elf64Loader::parse()`, (3) create per-process page table via `create_user_page_table()`, (4) load PT_LOAD segments via `load_elf_segments(cr3, &program, &bytes, 0)`, (5) allocate 32 KiB kernel stack as `Vec<u8>`, (6) register `Process` in `PROCESS_TABLE` with `LinuxMemoryInfo` (CR3, brk_start from LoadResult, mmap_next_addr=MMAP_BASE), (7) create `Task::new_user_process()` with entry point, user stack top, kernel stack top, and PID, (8) enqueue via `scheduler::spawn()`. Added `VfsError` and `SegmentLoadError` variants to `SpawnError`. Added `extract_name_from_path()` helper.
  - `kernel/src/main.rs` — Added `mod loader;` and `mod process;` declarations for crate-local access. Added Phase 12-3 integration test: builds a minimal 171-byte ELF64 binary (51 bytes of code: `lea rsi,[rip+0x15]` + SYS_WRITE "SPAWN OK\n" + SYS_EXIT(0)), registers at `/bin/spawn-test` via `vfs::write_all()`, calls `crate::process::manager::PROCESS_MANAGER.spawn_from_vfs("/bin/spawn-test")`.
- **Quality Gate**: ✅ PASSED — QEMU serial log contains:
  ```
  [SPAWN] loaded '/bin/spawn-test' pid=pending cr3=0xd434000 entry=0x400078 sp=0x7ffffffff000 brk=0x401000 (17 pages)
  [SPAWN] loaded '/bin/spawn-test' pid=2 cr3=0xd434000
  [SPAWN] spawn_from_vfs("/bin/spawn-test") succeeded: pid=2
  SPAWN OK
  ```
  `cargo build -p kpio-kernel` succeeds with no new errors. No panics or triple faults.

---

### 12-4: FAT32 Write Support ✅ COMPLETE

- **Goal**: The FAT32 filesystem supports file creation, writing, deletion, and directory creation — enabling persistent state on the VirtIO block device.
- **Status**: COMPLETE (2026-03-06)
- **Implementation**:
  - `storage/src/fs/fat32.rs` — Implemented full write support:
    - `write_sector()` — Write a single sector to the block device via `BlockDevice::write_blocks()`.
    - `write_fat_entry()` — Write a FAT entry with upper-4-bit preservation; mirrors to backup FAT if `num_fats == 2`.
    - `alloc_cluster()` — Scan FAT for free entry (`0x00000000`), mark as EOC (`0x0FFFFFFF`), zero the cluster, update `free_clusters` atomic counter.
    - `extend_chain()` — Link a new cluster to the end of a chain.
    - `free_chain()` — Walk chain, zero each FAT entry, increment `free_clusters`.
    - `find_free_dir_slot()` — Find free 32-byte slot in directory cluster chain; extends chain if full.
    - `write_dir_entry_at()` — Write a raw 32-byte directory entry at a chain byte-offset (read-modify-write of containing sector).
    - `update_dir_entry_cluster()` / `update_dir_entry_size()` — Targeted directory entry field updates.
    - `make_short_name()` — Convert filename to FAT32 8.3 uppercase short name format.
    - Changed `free_clusters` field from `u32` to `AtomicU32` for interior mutability.
  - `storage/src/fs/fat32.rs` — Implemented `Filesystem` trait write methods:
    - `create()` — Create a new empty file: find parent dir, allocate 32-byte dir entry slot, write entry with ARCHIVE attribute.
    - `write()` — Write data at offset: count/allocate clusters as needed, navigate to correct cluster, sector-by-sector read-modify-write, update directory entry file_size.
    - `unlink()` — Delete a file: find entry, free cluster chain, mark dir entry as deleted (0xE5).
    - `mkdir()` — Create directory: allocate cluster, write `.` and `..` entries, add entry in parent.
    - `rmdir()` — Remove empty directory: verify empty, free chain, mark deleted.
    - `truncate()` — Resize file: free excess clusters or extend, update dir entry size.
    - `fsync()` / `flush()` — Delegate to `device.flush()`.
    - `open()` — Added CREATE flag support (auto-create if not found) and TRUNCATE flag support.
  - `storage/src/fs/fat32.rs` — Changed `read_only: true` → `read_only: false` at mount time.
  - `storage/src/fs/fat32.rs` — Extended `OpenFile` struct with `parent_dir_cluster` and `dir_entry_chain_offset` fields for tracking directory entry location (needed for write-back of file size and first cluster).
  - `kernel/src/main.rs` — Changed FAT32 mount flags from `MountFlags::READ_ONLY` to `MountFlags::empty()`.
  - `kernel/src/main.rs` — Added Phase 12-4 integration test: creates `/mnt/test/WRITTEN.TXT`, writes `"Hello from KPIO"`, reads back and verifies content match.
- **Quality Gate**: ✅ PASSED — QEMU serial log contains:
  ```
  [FAT32] created WRITTEN.TXT
  [FAT32] write 15 bytes
  [VFS] readback verified: "Hello from KPIO"
  ```
  `cargo build -p kpio-kernel` succeeds with no errors. No panics or triple faults. All regression tests pass.

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
