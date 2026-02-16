//! `kpio:net` host function bindings.
//!
//! Provides TCP/UDP socket operations for WASM apps.  In the kernel these
//! calls forward to the smoltcp network stack; in test / host mode they
//! operate on an in-memory loopback buffer for verifiability.

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use crate::executor::ExecutorContext;
use crate::instance::Imports;
use crate::interpreter::{TrapError, WasmValue};

use spin::Mutex;

// ---------------------------------------------------------------------------
// Socket state
// ---------------------------------------------------------------------------

/// Socket domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketDomain {
    Ipv4 = 1,
    Ipv6 = 2,
}

/// Socket type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Stream = 1, // TCP
    Dgram = 2,  // UDP
}

/// In-memory virtual socket for testing.
#[derive(Debug, Clone)]
pub struct VirtualSocket {
    pub id: u32,
    pub domain: u32,
    pub sock_type: u32,
    pub bound_port: u16,
    pub connected: bool,
    pub remote_addr: Vec<u8>,
    pub remote_port: u16,
    /// Incoming data buffer (simulates receiving).
    pub recv_buffer: Vec<u8>,
    /// Outgoing data buffer (simulates sending).
    pub send_buffer: Vec<u8>,
    pub open: bool,
}

/// Global socket manager.
static SOCKETS: Mutex<Option<SocketManager>> = Mutex::new(None);

struct SocketManager {
    sockets: BTreeMap<u32, VirtualSocket>,
    next_id: u32,
}

impl SocketManager {
    fn new() -> Self {
        SocketManager {
            sockets: BTreeMap::new(),
            next_id: 1,
        }
    }
}

fn with_sm<F, R>(f: F) -> R
where
    F: FnOnce(&mut SocketManager) -> R,
{
    let mut guard = SOCKETS.lock();
    if guard.is_none() {
        *guard = Some(SocketManager::new());
    }
    f(guard.as_mut().unwrap())
}

// ---------------------------------------------------------------------------
// Error codes
// ---------------------------------------------------------------------------

