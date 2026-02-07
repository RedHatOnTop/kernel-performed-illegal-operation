//! System call handlers.
//!
//! This module implements the handlers for each system call.

use super::{SyscallContext, SyscallError, SyscallNumber, SyscallResult};
use crate::scheduler;
use crate::serial;
use crate::process::ProcessId;
use crate::ipc::{self, ChannelId, Message, IpcError};

/// Handle a system call.
pub fn handle(syscall: SyscallNumber, ctx: &SyscallContext) -> SyscallResult {
    match syscall {
        // Process Management
        SyscallNumber::Exit => handle_exit(ctx),
        SyscallNumber::Write => handle_write(ctx),
        SyscallNumber::Read => handle_read(ctx),
        SyscallNumber::Open => handle_open(ctx),
        SyscallNumber::Close => handle_close(ctx),
        SyscallNumber::Mmap => handle_mmap(ctx),
        SyscallNumber::Munmap => handle_munmap(ctx),
        SyscallNumber::Fork => handle_fork(ctx),
        SyscallNumber::Exec => handle_exec(ctx),
        SyscallNumber::Wait => handle_wait(ctx),
        
        // IPC
        SyscallNumber::ChannelCreate => handle_channel_create(ctx),
        SyscallNumber::ChannelSend => handle_channel_send(ctx),
        SyscallNumber::ChannelRecv => handle_channel_recv(ctx),
        SyscallNumber::ChannelClose => handle_channel_close(ctx),
        SyscallNumber::ShmCreate => handle_shm_create(ctx),
        SyscallNumber::ShmMap => handle_shm_map(ctx),
        SyscallNumber::ShmUnmap => handle_shm_unmap(ctx),
        
        // Process Info & Control
        SyscallNumber::ProcessInfo => handle_process_info(ctx),
        SyscallNumber::Yield => handle_yield(ctx),
        SyscallNumber::Sleep => handle_sleep(ctx),
        SyscallNumber::GetTime => handle_get_time(ctx),
        SyscallNumber::GetPid => handle_getpid(ctx),
        SyscallNumber::GetPpid => handle_getppid(ctx),
        SyscallNumber::Brk => handle_brk(ctx),
        
        // Sockets
        SyscallNumber::SocketCreate => handle_socket_create(ctx),
        SyscallNumber::SocketBind => handle_socket_bind(ctx),
        SyscallNumber::SocketListen => handle_socket_listen(ctx),
        SyscallNumber::SocketAccept => handle_socket_accept(ctx),
        SyscallNumber::SocketConnect => handle_socket_connect(ctx),
        SyscallNumber::SocketSend => handle_socket_send(ctx),
        SyscallNumber::SocketRecv => handle_socket_recv(ctx),
        
        // GPU
        SyscallNumber::GpuAlloc => handle_gpu_alloc(ctx),
        SyscallNumber::GpuSubmit => handle_gpu_submit(ctx),
        SyscallNumber::GpuPresent => handle_gpu_present(ctx),
        SyscallNumber::GpuSetPriority => handle_gpu_set_priority(ctx),
        SyscallNumber::GpuWait => handle_gpu_wait(ctx),
        
        // Threading
        SyscallNumber::ThreadCreate => handle_thread_create(ctx),
        SyscallNumber::ThreadExit => handle_thread_exit(ctx),
        SyscallNumber::ThreadJoin => handle_thread_join(ctx),
        SyscallNumber::FutexWait => handle_futex_wait(ctx),
        SyscallNumber::FutexWake => handle_futex_wake(ctx),
        
        // Epoll
        SyscallNumber::EpollCreate => handle_epoll_create(ctx),
        SyscallNumber::EpollCtl => handle_epoll_ctl(ctx),
        SyscallNumber::EpollWait => handle_epoll_wait(ctx),
        
        // KPIO Extensions
        SyscallNumber::DebugPrint => handle_debug_print(ctx),
        SyscallNumber::TabRegister => handle_tab_register(ctx),
        SyscallNumber::TabSetState => handle_tab_set_state(ctx),
        SyscallNumber::TabGetMemory => handle_tab_get_memory(ctx),
        SyscallNumber::WasmCacheGet => handle_wasm_cache_get(ctx),
        SyscallNumber::WasmCachePut => handle_wasm_cache_put(ctx),
    }
}

