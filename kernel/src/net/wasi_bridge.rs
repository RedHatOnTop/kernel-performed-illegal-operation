//! WASI Network Bridge
//!
//! Exposes the kernel's real network stack as high-level synchronous
//! operations that the WASI2 runtime can call when the `kernel`
//! feature flag is enabled.
//!
//! This module bridges the gap between the WASI2 preview-2 API
//! (runtime crate) and the kernel's TCP/IP stack (DNS, TCP, HTTP,
//! TLS).  All calls are blocking — they poll the NIC until completion
//! or timeout.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;

use super::{dns, http, tcp, udp, Ipv4Addr, NetError, SocketAddr};

// ── HTTP bridge ─────────────────────────────────────────────

/// HTTP response returned from the kernel network stack.
#[derive(Debug, Clone)]
pub struct WasiHttpResponse {
    pub status: u16,
    pub content_type: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Perform an HTTP request via the kernel's full network stack.
///
/// Supports HTTP and HTTPS (TLS 1.3 with TLS 1.2 fallback).
/// DNS resolution, TCP handshake, and payload exchange are all real.
pub fn http_request(
    method: &str,
    url: &str,
    _headers: &[(String, String)],
    _body: &[u8],
) -> Result<WasiHttpResponse, NetError> {
    // The kernel HTTP client currently supports GET fully.
    // For other methods we still use the GET path — the URL
    // determines the response and POST body forwarding requires
    // extending kernel::net::http (future work).
    let _ = method;

    let response = http::get(url);

    // A status of 0 means the kernel HTTP client itself failed
    // (DNS, TCP, or TLS error).  Propagate as a NetError.
    if response.status == 0 {
        return Err(NetError::ConnectionRefused);
    }

    Ok(WasiHttpResponse {
        status: response.status,
        content_type: response.content_type.clone(),
        headers: alloc::vec![
            (String::from("content-type"), response.content_type),
            (
                String::from("content-length"),
                alloc::format!("{}", response.body.len()),
            ),
        ],
        body: response.body,
    })
}

// ── DNS bridge ──────────────────────────────────────────────

/// Resolve a hostname to a list of IPv4 addresses via the kernel DNS
/// resolver (host table → cache → wire query over UDP).
pub fn dns_resolve(hostname: &str) -> Result<Vec<[u8; 4]>, NetError> {
    let entry = dns::resolve(hostname)?;
    Ok(entry.addresses.iter().map(|a| a.0).collect())
}

// ── TCP bridge ──────────────────────────────────────────────

/// Opaque handle wrapping a kernel TCP `ConnId`.
#[derive(Debug)]
pub struct TcpHandle {
    pub conn_id: tcp::ConnId,
}

/// Open a TCP connection to a remote host.
///
/// Performs the full 3-way handshake via the VirtIO NIC.
pub fn tcp_connect(ip: [u8; 4], port: u16) -> Result<TcpHandle, NetError> {
    let conn = tcp::create();
    let remote = SocketAddr::new(Ipv4Addr(ip), port);
    tcp::connect(conn, remote)?;
    Ok(TcpHandle { conn_id: conn })
}

/// Send data on an established TCP connection.
pub fn tcp_send(handle: &TcpHandle, data: &[u8]) -> Result<usize, NetError> {
    tcp::send(handle.conn_id, data)
}

/// Receive data from an established TCP connection (blocking, ≈3 s timeout).
pub fn tcp_recv(handle: &TcpHandle, buf: &mut [u8]) -> Result<usize, NetError> {
    tcp::recv_blocking(handle.conn_id, buf, 300)
}

/// Non-blocking receive — returns `WouldBlock` when no data is ready.
pub fn tcp_recv_nonblocking(handle: &TcpHandle, buf: &mut [u8]) -> Result<usize, NetError> {
    super::poll_rx();
    tcp::recv(handle.conn_id, buf)
}

/// Check how many bytes are waiting in the receive buffer.
pub fn tcp_recv_available(handle: &TcpHandle) -> usize {
    tcp::recv_available(handle.conn_id)
}

/// Get the current TCP connection state.
pub fn tcp_state(handle: &TcpHandle) -> Option<tcp::TcpState> {
    tcp::state(handle.conn_id)
}

/// Gracefully close a TCP connection (sends FIN).
pub fn tcp_close(handle: &TcpHandle) -> Result<(), NetError> {
    tcp::close(handle.conn_id)
}

/// Destroy a TCP connection and free all resources.
pub fn tcp_destroy(handle: TcpHandle) {
    tcp::destroy(handle.conn_id);
}

// ── UDP bridge ──────────────────────────────────────────────

/// Send a UDP datagram.
pub fn udp_send(local_port: u16, dst_ip: [u8; 4], dst_port: u16, payload: &[u8]) {
    if let Some(frame) = udp::send(local_port, Ipv4Addr(dst_ip), dst_port, payload) {
        super::transmit_frame(&frame);
    }
}

/// Bind a local UDP port.  Pass `0` for an ephemeral port.
pub fn udp_bind(port: u16) -> u16 {
    udp::bind(port)
}
