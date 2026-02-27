//! Property 7: Instance name conflict
//!
//! For any existing instance name, `create_store` with that name fails with
//! a NameConflict error (exit code 1) without modifying the existing instance.
//!
//! Validates: Requirements 3.11

use kpio_test::error::KpioTestError;
use kpio_test::state::{InstanceConfig, InstanceState, InstanceStatus};
use kpio_test::store;
use proptest::prelude::*;
use std::path::PathBuf;
use std::process::ExitCode;

fn sample_state(name: &str) -> InstanceState {
    InstanceState {
        name: name.to_string(),
        pid: 42,
        status: InstanceStatus::Running,
        qmp_socket: store::qmp_socket_path(name),
        serial_log: store::serial_log_path(name),
        qemu_log: store::qemu_log_path(name),
        screenshot_dir: store::screenshot_dir(name),
        created_at: "2024-06-01T12:00:00+00:00".to_string(),
        timeout_deadline: "2024-06-01T12:02:00+00:00".to_string(),
        timeout_seconds: 120,
        exit_code: None,
        terminated_at: None,
        config: InstanceConfig {
            image_path: PathBuf::from("kernel.img"),
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

    /// Creating a store with an existing name fails with NameConflict (exit
    /// code 1) and does not modify the existing state file.
    #[test]
    fn duplicate_name_fails_without_modifying_existing(suffix in "[a-z]{3,8}") {
        let name = format!("prop7-{suffix}");
        let _ = std::fs::remove_dir_all(store::instance_dir(&name));

        // Create the instance and write state
        store::create_store(&name).unwrap();
        let state = sample_state(&name);
        store::write_state(&state).unwrap();

        // Read the state file content before the conflict attempt
        let before = std::fs::read_to_string(store::state_path(&name)).unwrap();

        // Attempt to create again — should fail
        let err = store::create_store(&name).unwrap_err();
        prop_assert!(
            matches!(err, KpioTestError::NameConflict { .. }),
            "expected NameConflict, got: {:?}", err
        );

        // Exit code should be 1 (operational error)
        prop_assert_eq!(err.exit_code(), ExitCode::from(1),
            "NameConflict should have exit code 1");

        // State file should be unchanged
        let after = std::fs::read_to_string(store::state_path(&name)).unwrap();
        prop_assert_eq!(&before, &after,
            "state file should not be modified by failed create");

        // Cleanup
        let _ = store::delete_store(&name);
    }
}
