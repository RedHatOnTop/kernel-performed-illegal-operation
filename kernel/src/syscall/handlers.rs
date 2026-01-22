//! System call handlers.
//!
//! This module implements the handlers for each system call.

use super::{SyscallContext, SyscallError, SyscallNumber, SyscallResult};
use crate::scheduler;
use crate::ipc;
use crate::serial;

/// Handle a system call.
pub fn handle(syscall: SyscallNumber, ctx: &SyscallContext) -> SyscallResult {
    match syscall {
        SyscallNumber::Exit => handle_exit(ctx),
        SyscallNumber::Write => handle_write(ctx),
        SyscallNumber::Read => handle_read(ctx),
        SyscallNumber::Open => handle_open(ctx),
        SyscallNumber::Close => handle_close(ctx),
        SyscallNumber::Mmap => handle_mmap(ctx),
        SyscallNumber::Munmap => handle_munmap(ctx),
        SyscallNumber::ChannelCreate => handle_channel_create(ctx),
        SyscallNumber::ChannelSend => handle_channel_send(ctx),
        SyscallNumber::ChannelRecv => handle_channel_recv(ctx),
        SyscallNumber::ChannelClose => handle_channel_close(ctx),
        SyscallNumber::ProcessInfo => handle_process_info(ctx),
        SyscallNumber::Yield => handle_yield(ctx),
        SyscallNumber::Sleep => handle_sleep(ctx),
        SyscallNumber::GetTime => handle_get_time(ctx),
        SyscallNumber::SocketCreate => handle_socket_create(ctx),
        SyscallNumber::SocketBind => handle_socket_bind(ctx),
        SyscallNumber::SocketListen => handle_socket_listen(ctx),
        SyscallNumber::SocketAccept => handle_socket_accept(ctx),
        SyscallNumber::SocketConnect => handle_socket_connect(ctx),
        SyscallNumber::SocketSend => handle_socket_send(ctx),
        SyscallNumber::SocketRecv => handle_socket_recv(ctx),
        SyscallNumber::GpuAlloc => handle_gpu_alloc(ctx),
        SyscallNumber::GpuSubmit => handle_gpu_submit(ctx),
        SyscallNumber::GpuPresent => handle_gpu_present(ctx),
        SyscallNumber::DebugPrint => handle_debug_print(ctx),
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
    
    // TODO: Handle other file descriptors
    Err(SyscallError::NotFound)
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
    
    // TODO: Handle other file descriptors
    Err(SyscallError::NotFound)
}

/// Open a file.
fn handle_open(_ctx: &SyscallContext) -> SyscallResult {
    // TODO: Implement file opening with capability check
    Err(SyscallError::NotFound)
}

/// Close a file descriptor.
fn handle_close(_ctx: &SyscallContext) -> SyscallResult {
    // TODO: Implement file closing
    Ok(0)
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
            // Return both endpoints (packed into u64)
            Ok((id_a.0 << 32) | id_b.0)
        }
        None => Err(SyscallError::OutOfMemory),
    }
}

/// Send an IPC message.
fn handle_channel_send(ctx: &SyscallContext) -> SyscallResult {
    let channel_id = ipc::ChannelId(ctx.arg1);
    let buf_ptr = ctx.arg2 as *const u8;
    let len = ctx.arg3 as usize;
    
    if buf_ptr.is_null() {
        return Err(SyscallError::InvalidArgument);
    }
    
    let data = unsafe { core::slice::from_raw_parts(buf_ptr, len) };
    let message = ipc::Message::data(data.to_vec());
    
    match ipc::send(channel_id, message) {
        Ok(()) => Ok(len as u64),
        Err(ipc::IpcError::QueueFull) => Err(SyscallError::WouldBlock),
        Err(ipc::IpcError::ChannelClosed) => Err(SyscallError::NotConnected),
        Err(_) => Err(SyscallError::IoError),
    }
}

/// Receive an IPC message.
fn handle_channel_recv(ctx: &SyscallContext) -> SyscallResult {
    let channel_id = ipc::ChannelId(ctx.arg1);
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
        Err(ipc::IpcError::QueueEmpty) => Err(SyscallError::WouldBlock),
        Err(ipc::IpcError::ChannelClosed) => Err(SyscallError::NotConnected),
        Err(_) => Err(SyscallError::IoError),
    }
}

/// Close an IPC channel.
fn handle_channel_close(_ctx: &SyscallContext) -> SyscallResult {
    // TODO: Implement channel closing
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
    let _milliseconds = ctx.arg1;
    // TODO: Implement sleep with timer
    scheduler::yield_now();
    Ok(0)
}

/// Get current time.
fn handle_get_time(_ctx: &SyscallContext) -> SyscallResult {
    // TODO: Implement actual time retrieval
    // For now, return a placeholder
    Ok(0)
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
