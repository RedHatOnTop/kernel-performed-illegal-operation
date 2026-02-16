//! Kernel Bridge Module
//!
//! This module provides the bridge between the browser shell and kernel services.
//! It wraps syscalls and IPC channels to provide a high-level API for browser components.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Process ID type
pub type Pid = u32;

/// App state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// App is not running
    NotRunning,
    /// App is starting up
    Starting,
    /// App is running normally
    Running,
    /// App is suspended (background)
    Suspended,
    /// App is stopping
    Stopping,
    /// App has crashed
    Crashed,
}

/// Kernel bridge for browser-kernel communication
pub struct KernelBridge {
    /// Whether the bridge is initialized
    initialized: AtomicBool,
    /// Next process ID (for mock)
    next_pid: AtomicU32,
    /// Running processes (mock storage)
    processes: spin::Mutex<Vec<ProcessInfo>>,
}

/// Process information
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: Pid,
    /// App name
    pub app_name: String,
    /// Current state
    pub state: AppState,
    /// Memory usage in bytes
    pub memory_bytes: u64,
    /// CPU usage percentage (0-100)
    pub cpu_percent: u8,
}

impl KernelBridge {
    /// Create a new kernel bridge
    pub const fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            next_pid: AtomicU32::new(1),
            processes: spin::Mutex::new(Vec::new()),
        }
    }

    /// Initialize the kernel bridge
    pub fn init(&self) -> Result<(), BridgeError> {
        if self.initialized.swap(true, Ordering::SeqCst) {
            return Ok(()); // Already initialized
        }

        // TODO: Initialize IPC channels with kernel
        // For now, we're in a mock state

        Ok(())
    }

    /// Spawn an application by name
    pub fn spawn_app(&self, app_name: &str) -> Result<Pid, BridgeError> {
        if !self.initialized.load(Ordering::SeqCst) {
            return Err(BridgeError::NotInitialized);
        }

        let pid = self.next_pid.fetch_add(1, Ordering::SeqCst);

        let info = ProcessInfo {
            pid,
            app_name: String::from(app_name),
            state: AppState::Running,
            memory_bytes: 0,
            cpu_percent: 0,
        };

        self.processes.lock().push(info);

        // TODO: Actually spawn process via syscall
        // syscall::spawn(app_name)

        Ok(pid)
    }

    /// Check if a process is running
    pub fn is_process_running(&self, pid: Pid) -> bool {
        self.processes
            .lock()
            .iter()
            .any(|p| p.pid == pid && matches!(p.state, AppState::Running | AppState::Suspended))
    }

    /// Get process state
    pub fn get_state(&self, pid: Pid) -> AppState {
        self.processes
            .lock()
            .iter()
            .find(|p| p.pid == pid)
            .map(|p| p.state)
            .unwrap_or(AppState::NotRunning)
    }

    /// Suspend a process
    pub fn suspend(&self, pid: Pid) -> Result<(), BridgeError> {
        let mut processes = self.processes.lock();

        if let Some(proc) = processes.iter_mut().find(|p| p.pid == pid) {
            if proc.state == AppState::Running {
                proc.state = AppState::Suspended;
                // TODO: Send SIGSTOP equivalent
                Ok(())
            } else {
                Err(BridgeError::InvalidState)
            }
        } else {
            Err(BridgeError::ProcessNotFound)
        }
    }

    /// Resume a suspended process
    pub fn resume(&self, pid: Pid) -> Result<(), BridgeError> {
        let mut processes = self.processes.lock();

        if let Some(proc) = processes.iter_mut().find(|p| p.pid == pid) {
            if proc.state == AppState::Suspended {
                proc.state = AppState::Running;
                // TODO: Send SIGCONT equivalent
                Ok(())
            } else {
                Err(BridgeError::InvalidState)
            }
        } else {
            Err(BridgeError::ProcessNotFound)
        }
    }

    /// Terminate a process
    pub fn terminate(&self, pid: Pid) -> Result<(), BridgeError> {
        let mut processes = self.processes.lock();

        if let Some(pos) = processes.iter().position(|p| p.pid == pid) {
            processes.remove(pos);
            // TODO: Send SIGTERM equivalent
            Ok(())
        } else {
            Err(BridgeError::ProcessNotFound)
        }
    }

    /// List all running processes
    pub fn list_processes(&self) -> Vec<ProcessInfo> {
        self.processes.lock().clone()
    }

    /// Get process info by PID
    pub fn get_process(&self, pid: Pid) -> Option<ProcessInfo> {
        self.processes.lock().iter().find(|p| p.pid == pid).cloned()
    }

    /// Send a signal to a process
    pub fn send_signal(&self, pid: Pid, signal: Signal) -> Result<(), BridgeError> {
        let processes = self.processes.lock();

        if processes.iter().any(|p| p.pid == pid) {
            // TODO: Actually send signal via syscall
            match signal {
                Signal::Terminate => drop(processes), // Release lock before calling terminate
                Signal::Kill => drop(processes),
                Signal::Stop => return self.suspend(pid),
                Signal::Continue => return self.resume(pid),
                _ => {}
            }
            Ok(())
        } else {
            Err(BridgeError::ProcessNotFound)
        }
    }
}

impl Default for KernelBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Signal types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// Hangup
    Hangup,
    /// Interrupt (Ctrl+C)
    Interrupt,
    /// Quit
    Quit,
    /// Terminate gracefully
    Terminate,
    /// Kill immediately
    Kill,
    /// Stop process
    Stop,
    /// Continue stopped process
    Continue,
    /// User defined 1
    User1,
    /// User defined 2
    User2,
}

/// Bridge error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeError {
    /// Bridge not initialized
    NotInitialized,
    /// Process not found
    ProcessNotFound,
    /// Invalid state for operation
    InvalidState,
    /// Permission denied
    PermissionDenied,
    /// Resource exhausted
    ResourceExhausted,
    /// IPC error
    IpcError,
    /// Syscall failed
    SyscallFailed,
    /// Timeout
    Timeout,
}

/// Global kernel bridge instance
static KERNEL_BRIDGE: KernelBridge = KernelBridge::new();

/// Get the global kernel bridge
pub fn kernel_bridge() -> &'static KernelBridge {
    &KERNEL_BRIDGE
}

/// Initialize the kernel bridge (call once at startup)
pub fn init() -> Result<(), BridgeError> {
    KERNEL_BRIDGE.init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_spawn() {
        let bridge = KernelBridge::new();
        bridge.init().unwrap();

        let pid = bridge.spawn_app("calculator").unwrap();
        assert!(pid > 0);
        assert!(bridge.is_process_running(pid));

        bridge.terminate(pid).unwrap();
        assert!(!bridge.is_process_running(pid));
    }

    #[test]
    fn test_app_suspend_resume() {
        let bridge = KernelBridge::new();
        bridge.init().unwrap();

        let pid = bridge.spawn_app("text-editor").unwrap();
        assert_eq!(bridge.get_state(pid), AppState::Running);

        bridge.suspend(pid).unwrap();
        assert_eq!(bridge.get_state(pid), AppState::Suspended);

        bridge.resume(pid).unwrap();
        assert_eq!(bridge.get_state(pid), AppState::Running);
    }
}
