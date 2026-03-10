# Phase 13: IPC, Socket Syscalls & Kernel Threading

**Document Version:** 1.1.0
**Created:** 2026-03-10
**Status:** In Progress (13-1 ✅, 13-2 🔄)
**Depends On:** Phase 12 (User-Space & Writable FS) ✅

---

## Overview

Phase 13 exposes the kernel's existing network stack to Ring 3 user-space
via BSD socket syscalls, adds kernel-level threading (`clone(CLONE_THREAD)`),
and implements event multiplexing (`epoll`). These three capabilities are the
foundational IPC/concurrency layer required before any complex user-space
program (GUI compositor, browser engine, server daemon) can run.

### Motivation

| Capability Gap | Impact |
|----------------|--------|
| **No socket syscalls** | TCP/IP stack (Phase 9) is kernel-internal only; Ring 3 programs cannot open network connections |
| **No threads** | `clone()` falls through to `fork()` — no shared address space, no pthreads, no concurrent I/O |
| **No event multiplexing** | No `epoll`/`poll`/`select` — programs cannot wait on multiple FDs simultaneously |

All three are **prerequisites** for Phase 14+ (graphics compositor, browser
engine, server daemons). Implementing them now maximizes reuse of the mature
network and process infrastructure from Phases 9-12.

### Scope

**In scope:**
- BSD socket syscall family (socket, bind, listen, accept, connect, send/recv, shutdown, getpeername/getsockname, setsockopt/getsockopt)
- `FileResource::Socket` variant in per-process FD table
- `clone()` with `CLONE_THREAD | CLONE_VM | CLONE_SIGHAND | CLONE_FILES` flag support
- Thread group tracking (TGID, per-thread TID)
- Per-thread signal masks
- `epoll_create1`, `epoll_ctl`, `epoll_wait` syscalls
- Epoll interest list with level-triggered readiness notification
- Integration test: TCP echo server in QEMU, multi-threaded counter, epoll event loop

**Out of scope:**
- Edge-triggered epoll (EPOLLET) — deferred to a later phase
- Unix domain sockets (AF_UNIX) — deferred
- `poll()`/`select()` — epoll is the primary multiplexer
- File-backed mmap — deferred
- Shared memory (shm_open) — deferred
- FUTEX_REQUEUE / advanced futex operations — deferred

---

## Sub-Phase Breakdown

### Sub-Phase 13-1: Socket FD Infrastructure

- **Goal**: Extend the per-process file descriptor table with a `Socket`
  resource type so that socket handles can be read/written/closed like
  regular file descriptors.

- **Tasks**:
  1. `FileResource::Socket { socket_id: u64 }` already exists in
     `kernel/src/process/table.rs :: FileResource`; no new variant is needed.
     `SocketHandle(pub u32)` is already defined in `network/src/socket.rs`;
     do **not** redefine it. Wire `sys_read()` and `sys_write()` in
     `kernel/src/syscall/linux_handlers.rs` to match the existing `Socket`
     variant and delegate to `network::socket::recv()` /
     `network::socket::send()`.
  2. Extend `sys_close()` to call `network::socket::close()` for socket FDs
  3. Add syscall constants in `kernel/src/syscall/linux.rs`:
     - `SYS_SOCKET (41)`, `SYS_BIND (49)`, `SYS_LISTEN (50)`,
       `SYS_ACCEPT (43)`, `SYS_CONNECT (42)`, `SYS_SENDTO (44)`,
       `SYS_RECVFROM (45)`, `SYS_SHUTDOWN (48)`, `SYS_GETSOCKNAME (51)`,
       `SYS_GETPEERNAME (52)`, `SYS_SETSOCKOPT (54)`, `SYS_GETSOCKOPT (55)`,
       `SYS_ACCEPT4 (288)`
  4. Implement `sys_socket(domain, socktype, protocol)` in
     `kernel/src/syscall/linux_handlers.rs`:
     - Validate `domain` (AF_INET = 2 only; reject AF_UNIX, AF_INET6 with -EAFNOSUPPORT)
     - Map `socktype` to `network::socket::SocketType` (SOCK_STREAM=1, SOCK_DGRAM=2)
     - Call `network::socket::create()`, allocate FD, store `FileResource::Socket`
     - Return the FD number
  5. Wire `SYS_SOCKET` in the dispatch table (`linux.rs` or `linux_handlers.rs`)
  6. Add missing socket errno constants to `kernel/src/syscall/linux.rs`:
     `ENOTSOCK = 88`, `ENOPROTOOPT = 92`, `EAFNOSUPPORT = 97`,
     `EADDRINUSE = 98`, `EOPNOTSUPP = 95`, `ENOTCONN = 107`

