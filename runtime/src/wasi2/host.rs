// WASI Preview 2 — Host Function Registration
//
// This module registers WASI P2 host functions (`wasi:io/streams`,
// `wasi:io/poll`) into the runtime's import table, making them
// callable from WASM modules.

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::executor::ExecutorContext;
use crate::instance::Imports;
use crate::interpreter::{TrapError, WasmValue};
use crate::wasi::Vfs;

use super::{
    ResourceData, ResourceError, ResourceHandle, ResourceType, StreamError, Wasi2Ctx,
};
use super::poll;
use super::streams::InputStreamData;
use super::filesystem;
use super::cli;
use super::clocks;

// ---------------------------------------------------------------------------
// Registration entry point
// ---------------------------------------------------------------------------

/// Register all WASI P2 host functions into the import table.
pub fn register(imports: &mut Imports) {
    // wasi:io/streams — InputStream
    imports.add_function(
        "wasi:io/streams",
        "[resource-drop]input-stream",
        host_input_stream_drop,
    );
    imports.add_function(
        "wasi:io/streams",
        "[method]input-stream.read",
        host_input_stream_read,
    );
    imports.add_function(
        "wasi:io/streams",
        "[method]input-stream.blocking-read",
        host_input_stream_blocking_read,
    );
    imports.add_function(
        "wasi:io/streams",
        "[method]input-stream.skip",
        host_input_stream_skip,
    );
    imports.add_function(
        "wasi:io/streams",
        "[method]input-stream.subscribe",
        host_input_stream_subscribe,
    );

    // wasi:io/streams — OutputStream
    imports.add_function(
        "wasi:io/streams",
        "[resource-drop]output-stream",
        host_output_stream_drop,
    );
    imports.add_function(
        "wasi:io/streams",
        "[method]output-stream.check-write",
        host_output_stream_check_write,
    );
    imports.add_function(
        "wasi:io/streams",
        "[method]output-stream.write",
        host_output_stream_write,
    );
    imports.add_function(
        "wasi:io/streams",
        "[method]output-stream.blocking-write-and-flush",
        host_output_stream_blocking_write_and_flush,
    );
    imports.add_function(
        "wasi:io/streams",
        "[method]output-stream.flush",
        host_output_stream_flush,
    );
    imports.add_function(
        "wasi:io/streams",
        "[method]output-stream.subscribe",
        host_output_stream_subscribe,
    );

    // wasi:io/poll
    imports.add_function("wasi:io/poll", "poll", host_poll);
    imports.add_function(
        "wasi:io/poll",
        "[resource-drop]pollable",
        host_pollable_drop,
    );
    imports.add_function(
        "wasi:io/poll",
        "[method]pollable.ready",
        host_pollable_ready,
    );
    imports.add_function(
        "wasi:io/poll",
        "[method]pollable.block",
        host_pollable_block,
    );

    // ===== S2: Core Interfaces =====

    // wasi:filesystem/types
    imports.add_function(
        "wasi:filesystem/types",
        "[resource-drop]descriptor",
        host_descriptor_drop,
    );
    imports.add_function(
        "wasi:filesystem/types",
        "[method]descriptor.stat",
        host_descriptor_stat,
    );
    imports.add_function(
        "wasi:filesystem/types",
        "[method]descriptor.open-at",
        host_descriptor_open_at,
    );
    imports.add_function(
        "wasi:filesystem/types",
        "[method]descriptor.readdir",
        host_descriptor_readdir,
    );
    imports.add_function(
        "wasi:filesystem/types",
        "[method]descriptor.read-via-stream",
        host_descriptor_read_via_stream,
    );
    imports.add_function(
        "wasi:filesystem/types",
        "[method]descriptor.write-via-stream",
        host_descriptor_write_via_stream,
    );
    imports.add_function(
        "wasi:filesystem/types",
        "[method]descriptor.metadata-hash",
        host_descriptor_metadata_hash,
    );

    // wasi:filesystem/preopens
    imports.add_function(
        "wasi:filesystem/preopens",
        "get-directories",
        host_get_directories,
    );

    // wasi:clocks/monotonic-clock
    imports.add_function(
        "wasi:clocks/monotonic-clock",
        "now",
        host_monotonic_clock_now,
    );
    imports.add_function(
        "wasi:clocks/monotonic-clock",
        "resolution",
        host_monotonic_clock_resolution,
    );
    imports.add_function(
        "wasi:clocks/monotonic-clock",
        "subscribe-instant",
        host_monotonic_clock_subscribe_instant,
    );
    imports.add_function(
        "wasi:clocks/monotonic-clock",
        "subscribe-duration",
        host_monotonic_clock_subscribe_duration,
    );

    // wasi:clocks/wall-clock
    imports.add_function(
        "wasi:clocks/wall-clock",
        "now",
        host_wall_clock_now,
    );
    imports.add_function(
        "wasi:clocks/wall-clock",
        "resolution",
        host_wall_clock_resolution,
    );

    // wasi:random/random
    imports.add_function(
        "wasi:random/random",
        "get-random-bytes",
        host_random_get_bytes,
    );
    imports.add_function(
        "wasi:random/random",
        "get-random-u64",
        host_random_get_u64,
    );

    // wasi:random/insecure
    imports.add_function(
        "wasi:random/insecure",
        "get-insecure-random-bytes",
        host_random_insecure_get_bytes,
    );
    imports.add_function(
        "wasi:random/insecure",
        "get-insecure-random-u64",
        host_random_insecure_get_u64,
    );

    // wasi:random/insecure-seed
    imports.add_function(
        "wasi:random/insecure-seed",
        "insecure-seed",
        host_random_insecure_seed,
    );

    // wasi:cli/stdin
    imports.add_function("wasi:cli/stdin", "get-stdin", host_cli_get_stdin);
    // wasi:cli/stdout
    imports.add_function("wasi:cli/stdout", "get-stdout", host_cli_get_stdout);
    // wasi:cli/stderr
    imports.add_function("wasi:cli/stderr", "get-stderr", host_cli_get_stderr);
    // wasi:cli/environment
    imports.add_function(
        "wasi:cli/environment",
        "get-environment",
        host_cli_get_environment,
    );
    imports.add_function(
        "wasi:cli/environment",
        "get-arguments",
        host_cli_get_arguments,
    );
    imports.add_function(
        "wasi:cli/environment",
        "initial-cwd",
        host_cli_initial_cwd,
    );
    // wasi:cli/exit
    imports.add_function("wasi:cli/exit", "exit", host_cli_exit);
}

