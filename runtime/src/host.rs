//! Host function bindings for kernel services.
//!
//! This module provides the bridge between WASM modules and
//! kernel services through host functions. WASI functions read/write
//! linear memory for buffer passing and call into `WasiCtx` for
//! actual filesystem, clock, and process operations.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::executor::ExecutorContext;
use crate::instance::Imports;
use crate::interpreter::{TrapError, WasmValue};
use crate::wasi::{ClockId, FdFlags, FdRights, LookupFlags, OFlags, Whence};

// ─── Memory Helpers ────────────────────────────────────────────────

/// Read a u32 from linear memory at the given offset.
fn mem_read_u32(ctx: &ExecutorContext, addr: u32) -> Result<u32, TrapError> {
    let mem = ctx.memories.first().ok_or(TrapError::ExecutionError(
        String::from("no linear memory"),
    ))?;
    let bytes = mem.read_bytes(addr as usize, 4).map_err(|_| {
        TrapError::MemoryOutOfBounds {
            offset: addr as usize,
            size: 4,
            memory_size: mem.size(),
        }
    })?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// Write a u32 to linear memory at the given offset.
fn mem_write_u32(ctx: &mut ExecutorContext, addr: u32, val: u32) -> Result<(), TrapError> {
    let mem = ctx.memories.first_mut().ok_or(TrapError::ExecutionError(
        String::from("no linear memory"),
    ))?;
    mem.write_bytes(addr as usize, &val.to_le_bytes())
        .map_err(|_| TrapError::MemoryOutOfBounds {
            offset: addr as usize,
            size: 4,
            memory_size: mem.size(),
        })
}

/// Write a u64 to linear memory at the given offset.
fn mem_write_u64(ctx: &mut ExecutorContext, addr: u32, val: u64) -> Result<(), TrapError> {
    let mem = ctx.memories.first_mut().ok_or(TrapError::ExecutionError(
        String::from("no linear memory"),
    ))?;
    mem.write_bytes(addr as usize, &val.to_le_bytes())
        .map_err(|_| TrapError::MemoryOutOfBounds {
            offset: addr as usize,
            size: 8,
            memory_size: mem.size(),
        })
}

/// Read a byte slice from linear memory.
fn mem_read_bytes(ctx: &ExecutorContext, addr: u32, len: u32) -> Result<Vec<u8>, TrapError> {
    let mem = ctx.memories.first().ok_or(TrapError::ExecutionError(
        String::from("no linear memory"),
    ))?;
    let bytes = mem.read_bytes(addr as usize, len as usize).map_err(|_| {
        TrapError::MemoryOutOfBounds {
            offset: addr as usize,
            size: len as usize,
            memory_size: mem.size(),
        }
    })?;
    Ok(bytes.to_vec())
}

/// Write bytes to linear memory.
fn mem_write_bytes(ctx: &mut ExecutorContext, addr: u32, data: &[u8]) -> Result<(), TrapError> {
    let mem = ctx.memories.first_mut().ok_or(TrapError::ExecutionError(
        String::from("no linear memory"),
    ))?;
    mem.write_bytes(addr as usize, data).map_err(|_| {
        TrapError::MemoryOutOfBounds {
            offset: addr as usize,
            size: data.len(),
            memory_size: mem.size(),
        }
    })
}

/// Read iov (scatter/gather) data from linear memory.
/// Each iov entry is { buf_ptr: u32, buf_len: u32 } = 8 bytes.
fn read_iovs(ctx: &ExecutorContext, iovs_ptr: u32, iovs_cnt: u32) -> Result<Vec<u8>, TrapError> {
    let mut data = Vec::new();
    for i in 0..iovs_cnt {
        let iov_addr = iovs_ptr + i * 8;
        let buf_ptr = mem_read_u32(ctx, iov_addr)?;
        let buf_len = mem_read_u32(ctx, iov_addr + 4)?;
        let chunk = mem_read_bytes(ctx, buf_ptr, buf_len)?;
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

/// Write data into iov buffers in linear memory. Returns bytes written.
fn write_to_iovs(
    ctx: &mut ExecutorContext,
    iovs_ptr: u32,
    iovs_cnt: u32,
    data: &[u8],
) -> Result<u32, TrapError> {
    let mut written = 0u32;
    let mut data_offset = 0usize;

    for i in 0..iovs_cnt {
        if data_offset >= data.len() {
            break;
        }
        let iov_addr = iovs_ptr + i * 8;
        let buf_ptr = mem_read_u32(ctx, iov_addr)?;
        let buf_len = mem_read_u32(ctx, iov_addr + 4)? as usize;

        let to_write = core::cmp::min(buf_len, data.len() - data_offset);
        mem_write_bytes(ctx, buf_ptr, &data[data_offset..data_offset + to_write])?;
        data_offset += to_write;
        written += to_write as u32;
    }

    Ok(written)
}

/// Read a string from linear memory.
fn mem_read_string(ctx: &ExecutorContext, ptr: u32, len: u32) -> Result<String, TrapError> {
    let bytes = mem_read_bytes(ctx, ptr, len)?;
    String::from_utf8(bytes).map_err(|_| {
        TrapError::ExecutionError(String::from("invalid UTF-8 in WASI string"))
    })
}

/// Helper to get a WasmValue as i32 with a default.
fn arg_i32(args: &[WasmValue], idx: usize) -> i32 {
    args.get(idx).and_then(|v| v.as_i32()).unwrap_or(0)
}

/// Helper to get a WasmValue as i64 with a default.
fn arg_i64(args: &[WasmValue], idx: usize) -> i64 {
    args.get(idx).and_then(|v| v.as_i64()).unwrap_or(0)
}

// ─── Registration ──────────────────────────────────────────────────

/// Register all host functions.
pub fn register_all(imports: &mut Imports) {
    register_wasi_functions(imports);
    register_kpio_functions(imports);
    register_graphics_functions(imports);
    register_network_functions(imports);
}

/// Register WASI Preview 1 functions.
fn register_wasi_functions(imports: &mut Imports) {
    imports.add_function("wasi_snapshot_preview1", "args_get", host_args_get);
    imports.add_function("wasi_snapshot_preview1", "args_sizes_get", host_args_sizes_get);
    imports.add_function("wasi_snapshot_preview1", "environ_get", host_environ_get);
    imports.add_function("wasi_snapshot_preview1", "environ_sizes_get", host_environ_sizes_get);
    imports.add_function("wasi_snapshot_preview1", "clock_time_get", host_clock_time_get);
    imports.add_function("wasi_snapshot_preview1", "fd_close", host_fd_close);
    imports.add_function("wasi_snapshot_preview1", "fd_read", host_fd_read);
    imports.add_function("wasi_snapshot_preview1", "fd_write", host_fd_write);
    imports.add_function("wasi_snapshot_preview1", "fd_seek", host_fd_seek);
    imports.add_function("wasi_snapshot_preview1", "fd_tell", host_fd_tell);
    imports.add_function("wasi_snapshot_preview1", "fd_fdstat_get", host_fd_fdstat_get);
    imports.add_function("wasi_snapshot_preview1", "fd_prestat_get", host_fd_prestat_get);
    imports.add_function("wasi_snapshot_preview1", "fd_prestat_dir_name", host_fd_prestat_dir_name);
    imports.add_function("wasi_snapshot_preview1", "fd_readdir", host_fd_readdir);
    imports.add_function("wasi_snapshot_preview1", "path_open", host_path_open);
    imports.add_function("wasi_snapshot_preview1", "path_create_directory", host_path_create_directory);
    imports.add_function("wasi_snapshot_preview1", "path_remove_directory", host_path_remove_directory);
    imports.add_function("wasi_snapshot_preview1", "path_unlink_file", host_path_unlink_file);
    imports.add_function("wasi_snapshot_preview1", "path_rename", host_path_rename);
    imports.add_function("wasi_snapshot_preview1", "path_filestat_get", host_path_filestat_get);
    imports.add_function("wasi_snapshot_preview1", "proc_exit", host_proc_exit);
    imports.add_function("wasi_snapshot_preview1", "random_get", host_random_get);
}

/// Register KPIO-specific functions.
fn register_kpio_functions(imports: &mut Imports) {
    imports.add_function("kpio", "ipc_send", host_ipc_send);
    imports.add_function("kpio", "ipc_recv", host_ipc_recv);
    imports.add_function("kpio", "ipc_create_channel", host_ipc_create_channel);
    imports.add_function("kpio", "process_spawn", host_process_spawn);
    imports.add_function("kpio", "capability_derive", host_capability_derive);
}

/// Register graphics functions.
fn register_graphics_functions(imports: &mut Imports) {
    imports.add_function("kpio_gpu", "create_surface", host_gpu_create_surface);
    imports.add_function("kpio_gpu", "create_buffer", host_gpu_create_buffer);
    imports.add_function("kpio_gpu", "submit_commands", host_gpu_submit_commands);
    imports.add_function("kpio_gpu", "present", host_gpu_present);
}

/// Register network functions.
fn register_network_functions(imports: &mut Imports) {
    imports.add_function("kpio_net", "socket_create", host_socket_create);
    imports.add_function("kpio_net", "socket_bind", host_socket_bind);
    imports.add_function("kpio_net", "socket_connect", host_socket_connect);
    imports.add_function("kpio_net", "socket_send", host_socket_send);
    imports.add_function("kpio_net", "socket_recv", host_socket_recv);
}

// ─── WASI Implementations ─────────────────────────────────────────

/// fd_write(fd, iovs_ptr, iovs_cnt, nwritten_ptr) -> errno
fn host_fd_write(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let fd = arg_i32(args, 0) as u32;
    let iovs_ptr = arg_i32(args, 1) as u32;
    let iovs_cnt = arg_i32(args, 2) as u32;
    let nwritten_ptr = arg_i32(args, 3) as u32;

    // Read iov data from linear memory
    let data = read_iovs(ctx, iovs_ptr, iovs_cnt)?;

    // For stdout/stderr, also capture in ExecutorContext buffers
    if fd == 1 {
        ctx.stdout.extend_from_slice(&data);
    } else if fd == 2 {
        ctx.stderr.extend_from_slice(&data);
    }

    // Call WasiCtx
    let result = if let Some(ref mut wasi) = ctx.wasi_ctx {
        wasi.fd_write(fd, &data)
    } else {
        // No WASI context: stdout/stderr still work
        if fd == 1 || fd == 2 {
            Ok(data.len())
        } else {
            Ok(0)
        }
    };

    match result {
        Ok(n) => {
            mem_write_u32(ctx, nwritten_ptr, n as u32)?;
            Ok(vec![WasmValue::I32(0)]) // ESUCCESS
        }
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// fd_read(fd, iovs_ptr, iovs_cnt, nread_ptr) -> errno
fn host_fd_read(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let fd = arg_i32(args, 0) as u32;
    let iovs_ptr = arg_i32(args, 1) as u32;
    let iovs_cnt = arg_i32(args, 2) as u32;
    let nread_ptr = arg_i32(args, 3) as u32;

    // Calculate total buffer size from iovs
    let mut total_buf_size = 0u32;
    for i in 0..iovs_cnt {
        let buf_len = mem_read_u32(ctx, iovs_ptr + i * 8 + 4)?;
        total_buf_size += buf_len;
    }

    let wasi = match ctx.wasi_ctx {
        Some(ref mut w) => w,
        None => {
            mem_write_u32(ctx, nread_ptr, 0)?;
            return Ok(vec![WasmValue::I32(0)]);
        }
    };

    // Read from WASI into a temp buffer
    let mut read_buf = vec![0u8; total_buf_size as usize];
    let result = wasi.fd_read(fd, &mut read_buf);

    match result {
        Ok(n) => {
            let data = &read_buf[..n];
            let written = write_to_iovs(ctx, iovs_ptr, iovs_cnt, data)?;
            mem_write_u32(ctx, nread_ptr, written)?;
            Ok(vec![WasmValue::I32(0)])
        }
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// fd_close(fd) -> errno
fn host_fd_close(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let fd = arg_i32(args, 0) as u32;
    let result = if let Some(ref mut wasi) = ctx.wasi_ctx {
        wasi.fd_close(fd)
    } else {
        Ok(())
    };
    match result {
        Ok(()) => Ok(vec![WasmValue::I32(0)]),
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// fd_seek(fd, offset_lo, offset_hi, whence, newoffset_ptr) -> errno
/// Note: WASI ABI splits i64 offset into two i32 args on 32-bit platforms.
/// But the standard signature is: fd_seek(fd: i32, offset: i64, whence: i32, newoffset_ptr: i32) -> errno
fn host_fd_seek(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let fd = arg_i32(args, 0) as u32;
    let offset = arg_i64(args, 1);
    let whence_val = arg_i32(args, 2) as u8;
    let newoffset_ptr = arg_i32(args, 3) as u32;

    let whence = Whence::from_u8(whence_val).unwrap_or(Whence::Set);

    let result = if let Some(ref mut wasi) = ctx.wasi_ctx {
        wasi.fd_seek(fd, offset, whence)
    } else {
        Ok(0)
    };

    match result {
        Ok(new_offset) => {
            mem_write_u64(ctx, newoffset_ptr, new_offset)?;
            Ok(vec![WasmValue::I32(0)])
        }
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// fd_tell(fd, offset_ptr) -> errno
fn host_fd_tell(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let fd = arg_i32(args, 0) as u32;
    let offset_ptr = arg_i32(args, 1) as u32;

    let result = if let Some(ref wasi) = ctx.wasi_ctx {
        wasi.fd_tell(fd)
    } else {
        Ok(0)
    };

    match result {
        Ok(offset) => {
            mem_write_u64(ctx, offset_ptr, offset)?;
            Ok(vec![WasmValue::I32(0)])
        }
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// fd_fdstat_get(fd, fdstat_ptr) -> errno
/// FdStat layout: filetype(1) + pad(1) + flags(2) + rights_base(8) + rights_inheriting(8) = 24 bytes
fn host_fd_fdstat_get(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let fd = arg_i32(args, 0) as u32;
    let fdstat_ptr = arg_i32(args, 1) as u32;

    let result = if let Some(ref wasi) = ctx.wasi_ctx {
        wasi.fd_fdstat_get(fd)
    } else {
        return Ok(vec![WasmValue::I32(8)]); // EBADF
    };

    match result {
        Ok(stat) => {
            let mut buf = [0u8; 24];
            buf[0] = stat.fs_filetype as u8;
            buf[1] = 0; // padding
            buf[2..4].copy_from_slice(&(stat.fs_flags.bits()).to_le_bytes());
            buf[8..16].copy_from_slice(&stat.fs_rights_base.bits().to_le_bytes());
            buf[16..24].copy_from_slice(&stat.fs_rights_inheriting.bits().to_le_bytes());
            mem_write_bytes(ctx, fdstat_ptr, &buf)?;
            Ok(vec![WasmValue::I32(0)])
        }
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// fd_prestat_get(fd, prestat_ptr) -> errno
/// Prestat layout: tag(1) + pad(3) + dir_name_len(4) = 8 bytes
fn host_fd_prestat_get(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let fd = arg_i32(args, 0) as u32;
    let prestat_ptr = arg_i32(args, 1) as u32;

    let result = if let Some(ref wasi) = ctx.wasi_ctx {
        wasi.fd_prestat_get(fd)
    } else {
        return Ok(vec![WasmValue::I32(8)]); // EBADF
    };

    match result {
        Ok(prestat) => {
            let mut buf = [0u8; 8];
            buf[0] = prestat.tag as u8;
            let name_len = unsafe { prestat.inner.dir_name_len } as u32;
            buf[4..8].copy_from_slice(&name_len.to_le_bytes());
            mem_write_bytes(ctx, prestat_ptr, &buf)?;
            Ok(vec![WasmValue::I32(0)])
        }
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// fd_prestat_dir_name(fd, path_ptr, path_len) -> errno
fn host_fd_prestat_dir_name(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let fd = arg_i32(args, 0) as u32;
    let path_ptr = arg_i32(args, 1) as u32;
    let path_len = arg_i32(args, 2) as u32;

    let wasi = match ctx.wasi_ctx {
        Some(ref wasi) => wasi,
        None => return Ok(vec![WasmValue::I32(8)]), // EBADF
    };

    let mut buf = vec![0u8; path_len as usize];
    let result = wasi.fd_prestat_dir_name(fd, &mut buf);

    match result {
        Ok(()) => {
            mem_write_bytes(ctx, path_ptr, &buf)?;
            Ok(vec![WasmValue::I32(0)])
        }
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// fd_readdir(fd, buf_ptr, buf_len, cookie, bufused_ptr) -> errno
fn host_fd_readdir(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let fd = arg_i32(args, 0) as u32;
    let buf_ptr = arg_i32(args, 1) as u32;
    let buf_len = arg_i32(args, 2) as u32;
    let cookie = arg_i64(args, 3) as u64;
    let bufused_ptr = arg_i32(args, 4) as u32;

    let wasi = match ctx.wasi_ctx {
        Some(ref wasi) => wasi,
        None => return Ok(vec![WasmValue::I32(8)]),
    };

    let mut buf = vec![0u8; buf_len as usize];
    let result = wasi.fd_readdir(fd, &mut buf, cookie);

    match result {
        Ok(n) => {
            mem_write_bytes(ctx, buf_ptr, &buf[..n])?;
            mem_write_u32(ctx, bufused_ptr, n as u32)?;
            Ok(vec![WasmValue::I32(0)])
        }
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// path_open(dirfd, dirflags, path_ptr, path_len, oflags, fs_rights_base,
///           fs_rights_inheriting, fdflags, fd_ptr) -> errno
fn host_path_open(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let dir_fd = arg_i32(args, 0) as u32;
    let dirflags = arg_i32(args, 1) as u32;
    let path_ptr = arg_i32(args, 2) as u32;
    let path_len = arg_i32(args, 3) as u32;
    let oflags = arg_i32(args, 4) as u16;
    let rights_base = arg_i64(args, 5) as u64;
    let _rights_inheriting = arg_i64(args, 6) as u64;
    let fdflags = arg_i32(args, 7) as u16;
    let fd_ptr = arg_i32(args, 8) as u32;

    let path = mem_read_string(ctx, path_ptr, path_len)?;

    let wasi = match ctx.wasi_ctx {
        Some(ref mut w) => w,
        None => return Ok(vec![WasmValue::I32(52)]), // ENOSYS
    };

    let result = wasi.path_open(
        dir_fd,
        LookupFlags::from_bits_truncate(dirflags),
        &path,
        OFlags::from_bits_truncate(oflags),
        FdRights::from_bits_truncate(rights_base),
        FdRights::empty(),
        FdFlags::from_bits_truncate(fdflags),
    );

    match result {
        Ok(new_fd) => {
            mem_write_u32(ctx, fd_ptr, new_fd)?;
            Ok(vec![WasmValue::I32(0)])
        }
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// path_create_directory(dirfd, path_ptr, path_len) -> errno
fn host_path_create_directory(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let dir_fd = arg_i32(args, 0) as u32;
    let path_ptr = arg_i32(args, 1) as u32;
    let path_len = arg_i32(args, 2) as u32;

    let path = mem_read_string(ctx, path_ptr, path_len)?;

    let result = if let Some(ref mut wasi) = ctx.wasi_ctx {
        wasi.path_create_directory(dir_fd, &path)
    } else {
        return Ok(vec![WasmValue::I32(52)]);
    };

    match result {
        Ok(()) => Ok(vec![WasmValue::I32(0)]),
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// path_remove_directory(dirfd, path_ptr, path_len) -> errno
fn host_path_remove_directory(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let dir_fd = arg_i32(args, 0) as u32;
    let path_ptr = arg_i32(args, 1) as u32;
    let path_len = arg_i32(args, 2) as u32;

    let path = mem_read_string(ctx, path_ptr, path_len)?;

    let result = if let Some(ref mut wasi) = ctx.wasi_ctx {
        wasi.path_remove_directory(dir_fd, &path)
    } else {
        return Ok(vec![WasmValue::I32(52)]);
    };

    match result {
        Ok(()) => Ok(vec![WasmValue::I32(0)]),
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// path_unlink_file(dirfd, path_ptr, path_len) -> errno
fn host_path_unlink_file(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let dir_fd = arg_i32(args, 0) as u32;
    let path_ptr = arg_i32(args, 1) as u32;
    let path_len = arg_i32(args, 2) as u32;

    let path = mem_read_string(ctx, path_ptr, path_len)?;

    let result = if let Some(ref mut wasi) = ctx.wasi_ctx {
        wasi.path_unlink_file(dir_fd, &path)
    } else {
        return Ok(vec![WasmValue::I32(52)]);
    };

    match result {
        Ok(()) => Ok(vec![WasmValue::I32(0)]),
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// path_rename(old_dirfd, old_path_ptr, old_path_len, new_dirfd, new_path_ptr, new_path_len) -> errno
fn host_path_rename(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let old_dir_fd = arg_i32(args, 0) as u32;
    let old_path_ptr = arg_i32(args, 1) as u32;
    let old_path_len = arg_i32(args, 2) as u32;
    let new_dir_fd = arg_i32(args, 3) as u32;
    let new_path_ptr = arg_i32(args, 4) as u32;
    let new_path_len = arg_i32(args, 5) as u32;

    let old_path = mem_read_string(ctx, old_path_ptr, old_path_len)?;
    let new_path = mem_read_string(ctx, new_path_ptr, new_path_len)?;

    let result = if let Some(ref mut wasi) = ctx.wasi_ctx {
        wasi.path_rename(old_dir_fd, &old_path, new_dir_fd, &new_path)
    } else {
        return Ok(vec![WasmValue::I32(52)]);
    };

    match result {
        Ok(()) => Ok(vec![WasmValue::I32(0)]),
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// path_filestat_get(dirfd, flags, path_ptr, path_len, filestat_ptr) -> errno
/// FileStat layout: dev(8) + ino(8) + filetype(1) + pad(7) + nlink(8) + size(8) + atim(8) + mtim(8) + ctim(8) = 64 bytes
fn host_path_filestat_get(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let dir_fd = arg_i32(args, 0) as u32;
    let flags = arg_i32(args, 1) as u32;
    let path_ptr = arg_i32(args, 2) as u32;
    let path_len = arg_i32(args, 3) as u32;
    let filestat_ptr = arg_i32(args, 4) as u32;

    let path = mem_read_string(ctx, path_ptr, path_len)?;

    let wasi = match ctx.wasi_ctx {
        Some(ref wasi) => wasi,
        None => return Ok(vec![WasmValue::I32(52)]),
    };

    let result = wasi.path_filestat_get(dir_fd, LookupFlags::from_bits_truncate(flags), &path);

    match result {
        Ok(stat) => {
            let mut buf = [0u8; 64];
            buf[0..8].copy_from_slice(&stat.dev.to_le_bytes());
            buf[8..16].copy_from_slice(&stat.ino.to_le_bytes());
            buf[16] = stat.filetype;
            // pad bytes 17..24 are zero
            buf[24..32].copy_from_slice(&stat.nlink.to_le_bytes());
            buf[32..40].copy_from_slice(&stat.size.to_le_bytes());
            buf[40..48].copy_from_slice(&stat.atim.to_le_bytes());
            buf[48..56].copy_from_slice(&stat.mtim.to_le_bytes());
            buf[56..64].copy_from_slice(&stat.ctim.to_le_bytes());
            mem_write_bytes(ctx, filestat_ptr, &buf)?;
            Ok(vec![WasmValue::I32(0)])
        }
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// args_sizes_get(argc_ptr, argv_buf_size_ptr) -> errno
fn host_args_sizes_get(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let argc_ptr = arg_i32(args, 0) as u32;
    let buf_size_ptr = arg_i32(args, 1) as u32;

    let (argc, buf_size) = if let Some(ref wasi) = ctx.wasi_ctx {
        wasi.args_sizes_get()
    } else {
        (0, 0)
    };

    mem_write_u32(ctx, argc_ptr, argc as u32)?;
    mem_write_u32(ctx, buf_size_ptr, buf_size as u32)?;
    Ok(vec![WasmValue::I32(0)])
}

/// args_get(argv_ptr, argv_buf_ptr) -> errno
/// Writes pointers to arg strings into argv_ptr array,
/// and the actual NUL-terminated strings into argv_buf_ptr buffer.
fn host_args_get(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let argv_ptr = arg_i32(args, 0) as u32;
    let argv_buf_ptr = arg_i32(args, 1) as u32;

    let wasi_args: Vec<String> = if let Some(ref wasi) = ctx.wasi_ctx {
        wasi.args_get().to_vec()
    } else {
        Vec::new()
    };

    let mut buf_offset = argv_buf_ptr;
    for (i, arg) in wasi_args.iter().enumerate() {
        // Write pointer to this arg into argv array
        mem_write_u32(ctx, argv_ptr + (i as u32) * 4, buf_offset)?;
        // Write the arg string + NUL terminator into buffer
        let bytes = arg.as_bytes();
        mem_write_bytes(ctx, buf_offset, bytes)?;
        mem_write_bytes(ctx, buf_offset + bytes.len() as u32, &[0])?; // NUL terminator
        buf_offset += bytes.len() as u32 + 1;
    }

    Ok(vec![WasmValue::I32(0)])
}

/// environ_sizes_get(environc_ptr, environ_buf_size_ptr) -> errno
fn host_environ_sizes_get(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let environc_ptr = arg_i32(args, 0) as u32;
    let buf_size_ptr = arg_i32(args, 1) as u32;

    let (count, buf_size) = if let Some(ref wasi) = ctx.wasi_ctx {
        wasi.environ_sizes_get()
    } else {
        (0, 0)
    };

    mem_write_u32(ctx, environc_ptr, count as u32)?;
    mem_write_u32(ctx, buf_size_ptr, buf_size as u32)?;
    Ok(vec![WasmValue::I32(0)])
}

/// environ_get(environ_ptr, environ_buf_ptr) -> errno
fn host_environ_get(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let environ_ptr = arg_i32(args, 0) as u32;
    let environ_buf_ptr = arg_i32(args, 1) as u32;

    let envs: Vec<String> = if let Some(ref wasi) = ctx.wasi_ctx {
        wasi.environ_get()
    } else {
        Vec::new()
    };

    let mut buf_offset = environ_buf_ptr;
    for (i, env) in envs.iter().enumerate() {
        mem_write_u32(ctx, environ_ptr + (i as u32) * 4, buf_offset)?;
        let bytes = env.as_bytes();
        mem_write_bytes(ctx, buf_offset, bytes)?;
        mem_write_bytes(ctx, buf_offset + bytes.len() as u32, &[0])?;
        buf_offset += bytes.len() as u32 + 1;
    }

    Ok(vec![WasmValue::I32(0)])
}

/// clock_time_get(id, precision, time_ptr) -> errno
fn host_clock_time_get(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let clock_id_val = arg_i32(args, 0) as u32;
    let precision = arg_i64(args, 1) as u64;
    let time_ptr = arg_i32(args, 2) as u32;

    let clock_id = ClockId::from_u32(clock_id_val).unwrap_or(ClockId::Monotonic);

    let result = if let Some(ref mut wasi) = ctx.wasi_ctx {
        wasi.clock_time_get(clock_id, precision)
    } else {
        Ok(0)
    };

    match result {
        Ok(time) => {
            mem_write_u64(ctx, time_ptr, time)?;
            Ok(vec![WasmValue::I32(0)])
        }
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// random_get(buf_ptr, buf_len) -> errno
fn host_random_get(ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let buf_ptr = arg_i32(args, 0) as u32;
    let buf_len = arg_i32(args, 1) as u32;

    let mut buf = vec![0u8; buf_len as usize];

    let result = if let Some(ref mut wasi) = ctx.wasi_ctx {
        wasi.random_get(&mut buf)
    } else {
        // Fallback: simple PRNG
        let mut state: u64 = 0x12345678_9ABCDEF0;
        for byte in buf.iter_mut() {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            *byte = state as u8;
        }
        Ok(())
    };

    match result {
        Ok(()) => {
            mem_write_bytes(ctx, buf_ptr, &buf)?;
            Ok(vec![WasmValue::I32(0)])
        }
        Err(e) => Ok(vec![WasmValue::I32(e.to_errno())]),
    }
}

/// proc_exit(code) -> !
fn host_proc_exit(_ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let code = arg_i32(args, 0);
    Err(TrapError::ProcessExit(code))
}

// ─── KPIO Stubs ────────────────────────────────────────────────────

fn host_ipc_send(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)])
}

fn host_ipc_recv(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)])
}

fn host_ipc_create_channel(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I64(0)])
}

fn host_process_spawn(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I64(0)])
}

fn host_capability_derive(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I64(0)])
}

// ─── GPU Stubs ─────────────────────────────────────────────────────

fn host_gpu_create_surface(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I64(0)])
}

fn host_gpu_create_buffer(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I64(0)])
}

fn host_gpu_submit_commands(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)])
}

fn host_gpu_present(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)])
}

// ─── Network Stubs ─────────────────────────────────────────────────

fn host_socket_create(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)])
}

fn host_socket_bind(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)])
}

fn host_socket_connect(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)])
}

fn host_socket_send(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)])
}

fn host_socket_recv(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)])
}