/// Exit the current process.
fn handle_exit(ctx: &SyscallContext) -> SyscallResult {
    let exit_code = ctx.arg1 as i32;
    scheduler::exit_current(exit_code);
    Ok(0)
}

/// Write to a file descriptor.
fn handle_write(ctx: &SyscallContext) -> SyscallResult {
    let fd = ctx.arg1;
    let buf_ptr = ctx.arg2 as *const u8;
    let len = ctx.arg3 as usize;
    
    // Validate buffer pointer (simplified - should check page tables)
    if buf_ptr.is_null() {
        return Err(SyscallError::InvalidArgument);
    }
    
    // Special case: stdout (fd 1) and stderr (fd 2) go to serial
    if fd == 1 || fd == 2 {
        let slice = unsafe { core::slice::from_raw_parts(buf_ptr, len) };
        if let Ok(s) = core::str::from_utf8(slice) {
            serial::write_str(s);
        } else {
            for &byte in slice {
                serial::write_byte(byte);
            }
        }
        return Ok(len as u64);
    }
    
    // Other fds: route through VFS
    let slice = unsafe { core::slice::from_raw_parts(buf_ptr, len) };
    match crate::vfs::fd::write(fd as i32, slice) {
        Ok(n) => Ok(n as u64),
        Err(_) => Err(SyscallError::NotFound),
    }
}

/// Read from a file descriptor.
fn handle_read(ctx: &SyscallContext) -> SyscallResult {
    let fd = ctx.arg1;
    let buf_ptr = ctx.arg2 as *mut u8;
    let len = ctx.arg3 as usize;
    
    if buf_ptr.is_null() {
        return Err(SyscallError::InvalidArgument);
    }
    
    // Special case: stdin (fd 0) reads from serial
    if fd == 0 {
        let slice = unsafe { core::slice::from_raw_parts_mut(buf_ptr, len) };
        let mut count = 0;
        for byte in slice.iter_mut() {
            if let Some(b) = serial::try_read_byte() {
                *byte = b;
                count += 1;
            } else {
                break;
            }
        }
        return Ok(count as u64);
    }
    
    // Other fds: route through VFS
    match crate::vfs::fd::read(fd as i32, len) {
        Ok(data) => {
            let copy_len = data.len().min(len);
            let dest = unsafe { core::slice::from_raw_parts_mut(buf_ptr, copy_len) };
            dest.copy_from_slice(&data[..copy_len]);
            Ok(copy_len as u64)
        }
        Err(_) => Err(SyscallError::NotFound),
    }
}

/// Open a file.
fn handle_open(ctx: &SyscallContext) -> SyscallResult {
    let path_ptr = ctx.arg1 as *const u8;
    let path_len = ctx.arg2 as usize;
    let flags = ctx.arg3 as u32;

    if path_ptr.is_null() || path_len == 0 {
        return Err(SyscallError::InvalidArgument);
    }

    let slice = unsafe { core::slice::from_raw_parts(path_ptr, path_len) };
    let path = core::str::from_utf8(slice).map_err(|_| SyscallError::InvalidArgument)?;

    match crate::vfs::fd::open(path, flags) {
        Ok(fd) => Ok(fd as u64),
        Err(_) => Err(SyscallError::NotFound),
    }
}

/// Close a file descriptor.
fn handle_close(ctx: &SyscallContext) -> SyscallResult {
    let fd = ctx.arg1 as i32;
    match crate::vfs::fd::close(fd) {
        Ok(()) => Ok(0),
        Err(_) => Err(SyscallError::InvalidArgument),
    }
}

/// Memory map.
fn handle_mmap(ctx: &SyscallContext) -> SyscallResult {
    let _addr = ctx.arg1;
    let _len = ctx.arg2 as usize;
    let _prot = ctx.arg3 as u32;
    let _flags = ctx.arg4 as u32;
    
    // TODO: Implement memory mapping
    Err(SyscallError::OutOfMemory)
}

/// Memory unmap.
fn handle_munmap(_ctx: &SyscallContext) -> SyscallResult {
    // TODO: Implement memory unmapping
    Ok(0)
}

