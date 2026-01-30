# KPIO System Call Design

**Version:** 1.0  
**Status:** In design

---

## Overview

KPIO's system call interface provides the functionality required to run the Servo browser.
It partially preserves POSIX compatibility while adding extensions for OS-level optimizations.

---

## System Call Calling Convention

### x86_64 ABI

```
Register usage:
- RAX: system call number
- RDI: argument 1
- RSI: argument 2
- RDX: argument 3
- R10: argument 4
- R8:  argument 5
- R9:  argument 6

Return value:
- RAX: result or error code (negative)

Invocation:
- use the SYSCALL instruction
```

---

## System Call Table

### 1. Process management (0-19)

| No. | Name | Args | Description |
|-----|------|------|-------------|
| 0 | `sys_exit` | code: i32 | Exit the process |
| 1 | `sys_fork` | - | Fork the process |
| 2 | `sys_exec` | path, argv, envp | Execute a new program |
| 3 | `sys_wait` | pid, status, options | Wait for a child |
| 4 | `sys_getpid` | - | Return PID |
| 5 | `sys_getppid` | - | Return parent PID |
| 6 | `sys_kill` | pid, signal | Send a signal |
| 7 | `sys_yield` | - | Yield the CPU |

```rust
/// Exit the process
pub fn sys_exit(code: i32) -> ! {
    // never returns
}

/// Fork a process (copy-on-write)
pub fn sys_fork() -> Result<Pid, SyscallError> {
    // child: returns 0
    // parent: returns child PID
}
```

### 2. Memory management (20-39)

| No. | Name | Args | Description |
|-----|------|------|-------------|
| 20 | `sys_mmap` | addr, len, prot, flags, fd, offset | Map memory |
| 21 | `sys_munmap` | addr, len | Unmap memory |
| 22 | `sys_mprotect` | addr, len, prot | Change protection flags |
| 23 | `sys_brk` | addr | Extend the heap |
| 24 | `sys_madvise` | addr, len, advice | Memory advice/hint |

```rust
/// Map memory
pub fn sys_mmap(
    addr: Option<*mut u8>,  // hint address (if NULL, the kernel chooses)
    len: usize,
    prot: ProtFlags,        // PROT_READ | PROT_WRITE | PROT_EXEC
    flags: MapFlags,        // MAP_PRIVATE | MAP_SHARED | MAP_ANON
    fd: Option<Fd>,         // for file-backed mappings
    offset: u64,
) -> Result<*mut u8, SyscallError>;

bitflags! {
    pub struct ProtFlags: u32 {
        const READ  = 1 << 0;
        const WRITE = 1 << 1;
        const EXEC  = 1 << 2;
    }
    
    pub struct MapFlags: u32 {
        const SHARED    = 1 << 0;
        const PRIVATE   = 1 << 1;
        const ANONYMOUS = 1 << 2;
        const FIXED     = 1 << 3;
    }
}
```

### 3. File I/O (40-69)

| No. | Name | Args | Description |
|-----|------|------|-------------|
| 40 | `sys_open` | path, flags, mode | Open a file |
| 41 | `sys_close` | fd | Close a file |
| 42 | `sys_read` | fd, buf, count | Read |
| 43 | `sys_write` | fd, buf, count | Write |
| 44 | `sys_lseek` | fd, offset, whence | Seek |
| 45 | `sys_fstat` | fd, stat | File metadata (by fd) |
| 46 | `sys_stat` | path, stat | File metadata (by path) |
| 47 | `sys_dup` | oldfd | Duplicate an fd |
| 48 | `sys_dup2` | oldfd, newfd | Duplicate to a specific number |
| 49 | `sys_pipe` | fds[2] | Create a pipe |
| 50 | `sys_fcntl` | fd, cmd, arg | File control |
| 51 | `sys_ioctl` | fd, request, arg | Device control |
| 52 | `sys_readdir` | fd, dirent | Read a directory |
| 53 | `sys_mkdir` | path, mode | Create a directory |
| 54 | `sys_rmdir` | path | Remove a directory |
| 55 | `sys_unlink` | path | Delete a file |
| 56 | `sys_rename` | oldpath, newpath | Rename |

