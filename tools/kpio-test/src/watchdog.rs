//! Lazy watchdog — enforced on every CLI invocation targeting an instance.
//!
//! No background daemon. Each subcommand calls `enforce()` which:
//! 1. Checks the watchdog deadline and terminates expired instances.
//! 2. Detects stale PIDs (process dead but state shows `running`).

use chrono::Utc;

use crate::error::KpioTestError;
use crate::state::{InstanceState, InstanceStatus};
use crate::store;

/// Check the watchdog deadline and stale PID for the given instance state.
///
/// - If the instance is `Running` and the deadline has passed, kill the
///   process and transition to `TimedOut`.
/// - If the instance is `Running` but the PID is dead, transition to
///   `Crashed`.
/// - Writes updated state back to disk when a transition occurs.
/// - Preserves serial log and screenshots (only the state file is updated).
pub fn enforce(state: &mut InstanceState) -> Result<(), KpioTestError> {
    if state.status != InstanceStatus::Running {
        return Ok(());
    }

    // Check deadline first
    let deadline = chrono::DateTime::parse_from_rfc3339(&state.timeout_deadline)
        .map_err(|e| KpioTestError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

    let now = Utc::now();
    if now > deadline {
        // Deadline elapsed — kill and mark timed-out
        let _ = kill_process(state.pid);
        state.status = InstanceStatus::TimedOut;
        state.terminated_at = Some(now.to_rfc3339());
        store::write_state(state)?;
        return Ok(());
    }

    // Check if PID is still alive
    if !is_process_alive(state.pid) {
        state.status = InstanceStatus::Crashed;
        state.terminated_at = Some(now.to_rfc3339());
        store::write_state(state)?;
    }

    Ok(())
}

/// Compute the watchdog deadline from a creation timestamp and timeout.
///
/// Returns the deadline as an RFC 3339 string.
pub fn compute_deadline(created_at: &str, timeout_seconds: u64) -> Result<String, KpioTestError> {
    let created = chrono::DateTime::parse_from_rfc3339(created_at)
        .map_err(|e| KpioTestError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
    let deadline = created + chrono::Duration::seconds(timeout_seconds as i64);
    Ok(deadline.to_rfc3339())
}

/// Default watchdog timeout in seconds.
pub const DEFAULT_TIMEOUT: u64 = 120;

/// Kill a process by PID. Best-effort; errors are ignored.
#[cfg(unix)]
fn kill_process(pid: u32) -> Result<(), std::io::Error> {
    use std::process::Command;
    // Send SIGTERM first, then SIGKILL
    let _ = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output();
    // Give it a moment, then force kill
    std::thread::sleep(std::time::Duration::from_millis(100));
    let _ = Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .output();
    Ok(())
}

#[cfg(windows)]
fn kill_process(pid: u32) -> Result<(), std::io::Error> {
    use std::process::Command;
    let _ = Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output();
    Ok(())
}

/// Check whether a process with the given PID is still alive.
#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    // kill(pid, 0) checks existence without sending a signal
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(windows)]
fn is_process_alive(pid: u32) -> bool {
    use std::process::Command;
    // Use tasklist to check if the PID exists
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.contains(&pid.to_string())
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_deadline_adds_timeout() {
        let created = "2024-01-15T10:30:00+00:00";
        let deadline = compute_deadline(created, 120).unwrap();
        assert!(deadline.contains("10:32:00"));
    }

    #[test]
    fn compute_deadline_default_timeout() {
        let created = "2024-01-15T10:00:00+00:00";
        let deadline = compute_deadline(created, DEFAULT_TIMEOUT).unwrap();
        assert!(deadline.contains("10:02:00"));
    }

    #[test]
    fn compute_deadline_invalid_timestamp() {
        let result = compute_deadline("not-a-timestamp", 120);
        assert!(result.is_err());
    }

    #[test]
    fn is_process_alive_returns_false_for_nonexistent_pid() {
        // PID 999999999 should not exist
        assert!(!is_process_alive(999_999_999));
    }
}
