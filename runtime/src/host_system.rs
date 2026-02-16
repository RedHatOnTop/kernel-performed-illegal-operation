//! `kpio:system` host function bindings.
//!
//! Provides system-level APIs for WASM apps: clock, hostname, notifications,
//! clipboard access, locale, and debug logging.

use alloc::vec;
use alloc::vec::Vec;

use crate::executor::ExecutorContext;
use crate::instance::Imports;
use crate::interpreter::{TrapError, WasmValue};

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all `kpio_system` host functions.
pub fn register(imports: &mut Imports) {
    imports.add_function("kpio_system", "get_time", host_get_time);
    imports.add_function("kpio_system", "get_monotonic", host_get_monotonic);
    imports.add_function("kpio_system", "get_hostname", host_get_hostname);
    imports.add_function("kpio_system", "notify", host_notify);
    imports.add_function("kpio_system", "clipboard_read", host_clipboard_read);
    imports.add_function("kpio_system", "clipboard_write", host_clipboard_write);
    imports.add_function("kpio_system", "get_locale", host_get_locale);
    imports.add_function("kpio_system", "log", host_log);
}

// ---------------------------------------------------------------------------
// Monotonic counter (no_std compatible fallback)
// ---------------------------------------------------------------------------

use spin::Mutex;

static MONOTONIC_COUNTER: Mutex<u64> = Mutex::new(0);
static CLIPBOARD: Mutex<Option<Vec<u8>>> = Mutex::new(None);

