//! Property 6: Stale PID detection
//!
//! For any instance with status `running` but dead PID, status query
//! updates state to `crashed`.
//!
//! Validates: Requirements 4.4, 5.3

use kpio_test::state::{InstanceConfig, InstanceState, InstanceStatus};
use kpio_test::store;
use kpio_test::watchdog;
use proptest::prelude::*;
use std::path::PathBuf;

/// Create a running instance state with a dead PID but a deadline in the future.
fn make_stale_pid_state(name: &str) -> InstanceState {
    InstanceState {
        name: name.to_string(),
        // PID that definitely doesn't exist
        pid: 999_999_998,
        status: InstanceStatus::Running,
        qmp_socket: store::qmp_socket_path(name),
        serial_log: store::serial_log_path(name),
        qemu_log: store::qemu_log_path(name),
        screenshot_dir: store::screenshot_dir(name),
        created_at: "2024-01-01T00:00:00+00:00".to_string(),
        // Deadline far in the future so watchdog timeout doesn't trigger
        timeout_deadline: "2099-01-01T00:00:00+00:00".to_string(),
        timeout_seconds: 999_999,
        exit_code: None,
        terminated_at: None,
        config: InstanceConfig {
            image_path: PathBuf::from("test.img"),
            memory: "512M".to_string(),
            gui: false,
            virtio_net: false,
            virtio_blk: None,
            shared_dir: None,
            extra_args: vec![],
        },
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// For any instance name, a running instance with a dead PID transitions
    /// to crashed after enforcement.
    #[test]
    fn dead_pid_transitions_to_crashed(suffix in "[a-z]{3,8}") {
        let name = format!("prop6-{suffix}");
        let _ = std::fs::remove_dir_all(store::instance_dir(&name));
        store::create_store(&name).unwrap();

        let mut state = make_stale_pid_state(&name);
        store::write_state(&state).unwrap();

        // Enforce watchdog — should detect dead PID
        watchdog::enforce(&mut state).unwrap();

        prop_assert_eq!(state.status, InstanceStatus::Crashed,
            "instance with dead PID should transition to Crashed");
        prop_assert!(state.terminated_at.is_some(),
            "terminated_at should be set");

        // Verify state was persisted to disk
        let disk_state = store::read_state(&name).unwrap();
        prop_assert_eq!(disk_state.status, InstanceStatus::Crashed,
            "persisted state should show Crashed");

        // Cleanup
        let _ = store::delete_store(&name);
    }

    /// Dead PID with various non-existent PIDs all result in crashed.
    #[test]
    fn various_dead_pids_detected(
        suffix in "[a-z]{3,8}",
        dead_pid in 900_000_000u32..999_999_999
    ) {
        let name = format!("prop6v-{suffix}");
        let _ = std::fs::remove_dir_all(store::instance_dir(&name));
        store::create_store(&name).unwrap();

        let mut state = make_stale_pid_state(&name);
        state.pid = dead_pid;
        store::write_state(&state).unwrap();

        watchdog::enforce(&mut state).unwrap();

        prop_assert_eq!(state.status, InstanceStatus::Crashed,
            "PID {} should be detected as dead", dead_pid);

        let _ = store::delete_store(&name);
    }
}
