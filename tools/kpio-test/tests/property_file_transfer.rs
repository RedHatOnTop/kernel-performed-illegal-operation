//! Property 22: File transfer prerequisite check
//!
//! For any instance with `shared_dir = None`, copy-to/copy-from fails with
//! `SharedDirRequired`; with `shared_dir = Some(path)`, prerequisite passes
//! and returns the configured path.
//!
//! **Validates: Requirements 25.3, 25.4**

use std::path::PathBuf;
use std::process::ExitCode;

use kpio_test::error::KpioTestError;
use kpio_test::state::{InstanceConfig, InstanceState, InstanceStatus};
use kpio_test::transfer::ensure_shared_dir;
use proptest::prelude::*;

// ── Helpers ──────────────────────────────────────────────────────────

fn make_state(shared_dir: Option<PathBuf>) -> InstanceState {
    InstanceState {
        name: "test".to_string(),
        pid: 1234,
        status: InstanceStatus::Running,
        qmp_socket: PathBuf::from("qmp.sock"),
        serial_log: PathBuf::from("serial.log"),
        qemu_log: PathBuf::from("qemu.log"),
        screenshot_dir: PathBuf::from("screenshots"),
        created_at: "2024-01-15T10:30:00Z".to_string(),
        timeout_deadline: "2099-01-15T10:32:00Z".to_string(),
        timeout_seconds: 120,
        exit_code: None,
        terminated_at: None,
        config: InstanceConfig {
            image_path: PathBuf::from("test.img"),
            memory: "512M".to_string(),
            gui: false,
            virtio_net: false,
            virtio_blk: None,
            shared_dir,
            extra_args: vec![],
        },
    }
}

// ── Strategies ───────────────────────────────────────────────────────

/// Generate arbitrary non-empty path strings for shared directories.
fn arb_path() -> impl Strategy<Value = PathBuf> {
    prop::string::string_regex("[a-zA-Z0-9_/.-]{1,64}")
        .unwrap()
        .prop_filter("path must not be empty", |s| !s.is_empty())
        .prop_map(PathBuf::from)
}

// ── Property tests ───────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    // Feature: qemu-boot-testing-infrastructure, Property 22: File transfer prerequisite check

    /// For any instance with `shared_dir = None`, `ensure_shared_dir` fails
    /// with `SharedDirRequired` (exit code 2).
    #[test]
    fn shared_dir_none_fails_prerequisite(pid in 1u32..100_000) {
        let mut state = make_state(None);
        state.pid = pid;
        let err = ensure_shared_dir(&state).unwrap_err();
        prop_assert!(
            matches!(err, KpioTestError::SharedDirRequired),
            "expected SharedDirRequired, got: {err:?}"
        );
        prop_assert_eq!(err.exit_code(), ExitCode::from(2));
    }

    /// For any instance with `shared_dir = Some(path)`, `ensure_shared_dir`
    /// succeeds and returns the configured path.
    #[test]
    fn shared_dir_some_passes_prerequisite(path in arb_path(), pid in 1u32..100_000) {
        let mut state = make_state(Some(path.clone()));
        state.pid = pid;
        let result = ensure_shared_dir(&state);
        prop_assert!(result.is_ok(), "expected Ok, got: {result:?}");
        prop_assert_eq!(result.unwrap(), path);
    }

    /// The returned path from `ensure_shared_dir` matches the configured
    /// `shared_dir` path exactly — no transformation or normalisation.
    #[test]
    fn returned_path_matches_configured(path in arb_path()) {
        let state = make_state(Some(path.clone()));
        let returned = ensure_shared_dir(&state).unwrap();
        prop_assert_eq!(returned, path, "returned path must match configured shared_dir");
    }
}