- **Quality Gate**: `cargo build -p kpio-kernel` succeeds. A minimal ELF test
  program calls `syscall(SYS_SOCKET, AF_INET, SOCK_STREAM, 0)` and receives
  a valid fd >= 3 (not -ENOSYS). QEMU serial log shows `[Socket] created fd=N`
  trace. No panics or triple faults.

---

### Sub-Phase 13-2: BSD Socket Syscalls (bind/listen/accept/connect)

- **Goal**: Implement the core BSD socket lifecycle syscalls so that a
  user-space program can create a TCP server (bind + listen + accept)
  or a TCP client (connect) and exchange data.

- **Tasks**:
  1. Implement smoltcp send/recv integration in `network/src/socket.rs`:
     - Wire `send(handle, data)` to `smoltcp::socket::tcp::Socket::send_slice()`
     - Wire `recv(handle, buf)` to `smoltcp::socket::tcp::Socket::recv()`
     - Both must return `Ok(n)` with actual byte counts (replace `Err(WouldBlock)` stubs)
  2. Implement `network::socket::accept(handle: SocketHandle) -> Result<SocketHandle, NetworkError>`
     in `network/src/socket.rs`:
     - Query the listening socket's accept queue via smoltcp
     - Return a new `SocketHandle` for the accepted connection, or `Err(WouldBlock)` if none ready
  3. Implement `sys_bind(fd, addr_ptr, addrlen)`:
     - Read `struct sockaddr_in` (16 bytes) from user-space pointer
     - Extract port (network byte order) and IPv4 address
     - Call `network::socket::bind(handle, SocketAddr)`
     - Return 0 on success, -errno on failure
  4. Implement `sys_listen(fd, backlog)`:
     - Look up socket handle from FD table
     - Call `network::socket::listen(handle, backlog)`
     - Return 0 / -errno
  5. Implement `sys_accept(fd, addr_ptr, addrlen_ptr)` and `sys_accept4(fd, addr_ptr, addrlen_ptr, flags)`:
     - Call `network::socket::accept(handle)` which returns a new `SocketHandle`
     - Allocate a new FD for the accepted connection
     - Write remote `sockaddr_in` to user-space if `addr_ptr != 0`
     - Return the new FD
  6. Implement `sys_connect(fd, addr_ptr, addrlen)`:
     - Parse `sockaddr_in` from user-space
     - Call `network::socket::connect(handle, addr)`
     - Return 0 / -errno (EINPROGRESS for non-blocking)
  7. Implement `sys_sendto(fd, buf, len, flags, dest_addr, addrlen)`:
     - For connected TCP: ignore dest_addr, call `network::socket::send()`
     - For UDP: use dest_addr if provided
     - Return bytes sent / -errno
  8. Implement `sys_recvfrom(fd, buf, len, flags, src_addr, addrlen)`:
     - Call `network::socket::recv(handle, buf)`
     - Write source `sockaddr_in` to user-space for UDP
     - Return bytes received / -errno
  9. Implement `sys_shutdown(fd, how)`:
     - Map `how` (SHUT_RD=0, SHUT_WR=1, SHUT_RDWR=2)
     - Close read/write ends of the socket
  10. Implement `sys_getpeername(fd, addr, addrlen)` and `sys_getsockname(fd, addr, addrlen)`:
     - Return the remote/local sockaddr_in for the given socket FD
  11. Implement `sys_setsockopt(fd, level, optname, optval, optlen)` and
     `sys_getsockopt(fd, level, optname, optval, optlen)`:
     - Support minimal set: `SO_REUSEADDR`, `SO_KEEPALIVE`, `SO_RCVTIMEO`, `SO_SNDTIMEO`
     - Return -ENOPROTOOPT for unsupported options
  12. Wire all new syscalls in the dispatch table