/// Create an IPC channel.
fn handle_channel_create(_ctx: &SyscallContext) -> SyscallResult {
    match ipc::create_channel() {
        Some((id_a, id_b)) => {
            // Return both endpoints packed into u64
            Ok((id_a.0 << 32) | id_b.0)
        }
        None => Err(SyscallError::OutOfMemory),
    }
}

/// Send an IPC message.
fn handle_channel_send(ctx: &SyscallContext) -> SyscallResult {
    let channel_id = ChannelId(ctx.arg1);
    let buf_ptr = ctx.arg2 as *const u8;
    let len = ctx.arg3 as usize;
    
    if buf_ptr.is_null() {
        return Err(SyscallError::InvalidArgument);
    }
    
    // Copy data from userspace
    let data = unsafe { core::slice::from_raw_parts(buf_ptr, len) };
    let message = Message::with_data(data.to_vec());
    
    match ipc::send(channel_id, message) {
        Ok(()) => Ok(len as u64),
        Err(IpcError::QueueFull) => Err(SyscallError::WouldBlock),
        Err(IpcError::ChannelClosed) => Err(SyscallError::NotConnected),
        Err(IpcError::ChannelNotFound) => Err(SyscallError::NotFound),
        Err(IpcError::MessageTooLarge) => Err(SyscallError::InvalidArgument),
        Err(_) => Err(SyscallError::IoError),
    }
}

/// Receive an IPC message.
fn handle_channel_recv(ctx: &SyscallContext) -> SyscallResult {
    let channel_id = ChannelId(ctx.arg1);
    let buf_ptr = ctx.arg2 as *mut u8;
    let len = ctx.arg3 as usize;
    
    if buf_ptr.is_null() {
        return Err(SyscallError::InvalidArgument);
    }
    
    match ipc::receive(channel_id) {
        Ok(message) => {
            let data = message.data();
            let copy_len = core::cmp::min(data.len(), len);
            let slice = unsafe { core::slice::from_raw_parts_mut(buf_ptr, copy_len) };
            slice.copy_from_slice(&data[..copy_len]);
            Ok(copy_len as u64)
        }
        Err(IpcError::QueueEmpty) => Err(SyscallError::WouldBlock),
        Err(IpcError::ChannelClosed) => Err(SyscallError::NotConnected),
        Err(IpcError::ChannelNotFound) => Err(SyscallError::NotFound),
        Err(_) => Err(SyscallError::IoError),
    }
}

/// Close an IPC channel.
fn handle_channel_close(_ctx: &SyscallContext) -> SyscallResult {
    // TODO: Implement channel closing with proper cleanup
    Ok(0)
}

/// Get process information.
fn handle_process_info(_ctx: &SyscallContext) -> SyscallResult {
    let task_id = scheduler::current_task_id();
    Ok(task_id.0)
}

/// Yield CPU to other tasks.
fn handle_yield(_ctx: &SyscallContext) -> SyscallResult {
    scheduler::yield_now();
    Ok(0)
}

/// Sleep for a duration.
fn handle_sleep(ctx: &SyscallContext) -> SyscallResult {
    let milliseconds = ctx.arg1;
    // APIC timer runs at ~100 Hz, so 1 tick ≈ 10 ms
    let ticks = if milliseconds < 10 { 1 } else { milliseconds / 10 };
    scheduler::sleep_ticks(ticks);
    Ok(0)
}

/// Get current time.
fn handle_get_time(_ctx: &SyscallContext) -> SyscallResult {
    // Return APIC ticks * 10 to approximate milliseconds since boot
    let ticks = scheduler::boot_ticks();
    Ok(ticks * 10)
}

/// Create a socket.
fn handle_socket_create(_ctx: &SyscallContext) -> SyscallResult {
    // TODO: Implement socket creation
    Err(SyscallError::NotFound)
}

/// Bind a socket.
fn handle_socket_bind(_ctx: &SyscallContext) -> SyscallResult {
    Err(SyscallError::NotFound)
}

/// Listen on a socket.
fn handle_socket_listen(_ctx: &SyscallContext) -> SyscallResult {
    Err(SyscallError::NotFound)
}