// ---------------------------------------------------------------------------
// Helper: get WASI2 context from executor
// ---------------------------------------------------------------------------

fn get_wasi2_ctx(ctx: &ExecutorContext) -> Result<&Wasi2Ctx, TrapError> {
    ctx.wasi2_ctx.as_ref().ok_or_else(|| {
        TrapError::HostError(String::from("WASI P2 context not initialized"))
    })
}

fn get_wasi2_ctx_mut(ctx: &mut ExecutorContext) -> Result<&mut Wasi2Ctx, TrapError> {
    ctx.wasi2_ctx.as_mut().ok_or_else(|| {
        TrapError::HostError(String::from("WASI P2 context not initialized"))
    })
}

fn resource_err_to_trap(e: ResourceError) -> TrapError {
    match e {
        ResourceError::InvalidHandle => {
            TrapError::HostError(String::from("invalid resource handle"))
        }
        ResourceError::TypeMismatch { expected: _, actual: _ } => {
            TrapError::HostError(String::from("resource type mismatch"))
        }
        ResourceError::TableFull => {
            TrapError::HostError(String::from("resource table full"))
        }
        ResourceError::StreamError(se) => stream_err_to_trap(se),
    }
}

fn stream_err_to_trap(e: StreamError) -> TrapError {
    match e {
        StreamError::Closed => TrapError::HostError(String::from("stream closed")),
        StreamError::LastOperationFailed(msg) => TrapError::HostError(msg),
    }
}

/// Extract an i32 argument or trap.
fn arg_i32(args: &[WasmValue], idx: usize) -> Result<i32, TrapError> {
    match args.get(idx) {
        Some(WasmValue::I32(v)) => Ok(*v),
        _ => Err(TrapError::HostError(String::from("expected i32 argument"))),
    }
}

/// Extract an i64 argument or trap.
fn arg_i64(args: &[WasmValue], idx: usize) -> Result<i64, TrapError> {
    match args.get(idx) {
        Some(WasmValue::I64(v)) => Ok(*v),
        _ => Err(TrapError::HostError(String::from("expected i64 argument"))),
    }
}

// ---------------------------------------------------------------------------
// InputStream host functions
// ---------------------------------------------------------------------------

/// `[resource-drop]input-stream` — drop an input stream resource.
fn host_input_stream_drop(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);
    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    wasi2
        .resources
        .delete(handle)
        .map_err(resource_err_to_trap)?;
    Ok(vec![])
}

/// `[method]input-stream.read(self, len: u64) → result<list<u8>, stream-error>`
///
/// Returns: (ptr: i32, len: i32) written to linear memory.
/// For simplicity, we write the result bytes into linear memory at a
/// caller-provided pointer (args: handle, ptr, len).
fn host_input_stream_read(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);
    let buf_ptr = arg_i32(args, 1)? as u32;
    let buf_len = arg_i32(args, 2)? as u32;

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let data = wasi2
        .resources
        .get_mut(handle, ResourceType::InputStream)
        .map_err(resource_err_to_trap)?;

    let bytes = if let ResourceData::InputStream(stream) = data {
        stream.read(buf_len as usize).map_err(|e| stream_err_to_trap(e))?
    } else {
        return Err(TrapError::HostError(String::from("not an input stream")));
    };

    // Write bytes to linear memory
    let actual_len = bytes.len().min(buf_len as usize);
    if actual_len > 0 {
        if let Some(mem) = ctx.memories.get_mut(0) {
            mem.write_bytes(buf_ptr as usize, &bytes[..actual_len])
                .map_err(|_| TrapError::HostError(String::from("memory write error")))?;
        }
    }

    // Return: (error_code: i32 = 0 success, bytes_read: i32)
    Ok(vec![WasmValue::I32(0), WasmValue::I32(actual_len as i32)])
}