- **Quality Gate**: Embedded ELF test programs:
  (a) **TCP echo server**: binds to `0.0.0.0:7777`, accepts one connection,
  echoes received data back. A kernel-internal test helper acts as the client.
  **Do not use `127.0.0.1`** — QEMU SLIRP does not route loopback traffic
  back into the guest; coordinate the server and client tasks through shared
  smoltcp interface state, or connect via the SLIRP gateway (`10.0.2.2`).
  QEMU serial log shows `[E2E] TCP echo server PASSED`.
  (b) **UDP send/recv**: creates a SOCK_DGRAM socket, sends a datagram,
  receives a response.
  `cargo build -p kpio-kernel` succeeds. No panics or triple faults.

---

### Sub-Phase 13-3: Kernel Threading (clone with CLONE_THREAD)

- **Goal**: Implement `clone()` with thread semantics so that multiple
  threads share the same address space, file descriptor table, and signal
  handlers within a single process (thread group).

- **Tasks**:
  1. Add a thread group ID field to `Process` struct in
     `kernel/src/process/table.rs`:
     ```
     pub tgid: ProcessId,  // Thread group leader PID (= first thread's PID)
     ```
     `Process` already has `pub threads: Vec<Thread>` and
     `pub main_thread: ThreadId` — do **not** add a redundant `thread_group`
     field or a `tid` field at the `Process` level. Per-thread IDs are
     tracked by `Thread.tid: ThreadId`, which already exists.
  2. Modify `sys_getpid()` to return `tgid` (POSIX semantics: getpid = group leader PID)
  3. Modify `sys_gettid()` to return the current thread's `tid` (accessed via `Thread.tid: ThreadId`)
  4. Implement `sys_clone(flags, child_stack, ptid_ptr, ctid_ptr, tls)` in
     `kernel/src/syscall/linux_handlers.rs`:
     - Parse flags bitmask:
       - `CLONE_VM (0x100)`: share page table (same `page_table_root`) instead of CoW copy
       - `CLONE_THREAD (0x10000)`: same thread group (tgid), shared `Process` entry
       - `CLONE_SIGHAND (0x800)`: share signal handlers
       - `CLONE_FILES (0x400)`: share file descriptor table
       - `CLONE_SETTLS (0x80000)`: set `FS_BASE` MSR from `tls` argument
       - `CLONE_PARENT_SETTID (0x100000)`: write child TID to `*ptid_ptr`
       - `CLONE_CHILD_CLEARTID (0x200000)`: set `clear_child_tid` pointer (for futex wake on exit)
     - If `CLONE_THREAD`: allocate new TaskId but reuse parent's tgid,
       CR3, FD table reference, and signal handlers
     - If no `CLONE_THREAD`: fall back to fork behavior (existing `sys_fork()`)
     - If `child_stack != 0`: set the new thread's RSP to `child_stack`
     - Create `Task::new_thread()` that shares the parent's `cr3` and enters
       at the return address after the clone syscall with RAX=0
  5. Implement `Task::new_thread(parent_task, child_stack, tls, entry_rip)` in
     `kernel/src/scheduler/task.rs`:
     - Allocate a new kernel stack for the thread
     - Set up initial context with shared CR3, new RSP, and clone return point
  6. Add per-thread signal mask:
     - Move `blocked: u64` from `SignalState` (per-process) to per-Task storage
     - Keep signal actions (`rt_sigaction` handlers) shared at process level
  7. Handle thread exit:
     - When a thread exits, remove it from the process's `threads: Vec<Thread>`
     - If `clear_child_tid` is set, write 0 to that address and `futex_wake(ctid, 1)`
       (this is how glibc pthread_join works)
     - If the thread group leader exits, the entire process exits (all threads killed)
  8. Implement `sys_set_tid_address(tidptr)` properly:
     - Store `tidptr` as `clear_child_tid` in the current task
     - Return the caller's TID
  9. Wire `SYS_CLONE (56)` with full argument parsing (not just fork fallthrough)

