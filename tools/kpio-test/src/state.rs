use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Current lifecycle status of a QEMU instance.
///
/// Valid transitions:
/// - `Creating` → `Running` (QEMU spawned successfully)
/// - `Running`  → `Stopped`  (QEMU exits normally)
/// - `Running`  → `Crashed`  (QEMU exits abnormally or PID dead)
/// - `Running`  → `TimedOut` (watchdog deadline elapsed)
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceStatus {
    #[serde(rename = "creating")]
    Creating,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "crashed")]
    Crashed,
    #[serde(rename = "timed-out")]
    TimedOut,
}

impl InstanceStatus {
    /// Returns `true` when transitioning from `self` to `target` is valid.
    pub fn can_transition_to(self, target: InstanceStatus) -> bool {
        matches!(
            (self, target),
            (InstanceStatus::Creating, InstanceStatus::Running)
                | (InstanceStatus::Running, InstanceStatus::Stopped)
                | (InstanceStatus::Running, InstanceStatus::Crashed)
                | (InstanceStatus::Running, InstanceStatus::TimedOut)
        )
    }
}

impl std::fmt::Display for InstanceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstanceStatus::Creating => write!(f, "creating"),
            InstanceStatus::Running => write!(f, "running"),
            InstanceStatus::Stopped => write!(f, "stopped"),
            InstanceStatus::Crashed => write!(f, "crashed"),
            InstanceStatus::TimedOut => write!(f, "timed-out"),
        }
    }
}

/// Configuration used when creating a QEMU instance.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct InstanceConfig {
    /// Path to the UEFI disk image.
    pub image_path: PathBuf,
    /// Memory size (e.g. `"512M"`, `"1G"`).
    pub memory: String,
    /// Whether to launch with a VNC display backend.
    pub gui: bool,
    /// Whether a VirtIO network device is attached.
    pub virtio_net: bool,
    /// Optional VirtIO block device disk image path.
    pub virtio_blk: Option<PathBuf>,
    /// Optional VirtIO-9p shared directory for host-guest file transfer.
    pub shared_dir: Option<PathBuf>,
    /// Extra QEMU command-line arguments.
    pub extra_args: Vec<String>,
}

/// Full persisted state for a single QEMU instance (`state.json`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct InstanceState {
    /// User-provided instance name.
    pub name: String,
    /// QEMU process ID.
    pub pid: u32,
    /// Current lifecycle status.
    pub status: InstanceStatus,
    /// Path to the QMP Unix socket (or named pipe on Windows).
    pub qmp_socket: PathBuf,
    /// Path to the serial log file.
    pub serial_log: PathBuf,
    /// Path to the QEMU stderr/stdout log.
    pub qemu_log: PathBuf,
    /// Path to the screenshot output directory.
    pub screenshot_dir: PathBuf,
    /// Instance creation timestamp (ISO 8601).
    pub created_at: String,
    /// Watchdog deadline timestamp (ISO 8601).
    pub timeout_deadline: String,
    /// Watchdog timeout in seconds (original config value).
    pub timeout_seconds: u64,
    /// QEMU exit code, populated when stopped or crashed.
    pub exit_code: Option<i32>,
    /// Termination timestamp, populated when no longer running.
    pub terminated_at: Option<String>,
    /// Configuration used at creation time.
    pub config: InstanceConfig,
}