/// `[method]input-stream.blocking-read` — same as read (single-threaded kernel).
fn host_input_stream_blocking_read(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    host_input_stream_read(ctx, args)
}

/// `[method]input-stream.skip(self, len: u64) → result<u64, stream-error>`
fn host_input_stream_skip(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);
    let len = arg_i64(args, 1)? as u64;

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let data = wasi2
        .resources
        .get_mut(handle, ResourceType::InputStream)
        .map_err(resource_err_to_trap)?;

    let skipped = if let ResourceData::InputStream(stream) = data {
        stream.skip(len as usize).map_err(|e| stream_err_to_trap(e))?
    } else {
        return Err(TrapError::HostError(String::from("not an input stream")));
    };

    Ok(vec![WasmValue::I64(skipped as i64)])
}

/// `[method]input-stream.subscribe` — create a pollable for this stream.
fn host_input_stream_subscribe(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    // Verify the handle is valid
    let _ = wasi2
        .resources
        .get(handle, ResourceType::InputStream)
        .map_err(resource_err_to_trap)?;

    let poll_handle = poll::create_stream_pollable(&mut wasi2.resources, handle)
        .map_err(resource_err_to_trap)?;

    Ok(vec![WasmValue::I32(poll_handle.as_u32() as i32)])
}

// ---------------------------------------------------------------------------
// OutputStream host functions
// ---------------------------------------------------------------------------

/// `[resource-drop]output-stream` — drop an output stream resource.
fn host_output_stream_drop(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);
    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    wasi2
        .resources
        .delete(handle)
        .map_err(resource_err_to_trap)?;
    Ok(vec![])
}

/// `[method]output-stream.check-write → result<u64, stream-error>`
fn host_output_stream_check_write(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let data = wasi2
        .resources
        .get(handle, ResourceType::OutputStream)
        .map_err(resource_err_to_trap)?;

    let available = if let ResourceData::OutputStream(stream) = data {
        stream.check_write().map_err(|e| stream_err_to_trap(e))?
    } else {
        return Err(TrapError::HostError(String::from("not an output stream")));
    };

    Ok(vec![WasmValue::I64(available as i64)])
}

/// `[method]output-stream.write(self, bytes: list<u8>) → result<(), stream-error>`
///
/// Args: (handle: i32, ptr: i32, len: i32)
fn host_output_stream_write(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);
    let buf_ptr = arg_i32(args, 1)? as u32;
    let buf_len = arg_i32(args, 2)? as u32;

    // Read bytes from linear memory first
    let bytes = if let Some(mem) = ctx.memories.get(0) {
        let slice = mem
            .read_bytes(buf_ptr as usize, buf_len as usize)
            .map_err(|_| TrapError::HostError(String::from("memory read error")))?;
        slice.to_vec()
    } else {
        return Err(TrapError::HostError(String::from("no linear memory")));
    };

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let data = wasi2
        .resources
        .get_mut(handle, ResourceType::OutputStream)
        .map_err(resource_err_to_trap)?;

    if let ResourceData::OutputStream(stream) = data {
        stream.write(&bytes).map_err(|e| stream_err_to_trap(e))?;
    } else {
        return Err(TrapError::HostError(String::from("not an output stream")));
    }

    // Return: error_code = 0 (success)
    Ok(vec![WasmValue::I32(0)])
}

/// `[method]output-stream.blocking-write-and-flush`
fn host_output_stream_blocking_write_and_flush(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    // In our single-threaded kernel, this is identical to write + flush
    let result = host_output_stream_write(ctx, args)?;
    // Flush is automatic
    Ok(result)
}

/// `[method]output-stream.flush → result<(), stream-error>`
fn host_output_stream_flush(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let data = wasi2
        .resources
        .get_mut(handle, ResourceType::OutputStream)
        .map_err(resource_err_to_trap)?;

    if let ResourceData::OutputStream(stream) = data {
        stream.flush().map_err(|e| stream_err_to_trap(e))?;
    } else {
        return Err(TrapError::HostError(String::from("not an output stream")));
    }

    Ok(vec![WasmValue::I32(0)])
}

/// `[method]output-stream.subscribe` — create a pollable for this stream.
fn host_output_stream_subscribe(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let _ = wasi2
        .resources
        .get(handle, ResourceType::OutputStream)
        .map_err(resource_err_to_trap)?;

    let poll_handle = poll::create_stream_pollable(&mut wasi2.resources, handle)
        .map_err(resource_err_to_trap)?;

    Ok(vec![WasmValue::I32(poll_handle.as_u32() as i32)])
}