```rust
/// Open a file
pub fn sys_open(
    path: *const u8,
    path_len: usize,
    flags: OpenFlags,
    mode: FileMode,
) -> Result<Fd, SyscallError>;

bitflags! {
    pub struct OpenFlags: u32 {
        const RDONLY    = 0;
        const WRONLY    = 1 << 0;
        const RDWR      = 1 << 1;
        const CREAT     = 1 << 2;
        const EXCL      = 1 << 3;
        const TRUNC     = 1 << 4;
        const APPEND    = 1 << 5;
        const NONBLOCK  = 1 << 6;
        const CLOEXEC   = 1 << 7;
    }
}
```

### 4. Threading (70-89)

| No. | Name | Args | Description |
|-----|------|------|-------------|
| 70 | `sys_thread_create` | entry, arg, stack, stack_size | Create a thread |
| 71 | `sys_thread_exit` | retval | Exit a thread |
| 72 | `sys_thread_join` | tid, retval | Join a thread |
| 73 | `sys_thread_detach` | tid | Detach a thread |
| 74 | `sys_thread_self` | - | Return current TID |
| 75 | `sys_futex` | addr, op, val, timeout | Futex operations |
| 76 | `sys_thread_setname` | tid, name | Set a thread name |

```rust
/// Create a thread
pub fn sys_thread_create(
    entry: fn(*mut u8) -> *mut u8,
    arg: *mut u8,
    stack: *mut u8,        // user-provided stack
    stack_size: usize,
) -> Result<Tid, SyscallError>;

/// Futex operations (fundamental synchronization primitive)
pub fn sys_futex(
    addr: *mut u32,
    op: FutexOp,
    val: u32,
    timeout: Option<Duration>,
) -> Result<i32, SyscallError>;

pub enum FutexOp {
    Wait,           // wait if *addr == val
    Wake,           // wake up to val threads
    WakeOp,         // conditional wake
    Requeue,        // move waiters to another futex
}
```

### 5. Networking (90-109)

| No. | Name | Args | Description |
|-----|------|------|-------------|
| 90 | `sys_socket` | domain, type, protocol | Create a socket |
| 91 | `sys_bind` | fd, addr, addrlen | Bind an address |
| 92 | `sys_listen` | fd, backlog | Listen |
| 93 | `sys_accept` | fd, addr, addrlen | Accept a connection |
| 94 | `sys_connect` | fd, addr, addrlen | Connect |
| 95 | `sys_send` | fd, buf, len, flags | Send |
| 96 | `sys_recv` | fd, buf, len, flags | Receive |
| 97 | `sys_sendto` | fd, buf, len, flags, addr | Send UDP |
| 98 | `sys_recvfrom` | fd, buf, len, flags, addr | Receive UDP |
| 99 | `sys_shutdown` | fd, how | Shutdown |
| 100 | `sys_getsockopt` | fd, level, optname, ... | Get options |
| 101 | `sys_setsockopt` | fd, level, optname, ... | Set options |
| 102 | `sys_getpeername` | fd, addr, addrlen | Peer address |
| 103 | `sys_getsockname` | fd, addr, addrlen | Local address |

```rust
/// Create a socket
pub fn sys_socket(
    domain: SocketDomain,   // AF_INET, AF_INET6
    sock_type: SocketType,  // SOCK_STREAM, SOCK_DGRAM
    protocol: u32,
) -> Result<Fd, SyscallError>;

/// TCP connect
pub fn sys_connect(
    fd: Fd,
    addr: *const SocketAddr,
    addrlen: u32,
) -> Result<(), SyscallError>;
```

### 6. Time (110-119)

