//! Property 5: Lazy watchdog enforcement
//!
//! For any instance with status `running` and deadline in the past,
//! enforcement transitions to `timed-out` and preserves logs.
//!
//! Validates: Requirements 4.1, 4.2, 4.3, 4.5

use kpio_test::state::{InstanceConfig, InstanceState, InstanceStatus};
use kpio_test::store;
use kpio_test::watchdog;
use proptest::prelude::*;
use std::path::PathBuf;

/// Create a running instance state with a deadline in the past.
fn make_expired_state(name: &str) -> InstanceState {
    InstanceState {
        name: name.to_string(),
        // Use a PID that definitely doesn't exist
        pid: 999_999_999,
        status: InstanceStatus::Running,
        qmp_socket: store::qmp_socket_path(name),
        serial_log: store::serial_log_path(name),
        qemu_log: store::qemu_log_path(name),
        screenshot_dir: store::screenshot_dir(name),
        created_at: "2020-01-01T00:00:00+00:00".to_string(),
        // Deadline far in the past
        timeout_deadline: "2020-01-01T00:02:00+00:00".to_string(),
        timeout_seconds: 120,
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

    /// For any instance name, an expired running instance transitions to
    /// timed-out after enforcement, and the serial log / screenshot dir
    /// are preserved.
    #[test]
    fn expired_instance_transitions_to_timed_out(
        suffix in "[a-z]{3,8}"
    ) {
        let name = format!("prop5-{suffix}");
        // Setup: create store and write an expired state
        let _ = std::fs::remove_dir_all(store::instance_dir(&name));
        store::create_store(&name).unwrap();

        // Write a dummy serial log to verify preservation
        std::fs::write(store::serial_log_path(&name), "boot log line 1\nboot log line 2\n").unwrap();

        let mut state = make_expired_state(&name);
        store::write_state(&state).unwrap();

        // Enforce watchdog
        watchdog::enforce(&mut state).unwrap();

        // Verify transition
        prop_assert_eq!(state.status, InstanceStatus::TimedOut,
            "expired instance should transition to TimedOut");
        prop_assert!(state.terminated_at.is_some(),
            "terminated_at should be set");

        // Verify serial log is preserved
        let serial_content = std::fs::read_to_string(store::serial_log_path(&name)).unwrap();
        prop_assert!(serial_content.contains("boot log line 1"),
            "serial log should be preserved after timeout");

        // Verify screenshot dir is preserved
        prop_assert!(store::screenshot_dir(&name).exists(),
            "screenshot dir should be preserved after timeout");

        // Verify state was persisted to disk
        let disk_state = store::read_state(&name).unwrap();
        prop_assert_eq!(disk_state.status, InstanceStatus::TimedOut,
            "persisted state should show TimedOut");

        // Cleanup
        let _ = store::delete_store(&name);
    }

    /// Non-running instances are not affected by enforcement.
    #[test]
    fn non_running_instance_unchanged(
        suffix in "[a-z]{3,8}",
        status in prop_oneof![
            Just(InstanceStatus::Stopped),
            Just(InstanceStatus::Crashed),
            Just(InstanceStatus::TimedOut),
            Just(InstanceStatus::Creating),
        ]
    ) {
        let name = format!("prop5nr-{suffix}");
        let _ = std::fs::remove_dir_all(store::instance_dir(&name));
        store::create_store(&name).unwrap();

        let mut state = make_expired_state(&name);
        state.status = status;
        store::write_state(&state).unwrap();

        let original_status = state.status;
        watchdog::enforce(&mut state).unwrap();

        prop_assert_eq!(state.status, original_status,
            "non-running instance status should not change");

        let _ = store::delete_store(&name);
    }
}
