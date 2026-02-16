//! Host functions for WASM modules.
//!
//! This provides the kernel-side functions that WASM modules can call.

use alloc::string::String;
use alloc::vec::Vec;

/// Host functions state for WASM execution.
pub struct HostFunctions {
    /// Output buffer for print operations.
    output_buffer: Vec<u8>,

    /// Exit code (set when module calls exit).
    exit_code: Option<i32>,

    /// Fuel remaining (for execution limiting).
    fuel_remaining: u64,
}

impl HostFunctions {
    /// Create new host functions state.
    pub fn new() -> Self {
        HostFunctions {
            output_buffer: Vec::new(),
            exit_code: None,
            fuel_remaining: 1_000_000,
        }
    }

    /// Get the output buffer contents.
    pub fn get_output(&self) -> &[u8] {
        &self.output_buffer
    }

    /// Clear the output buffer.
    pub fn clear_output(&mut self) {
        self.output_buffer.clear();
    }

    /// Append to output buffer.
    pub fn write_output(&mut self, data: &[u8]) {
        self.output_buffer.extend_from_slice(data);
    }

    /// Print a string to serial output.
    pub fn print(&mut self, s: &str) {
        use crate::serial::_print;
        use core::fmt::Write;
        _print(format_args!("{}", s));
        self.output_buffer.extend_from_slice(s.as_bytes());
    }

    /// Print a line to serial output.
    pub fn println(&mut self, s: &str) {
        use crate::serial::_print;
        _print(format_args!("{}\n", s));
        self.output_buffer.extend_from_slice(s.as_bytes());
        self.output_buffer.push(b'\n');
    }

    /// Set exit code.
    pub fn exit(&mut self, code: i32) {
        self.exit_code = Some(code);
    }

    /// Get exit code if set.
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    /// Consume fuel.
    pub fn consume_fuel(&mut self, amount: u64) -> bool {
        if self.fuel_remaining >= amount {
            self.fuel_remaining -= amount;
            true
        } else {
            false
        }
    }

    /// Get remaining fuel.
    pub fn fuel_remaining(&self) -> u64 {
        self.fuel_remaining
    }
}

impl Default for HostFunctions {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// WASI-like host function implementations
// These would be registered with the linker for full WASI support
// ============================================================================

/// fd_write - write to a file descriptor (stub for stdout/stderr)
pub fn fd_write(
    host: &mut HostFunctions,
    fd: i32,
    iovs_ptr: i32,
    iovs_len: i32,
    nwritten_ptr: i32,
) -> i32 {
    // Simplified: only support stdout (1) and stderr (2)
    if fd != 1 && fd != 2 {
        return 8; // EBADF
    }

    // In a full implementation, we'd read from WASM memory
    // For now, just return success
    0
}

/// proc_exit - exit the process
pub fn proc_exit(host: &mut HostFunctions, code: i32) {
    host.exit(code);
}

/// environ_get - get environment variables (stub)
pub fn environ_get(_host: &mut HostFunctions, _environ: i32, _environ_buf: i32) -> i32 {
    0 // Success, no environment variables
}

/// environ_sizes_get - get environment variable sizes (stub)
pub fn environ_sizes_get(_host: &mut HostFunctions, _count_ptr: i32, _buf_size_ptr: i32) -> i32 {
    0 // Success, 0 variables, 0 bytes
}

/// args_get - get command line arguments (stub)
pub fn args_get(_host: &mut HostFunctions, _argv: i32, _argv_buf: i32) -> i32 {
    0 // Success
}

/// args_sizes_get - get argument sizes (stub)
pub fn args_sizes_get(_host: &mut HostFunctions, _count_ptr: i32, _buf_size_ptr: i32) -> i32 {
    0 // Success, 0 args, 0 bytes
}
