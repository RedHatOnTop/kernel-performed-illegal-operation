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

use super::{
    ResourceData, ResourceError, ResourceHandle, ResourceType, StreamError, Wasi2Ctx,
};
use super::poll;
use super::streams::InputStreamData;

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
