//! TCP protocol handling.

use crate::NetworkError;

/// TCP connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

/// TCP connection.
pub struct TcpConnection {
    state: TcpState,
    local_seq: u32,
    remote_seq: u32,
    window_size: u16,
}

impl TcpConnection {
    /// Create a new TCP connection.
    pub fn new() -> Self {
        TcpConnection {
            state: TcpState::Closed,
            local_seq: 0,
            remote_seq: 0,
            window_size: 65535,
        }
    }
    
    /// Get the connection state.
    pub fn state(&self) -> TcpState {
        self.state
    }
}

impl Default for TcpConnection {
    fn default() -> Self {
        Self::new()
    }
}