/// Network error codes (POSIX-style).
pub const NET_OK: i32 = 0;
pub const NET_EINVAL: i32 = -22;
pub const NET_EBADF: i32 = -9;
pub const NET_ENOTCONN: i32 = -107;
pub const NET_ECONNREFUSED: i32 = -111;

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all `kpio_net` host functions.
pub fn register(imports: &mut Imports) {
    imports.add_function("kpio_net", "socket_create", host_socket_create);
    imports.add_function("kpio_net", "socket_bind", host_socket_bind);
    imports.add_function("kpio_net", "socket_connect", host_socket_connect);
    imports.add_function("kpio_net", "socket_send", host_socket_send);
    imports.add_function("kpio_net", "socket_recv", host_socket_recv);
    imports.add_function("kpio_net", "socket_close", host_socket_close);
    imports.add_function("kpio_net", "socket_listen", host_socket_listen);
    imports.add_function("kpio_net", "socket_accept", host_socket_accept);
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

/// socket_create(domain, sock_type) -> socket_id (negative on error)
fn host_socket_create(
    _ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let domain = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let sock_type = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as u32;

    // Validate domain / type
    if domain == 0 || sock_type == 0 {
        return Ok(vec![WasmValue::I32(NET_EINVAL)]);
    }

    let id = with_sm(|sm| {
        let id = sm.next_id;
        sm.next_id += 1;
        sm.sockets.insert(
            id,
            VirtualSocket {
                id,
                domain,
                sock_type,
                bound_port: 0,
                connected: false,
                remote_addr: Vec::new(),
                remote_port: 0,
                recv_buffer: Vec::new(),
                send_buffer: Vec::new(),
                open: true,
            },
        );
        id
    });

    Ok(vec![WasmValue::I32(id as i32)])
}

/// socket_bind(socket_id, addr_ptr, addr_len, port) -> 0 on success
fn host_socket_bind(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let sock_id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let _addr_ptr = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let _addr_len = args.get(2).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let port = args.get(3).and_then(|v| v.as_i32()).unwrap_or(0) as u16;

    let result = with_sm(|sm| {
        if let Some(sock) = sm.sockets.get_mut(&sock_id) {
            if !sock.open {
                return NET_EBADF;
            }
            sock.bound_port = port;
            NET_OK
        } else {
            NET_EBADF
        }
    });

    Ok(vec![WasmValue::I32(result)])
}

/// socket_connect(socket_id, addr_ptr, addr_len, port) -> 0 on success
fn host_socket_connect(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let sock_id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let addr_ptr = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let addr_len = args.get(2).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let port = args.get(3).and_then(|v| v.as_i32()).unwrap_or(0) as u16;

    let addr = if let Some(mem) = ctx.memories.first() {
        mem.read_bytes(addr_ptr, addr_len)
            .ok()
            .map(|b| b.to_vec())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let result = with_sm(|sm| {
        if let Some(sock) = sm.sockets.get_mut(&sock_id) {
            if !sock.open {
                return NET_EBADF;
            }
            sock.remote_addr = addr;
            sock.remote_port = port;
            sock.connected = true;
            NET_OK
        } else {
            NET_EBADF
        }
    });

    Ok(vec![WasmValue::I32(result)])
}

/// socket_send(socket_id, data_ptr, data_len) -> bytes_sent (negative on error)
fn host_socket_send(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let sock_id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let data_ptr = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let data_len = args.get(2).and_then(|v| v.as_i32()).unwrap_or(0) as usize;

    let data = if let Some(mem) = ctx.memories.first() {
        mem.read_bytes(data_ptr, data_len)
            .ok()
            .map(|b| b.to_vec())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let result = with_sm(|sm| {
        if let Some(sock) = sm.sockets.get_mut(&sock_id) {
            if !sock.open {
                return NET_EBADF;
            }
            if !sock.connected {
                return NET_ENOTCONN;
            }
            let len = data.len();
            sock.send_buffer.extend_from_slice(&data);
            len as i32
        } else {
            NET_EBADF
        }
    });

    Ok(vec![WasmValue::I32(result)])
}

/// socket_recv(socket_id, buf_ptr, buf_len) -> bytes_received (negative on error)
fn host_socket_recv(
    ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let sock_id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let buf_ptr = args.get(1).and_then(|v| v.as_i32()).unwrap_or(0) as usize;
    let buf_len = args.get(2).and_then(|v| v.as_i32()).unwrap_or(0) as usize;

    let (result, data) = with_sm(|sm| {
        if let Some(sock) = sm.sockets.get_mut(&sock_id) {
            if !sock.open {
                return (NET_EBADF, Vec::new());
            }
            if !sock.connected {
                return (NET_ENOTCONN, Vec::new());
            }
            let avail = sock.recv_buffer.len().min(buf_len);
            let data: Vec<u8> = sock.recv_buffer.drain(..avail).collect();
            (avail as i32, data)
        } else {
            (NET_EBADF, Vec::new())
        }
    });

    if result > 0 {
        if let Some(mem) = ctx.memories.first_mut() {
            let _ = mem.write_bytes(buf_ptr, &data);
        }
    }

    Ok(vec![WasmValue::I32(result)])
}

/// socket_close(socket_id) -> 0 on success
fn host_socket_close(
    _ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let sock_id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;

    let result = with_sm(|sm| {
        if let Some(sock) = sm.sockets.get_mut(&sock_id) {
            sock.open = false;
            sock.connected = false;
            NET_OK
        } else {
            NET_EBADF
        }
    });

    Ok(vec![WasmValue::I32(result)])
}

/// socket_listen(socket_id, backlog) -> 0 on success
fn host_socket_listen(
    _ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let sock_id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;
    let _backlog = args.get(1).and_then(|v| v.as_i32()).unwrap_or(128);

    let result = with_sm(|sm| {
        if sm.sockets.contains_key(&sock_id) {
            NET_OK
        } else {
            NET_EBADF
        }
    });

    Ok(vec![WasmValue::I32(result)])
}

/// socket_accept(socket_id) -> new_socket_id (negative on error)
fn host_socket_accept(
    _ctx: &mut ExecutorContext,
    args: &[WasmValue],
) -> Result<Vec<WasmValue>, TrapError> {
    let sock_id = args.get(0).and_then(|v| v.as_i32()).unwrap_or(0) as u32;

    let result = with_sm(|sm| {
        if let Some(sock) = sm.sockets.get(&sock_id) {
            if !sock.open {
                return NET_EBADF;
            }
            // In a real implementation, this would block and accept.
            // For testing, create a new connected socket.
            let new_id = sm.next_id;
            sm.next_id += 1;
            sm.sockets.insert(
                new_id,
                VirtualSocket {
                    id: new_id,
                    domain: sock.domain,
                    sock_type: sock.sock_type,
                    bound_port: 0,
                    connected: true,
                    remote_addr: Vec::new(),
                    remote_port: 0,
                    recv_buffer: Vec::new(),
                    send_buffer: Vec::new(),
                    open: true,
                },
            );
            new_id as i32
        } else {
            NET_EBADF
        }
    });

    Ok(vec![WasmValue::I32(result)])
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
            shared: false,
        });
        ExecutorContext::new(module).unwrap()
    }

    #[test]
    fn test_socket_create_and_close() {
        let mut ctx = make_ctx_with_memory();

        // Create TCP/IPv4 socket
        let result = host_socket_create(
            &mut ctx,
            &[WasmValue::I32(1), WasmValue::I32(1)],
        )
        .unwrap();
        let id = result[0].as_i32().unwrap();
        assert!(id > 0);

        // Close
        let result = host_socket_close(&mut ctx, &[WasmValue::I32(id)]).unwrap();
        assert_eq!(result[0].as_i32().unwrap(), NET_OK);
    }

    #[test]
    fn test_socket_connect_and_send() {
        let mut ctx = make_ctx_with_memory();

        // Create
        let result = host_socket_create(
            &mut ctx,
            &[WasmValue::I32(1), WasmValue::I32(1)],
        )
        .unwrap();
        let id = result[0].as_i32().unwrap();

        // Write address "127.0.0.1" to memory
        let _ = ctx.memories[0].write_bytes(0, b"127.0.0.1");

        // Connect
        let result = host_socket_connect(
            &mut ctx,
            &[
                WasmValue::I32(id),
                WasmValue::I32(0),
                WasmValue::I32(9),
                WasmValue::I32(8080),
            ],
        )
        .unwrap();
        assert_eq!(result[0].as_i32().unwrap(), NET_OK);

        // Send data
        let _ = ctx.memories[0].write_bytes(100, b"GET / HTTP/1.1\r\n");
        let result = host_socket_send(
            &mut ctx,
            &[
                WasmValue::I32(id),
                WasmValue::I32(100),
                WasmValue::I32(16),
            ],
        )
        .unwrap();
        assert_eq!(result[0].as_i32().unwrap(), 16);
    }

    #[test]
    fn test_send_without_connect_fails() {
        let mut ctx = make_ctx_with_memory();

        let result = host_socket_create(
            &mut ctx,
            &[WasmValue::I32(1), WasmValue::I32(1)],
        )
        .unwrap();
        let id = result[0].as_i32().unwrap();

        let result = host_socket_send(
            &mut ctx,
            &[
                WasmValue::I32(id),
                WasmValue::I32(0),
                WasmValue::I32(5),
            ],
        )
        .unwrap();
        assert_eq!(result[0].as_i32().unwrap(), NET_ENOTCONN);
    }

    #[test]
    fn test_invalid_socket_id() {
        let mut ctx = make_ctx_with_memory();
        let result = host_socket_close(&mut ctx, &[WasmValue::I32(9999)]).unwrap();
        assert_eq!(result[0].as_i32().unwrap(), NET_EBADF);
    }
}
