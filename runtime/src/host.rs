//! Host function bindings for kernel services.
//!
//! This module provides the bridge between WASM modules and
//! kernel services through host functions.

use alloc::vec;
use alloc::vec::Vec;

use crate::executor::{ExecutorContext, HostFn};
use crate::instance::Imports;
use crate::interpreter::{TrapError, WasmValue};

/// Register all host functions.
pub fn register_all(imports: &mut Imports) {
    // WASI Preview 2 functions
    register_wasi_functions(imports);

    // KPIO-specific functions
    register_kpio_functions(imports);

    // Graphics functions
    register_graphics_functions(imports);

    // Network functions
    register_network_functions(imports);
}

/// Register WASI Preview 2 functions.
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
    imports.add_function("wasi_snapshot_preview1", "path_open", host_path_open);
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

// WASI implementations

fn host_args_get(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_args_sizes_get(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_environ_get(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_environ_sizes_get(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_clock_time_get(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    // Return current time in nanoseconds
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_fd_close(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_fd_read(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_fd_write(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_fd_seek(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_path_open(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_proc_exit(_ctx: &mut ExecutorContext, args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    let code = args.first().and_then(|v| v.as_i32()).unwrap_or(0);
    Err(TrapError::ProcessExit(code))
}

fn host_random_get(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

// KPIO implementations

fn host_ipc_send(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_ipc_recv(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_ipc_create_channel(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I64(0)]) // Channel ID
}

fn host_process_spawn(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I64(0)]) // Process ID
}

fn host_capability_derive(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I64(0)]) // New capability ID
}

// GPU implementations

fn host_gpu_create_surface(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I64(0)]) // Surface handle
}

fn host_gpu_create_buffer(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I64(0)]) // Buffer handle
}

fn host_gpu_submit_commands(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_gpu_present(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

// Network implementations

fn host_socket_create(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Socket FD
}

fn host_socket_bind(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_socket_connect(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Success
}

fn host_socket_send(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Bytes sent
}

fn host_socket_recv(_ctx: &mut ExecutorContext, _args: &[WasmValue]) -> Result<Vec<WasmValue>, TrapError> {
    Ok(vec![WasmValue::I32(0)]) // Bytes received
}