- **Quality Gate**: Embedded ELF test program creates two threads via
  `clone(CLONE_THREAD | CLONE_VM | CLONE_SIGHAND | CLONE_FILES | CLONE_SETTLS, stack, ...)`.
  Both threads increment a shared `AtomicU32` counter 1000 times each. The main
  thread waits (via futex on `clear_child_tid`) for both threads to finish and
  verifies the counter equals 2000. QEMU serial log shows:
  `[IPC] Clone thread tid=N tgid=M`,
  `[IPC] Thread shared memory counter=2000`,
  `[IPC] Thread exit`.
  These patterns satisfy the `qemu-test.ps1 -Mode ipc` checklist items:
  `[IPC] Clone thread`, `[IPC] Thread shared memory`, `[IPC] Thread exit`,
  and `[IPC] TID vs PID`.
  `[E2E] Threading test PASSED`.
  No panics, no triple faults. `cargo build -p kpio-kernel` succeeds.

---

### Sub-Phase 13-4: Epoll Event Multiplexing

- **Goal**: Implement the `epoll` syscall family so that user-space programs
  can wait on readiness events from multiple file descriptors (sockets, pipes)
  simultaneously.

- **Tasks**:
  1. Create `kernel/src/sync/epoll.rs` module:
     - Define `EpollInstance` struct:
       ```
       pub struct EpollInstance {
           interest_list: BTreeMap<i32, EpollEntry>,  // fd -> interest
           ready_list: VecDeque<EpollEvent>,           // pending events
       }
       pub struct EpollEntry {
           fd: i32,
           events: u32,   // EPOLLIN, EPOLLOUT, EPOLLERR, EPOLLHUP
           data: u64,     // user-supplied opaque data (epoll_data_t)
       }
       #[repr(C, packed)]  // 12 bytes: matches Linux ABI (u32 events + u64 data)
       pub struct EpollEvent {
           events: u32,
           data: u64,
       }
       ```
     - Global epoll table: `static EPOLL_TABLE: Mutex<BTreeMap<u64, EpollInstance>>`
       Each `epoll_id` is generated by a monotonically increasing `AtomicU64`
       counter (`NEXT_EPOLL_ID`) to ensure system-wide uniqueness. Process
       isolation is enforced by the FD table: an `epoll_id` is only reachable
       via the owning process's FD table; other processes cannot reference it.
  2. Implement `sys_epoll_create1(flags)`:
     - Allocate a new `EpollInstance`
     - Create a special FD with `FileResource::Epoll { epoll_id: u64 }`
     - Support `EPOLL_CLOEXEC` flag (store in FD flags)
     - Return the epoll FD
  3. Add `FileResource::Epoll { epoll_id: u64 }` variant to `FileResource` enum
  4. Implement `sys_epoll_ctl(epfd, op, fd, event_ptr)`:
     - `EPOLL_CTL_ADD (1)`: add fd to interest list with requested events
     - `EPOLL_CTL_MOD (2)`: modify events for an already-registered fd
     - `EPOLL_CTL_DEL (3)`: remove fd from interest list
     - Read `struct epoll_event` (12 bytes: u32 events + u64 data) from user-space
     - Validate that `fd` refers to a valid open FD (socket or pipe)
  5. Implement `sys_epoll_wait(epfd, events_ptr, maxevents, timeout)`:
     - Check each fd in the interest list for readiness:
       - **Socket**: query `network::socket::poll(handle)` → returns EPOLLIN/EPOLLOUT/EPOLLERR
       - **Pipe**: check if pipe buffer has data (EPOLLIN) or has space (EPOLLOUT)
     - If ready events > 0: write up to `maxevents` `epoll_event` structs to user-space, return count
     - If no ready events and `timeout > 0`: block the task, wake on:
       (a) Any monitored FD becomes ready, or
       (b) Timeout expires (use TSC-based timer)
     - If `timeout == 0`: return immediately with 0 (non-blocking poll)
     - If `timeout == -1`: block indefinitely until an event occurs
  6. Define `PollFlags` bitflags type in `network/src/socket.rs`:
     ```
     bitflags::bitflags! {
         pub struct PollFlags: u32 {
             const READABLE = 0x1;
             const WRITABLE = 0x2;
             const ERROR    = 0x4;
             const HANGUP   = 0x8;
         }
     }
     ```
  7. Add readiness query API to `network/src/socket.rs`:
     - `pub fn poll(handle: SocketHandle) -> PollFlags` — returns bitmask of
       `READABLE | WRITABLE | ERROR | HANGUP`
  8. Add readiness query for pipes:
     - `pub fn pipe_poll(pipe_id: u64) -> PollFlags` — check buffer occupancy
  9. Wire epoll syscall constants and dispatch:
     - `SYS_EPOLL_CREATE1 (291)`, `SYS_EPOLL_CTL (233)`, `SYS_EPOLL_WAIT (232)`
  10. Clean up epoll instance when its FD is closed