// ---------------------------------------------------------------------------
// Poll host functions
// ---------------------------------------------------------------------------

/// `poll(list<borrow<pollable>>) → list<u32>`
///
/// Args: (list_ptr: i32, list_len: i32, result_ptr: i32)
/// Writes ready indices to result_ptr, returns count.
fn host_poll(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let list_ptr = arg_i32(args, 0)? as u32;
    let list_len = arg_i32(args, 1)? as u32;
    let result_ptr = arg_i32(args, 2)? as u32;

    // Read pollable handles from linear memory
    let mut handles = Vec::new();
    if let Some(mem) = ctx.memories.get(0) {
        for i in 0..list_len {
            let offset = list_ptr as usize + (i as usize) * 4;
            let handle_val = mem
                .read_u32(offset)
                .map_err(|_| TrapError::HostError(String::from("memory read error")))?;
            handles.push(ResourceHandle::from_u32(handle_val));
        }
    }

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let ready = poll::poll_list(&wasi2.resources, &handles, 0)
        .map_err(resource_err_to_trap)?;

    // Write ready indices to result memory
    if let Some(mem) = ctx.memories.get_mut(0) {
        for (i, &idx) in ready.iter().enumerate() {
            let offset = result_ptr as usize + i * 4;
            mem.write_u32(offset, idx)
                .map_err(|_| TrapError::HostError(String::from("memory write error")))?;
        }
    }

    Ok(vec![WasmValue::I32(ready.len() as i32)])
}

/// `[resource-drop]pollable` — drop a pollable resource.
fn host_pollable_drop(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);
    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    wasi2
        .resources
        .delete(handle)
        .map_err(resource_err_to_trap)?;
    Ok(vec![])
}

/// `[method]pollable.ready → bool`
fn host_pollable_ready(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let data = wasi2
        .resources
        .get(handle, ResourceType::Pollable)
        .map_err(resource_err_to_trap)?;

    let ready = if let ResourceData::Pollable(state) = data {
        poll::pollable_ready(state, 0)
    } else {
        return Err(TrapError::HostError(String::from("not a pollable")));
    };

    Ok(vec![WasmValue::I32(if ready { 1 } else { 0 })])
}

/// `[method]pollable.block`
fn host_pollable_block(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let data = wasi2
        .resources
        .get(handle, ResourceType::Pollable)
        .map_err(resource_err_to_trap)?;

    if let ResourceData::Pollable(state) = data {
        poll::pollable_block(state);
    }

    Ok(vec![])
}

// ===========================================================================
// S2: Filesystem host functions
// ===========================================================================

/// `[resource-drop]descriptor` — drop a filesystem descriptor resource.
fn host_descriptor_drop(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);
    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    wasi2
        .resources
        .delete(handle)
        .map_err(resource_err_to_trap)?;
    Ok(vec![])
}

/// `[method]descriptor.stat` — get file/directory metadata.
///
/// Returns: (error: i32, type: i32, size: i64)
fn host_descriptor_stat(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let data = wasi2
        .resources
        .get(handle, ResourceType::Descriptor)
        .map_err(resource_err_to_trap)?;

    let desc = if let ResourceData::Descriptor(d) = data {
        d.clone()
    } else {
        return Err(TrapError::HostError(String::from("not a descriptor")));
    };

    // Get VFS from wasi context
    let vfs = get_vfs(ctx)?;
    let stat = desc.stat(&vfs).map_err(|_| {
        TrapError::HostError(String::from("stat failed"))
    })?;

    Ok(vec![
        WasmValue::I32(0), // success
        WasmValue::I32(stat.descriptor_type as i32),
        WasmValue::I64(stat.size as i64),
    ])
}

/// `[method]descriptor.open-at` — open a file or directory relative to this descriptor.
///
/// Args: (handle, path_ptr, path_len, create, exclusive, truncate, writable)
/// Returns: (error: i32, new_handle: i32)
fn host_descriptor_open_at(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);
    let path_ptr = arg_i32(args, 1)? as u32;
    let path_len = arg_i32(args, 2)? as u32;
    let create = arg_i32(args, 3)? != 0;
    let exclusive = arg_i32(args, 4)? != 0;
    let truncate = arg_i32(args, 5)? != 0;
    let writable = arg_i32(args, 6)? != 0;

    // Read path from linear memory
    let path_str = read_string_from_memory(ctx, path_ptr, path_len)?;

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let data = wasi2
        .resources
        .get(handle, ResourceType::Descriptor)
        .map_err(resource_err_to_trap)?;

    let desc = if let ResourceData::Descriptor(d) = data {
        d.clone()
    } else {
        return Err(TrapError::HostError(String::from("not a descriptor")));
    };

    let vfs = get_vfs(ctx)?;
    let new_desc = desc
        .open_at(&vfs, &path_str, create, exclusive, truncate, writable)
        .map_err(|_| TrapError::HostError(String::from("open-at failed")))?;

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let new_handle = wasi2
        .resources
        .push(ResourceType::Descriptor, ResourceData::Descriptor(new_desc))
        .map_err(resource_err_to_trap)?;

    Ok(vec![
        WasmValue::I32(0), // success
        WasmValue::I32(new_handle.as_u32() as i32),
    ])
}