/// Accept a connection.
fn handle_socket_accept(_ctx: &SyscallContext) -> SyscallResult {
    Err(SyscallError::NotFound)
}

/// Connect to a remote address.
fn handle_socket_connect(_ctx: &SyscallContext) -> SyscallResult {
    Err(SyscallError::NotFound)
}

/// Send data on a socket.
fn handle_socket_send(_ctx: &SyscallContext) -> SyscallResult {
    Err(SyscallError::NotFound)
}

/// Receive data on a socket.
fn handle_socket_recv(_ctx: &SyscallContext) -> SyscallResult {
    Err(SyscallError::NotFound)
}

/// Allocate GPU memory.
fn handle_gpu_alloc(_ctx: &SyscallContext) -> SyscallResult {
    Err(SyscallError::NotFound)
}

/// Submit GPU commands.
fn handle_gpu_submit(_ctx: &SyscallContext) -> SyscallResult {
    Err(SyscallError::NotFound)
}

/// Present a frame.
fn handle_gpu_present(_ctx: &SyscallContext) -> SyscallResult {
    Err(SyscallError::NotFound)
}

/// Debug print.
fn handle_debug_print(ctx: &SyscallContext) -> SyscallResult {
    let buf_ptr = ctx.arg1 as *const u8;
    let len = ctx.arg2 as usize;
    
    if buf_ptr.is_null() {
        return Err(SyscallError::InvalidArgument);
    }
    
    let slice = unsafe { core::slice::from_raw_parts(buf_ptr, len) };
    if let Ok(s) = core::str::from_utf8(slice) {
        serial::write_str(s);
    }
    
    Ok(len as u64)
}

// ==========================================
// Process Management Extensions
// ==========================================

/// Fork the current process.
fn handle_fork(_ctx: &SyscallContext) -> SyscallResult {
    // TODO: Implement copy-on-write fork
    // 1. Clone page tables with COW
    // 2. Copy process state
    // 3. Return 0 to child, child PID to parent
    Err(SyscallError::NotFound)
}

/// Execute a new program.
fn handle_exec(ctx: &SyscallContext) -> SyscallResult {
    let _path_ptr = ctx.arg1 as *const u8;
    let _path_len = ctx.arg2 as usize;
    let _argv_ptr = ctx.arg3 as *const *const u8;
    let _envp_ptr = ctx.arg4 as *const *const u8;
    
    // TODO: Implement exec
    // 1. Load ELF from path
    // 2. Replace current address space
    // 3. Set up new stack with argv/envp
    // 4. Jump to entry point
    Err(SyscallError::NotFound)
}

/// Wait for a child process.
fn handle_wait(ctx: &SyscallContext) -> SyscallResult {
    let pid = ctx.arg1 as i64;
    let _status_ptr = ctx.arg2 as *mut i32;
    let _options = ctx.arg3 as u32;
    
    // pid == -1: wait for any child
    // pid > 0: wait for specific child
    let target_pid = if pid > 0 {
        Some(ProcessId::from_u64(pid as u64))
    } else {
        None
    };
    
    // TODO: Block until child exits
    // For now, check if any zombie children exist
    let _ = target_pid;
    Err(SyscallError::WouldBlock)
}

/// Get current process ID.
fn handle_getpid(_ctx: &SyscallContext) -> SyscallResult {
    let task_id = scheduler::current_task_id();
    Ok(task_id.0)
}

/// Get parent process ID.
fn handle_getppid(_ctx: &SyscallContext) -> SyscallResult {
    // TODO: Look up parent from process table
    Ok(1) // Return init (PID 1) as default parent
}

/// Set program break (heap allocation).
fn handle_brk(ctx: &SyscallContext) -> SyscallResult {
    let new_brk = ctx.arg1;
    
    // TODO: Extend heap in process memory map
    // For now, just return the requested address
    if new_brk == 0 {
        // Query current brk
        Ok(0x1000_0000) // Placeholder
    } else {
        Ok(new_brk)
    }
}

// ==========================================
// Shared Memory
// ==========================================