impl InstanceState {
    /// Attempt to transition this instance to `new_status`.
    ///
    /// Returns `Ok(())` if the transition is valid, or `Err` with a
    /// description of the invalid transition.
    pub fn transition_to(&mut self, new_status: InstanceStatus) -> Result<(), String> {
        if self.status.can_transition_to(new_status) {
            self.status = new_status;
            Ok(())
        } else {
            Err(format!(
                "invalid status transition: {} -> {}",
                self.status, new_status
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── InstanceStatus transition tests ──────────────────────────────

    #[test]
    fn creating_can_transition_to_running() {
        assert!(InstanceStatus::Creating.can_transition_to(InstanceStatus::Running));
    }

    #[test]
    fn running_can_transition_to_stopped() {
        assert!(InstanceStatus::Running.can_transition_to(InstanceStatus::Stopped));
    }

    #[test]
    fn running_can_transition_to_crashed() {
        assert!(InstanceStatus::Running.can_transition_to(InstanceStatus::Crashed));
    }

    #[test]
    fn running_can_transition_to_timed_out() {
        assert!(InstanceStatus::Running.can_transition_to(InstanceStatus::TimedOut));
    }

    #[test]
    fn creating_cannot_transition_to_stopped() {
        assert!(!InstanceStatus::Creating.can_transition_to(InstanceStatus::Stopped));
    }

    #[test]
    fn stopped_cannot_transition_to_running() {
        assert!(!InstanceStatus::Stopped.can_transition_to(InstanceStatus::Running));
    }

    #[test]
    fn crashed_cannot_transition_to_running() {
        assert!(!InstanceStatus::Crashed.can_transition_to(InstanceStatus::Running));
    }

    #[test]
    fn timed_out_cannot_transition_to_running() {
        assert!(!InstanceStatus::TimedOut.can_transition_to(InstanceStatus::Running));
    }

    // ── InstanceState::transition_to tests ───────────────────────────

    fn sample_config() -> InstanceConfig {
        InstanceConfig {
            image_path: PathBuf::from("test.img"),
            memory: "512M".to_string(),
            gui: false,
            virtio_net: false,
            virtio_blk: None,
            shared_dir: None,
            extra_args: vec![],
        }
    }

    fn sample_state(status: InstanceStatus) -> InstanceState {
        InstanceState {
            name: "test".to_string(),
            pid: 1234,
            status,
            qmp_socket: PathBuf::from("qmp.sock"),
            serial_log: PathBuf::from("serial.log"),
            qemu_log: PathBuf::from("qemu.log"),
            screenshot_dir: PathBuf::from("screenshots"),
            created_at: "2024-01-15T10:30:00Z".to_string(),
            timeout_deadline: "2024-01-15T10:32:00Z".to_string(),
            timeout_seconds: 120,
            exit_code: None,
            terminated_at: None,
            config: sample_config(),
        }
    }

    #[test]
    fn transition_creating_to_running_succeeds() {
        let mut state = sample_state(InstanceStatus::Creating);
        assert!(state.transition_to(InstanceStatus::Running).is_ok());
        assert_eq!(state.status, InstanceStatus::Running);
    }

    #[test]
    fn transition_running_to_stopped_succeeds() {
        let mut state = sample_state(InstanceStatus::Running);
        assert!(state.transition_to(InstanceStatus::Stopped).is_ok());
        assert_eq!(state.status, InstanceStatus::Stopped);
    }

    #[test]
    fn transition_stopped_to_running_fails() {
        let mut state = sample_state(InstanceStatus::Stopped);
        let result = state.transition_to(InstanceStatus::Running);
        assert!(result.is_err());
        assert_eq!(state.status, InstanceStatus::Stopped);
    }

    // ── Serde round-trip smoke test ──────────────────────────────────

    #[test]
    fn status_serde_round_trip() {
        for status in [
            InstanceStatus::Creating,
            InstanceStatus::Running,
            InstanceStatus::Stopped,
            InstanceStatus::Crashed,
            InstanceStatus::TimedOut,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let back: InstanceStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back);
        }
    }

    #[test]
    fn status_serde_rename_values() {
        assert_eq!(serde_json::to_string(&InstanceStatus::Creating).unwrap(), "\"creating\"");
        assert_eq!(serde_json::to_string(&InstanceStatus::Running).unwrap(), "\"running\"");
        assert_eq!(serde_json::to_string(&InstanceStatus::Stopped).unwrap(), "\"stopped\"");
        assert_eq!(serde_json::to_string(&InstanceStatus::Crashed).unwrap(), "\"crashed\"");
        assert_eq!(serde_json::to_string(&InstanceStatus::TimedOut).unwrap(), "\"timed-out\"");
    }

    #[test]
    fn instance_state_serde_round_trip() {
        let state = sample_state(InstanceStatus::Running);
        let json = serde_json::to_string_pretty(&state).unwrap();
        let back: InstanceState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, back);
    }

    #[test]
    fn instance_config_with_optional_fields() {
        let config = InstanceConfig {
            image_path: PathBuf::from("disk.img"),
            memory: "1G".to_string(),
            gui: true,
            virtio_net: true,
            virtio_blk: Some(PathBuf::from("extra.img")),
            shared_dir: Some(PathBuf::from("/tmp/share")),
            extra_args: vec!["-cpu".to_string(), "host".to_string()],
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: InstanceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, back);
    }
}
