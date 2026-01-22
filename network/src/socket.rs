//! Socket API implementation.

use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicU32, Ordering};
use spin::Mutex;

use crate::{NetworkError, SocketAddr};

/// Socket handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SocketHandle(pub u32);

/// Socket type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    /// TCP stream socket.
    Stream,
    /// UDP datagram socket.
    Datagram,
}

/// Socket state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Closed,
    Bound,
    Listening,
    Connecting,
    Connected,
}

/// Global socket table.
static SOCKETS: Mutex<BTreeMap<SocketHandle, Socket>> = Mutex::new(BTreeMap::new());
static NEXT_HANDLE: AtomicU32 = AtomicU32::new(1);

/// A socket.
pub struct Socket {
    handle: SocketHandle,
    socket_type: SocketType,
    state: SocketState,
    local_addr: Option<SocketAddr>,
    remote_addr: Option<SocketAddr>,
}

/// Create a new socket.
pub fn create(socket_type: SocketType) -> Result<SocketHandle, NetworkError> {
    let handle = SocketHandle(NEXT_HANDLE.fetch_add(1, Ordering::Relaxed));
    let socket = Socket {
        handle,
        socket_type,
        state: SocketState::Closed,
        local_addr: None,
        remote_addr: None,
    };
    SOCKETS.lock().insert(handle, socket);
    Ok(handle)
}

/// Bind a socket to an address.
pub fn bind(handle: SocketHandle, addr: SocketAddr) -> Result<(), NetworkError> {
    let mut sockets = SOCKETS.lock();
    let socket = sockets.get_mut(&handle).ok_or(NetworkError::NotConnected)?;
    socket.local_addr = Some(addr);
    socket.state = SocketState::Bound;
    Ok(())
}

/// Listen on a socket.
pub fn listen(handle: SocketHandle, _backlog: u32) -> Result<(), NetworkError> {
    let mut sockets = SOCKETS.lock();
    let socket = sockets.get_mut(&handle).ok_or(NetworkError::NotConnected)?;
    socket.state = SocketState::Listening;
    Ok(())
}

/// Connect a socket.
pub fn connect(handle: SocketHandle, addr: SocketAddr) -> Result<(), NetworkError> {
    let mut sockets = SOCKETS.lock();
    let socket = sockets.get_mut(&handle).ok_or(NetworkError::NotConnected)?;
    socket.remote_addr = Some(addr);
    socket.state = SocketState::Connected;
    Ok(())
}

/// Send data on a socket.
pub fn send(_handle: SocketHandle, _data: &[u8]) -> Result<usize, NetworkError> {
    // Would use smoltcp to send
    Err(NetworkError::WouldBlock)
}

/// Receive data from a socket.
pub fn recv(_handle: SocketHandle, _buffer: &mut [u8]) -> Result<usize, NetworkError> {
    // Would use smoltcp to receive
    Err(NetworkError::WouldBlock)
}

/// Close a socket.
pub fn close(handle: SocketHandle) -> Result<(), NetworkError> {
    SOCKETS.lock().remove(&handle);
    Ok(())
}