- **Quality Gate**: Embedded ELF test program:
  (a) Creates an epoll instance, a pipe, and a socket.
  (b) Registers the pipe read-end with EPOLLIN.
  (c) Writes to the pipe write-end from a separate code path (or kernel helper).
  (d) Calls `epoll_wait` with timeout=1000ms.
  (e) Receives EPOLLIN event on the pipe FD.
  QEMU serial log shows `[Epoll] ready fd=N events=EPOLLIN`,
  `[E2E] Epoll test PASSED`.
  `cargo build -p kpio-kernel` succeeds. No panics or triple faults.

---

### Sub-Phase 13-5: Integration Test & QEMU Validation

- **Goal**: End-to-end automated integration test validating all Phase 13
  features in a single QEMU boot session via `qemu-test.ps1 -Mode ipc`.

- **Tasks**:
  1. Add `-Mode ipc` to `scripts/qemu-test.ps1` with the following checks:
     - `[IPC] Socket create` — socket syscall returns valid fd
     - `[IPC] Socket bind` — bind to port succeeds
     - `[IPC] Socket listen` — listen returns 0
     - `[IPC] Socket connect` — connection established
     - `[IPC] Socket accept` — accepted connection returns new fd
     - `[IPC] TCP echo` — data sent equals data received
     - `[IPC] Socket close` — close returns 0
     - `[IPC] UDP sendto` — UDP datagram sent
     - `[IPC] UDP recvfrom` — UDP datagram received
     - `[IPC] Clone thread` — clone with CLONE_THREAD returns tid > 0
     - `[IPC] Thread shared memory` — shared counter == expected value
     - `[IPC] Thread exit` — thread exits, clear_child_tid futex fires
     - `[IPC] TID vs PID` — gettid != getpid for child threads, gettid == getpid for leader
     - `[IPC] Epoll create` — epoll_create1 returns valid fd
     - `[IPC] Epoll ctl add` — register pipe fd succeeds
     - `[IPC] Epoll wait` — receives EPOLLIN from pipe
     - `[IPC] Epoll timeout` — returns 0 on empty poll with timeout=0
  2. Create embedded ELF test binaries in `kernel/src/` (inline assembly, no libc):
     - `test_socket_echo.rs` — TCP echo server/client pair
     - `test_threading.rs` — multi-thread shared counter
     - `test_epoll.rs` — epoll pipe readiness
  3. Integrate test dispatch in `kernel/src/main.rs` when built with
     `#[cfg(feature = "test-ipc")]` or detected via QEMU test mode
  4. Update `scripts/qemu-test.ps1` to support `-Mode ipc` with all check patterns
  5. Run full regression across all existing modes:
     - `qemu-test.ps1 -Mode boot` — 5/5
     - `qemu-test.ps1 -Mode process` — 22/22
     - `qemu-test.ps1 -Mode hardening` — 29/29
     - `qemu-test.ps1 -Mode userspace` — 21/21
     - `qemu-test.ps1 -Mode ipc` — 17/17 (new)

- **Quality Gate**: All five QEMU test modes pass with zero failures.
  `qemu-test.ps1 -Mode ipc` reports `Result: ALL PASS (17/17)`.
  Serial log contains all `[IPC]` check markers listed above.
  No regression in existing modes. No panics or triple faults.