| No. | Name | Args | Description |
|-----|------|------|-------------|
| 110 | `sys_clock_gettime` | clockid, timespec | Current time |
| 111 | `sys_clock_nanosleep` | clockid, flags, req, rem | High-precision sleep |
| 112 | `sys_gettimeofday` | tv, tz | System time |
| 113 | `sys_timer_create` | clockid, evp, timerid | Create a timer |
| 114 | `sys_timer_settime` | timerid, flags, new, old | Arm/set a timer |
| 115 | `sys_timer_delete` | timerid | Delete a timer |

### 7. Event polling (120-129)

| No. | Name | Args | Description |
|-----|------|------|-------------|
| 120 | `sys_epoll_create` | flags | Create an epoll instance |
| 121 | `sys_epoll_ctl` | epfd, op, fd, event | Control epoll |
| 122 | `sys_epoll_wait` | epfd, events, maxevents, timeout | Wait for events |
| 123 | `sys_eventfd` | initval, flags | Create an eventfd |

```rust
/// Wait for epoll events
pub fn sys_epoll_wait(
    epfd: Fd,
    events: *mut EpollEvent,
    maxevents: i32,
    timeout: i32,  // ms, -1 = wait forever
) -> Result<i32, SyscallError>;
```

---

## KPIO Extension System Calls (200+)

### 8. IPC channels (200-219)

| No. | Name | Description |
|-----|------|-------------|
| 200 | `sys_shm_create` | Create shared memory |
| 201 | `sys_shm_open` | Open shared memory |
| 202 | `sys_shm_map` | Map shared memory |
| 203 | `sys_shm_unlink` | Delete shared memory |
| 210 | `sys_channel_create` | Create an IPC channel |
| 211 | `sys_channel_send` | Send a message to a channel |
| 212 | `sys_channel_recv` | Receive a message from a channel |

```rust
/// Create an IPC channel
pub fn sys_channel_create(
    name: *const u8,
    name_len: usize,
    buffer_size: usize,
) -> Result<ChannelId, SyscallError>;

/// Channel send (zero-copy)
pub fn sys_channel_send(
    channel: ChannelId,
    msg: *const u8,
    len: usize,
    caps: *const CapabilityId,
    cap_count: usize,
) -> Result<(), SyscallError>;
```

### 9. Capabilities (220-229)

| No. | Name | Description |
|-----|------|-------------|
| 220 | `sys_cap_create` | Create a capability |
| 221 | `sys_cap_diminish` | Reduce authority |
| 222 | `sys_cap_transfer` | Transfer a capability |
| 223 | `sys_cap_revoke` | Revoke a capability |
| 224 | `sys_cap_check` | Check permissions |

### 10. GPU (230-249)

| No. | Name | Description |
|-----|------|-------------|
| 230 | `sys_gpu_alloc` | Allocate GPU memory |
| 231 | `sys_gpu_free` | Free GPU memory |
| 232 | `sys_gpu_map` | Map GPU memory |
| 233 | `sys_gpu_submit` | Submit GPU commands |
| 234 | `sys_gpu_wait` | Wait for GPU completion |
| 235 | `sys_gpu_set_priority` | Set per-tab GPU priority |

```rust
/// Submit GPU work (browser-optimized)
pub fn sys_gpu_submit(
    tab_id: u32,
    command_buffer: *const u8,
    len: usize,
    priority: GpuPriority,
) -> Result<GpuFenceId, SyscallError>;

/// Wait for GPU completion
pub fn sys_gpu_wait(
    fence: GpuFenceId,
    timeout_ns: u64,
) -> Result<(), SyscallError>;
```

### 11. Browser-specific (250-269)

| No. | Name | Description |
|-----|------|-------------|
| 250 | `sys_tab_register` | Register a tab |
| 251 | `sys_tab_set_state` | Change tab state |
| 252 | `sys_tab_get_memory` | Get tab memory usage |
| 253 | `sys_wasm_cache_get` | Query WASM AOT cache |
| 254 | `sys_wasm_cache_put` | Store WASM AOT cache |
| 255 | `sys_network_zero_copy` | Zero-copy network request |

