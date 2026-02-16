//! Browser IPC Protocol Definitions
//!
//! This module defines the message types used for communication
//! between the kernel and Servo browser processes.

use alloc::string::String;
use alloc::vec::Vec;

/// Maximum size for inline data in messages.
pub const MAX_INLINE_DATA: usize = 4096;

/// Browser message types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MessageType {
    // ==========================================
    // Tab Management (0-9)
    // ==========================================
    /// Register a new tab.
    TabRegister = 0,
    /// Tab registration response.
    TabRegistered = 1,
    /// Unregister a tab.
    TabUnregister = 2,
    /// Tab state update.
    TabStateChange = 3,
    /// Request tab info.
    TabGetInfo = 4,
    /// Tab info response.
    TabInfo = 5,

    // ==========================================
    // GPU Operations (10-29)
    // ==========================================
    /// Allocate GPU memory.
    GpuAlloc = 10,
    /// GPU allocation response (with handle).
    GpuAllocResponse = 11,
    /// Free GPU memory.
    GpuFree = 12,
    /// Submit GPU commands.
    GpuSubmit = 13,
    /// GPU command completion.
    GpuComplete = 14,
    /// Present framebuffer.
    GpuPresent = 15,
    /// Present acknowledgment.
    GpuPresentAck = 16,
    /// Create GPU fence.
    GpuFenceCreate = 17,
    /// Wait for GPU fence.
    GpuFenceWait = 18,
    /// GPU fence signaled.
    GpuFenceSignaled = 19,

    // ==========================================
    // Network Operations (30-49)
    // ==========================================
    /// DNS lookup request.
    NetDnsLookup = 30,
    /// DNS lookup response.
    NetDnsResponse = 31,
    /// TCP connect request.
    NetTcpConnect = 32,
    /// TCP connected response.
    NetTcpConnected = 33,
    /// TCP send data.
    NetTcpSend = 34,
    /// TCP receive data.
    NetTcpReceive = 35,
    /// TCP close.
    NetTcpClose = 36,
    /// TLS handshake request.
    NetTlsHandshake = 37,
    /// TLS handshake complete.
    NetTlsReady = 38,
    /// HTTP request.
    NetHttpRequest = 39,
    /// HTTP response headers.
    NetHttpHeaders = 40,
    /// HTTP response body chunk.
    NetHttpBody = 41,

    // ==========================================
    // Input Events (50-69)
    // ==========================================
    /// Key press event.
    InputKeyDown = 50,
    /// Key release event.
    InputKeyUp = 51,
    /// Mouse move event.
    InputMouseMove = 52,
    /// Mouse button down.
    InputMouseDown = 53,
    /// Mouse button up.
    InputMouseUp = 54,
    /// Mouse scroll event.
    InputScroll = 55,
    /// Touch start event.
    InputTouchStart = 56,
    /// Touch move event.
    InputTouchMove = 57,
    /// Touch end event.
    InputTouchEnd = 58,

    // ==========================================
    // Window/Compositor (70-89)
    // ==========================================
    /// Request window dimensions.
    WindowGetSize = 70,
    /// Window size response.
    WindowSize = 71,
    /// Window resize notification.
    WindowResize = 72,
    /// Window focus change.
    WindowFocus = 73,
    /// Compositor layer create.
    CompositorLayerCreate = 74,
    /// Compositor layer update.
    CompositorLayerUpdate = 75,
    /// Compositor layer destroy.
    CompositorLayerDestroy = 76,
    /// Compositor commit.
    CompositorCommit = 77,

    // ==========================================
    // System (90-99)
    // ==========================================
    /// Ping (keepalive).
    Ping = 90,
    /// Pong response.
    Pong = 91,
    /// Error message.
    Error = 92,
    /// Shutdown request.
    Shutdown = 93,
}

/// Header for all browser messages.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BrowserMessageHeader {
    /// Message type.
    pub msg_type: MessageType,
    /// Sequence number for request/response matching.
    pub sequence: u32,
    /// Source tab ID (0 for coordinator).
    pub source_tab: u32,
    /// Destination tab ID (0 for coordinator).
    pub dest_tab: u32,
    /// Flags.
    pub flags: u32,
    /// Payload length.
    pub payload_len: u32,
}