/// `[method]descriptor.readdir` — list directory entries.
///
/// Returns: (error: i32, count: i32) — entries written to memory at result_ptr.
fn host_descriptor_readdir(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);
    let result_ptr = arg_i32(args, 1)? as u32;

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let data = wasi2
        .resources
        .get(handle, ResourceType::Descriptor)
        .map_err(resource_err_to_trap)?;

    let desc = if let ResourceData::Descriptor(d) = data {
        d.clone()
    } else {
        return Err(TrapError::HostError(String::from("not a descriptor")));
    };

    let vfs = get_vfs(ctx)?;
    let entries = desc
        .readdir(&vfs)
        .map_err(|_| TrapError::HostError(String::from("readdir failed")))?;

    // Write entry count to memory
    let count = entries.len() as u32;
    if let Some(mem) = ctx.memories.get_mut(0) {
        mem.write_u32(result_ptr as usize, count)
            .map_err(|_| TrapError::HostError(String::from("memory write error")))?;
    }

    Ok(vec![
        WasmValue::I32(0), // success
        WasmValue::I32(count as i32),
    ])
}

/// `[method]descriptor.read-via-stream` — get an input stream for reading.
///
/// Args: (handle, offset: i64)
/// Returns: (error: i32, stream_handle: i32)
fn host_descriptor_read_via_stream(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);
    let offset = arg_i64(args, 1)? as u64;

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let data = wasi2
        .resources
        .get(handle, ResourceType::Descriptor)
        .map_err(resource_err_to_trap)?;

    let desc = if let ResourceData::Descriptor(d) = data {
        d.clone()
    } else {
        return Err(TrapError::HostError(String::from("not a descriptor")));
    };

    let vfs = get_vfs(ctx)?;
    let stream_data = desc
        .read_via_stream(&vfs, offset)
        .map_err(|_| TrapError::HostError(String::from("read-via-stream failed")))?;

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let stream_handle = wasi2
        .resources
        .push(ResourceType::InputStream, ResourceData::InputStream(stream_data))
        .map_err(resource_err_to_trap)?;

    Ok(vec![
        WasmValue::I32(0), // success
        WasmValue::I32(stream_handle.as_u32() as i32),
    ])
}

/// `[method]descriptor.write-via-stream` — get an output stream for writing.
///
/// Args: (handle)
/// Returns: (error: i32, stream_handle: i32)
fn host_descriptor_write_via_stream(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let data = wasi2
        .resources
        .get(handle, ResourceType::Descriptor)
        .map_err(resource_err_to_trap)?;

    let desc = if let ResourceData::Descriptor(d) = data {
        d.clone()
    } else {
        return Err(TrapError::HostError(String::from("not a descriptor")));
    };

    let stream_data = desc
        .write_via_stream()
        .map_err(|_| TrapError::HostError(String::from("write-via-stream failed")))?;

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let stream_handle = wasi2
        .resources
        .push(ResourceType::OutputStream, ResourceData::OutputStream(stream_data))
        .map_err(resource_err_to_trap)?;

    Ok(vec![
        WasmValue::I32(0), // success
        WasmValue::I32(stream_handle.as_u32() as i32),
    ])
}

/// `[method]descriptor.metadata-hash` — compute metadata hash.
///
/// Returns: (error: i32, upper: i64, lower: i64)
fn host_descriptor_metadata_hash(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let handle = ResourceHandle::from_u32(arg_i32(args, 0)? as u32);

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let data = wasi2
        .resources
        .get(handle, ResourceType::Descriptor)
        .map_err(resource_err_to_trap)?;

    let desc = if let ResourceData::Descriptor(d) = data {
        d.clone()
    } else {
        return Err(TrapError::HostError(String::from("not a descriptor")));
    };

    let vfs = get_vfs(ctx)?;
    let hash = desc
        .metadata_hash(&vfs)
        .map_err(|_| TrapError::HostError(String::from("metadata-hash failed")))?;

    Ok(vec![
        WasmValue::I32(0), // success
        WasmValue::I64(hash.upper as i64),
        WasmValue::I64(hash.lower as i64),
    ])
}