---

## Dependency Graph

```
13-1 (Socket FD)
  │
  ├──► 13-2 (BSD Socket Syscalls)  ── requires socket FDs
  │         │
  │         └──► 13-5 (Integration)
  │
  13-3 (Threading / clone)
  │         │
  │         └──► 13-5 (Integration)
  │
  13-4 (Epoll)  ── requires socket + pipe FD readiness
  │         │
  │         └──► 13-5 (Integration)
```

Sub-phases 13-1 must be completed first (FD infrastructure). After that,
13-2 (sockets), 13-3 (threading), and 13-4 (epoll) can proceed in any
order, though 13-4 benefits from 13-2 being done first (to test socket
readiness). 13-5 (integration) depends on all prior sub-phases.

---

## Files to Create / Modify

### New Files

| File | Purpose |
|------|---------|
| `kernel/src/sync/epoll.rs` | Epoll instance, interest list, readiness polling |

### Modified Files

| File | Changes |
|------|---------|
| `kernel/Cargo.toml` | Add `network = { path = "../network" }` dependency |
| `kernel/src/process/table.rs` | Add `FileResource::Epoll`, thread group field (`tgid: ProcessId`); `FileResource::Socket { socket_id: u64 }` already present |
| `kernel/src/syscall/linux.rs` | Add socket/epoll/clone syscall constants, dispatch entries, missing socket errno constants |
| `kernel/src/syscall/linux_handlers.rs` | Implement all new syscall handlers (sys_socket, sys_bind, sys_listen, sys_accept, sys_connect, sys_sendto, sys_recvfrom, sys_shutdown, sys_getpeername, sys_getsockname, sys_setsockopt, sys_getsockopt, sys_clone with flags, sys_epoll_create1, sys_epoll_ctl, sys_epoll_wait) |
| `kernel/src/scheduler/task.rs` | Add `Task::new_thread()`, per-thread signal mask, `clear_child_tid` field |
| `kernel/src/scheduler/mod.rs` | Thread exit handling (futex wake on clear_child_tid) |
| `kernel/src/sync/mod.rs` | Add `pub mod epoll;` |
| `network/src/socket.rs` | Implement `accept()`, wire `send()`/`recv()` to smoltcp, add `PollFlags` type, add `poll()` readiness query |
| `scripts/qemu-test.ps1` | Add `-Mode ipc` with 17 check patterns |
| `kernel/src/main.rs` | IPC test dispatch for QEMU integration mode |

### New Crate Dependencies

Add `network = { path = "../network" }` to `kernel/Cargo.toml`'s
`[dependencies]` section. The `network` crate is **not** currently a
dependency of `kpio-kernel` and must be added before any socket syscall
handler can call into it.

All other implementation uses existing workspace crates (`kernel`, `network`).
No other new `Cargo.toml` entries are required.

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| `network::socket` API may not support `accept()` blocking semantics | Extend with blocking via scheduler (block task, wake on connection) |
| Thread exit race: leader exits while children still running | Thread group leader exit kills all threads in group first (SIGKILL) |
| Shared FD table concurrency: two threads closing the same FD | Use `AtomicU32` refcount or `Arc`-equivalent per FD entry |
| Epoll blocking without timer interrupt wake-up | Reuse existing APIC timer + TSC-based nanosleep infrastructure |
| Network crate `send`/`recv` may not be interrupt-safe | All socket ops run in process context (not ISR); already safe |

---

## Success Metrics

When Phase 13 is complete, the following should be true:

1. A Ring 3 ELF program can `socket() → bind() → listen() → accept()` and serve TCP connections
2. A Ring 3 ELF program can `socket() → connect()` to a remote host and `send()`/`recv()` data
3. A Ring 3 ELF program can create threads via `clone(CLONE_THREAD|CLONE_VM|...)` and share memory
4. A Ring 3 ELF program can use `epoll_wait()` to multiplex events from sockets and pipes
5. All 5 QEMU test modes pass (boot, process, hardening, userspace, ipc)
6. No regressions in existing functionality