impl BrowserMessageHeader {
    /// Create a new header.
    pub fn new(msg_type: MessageType, sequence: u32) -> Self {
        BrowserMessageHeader {
            msg_type,
            sequence,
            source_tab: 0,
            dest_tab: 0,
            flags: 0,
            payload_len: 0,
        }
    }

    /// Size of the header.
    pub const fn size() -> usize {
        core::mem::size_of::<Self>()
    }
}

/// Complete browser message.
#[derive(Debug, Clone)]
pub struct BrowserMessage {
    /// Message header.
    pub header: BrowserMessageHeader,
    /// Message payload.
    pub payload: Vec<u8>,
}

impl BrowserMessage {
    /// Create a new message.
    pub fn new(msg_type: MessageType, sequence: u32) -> Self {
        BrowserMessage {
            header: BrowserMessageHeader::new(msg_type, sequence),
            payload: Vec::new(),
        }
    }

    /// Create with payload.
    pub fn with_payload(msg_type: MessageType, sequence: u32, payload: Vec<u8>) -> Self {
        let mut msg = Self::new(msg_type, sequence);
        msg.header.payload_len = payload.len() as u32;
        msg.payload = payload;
        msg
    }

    /// Set source and destination tabs.
    pub fn with_routing(mut self, source: u32, dest: u32) -> Self {
        self.header.source_tab = source;
        self.header.dest_tab = dest;
        self
    }
}

// ==========================================
// Request/Response Types
// ==========================================

/// Tab registration request.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct TabRegisterRequest {
    /// Tab type (renderer, network, etc.).
    pub tab_type: u32,
    /// Requested priority.
    pub priority: u32,
    /// Name length.
    pub name_len: u32,
    // Followed by name bytes
}

/// Tab registered response.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TabRegisteredResponse {
    /// Assigned tab ID.
    pub tab_id: u32,
    /// GPU channel ID.
    pub gpu_channel: u64,
    /// Network channel ID.
    pub net_channel: u64,
    /// Input channel ID.
    pub input_channel: u64,
}

/// GPU allocation request.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GpuAllocRequest {
    /// Size in bytes.
    pub size: u64,
    /// Alignment requirement.
    pub alignment: u32,
    /// Usage flags.
    pub usage: u32,
}

/// GPU allocation response.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GpuAllocResponse {
    /// Allocated handle.
    pub handle: u64,
    /// Mapped virtual address (if applicable).
    pub vaddr: u64,
    /// Physical address (for DMA).
    pub paddr: u64,
    /// Actual size.
    pub size: u64,
}

/// GPU submit request.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GpuSubmitRequest {
    /// Command buffer handle.
    pub cmd_buffer: u64,
    /// Command buffer offset.
    pub offset: u32,
    /// Command buffer size.
    pub size: u32,
    /// Fence to signal on completion.
    pub fence: u64,
}

/// Error response.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ErrorResponse {
    /// Error code.
    pub code: i32,
    /// Message length.
    pub message_len: u32,
    // Followed by message bytes
}

/// Browser request enum (for high-level API).
#[derive(Debug, Clone)]
pub enum BrowserRequest {
    /// Register a new tab.
    RegisterTab { tab_type: u32, name: String },
    /// Allocate GPU memory.
    GpuAlloc { size: u64, usage: u32 },
    /// Submit GPU commands.
    GpuSubmit { cmd_buffer: u64, fence: u64 },
    /// Present frame.
    Present { layer: u32 },
    /// DNS lookup.
    DnsLookup { hostname: String },
    /// TCP connect.
    TcpConnect { addr: String, port: u16 },
}

/// Browser response enum.
#[derive(Debug, Clone)]
pub enum BrowserResponse {
    /// Tab registered.
    TabRegistered {
        tab_id: u32,
        channels: TabRegisteredResponse,
    },
    /// GPU memory allocated.
    GpuAllocated { handle: u64, vaddr: u64 },
    /// GPU command completed.
    GpuCompleted { fence: u64 },
    /// Present acknowledged.
    Presented { frame: u64 },
    /// DNS resolved.
    DnsResolved { addresses: Vec<[u8; 4]> },
    /// TCP connected.
    TcpConnected { socket_id: u64 },
    /// Error occurred.
    Error { code: i32, message: String },
}