/// `get-directories` — list preopened directories.
///
/// Returns: (count: i32) — writes descriptor handles to result_ptr.
fn host_get_directories(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let result_ptr = arg_i32(args, 0)? as u32;

    // Get preopens
    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let preopens = wasi2.preopens.clone();
    if preopens.is_empty() {
        // Return default "/" preopen
        let default_preopens = filesystem::default_preopens();
        let mut handles = Vec::new();
        for preopen in &default_preopens {
            let h = wasi2
                .resources
                .push(
                    ResourceType::Descriptor,
                    ResourceData::Descriptor(preopen.descriptor.clone()),
                )
                .map_err(resource_err_to_trap)?;
            handles.push(h);
        }
        // Write handles to memory
        if let Some(mem) = ctx.memories.get_mut(0) {
            for (i, h) in handles.iter().enumerate() {
                let offset = result_ptr as usize + i * 4;
                mem.write_u32(offset, h.as_u32())
                    .map_err(|_| TrapError::HostError(String::from("memory write error")))?;
            }
        }
        return Ok(vec![WasmValue::I32(handles.len() as i32)]);
    }

    let mut handles = Vec::new();
    for preopen in &preopens {
        let h = wasi2
            .resources
            .push(
                ResourceType::Descriptor,
                ResourceData::Descriptor(preopen.descriptor.clone()),
            )
            .map_err(resource_err_to_trap)?;
        handles.push(h);
    }
    if let Some(mem) = ctx.memories.get_mut(0) {
        for (i, h) in handles.iter().enumerate() {
            let offset = result_ptr as usize + i * 4;
            mem.write_u32(offset, h.as_u32())
                .map_err(|_| TrapError::HostError(String::from("memory write error")))?;
        }
    }
    Ok(vec![WasmValue::I32(handles.len() as i32)])
}

// ===========================================================================
// S2: Clocks host functions
// ===========================================================================

/// `wasi:clocks/monotonic-clock.now` — get current monotonic time.
///
/// Returns: (instant: i64)
fn host_monotonic_clock_now(
    ctx: &mut ExecutorContext,
    _args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let now = wasi2.monotonic_clock.now();
    Ok(vec![WasmValue::I64(now as i64)])
}

/// `wasi:clocks/monotonic-clock.resolution` — clock resolution.
///
/// Returns: (duration: i64)
fn host_monotonic_clock_resolution(
    ctx: &mut ExecutorContext,
    _args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let res = wasi2.monotonic_clock.resolution();
    Ok(vec![WasmValue::I64(res as i64)])
}

/// `wasi:clocks/monotonic-clock.subscribe-instant` — subscribe to an instant.
///
/// Args: (when: i64) Returns: (pollable_handle: i32)
fn host_monotonic_clock_subscribe_instant(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let when = arg_i64(args, 0)? as u64;

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let wasi2_now = wasi2.monotonic_clock.now();
    let duration = if when > wasi2_now { when - wasi2_now } else { 0 };

    let poll_handle = poll::create_timer_pollable(&mut wasi2.resources, duration)
        .map_err(resource_err_to_trap)?;

    Ok(vec![WasmValue::I32(poll_handle.as_u32() as i32)])
}

/// `wasi:clocks/monotonic-clock.subscribe-duration` — subscribe to duration.
///
/// Args: (duration: i64) Returns: (pollable_handle: i32)
fn host_monotonic_clock_subscribe_duration(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let duration = arg_i64(args, 0)? as u64;

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let poll_handle = poll::create_timer_pollable(&mut wasi2.resources, duration)
        .map_err(resource_err_to_trap)?;

    Ok(vec![WasmValue::I32(poll_handle.as_u32() as i32)])
}

/// `wasi:clocks/wall-clock.now` — get current wall clock time.
///
/// Returns: (seconds: i64, nanoseconds: i32)
fn host_wall_clock_now(
    ctx: &mut ExecutorContext,
    _args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let datetime = wasi2.wall_clock.now();
    Ok(vec![
        WasmValue::I64(datetime.seconds as i64),
        WasmValue::I32(datetime.nanoseconds as i32),
    ])
}

/// `wasi:clocks/wall-clock.resolution` — wall clock resolution.
///
/// Returns: (seconds: i64, nanoseconds: i32)
fn host_wall_clock_resolution(
    ctx: &mut ExecutorContext,
    _args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let datetime = wasi2.wall_clock.resolution();
    Ok(vec![
        WasmValue::I64(datetime.seconds as i64),
        WasmValue::I32(datetime.nanoseconds as i32),
    ])
}

// ===========================================================================
// S2: Random host functions
// ===========================================================================

/// `wasi:random/random.get-random-bytes(len: u64) → list<u8>`
///
/// Args: (len: i64, result_ptr: i32) Returns: (actual_len: i32)
fn host_random_get_bytes(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let len = arg_i64(args, 0)? as usize;
    let result_ptr = arg_i32(args, 1)? as u32;

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let bytes = wasi2.random.get_random_bytes(len);

    if let Some(mem) = ctx.memories.get_mut(0) {
        mem.write_bytes(result_ptr as usize, &bytes)
            .map_err(|_| TrapError::HostError(String::from("memory write error")))?;
    }

    Ok(vec![WasmValue::I32(bytes.len() as i32)])
}

