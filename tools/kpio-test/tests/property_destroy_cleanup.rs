//! Property 18: Destroy cleanup completeness.
//!
//! After `destroy`, the Instance_Store directory does not exist;
//! after `destroy-all`, no Instance_Store directories remain.
//!
//! Validates: Requirements 12.1, 12.2

use std::fs;
use std::path::PathBuf;

use kpio_test::state::{InstanceConfig, InstanceState, InstanceStatus};
use kpio_test::store;

fn sample_state(name: &str) -> InstanceState {
    InstanceState {
        name: name.to_string(),
        pid: 999_999_999, // non-existent PID
        status: InstanceStatus::Stopped,
        qmp_socket: store::qmp_socket_path(name),
        serial_log: store::serial_log_path(name),
        qemu_log: store::qemu_log_path(name),
        screenshot_dir: store::screenshot_dir(name),
        created_at: "2024-01-15T10:30:00+00:00".to_string(),
        timeout_deadline: "2024-01-15T10:32:00+00:00".to_string(),
        timeout_seconds: 120,
        exit_code: Some(0),
        terminated_at: Some("2024-01-15T10:31:00+00:00".to_string()),
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

/// After destroy, the instance directory no longer exists.
#[test]
fn destroy_removes_instance_dir() {
    let name = "prop18-destroy-single";
    let _ = fs::remove_dir_all(store::instance_dir(name));

    // Create store and write state
    store::create_store(name).unwrap();
    store::write_state(&sample_state(name)).unwrap();
    assert!(store::instance_dir(name).exists());

    // Destroy
    let result = kpio_test::instance::destroy(name);
    assert!(result.is_ok());
    assert!(
        !store::instance_dir(name).exists(),
        "instance directory should not exist after destroy"
    );
}

/// After destroy-all, no instance directories remain.
#[test]
fn destroy_all_removes_all_instance_dirs() {
    let names = ["prop18-da-a", "prop18-da-b", "prop18-da-c"];

    // Clean up any leftovers
    for name in &names {
        let _ = fs::remove_dir_all(store::instance_dir(name));
    }

    // Create multiple instances
    for name in &names {
        store::create_store(name).unwrap();
        store::write_state(&sample_state(name)).unwrap();
    }

    // Verify they exist
    for name in &names {
        assert!(store::instance_dir(name).exists());
    }

    // Destroy all
    let result = kpio_test::instance::destroy_all();
    assert!(result.is_ok());

    // Verify none remain
    for name in &names {
        assert!(
            !store::instance_dir(name).exists(),
            "instance '{}' should not exist after destroy-all",
            name
        );
    }
}

/// Destroy of a non-existent instance returns an error.
#[test]
fn destroy_nonexistent_returns_error() {
    let result = kpio_test::instance::destroy("prop18-nonexistent-xyz");
    assert!(result.is_err());
}

/// Destroy cleans up all files within the instance store.
#[test]
fn destroy_removes_all_files() {
    let name = "prop18-destroy-files";
    let _ = fs::remove_dir_all(store::instance_dir(name));

    store::create_store(name).unwrap();
    store::write_state(&sample_state(name)).unwrap();

    // Create some extra files to simulate serial log and screenshots
    fs::write(store::serial_log_path(name), "boot log content").unwrap();
    fs::write(store::qemu_log_path(name), "qemu stderr").unwrap();
    fs::write(
        store::screenshot_dir(name).join("shot-001.ppm"),
        "PPM data",
    )
    .unwrap();

    // Destroy
    kpio_test::instance::destroy(name).unwrap();

    assert!(!store::instance_dir(name).exists());
    assert!(!store::serial_log_path(name).exists());
    assert!(!store::qemu_log_path(name).exists());
    assert!(!store::screenshot_dir(name).exists());
}