// ─── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{MemoryType, Module};
    use crate::wasi::WasiCtx;
    use alloc::string::String;
    use alloc::vec;

    /// Create a minimal module with one memory page for testing host functions.
    fn test_module() -> Module {
        Module {
            types: vec![],
            imports: vec![],
            functions: vec![],
            tables: vec![],
            memories: vec![MemoryType {
                min: 1,
                max: Some(10),
                shared: false,
            }],
            globals: vec![],
            exports: vec![],
            start: None,
            elements: vec![],
            code: vec![],
            data: vec![],
            name: None,
            data_count: None,
        }
    }

    /// Create an ExecutorContext with a WasiCtx attached.
    fn test_ctx_with_wasi() -> ExecutorContext {
        let module = test_module();
        let mut ctx = ExecutorContext::new(module).unwrap();
        ctx.wasi_ctx = Some(WasiCtx::new());
        ctx
    }

    // C-QG1: stdout via host_fd_write
    #[test]
    fn test_host_cqg1_fd_write_stdout() {
        let mut ctx = test_ctx_with_wasi();
        let msg = b"Hello, WASI!";

        // Write message into linear memory at offset 0
        ctx.memories[0].write_bytes(0, msg).unwrap();

        // Write iov at offset 100: { buf_ptr=0, buf_len=12 }
        ctx.memories[0].write_bytes(100, &0u32.to_le_bytes()).unwrap();
        ctx.memories[0].write_bytes(104, &(msg.len() as u32).to_le_bytes()).unwrap();

        // Call host_fd_write(fd=1, iovs_ptr=100, iovs_cnt=1, nwritten_ptr=200)
        let result = host_fd_write(
            &mut ctx,
            &[
                WasmValue::I32(1),   // fd = stdout
                WasmValue::I32(100), // iovs_ptr
                WasmValue::I32(1),   // iovs_cnt
                WasmValue::I32(200), // nwritten_ptr
            ],
        )
        .unwrap();

        assert_eq!(result[0], WasmValue::I32(0)); // ESUCCESS
        assert_eq!(&ctx.stdout, b"Hello, WASI!");

        // Check nwritten
        let nwritten_bytes = ctx.memories[0].read_bytes(200, 4).unwrap();
        let nwritten = u32::from_le_bytes([nwritten_bytes[0], nwritten_bytes[1], nwritten_bytes[2], nwritten_bytes[3]]);
        assert_eq!(nwritten, 12);
    }

    // C-QG2: File read via host functions
    #[test]
    fn test_host_cqg2_file_read() {
        let mut ctx = test_ctx_with_wasi();
        let wasi = ctx.wasi_ctx.as_mut().unwrap();
        let dir_fd = wasi.preopen_dir("/app");
        wasi.vfs.create_file("/app/test.txt", b"File content!".to_vec()).unwrap();

        // path_open: write path "test.txt" into memory at offset 0
        let path = b"test.txt";
        ctx.memories[0].write_bytes(0, path).unwrap();

        // fd_ptr at offset 100
        let result = host_path_open(
            &mut ctx,
            &[
                WasmValue::I32(dir_fd as i32), // dirfd
                WasmValue::I32(0),              // dirflags
                WasmValue::I32(0),              // path_ptr
                WasmValue::I32(path.len() as i32), // path_len
                WasmValue::I32(0),              // oflags
                WasmValue::I64(FdRights::READ.bits() as i64), // rights
                WasmValue::I64(0),              // inheriting
                WasmValue::I32(0),              // fdflags
                WasmValue::I32(100),            // fd_ptr
            ],
        ).unwrap();
        assert_eq!(result[0], WasmValue::I32(0));

        let fd_bytes = ctx.memories[0].read_bytes(100, 4).unwrap();
        let file_fd = u32::from_le_bytes([fd_bytes[0], fd_bytes[1], fd_bytes[2], fd_bytes[3]]);

        // fd_read: iov at 200 { buf_ptr=300, buf_len=64 }, nread at 400
        ctx.memories[0].write_bytes(200, &300u32.to_le_bytes()).unwrap();
        ctx.memories[0].write_bytes(204, &64u32.to_le_bytes()).unwrap();

        let result = host_fd_read(
            &mut ctx,
            &[
                WasmValue::I32(file_fd as i32),
                WasmValue::I32(200), // iovs_ptr
                WasmValue::I32(1),   // iovs_cnt
                WasmValue::I32(400), // nread_ptr
            ],
        ).unwrap();
        assert_eq!(result[0], WasmValue::I32(0));

        let nread_bytes = ctx.memories[0].read_bytes(400, 4).unwrap();
        let nread = u32::from_le_bytes([nread_bytes[0], nread_bytes[1], nread_bytes[2], nread_bytes[3]]);
        assert_eq!(nread, 13);

        let content = ctx.memories[0].read_bytes(300, 13).unwrap();
        assert_eq!(content, b"File content!");
    }

    // C-QG3: File write via host functions
    #[test]
    fn test_host_cqg3_file_write() {
        let mut ctx = test_ctx_with_wasi();
        let wasi = ctx.wasi_ctx.as_mut().unwrap();
        let dir_fd = wasi.preopen_dir("/data");

        // path_open with O_CREAT: write path "out.txt" at offset 0
        let path = b"out.txt";
        ctx.memories[0].write_bytes(0, path).unwrap();

        let result = host_path_open(
            &mut ctx,
            &[
                WasmValue::I32(dir_fd as i32),
                WasmValue::I32(0),
                WasmValue::I32(0),
                WasmValue::I32(path.len() as i32),
                WasmValue::I32(OFlags::CREAT.bits() as i32),
                WasmValue::I64(FdRights::WRITE.bits() as i64),
                WasmValue::I64(0),
                WasmValue::I32(0),
                WasmValue::I32(100), // fd_ptr
            ],
        ).unwrap();
        assert_eq!(result[0], WasmValue::I32(0));

        let fd_bytes = ctx.memories[0].read_bytes(100, 4).unwrap();
        let file_fd = u32::from_le_bytes([fd_bytes[0], fd_bytes[1], fd_bytes[2], fd_bytes[3]]);

        // fd_write: write "output data" into the file
        let data = b"output data";
        ctx.memories[0].write_bytes(200, data).unwrap();
        // iov at 300
        ctx.memories[0].write_bytes(300, &200u32.to_le_bytes()).unwrap();
        ctx.memories[0].write_bytes(304, &(data.len() as u32).to_le_bytes()).unwrap();

        let result = host_fd_write(
            &mut ctx,
            &[
                WasmValue::I32(file_fd as i32),
                WasmValue::I32(300),
                WasmValue::I32(1),
                WasmValue::I32(400), // nwritten_ptr
            ],
        ).unwrap();
        assert_eq!(result[0], WasmValue::I32(0));

        // Verify in VFS
        let vfs_data = ctx.wasi_ctx.as_ref().unwrap().vfs.read_file("/data/out.txt").unwrap();
        assert_eq!(vfs_data, b"output data");
    }

    // C-QG5: Clock via host function
    #[test]
    fn test_host_cqg5_clock() {
        let mut ctx = test_ctx_with_wasi();

        // clock_time_get(MONOTONIC=1, precision=0, time_ptr=0)
        let result = host_clock_time_get(
            &mut ctx,
            &[
                WasmValue::I32(1),  // MONOTONIC
                WasmValue::I64(0),  // precision
                WasmValue::I32(0),  // time_ptr
            ],
        ).unwrap();
        assert_eq!(result[0], WasmValue::I32(0));

        let time_bytes = ctx.memories[0].read_bytes(0, 8).unwrap();
        let time = u64::from_le_bytes([
            time_bytes[0], time_bytes[1], time_bytes[2], time_bytes[3],
            time_bytes[4], time_bytes[5], time_bytes[6], time_bytes[7],
        ]);
        assert!(time > 0, "Clock time should be non-zero");
    }

    // C-QG6: Random via host function
    #[test]
    fn test_host_cqg6_random() {
        let mut ctx = test_ctx_with_wasi();

        let result = host_random_get(
            &mut ctx,
            &[
                WasmValue::I32(0),   // buf_ptr
                WasmValue::I32(32),  // buf_len
            ],
        ).unwrap();
        assert_eq!(result[0], WasmValue::I32(0));

        let bytes = ctx.memories[0].read_bytes(0, 32).unwrap();
        let non_zero = bytes.iter().filter(|&&b| b != 0).count();
        assert!(non_zero > 0, "Expected some non-zero random bytes");
    }

    // C-QG7: Args via host functions
    #[test]
    fn test_host_cqg7_args() {
        let mut ctx = test_ctx_with_wasi();
        {
            let wasi = ctx.wasi_ctx.as_mut().unwrap();
            wasi.set_args(vec![String::from("app"), String::from("--flag")]);
        }

        // args_sizes_get(argc_ptr=0, buf_size_ptr=4)
        let result = host_args_sizes_get(
            &mut ctx,
            &[WasmValue::I32(0), WasmValue::I32(4)],
        ).unwrap();
        assert_eq!(result[0], WasmValue::I32(0));

        let argc_bytes = ctx.memories[0].read_bytes(0, 4).unwrap();
        let argc = u32::from_le_bytes([argc_bytes[0], argc_bytes[1], argc_bytes[2], argc_bytes[3]]);
        assert_eq!(argc, 2);

        let buf_size_bytes = ctx.memories[0].read_bytes(4, 4).unwrap();
        let buf_size = u32::from_le_bytes([buf_size_bytes[0], buf_size_bytes[1], buf_size_bytes[2], buf_size_bytes[3]]);
        assert_eq!(buf_size, 11); // "app\0" + "--flag\0"

        // args_get(argv_ptr=100, argv_buf_ptr=200)
        let result = host_args_get(
            &mut ctx,
            &[WasmValue::I32(100), WasmValue::I32(200)],
        ).unwrap();
        assert_eq!(result[0], WasmValue::I32(0));

        // Read argv[0] pointer
        let ptr_bytes = ctx.memories[0].read_bytes(100, 4).unwrap();
        let ptr0 = u32::from_le_bytes([ptr_bytes[0], ptr_bytes[1], ptr_bytes[2], ptr_bytes[3]]);
        assert_eq!(ptr0, 200);

        // Read "app\0" from buf
        let arg0 = ctx.memories[0].read_bytes(200, 4).unwrap();
        assert_eq!(&arg0[..3], b"app");
        assert_eq!(arg0[3], 0); // NUL

        // Read argv[1] pointer
        let ptr_bytes = ctx.memories[0].read_bytes(104, 4).unwrap();
        let ptr1 = u32::from_le_bytes([ptr_bytes[0], ptr_bytes[1], ptr_bytes[2], ptr_bytes[3]]);
        assert_eq!(ptr1, 204); // 200 + 4 (app\0)

        let arg1 = ctx.memories[0].read_bytes(204, 7).unwrap();
        assert_eq!(&arg1[..6], b"--flag");
        assert_eq!(arg1[6], 0);
    }

    // C-QG8: proc_exit
    #[test]
    fn test_host_cqg8_proc_exit() {
        let mut ctx = test_ctx_with_wasi();
        let result = host_proc_exit(&mut ctx, &[WasmValue::I32(42)]);
        match result {
            Err(TrapError::ProcessExit(code)) => assert_eq!(code, 42),
            _ => panic!("Expected ProcessExit(42)"),
        }
    }

    // C-QG9: Sandbox via host path_open
    #[test]
    fn test_host_cqg9_sandbox() {
        let mut ctx = test_ctx_with_wasi();
        let wasi = ctx.wasi_ctx.as_mut().unwrap();
        let dir_fd = wasi.preopen_dir("/app");

        // Try to open "../etc/passwd"
        let path = b"../etc/passwd";
        ctx.memories[0].write_bytes(0, path).unwrap();

        let result = host_path_open(
            &mut ctx,
            &[
                WasmValue::I32(dir_fd as i32),
                WasmValue::I32(0),
                WasmValue::I32(0),
                WasmValue::I32(path.len() as i32),
                WasmValue::I32(0),
                WasmValue::I64(FdRights::READ.bits() as i64),
                WasmValue::I64(0),
                WasmValue::I32(0),
                WasmValue::I32(100),
            ],
        ).unwrap();
        // Should return EACCES (2)
        assert_eq!(result[0], WasmValue::I32(2));
    }

    // C-QG10: Integration test - full file read/write cycle via host functions
    #[test]
    fn test_host_cqg10_full_integration() {
        let mut ctx = test_ctx_with_wasi();
        let wasi = ctx.wasi_ctx.as_mut().unwrap();
        wasi.set_args(vec![String::from("myapp")]);
        let dir_fd = wasi.preopen_dir("/app");
        wasi.vfs.create_file("/app/input.txt", b"hello world".to_vec()).unwrap();

        // 1. Read input file
        let path = b"input.txt";
        ctx.memories[0].write_bytes(0, path).unwrap();
        host_path_open(&mut ctx, &[
            WasmValue::I32(dir_fd as i32), WasmValue::I32(0),
            WasmValue::I32(0), WasmValue::I32(path.len() as i32),
            WasmValue::I32(0), WasmValue::I64(FdRights::READ.bits() as i64),
            WasmValue::I64(0), WasmValue::I32(0), WasmValue::I32(100),
        ]).unwrap();
        let fd_bytes = ctx.memories[0].read_bytes(100, 4).unwrap();
        let input_fd = u32::from_le_bytes([fd_bytes[0], fd_bytes[1], fd_bytes[2], fd_bytes[3]]);

        // Read content
        ctx.memories[0].write_bytes(200, &500u32.to_le_bytes()).unwrap();
        ctx.memories[0].write_bytes(204, &64u32.to_le_bytes()).unwrap();
        host_fd_read(&mut ctx, &[
            WasmValue::I32(input_fd as i32), WasmValue::I32(200),
            WasmValue::I32(1), WasmValue::I32(300),
        ]).unwrap();
        let nread_bytes = ctx.memories[0].read_bytes(300, 4).unwrap();
        let nread = u32::from_le_bytes([nread_bytes[0], nread_bytes[1], nread_bytes[2], nread_bytes[3]]);
        assert_eq!(nread, 11);

        // Close input
        host_fd_close(&mut ctx, &[WasmValue::I32(input_fd as i32)]).unwrap();

        // 2. Write output file
        let path = b"output.txt";
        ctx.memories[0].write_bytes(0, path).unwrap();
        host_path_open(&mut ctx, &[
            WasmValue::I32(dir_fd as i32), WasmValue::I32(0),
            WasmValue::I32(0), WasmValue::I32(path.len() as i32),
            WasmValue::I32(OFlags::CREAT.bits() as i32),
            WasmValue::I64(FdRights::WRITE.bits() as i64),
            WasmValue::I64(0), WasmValue::I32(0), WasmValue::I32(100),
        ]).unwrap();
        let fd_bytes = ctx.memories[0].read_bytes(100, 4).unwrap();
        let output_fd = u32::from_le_bytes([fd_bytes[0], fd_bytes[1], fd_bytes[2], fd_bytes[3]]);

        // Write "hello world" (from memory at 500)
        ctx.memories[0].write_bytes(600, &500u32.to_le_bytes()).unwrap();
        ctx.memories[0].write_bytes(604, &nread.to_le_bytes()).unwrap();
        host_fd_write(&mut ctx, &[
            WasmValue::I32(output_fd as i32), WasmValue::I32(600),
            WasmValue::I32(1), WasmValue::I32(700),
        ]).unwrap();
        host_fd_close(&mut ctx, &[WasmValue::I32(output_fd as i32)]).unwrap();

        // Verify VFS
        let output = ctx.wasi_ctx.as_ref().unwrap().vfs.read_file("/app/output.txt").unwrap();
        assert_eq!(output, b"hello world");

        // 3. Write to stdout
        let msg = b"Done!";
        ctx.memories[0].write_bytes(800, msg).unwrap();
        ctx.memories[0].write_bytes(900, &800u32.to_le_bytes()).unwrap();
        ctx.memories[0].write_bytes(904, &(msg.len() as u32).to_le_bytes()).unwrap();
        host_fd_write(&mut ctx, &[
            WasmValue::I32(1), WasmValue::I32(900),
            WasmValue::I32(1), WasmValue::I32(1000),
        ]).unwrap();
        assert_eq!(&ctx.stdout, b"Done!");

        // 4. proc_exit(0)
        let exit_result = host_proc_exit(&mut ctx, &[WasmValue::I32(0)]);
        match exit_result {
            Err(TrapError::ProcessExit(0)) => {} // expected
            _ => panic!("Expected ProcessExit(0)"),
        }
    }
}