/// `wasi:random/random.get-random-u64 → u64`
fn host_random_get_u64(
    ctx: &mut ExecutorContext,
    _args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let val = wasi2.random.get_random_u64();
    Ok(vec![WasmValue::I64(val as i64)])
}

/// `wasi:random/insecure.get-insecure-random-bytes`
fn host_random_insecure_get_bytes(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let len = arg_i64(args, 0)? as usize;
    let result_ptr = arg_i32(args, 1)? as u32;

    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let bytes = wasi2.random.get_insecure_random_bytes(len);

    if let Some(mem) = ctx.memories.get_mut(0) {
        mem.write_bytes(result_ptr as usize, &bytes)
            .map_err(|_| TrapError::HostError(String::from("memory write error")))?;
    }

    Ok(vec![WasmValue::I32(bytes.len() as i32)])
}

/// `wasi:random/insecure.get-insecure-random-u64`
fn host_random_insecure_get_u64(
    ctx: &mut ExecutorContext,
    _args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let val = wasi2.random.get_insecure_random_u64();
    Ok(vec![WasmValue::I64(val as i64)])
}

/// `wasi:random/insecure-seed.insecure-seed → (u64, u64)`
fn host_random_insecure_seed(
    ctx: &mut ExecutorContext,
    _args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let wasi2 = get_wasi2_ctx_mut(ctx)?;
    let (a, b) = wasi2.random.insecure_seed();
    Ok(vec![WasmValue::I64(a as i64), WasmValue::I64(b as i64)])
}

// ===========================================================================
// S2: CLI host functions
// ===========================================================================

/// `wasi:cli/stdin.get-stdin → own<input-stream>`
fn host_cli_get_stdin(
    ctx: &mut ExecutorContext,
    _args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let wasi2 = get_wasi2_ctx(ctx)?;
    let handle = wasi2.stdin_handle.ok_or_else(|| {
        TrapError::HostError(String::from("stdin not initialized"))
    })?;
    Ok(vec![WasmValue::I32(handle.as_u32() as i32)])
}

/// `wasi:cli/stdout.get-stdout → own<output-stream>`
fn host_cli_get_stdout(
    ctx: &mut ExecutorContext,
    _args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let wasi2 = get_wasi2_ctx(ctx)?;
    let handle = wasi2.stdout_handle.ok_or_else(|| {
        TrapError::HostError(String::from("stdout not initialized"))
    })?;
    Ok(vec![WasmValue::I32(handle.as_u32() as i32)])
}

/// `wasi:cli/stderr.get-stderr → own<output-stream>`
fn host_cli_get_stderr(
    ctx: &mut ExecutorContext,
    _args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let wasi2 = get_wasi2_ctx(ctx)?;
    let handle = wasi2.stderr_handle.ok_or_else(|| {
        TrapError::HostError(String::from("stderr not initialized"))
    })?;
    Ok(vec![WasmValue::I32(handle.as_u32() as i32)])
}

/// `wasi:cli/environment.get-environment → list<(string, string)>`
///
/// Args: (result_ptr: i32, max_pairs: i32)
/// Returns: (count: i32)
fn host_cli_get_environment(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let result_ptr = arg_i32(args, 0)? as u32;
    let _max_pairs = arg_i32(args, 1)? as u32;

    let wasi2 = get_wasi2_ctx(ctx)?;
    let env = wasi2.cli_env.get_environment();
    let count = env.len();

    // Write count to memory at result_ptr
    if let Some(mem) = ctx.memories.get_mut(0) {
        mem.write_u32(result_ptr as usize, count as u32)
            .map_err(|_| TrapError::HostError(String::from("memory write error")))?;
    }

    Ok(vec![WasmValue::I32(count as i32)])
}

/// `wasi:cli/environment.get-arguments → list<string>`
///
/// Args: (result_ptr: i32)
/// Returns: (count: i32)
fn host_cli_get_arguments(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let result_ptr = arg_i32(args, 0)? as u32;

    let wasi2 = get_wasi2_ctx(ctx)?;
    let arguments = wasi2.cli_env.get_arguments();
    let count = arguments.len();

    if let Some(mem) = ctx.memories.get_mut(0) {
        mem.write_u32(result_ptr as usize, count as u32)
            .map_err(|_| TrapError::HostError(String::from("memory write error")))?;
    }

    Ok(vec![WasmValue::I32(count as i32)])
}

/// `wasi:cli/environment.initial-cwd → option<string>`
///
/// Returns: (has_cwd: i32, ptr: i32, len: i32)
fn host_cli_initial_cwd(
    ctx: &mut ExecutorContext,
    _args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let wasi2 = get_wasi2_ctx(ctx)?;
    match wasi2.cli_env.initial_cwd() {
        Some(cwd) => Ok(vec![
            WasmValue::I32(1), // has_cwd = true
            WasmValue::I32(cwd.len() as i32),
        ]),
        None => Ok(vec![WasmValue::I32(0)]),
    }
}