/// Create shared memory region.
fn handle_shm_create(ctx: &SyscallContext) -> SyscallResult {
    let size = ctx.arg1 as usize;
    let flags = ctx.arg2 as u32;
    
    if size == 0 || size > 1024 * 1024 * 1024 {
        return Err(SyscallError::InvalidArgument);
    }
    
    // Get current process ID
    let pid = scheduler::current_task_id().0;
    
    match ipc::shm::create("anonymous", size, pid, flags) {
        Ok(id) => Ok(id.0),
        Err(ipc::shm::ShmError::OutOfMemory) => Err(SyscallError::OutOfMemory),
        Err(ipc::shm::ShmError::InvalidSize) => Err(SyscallError::InvalidArgument),
        Err(ipc::shm::ShmError::LimitReached) => Err(SyscallError::OutOfMemory),
        Err(_) => Err(SyscallError::IoError),
    }
}

/// Map shared memory into address space.
fn handle_shm_map(ctx: &SyscallContext) -> SyscallResult {
    let shm_id = ipc::ShmId(ctx.arg1);
    let addr_hint = ctx.arg2;
    let prot = ctx.arg3 as u32;
    
    let pid = scheduler::current_task_id().0;
    
    match ipc::shm::map(shm_id, pid, addr_hint, prot) {
        Ok(addr) => Ok(addr),
        Err(ipc::shm::ShmError::NotFound) => Err(SyscallError::NotFound),
        Err(ipc::shm::ShmError::AlreadyMapped) => Err(SyscallError::InvalidArgument),
        Err(ipc::shm::ShmError::PermissionDenied) => Err(SyscallError::PermissionDenied),
        Err(_) => Err(SyscallError::IoError),
    }
}

/// Unmap shared memory.
fn handle_shm_unmap(ctx: &SyscallContext) -> SyscallResult {
    let shm_id = ipc::ShmId(ctx.arg1);
    let _size = ctx.arg2 as usize;
    
    let pid = scheduler::current_task_id().0;
    
    match ipc::shm::unmap(shm_id, pid) {
        Ok(()) => Ok(0),
        Err(ipc::shm::ShmError::NotFound) => Err(SyscallError::NotFound),
        Err(ipc::shm::ShmError::NotMapped) => Err(SyscallError::InvalidArgument),
        Err(_) => Err(SyscallError::IoError),
    }
}

// ==========================================
// Threading
// ==========================================

/// Create a new thread.
fn handle_thread_create(ctx: &SyscallContext) -> SyscallResult {
    let entry_point = ctx.arg1;
    let stack_ptr = ctx.arg2;
    let arg = ctx.arg3;
    let _flags = ctx.arg4 as u32;
    
    if entry_point == 0 || stack_ptr == 0 {
        return Err(SyscallError::InvalidArgument);
    }
    
    // TODO: Create new thread in current process
    // 1. Allocate kernel stack
    // 2. Set up thread context
    // 3. Add to process thread list
    // 4. Make ready to run
    let _ = arg;
    Err(SyscallError::OutOfMemory)
}

/// Exit current thread.
fn handle_thread_exit(ctx: &SyscallContext) -> SyscallResult {
    let exit_code = ctx.arg1 as i32;
    
    // TODO: Mark thread as exited
    // If last thread, exit process
    scheduler::exit_current(exit_code);
    Ok(0)
}

/// Join a thread (wait for it to exit).
fn handle_thread_join(ctx: &SyscallContext) -> SyscallResult {
    let _thread_id = ctx.arg1;
    let _retval_ptr = ctx.arg2 as *mut u64;
    
    // TODO: Block until thread exits
    Err(SyscallError::WouldBlock)
}

/// Futex wait - block until value changes.
fn handle_futex_wait(ctx: &SyscallContext) -> SyscallResult {
    let futex_addr = ctx.arg1;
    let expected_val = ctx.arg2 as u32;
    let _timeout_ns = ctx.arg3;
    
    if futex_addr == 0 || futex_addr % 4 != 0 {
        return Err(SyscallError::InvalidArgument);
    }
    
    // Read current value
    let current = unsafe { *(futex_addr as *const u32) };
    
    if current != expected_val {
        // Value already changed, return immediately
        return Err(SyscallError::WouldBlock);
    }
    
    // TODO: Add to futex wait queue and block
    Err(SyscallError::WouldBlock)
}