```rust
/// Register a tab (notify the kernel about a browser tab)
pub fn sys_tab_register(
    tab_id: u32,
    initial_state: TabState,
) -> Result<(), SyscallError>;

/// Change tab state
pub fn sys_tab_set_state(
    tab_id: u32,
    state: TabState,
) -> Result<(), SyscallError>;

/// Query WASM AOT cache
pub fn sys_wasm_cache_get(
    module_hash: *const [u8; 32],
    out_buffer: *mut u8,
    buffer_len: usize,
) -> Result<Option<usize>, SyscallError>;
```

---

## Error Codes

```rust
#[repr(i32)]
pub enum SyscallError {
    /// Success
    Success = 0,
    
    /// Generic errors
    PermissionDenied = -1,
    NoSuchFile = -2,
    IOError = -3,
    NoMemory = -4,
    InvalidArgument = -5,
    BadFileDescriptor = -6,
    
    /// Blocking/async errors
    WouldBlock = -11,
    Interrupted = -12,
    
    /// Process/thread errors
    NoSuchProcess = -20,
    NoSuchThread = -21,
    
    /// Network errors
    ConnectionRefused = -30,
    ConnectionReset = -31,
    NetworkUnreachable = -32,
    
    /// Resource errors
    TooManyOpenFiles = -40,
    ResourceBusy = -41,
    
    /// KPIO extension errors
    CapabilityDenied = -100,
    ChannelFull = -101,
    ChannelEmpty = -102,
    GpuBusy = -103,
    CacheMiss = -104,
}
```

---

## Minimum System Calls Needed for Servo

**Minimum implementation priority** for running Servo:

### Priority 1 (run html5ever)
1. `sys_mmap` - memory allocation
2. `sys_munmap` - memory release
3. `sys_write` - console output
4. `sys_exit` - process exit
5. `sys_thread_create` - thread creation
6. `sys_futex` - synchronization

### Priority 2 (networking)
7. `sys_socket` - socket creation
8. `sys_connect` - TCP connect
9. `sys_send` / `sys_recv` - data transfer
10. `sys_epoll_*` - async I/O

### Priority 3 (filesystem)
11. `sys_open` / `sys_close`
12. `sys_read` / `sys_write`
13. `sys_stat` / `sys_fstat`

### Priority 4 (GPU)
14. `sys_gpu_alloc`
15. `sys_gpu_submit`
16. `sys_gpu_wait`

---

## Implementation Example

```rust
// kernel/src/syscall/mod.rs

use crate::process::current_task;
use crate::memory::UserPtr;

/// System call handler
#[no_mangle]
pub extern "C" fn syscall_handler(
    num: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
    arg6: usize,
) -> isize {
    match num {
        SYS_EXIT => sys_exit(arg1 as i32),
        SYS_READ => sys_read(arg1 as Fd, UserPtr::new(arg2), arg3),
        SYS_WRITE => sys_write(arg1 as Fd, UserPtr::new(arg2), arg3),
        SYS_MMAP => sys_mmap(
            arg1,
            arg2,
            ProtFlags::from_bits_truncate(arg3 as u32),
            MapFlags::from_bits_truncate(arg4 as u32),
            arg5 as i32,
            arg6 as u64,
        ),
        SYS_THREAD_CREATE => sys_thread_create(
            arg1 as fn(*mut u8) -> *mut u8,
            arg2 as *mut u8,
            arg3 as *mut u8,
            arg4,
        ),
        SYS_FUTEX => sys_futex(
            UserPtr::new(arg1),
            FutexOp::from_u32(arg2 as u32),
            arg3 as u32,
            arg4,
        ),
        // ... more syscalls
        _ => SyscallError::InvalidArgument as isize,
    }
}
```