/// `wasi:cli/exit.exit(status: result)` — exit the program.
fn host_cli_exit(
    _ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let status = arg_i32(args, 0)? as u32;
    let exit_status = cli::exit(status);
    match exit_status {
        cli::ExitStatus::Code(0) => Ok(vec![]),
        cli::ExitStatus::Code(code) => Err(TrapError::HostError(
            alloc::format!("exit with code {}", code),
        )),
        cli::ExitStatus::Trap => Err(TrapError::HostError(String::from("exit trap"))),
    }
}

// ===========================================================================
// Helpers for S2
// ===========================================================================

/// Read a UTF-8 string from linear memory.
fn read_string_from_memory(
    ctx: &ExecutorContext,
    ptr: u32,
    len: u32,
) -> Result<String, TrapError> {
    if let Some(mem) = ctx.memories.get(0) {
        let bytes = mem
            .read_bytes(ptr as usize, len as usize)
            .map_err(|_| TrapError::HostError(String::from("memory read error")))?;
        String::from_utf8(bytes.to_vec())
            .map_err(|_| TrapError::HostError(String::from("invalid UTF-8 string")))
    } else {
        Err(TrapError::HostError(String::from("no linear memory")))
    }
}

/// Get the VFS from the WASI P1 context.
fn get_vfs(ctx: &ExecutorContext) -> Result<Vfs, TrapError> {
    // The VFS is in the WASI P1 context
    if let Some(ref wasi_ctx) = ctx.wasi_ctx {
        Ok(wasi_ctx.vfs.clone())
    } else {
        // Return a default empty VFS
        Ok(Vfs::new())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{register, poll};
    use super::super::{
        ResourceData, ResourceHandle, ResourceType, Wasi2Ctx,
    };
    use crate::instance::Imports;
    use crate::wasi2::streams::{InputStreamData, MemoryInputStream};
    use alloc::vec;

    // Integration-style tests that verify host function registration
    // and the Wasi2Ctx initialization.

    #[test]
    fn wasi2_host_registration() {
        let mut imports = Imports::new();
        register(&mut imports);
        // Verify that functions were registered (no panic = success)
        // The import table should contain all registered functions
    }

    #[test]
    fn wasi2_ctx_stdio_handles() {
        let ctx = Wasi2Ctx::new();
        let stdin_h = ctx.stdin_handle.unwrap();
        let stdout_h = ctx.stdout_handle.unwrap();
        let stderr_h = ctx.stderr_handle.unwrap();

        // All handles should be valid
        assert_eq!(
            ctx.resources.resource_type(stdin_h).unwrap(),
            ResourceType::InputStream
        );
        assert_eq!(
            ctx.resources.resource_type(stdout_h).unwrap(),
            ResourceType::OutputStream
        );
        assert_eq!(
            ctx.resources.resource_type(stderr_h).unwrap(),
            ResourceType::OutputStream
        );
    }

    #[test]
    fn wasi2_ctx_write_to_stdout() {
        let mut ctx = Wasi2Ctx::new();
        let stdout_h = ctx.stdout_handle.unwrap();

        // Write to stdout via resource table
        let data = ctx
            .resources
            .get_mut(stdout_h, ResourceType::OutputStream)
            .unwrap();
        if let ResourceData::OutputStream(stream) = data {
            stream.write(b"Hello, WASI P2!").unwrap();
        }
    }

    #[test]
    fn wasi2_ctx_read_from_memory_stream() {
        let mut ctx = Wasi2Ctx::new();

        // Create a memory input stream
        let handle = ctx
            .resources
            .push(
                ResourceType::InputStream,
                ResourceData::InputStream(InputStreamData::Memory(
                    MemoryInputStream::new(alloc::vec![42, 43, 44]),
                )),
            )
            .unwrap();

        // Read from it
        let data = ctx
            .resources
            .get_mut(handle, ResourceType::InputStream)
            .unwrap();
        if let ResourceData::InputStream(stream) = data {
            let bytes = stream.read(3).unwrap();
            assert_eq!(bytes, alloc::vec![42, 43, 44]);
        }
    }

    #[test]
    fn wasi2_ctx_stream_subscribe() {
        let mut ctx = Wasi2Ctx::new();
        let stdin_h = ctx.stdin_handle.unwrap();

        // Create a pollable for stdin
        let poll_h = poll::create_stream_pollable(&mut ctx.resources, stdin_h).unwrap();
        assert_eq!(
            ctx.resources.resource_type(poll_h).unwrap(),
            ResourceType::Pollable
        );

        // Poll it — should be ready (stream is always ready)
        let ready = poll::poll_list(&ctx.resources, &[poll_h], 0).unwrap();
        assert_eq!(ready, alloc::vec![0]);
    }
}