/// Advance the monotonic counter and return the new value.
/// In the kernel this would read from the platform's monotonic clock source.
fn monotonic_now() -> u64 {
    let mut counter = MONOTONIC_COUNTER.lock();
    *counter += 1_000_000; // simulate 1ms per call
    *counter
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

/// get_time() -> u64 (milliseconds since epoch)
fn host_get_time(
    _ctx: &mut ExecutorContext,
    _args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    // In kernel: read RTC or system clock.
    // Fallback: monotonic tick (no real-time source in no_std).
    let time_ms = monotonic_now() / 1_000_000; // convert ns → ms approximation
    Ok(vec![WasmValue::I64(time_ms as i64)])
}

/// get_monotonic() -> u64 (nanoseconds, monotonic)
fn host_get_monotonic(
    _ctx: &mut ExecutorContext,
    _args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let ns = monotonic_now();
    Ok(vec![WasmValue::I64(ns as i64)])
}

/// get_hostname(buf_ptr, buf_len) -> actual_len
fn host_get_hostname(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let buf_ptr = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let buf_len = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as usize;

    let hostname = b"kpio";
    let copy_len = hostname.len().min(buf_len);

    if let Some(mem) = ctx.memories.first_mut() {
        let _ = mem.write_bytes(buf_ptr, &hostname[..copy_len]);
    }

    Ok(vec![WasmValue::I32(copy_len as i32)])
}

/// notify(title_ptr, title_len, body_ptr, body_len)
fn host_notify(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let title_ptr = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let title_len = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let body_ptr = args.get(2).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let body_len = args.get(3).and_then(|v| v.as_i32()).unwrap_or(0) as usize;

    if let Some(mem) = ctx.memories.first() {
        let _title = mem
            .read_bytes(title_ptr, title_len)
            .ok()
            .and_then(|b| core::str::from_utf8(b).ok());
        let _body = mem
            .read_bytes(body_ptr, body_len)
            .ok()
            .and_then(|b| core::str::from_utf8(b).ok());
        // In kernel: NotificationCenter::show(title, body, app_id)
        // In test: notification is a no-op (data is read and discarded)
    }

    Ok(vec![WasmValue::I32(0)])
}

/// clipboard_read(buf_ptr, buf_len) -> actual_len (0 if empty)
fn host_clipboard_read(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let buf_ptr = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let buf_len = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as usize;

    let clip = CLIPBOARD.lock();
    if let Some(ref data) = *clip {
        let copy_len = data.len().min(buf_len);
        if let Some(mem) = ctx.memories.first_mut() {
            let _ = mem.write_bytes(buf_ptr, &data[..copy_len]);
        }
        Ok(vec![WasmValue::I32(copy_len as i32)])
    } else {
        Ok(vec![WasmValue::I32(0)])
    }
}

/// clipboard_write(data_ptr, data_len)
fn host_clipboard_write(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let data_ptr = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let data_len = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as usize;

    if let Some(mem) = ctx.memories.first() {
        if let Ok(bytes) = mem.read_bytes(data_ptr, data_len) {
            let mut clip = CLIPBOARD.lock();
            *clip = Some(bytes.to_vec());
        }
    }

    Ok(vec![WasmValue::I32(0)])
}

/// get_locale(buf_ptr, buf_len) -> actual_len
fn host_get_locale(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let buf_ptr = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let buf_len = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as usize;

    let locale = b"ko-KR";
    let copy_len = locale.len().min(buf_len);

    if let Some(mem) = ctx.memories.first_mut() {
        let _ = mem.write_bytes(buf_ptr, &locale[..copy_len]);
    }

    Ok(vec![WasmValue::I32(copy_len as i32)])
}

/// log(level, msg_ptr, msg_len)
/// level: 0=debug, 1=info, 2=warn, 3=error
fn host_log(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let level = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0);
    let msg_ptr = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let msg_len = args.get(2).and_then(|v| v.as_i32()).unwrap_or(0) as usize;

    if let Some(mem) = ctx.memories.first() {
        if let Ok(bytes) = mem.read_bytes(msg_ptr, msg_len) {
            if let Ok(msg) = core::str::from_utf8(bytes) {
                // Write to stderr buffer with level prefix
                let prefix = match level {
                    0 => b"[DEBUG] ",
                    1 => b"[INFO]  ",
                    2 => b"[WARN]  ",
                    3 => b"[ERROR] ",
                    _ => b"[LOG]   ",
                };
                ctx.stderr.extend_from_slice(prefix);
                ctx.stderr.extend_from_slice(msg.as_bytes());
                ctx.stderr.push(b'\n');
            }
        }
    }

    Ok(vec![WasmValue::I32(0)])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{MemoryType, Module};

    fn make_ctx_with_memory() -> ExecutorContext {
        let mut module = Module::empty();
        module.memories.push(MemoryType {
            min: 1,
            max: None,
        });
        ExecutorContext::new(module).unwrap()
    }

    #[test]
    fn test_get_monotonic_increases() {
        let mut ctx = make_ctx_with_memory();
        let r1 = host_get_monotonic(&mut ctx, &[]).unwrap();
        let r2 = host_get_monotonic(&mut ctx, &[]).unwrap();
        let v1 = r1[0].as_i64().unwrap();
        let v2 = r2[0].as_i64().unwrap();
        assert!(v2 > v1);
    }

    #[test]
    fn test_get_hostname() {
        let mut ctx = make_ctx_with_memory();
        let result = host_get_hostname(
            &mut ctx,
            &[WasmValue::I32(0), WasmValue::I32(100)],
        )
        .unwrap();
        let len = result[0].as_i32().unwrap() as usize;
        assert_eq!(len, 4); // "kpio"
        let bytes = ctx.memories[0].read_bytes(0, len).unwrap();
        assert_eq!(bytes, b"kpio");
    }

    #[test]
    fn test_get_locale() {
        let mut ctx = make_ctx_with_memory();
        let result = host_get_locale(
            &mut ctx,
            &[WasmValue::I32(0), WasmValue::I32(100)],
        )
        .unwrap();
        let len = result[0].as_i32().unwrap() as usize;
        assert_eq!(len, 5); // "ko-KR"
    }

    #[test]
    fn test_clipboard_roundtrip() {
        let mut ctx = make_ctx_with_memory();
        // Write "hello" to memory at offset 0
        let _ = ctx.memories[0].write_bytes(0, b"hello");

        // clipboard_write
        let _ = host_clipboard_write(
            &mut ctx,
            &[WasmValue::I32(0), WasmValue::I32(5)],
        );

        // clipboard_read to offset 100
        let result = host_clipboard_read(
            &mut ctx,
            &[WasmValue::I32(100), WasmValue::I32(100)],
        )
        .unwrap();
        let len = result[0].as_i32().unwrap() as usize;
        assert_eq!(len, 5);
        let bytes = ctx.memories[0].read_bytes(100, len).unwrap();
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn test_log_writes_stderr() {
        let mut ctx = make_ctx_with_memory();
        let _ = ctx.memories[0].write_bytes(0, b"test message");

        let _ = host_log(
            &mut ctx,
            &[
                WasmValue::I32(1), // info
                WasmValue::I32(0),
                WasmValue::I32(12),
            ],
        );

        let stderr = String::from_utf8(ctx.stderr.clone()).unwrap();
        assert!(stderr.contains("[INFO]"));
        assert!(stderr.contains("test message"));
    }
}