/// Futex wake - wake waiting threads.
fn handle_futex_wake(ctx: &SyscallContext) -> SyscallResult {
    let futex_addr = ctx.arg1;
    let num_wake = ctx.arg2 as u32;
    
    if futex_addr == 0 || futex_addr % 4 != 0 {
        return Err(SyscallError::InvalidArgument);
    }
    
    // TODO: Wake up to num_wake threads from wait queue
    let _ = num_wake;
    Ok(0) // Return number of threads woken
}

// ==========================================
// Epoll
// ==========================================

/// Create epoll instance.
fn handle_epoll_create(_ctx: &SyscallContext) -> SyscallResult {
    // TODO: Allocate epoll file descriptor
    Err(SyscallError::OutOfMemory)
}

/// Control epoll (add/modify/delete).
fn handle_epoll_ctl(ctx: &SyscallContext) -> SyscallResult {
    let _epfd = ctx.arg1;
    let _op = ctx.arg2 as u32; // EPOLL_CTL_ADD, MOD, DEL
    let _fd = ctx.arg3;
    let _event_ptr = ctx.arg4 as *const u8;
    
    // TODO: Modify epoll interest list
    Err(SyscallError::NotFound)
}

/// Wait for epoll events.
fn handle_epoll_wait(ctx: &SyscallContext) -> SyscallResult {
    let _epfd = ctx.arg1;
    let _events_ptr = ctx.arg2 as *mut u8;
    let _max_events = ctx.arg3 as i32;
    let _timeout_ms = ctx.arg4 as i32;
    
    // TODO: Block until events are ready
    Err(SyscallError::WouldBlock)
}

// ==========================================
// GPU Extensions
// ==========================================

/// Set GPU scheduling priority.
fn handle_gpu_set_priority(ctx: &SyscallContext) -> SyscallResult {
    let _priority = ctx.arg1 as u32;
    
    // TODO: Update process GPU priority
    Ok(0)
}

/// Wait for GPU fence.
fn handle_gpu_wait(ctx: &SyscallContext) -> SyscallResult {
    let _fence_id = ctx.arg1;
    let _timeout_ns = ctx.arg2;
    
    // TODO: Block until GPU completes
    Err(SyscallError::WouldBlock)
}

// ==========================================
// KPIO Browser Extensions
// ==========================================

/// Register a browser tab.
fn handle_tab_register(ctx: &SyscallContext) -> SyscallResult {
    let _tab_type = ctx.arg1 as u32; // 0=renderer, 1=network, etc.
    
    // TODO: Register tab process with browser coordinator
    // Return tab ID
    Ok(0)
}

/// Set tab state.
fn handle_tab_set_state(ctx: &SyscallContext) -> SyscallResult {
    let _tab_id = ctx.arg1;
    let _state = ctx.arg2 as u32; // 0=loading, 1=ready, 2=crashed
    
    // TODO: Update tab state in coordinator
    Ok(0)
}

/// Get tab memory usage.
fn handle_tab_get_memory(ctx: &SyscallContext) -> SyscallResult {
    let _tab_id = ctx.arg1;
    
    // TODO: Return memory usage for tab
    // Query process memory from process manager
    Ok(0)
}

/// WASM cache lookup.
fn handle_wasm_cache_get(ctx: &SyscallContext) -> SyscallResult {
    let _hash_ptr = ctx.arg1 as *const u8;
    let _hash_len = ctx.arg2 as usize;
    let _out_ptr = ctx.arg3 as *mut u8;
    let _out_len = ctx.arg4 as usize;
    
    // TODO: Look up compiled WASM in cache
    Err(SyscallError::NotFound)
}

/// WASM cache store.
fn handle_wasm_cache_put(ctx: &SyscallContext) -> SyscallResult {
    let _hash_ptr = ctx.arg1 as *const u8;
    let _hash_len = ctx.arg2 as usize;
    let _data_ptr = ctx.arg3 as *const u8;
    let _data_len = ctx.arg4 as usize;
    
    // TODO: Store compiled WASM in cache
    Ok(0)
}
