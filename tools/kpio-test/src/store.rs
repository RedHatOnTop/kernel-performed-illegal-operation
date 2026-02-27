//! Instance Store — filesystem operations for managing per-instance state.
//!
//! Each instance lives at `target/qemu-instances/<name>/` with:
//! - `state.json`   — serialized `InstanceState`
//! - `serial.log`   — QEMU serial output
//! - `qemu.log`     — QEMU process stderr/stdout
//! - `qmp.sock`     — QMP Unix socket (or named pipe path on Windows)
//! - `screenshots/` — captured screenshots

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::KpioTestError;
use crate::state::InstanceState;

/// Base directory for all instance stores.
const INSTANCES_DIR: &str = "target/qemu-instances";

/// Return the base directory for all instance stores.
pub fn base_dir() -> PathBuf {
    PathBuf::from(INSTANCES_DIR)
}

/// Return the Instance_Store directory for the given instance name.
pub fn instance_dir(name: &str) -> PathBuf {
    base_dir().join(name)
}

/// Return the path to `state.json` for the given instance.
pub fn state_path(name: &str) -> PathBuf {
    instance_dir(name).join("state.json")
}

/// Return the path to `serial.log` for the given instance.
pub fn serial_log_path(name: &str) -> PathBuf {
    instance_dir(name).join("serial.log")
}

/// Return the path to `qemu.log` for the given instance.
pub fn qemu_log_path(name: &str) -> PathBuf {
    instance_dir(name).join("qemu.log")
}

/// Return the QMP socket path for the given instance.
pub fn qmp_socket_path(name: &str) -> PathBuf {
    instance_dir(name).join("qmp.sock")
}

/// Return the screenshot output directory for the given instance.
pub fn screenshot_dir(name: &str) -> PathBuf {
    instance_dir(name).join("screenshots")
}

/// Create the Instance_Store directory structure for a new instance.
///
/// Returns `Err(NameConflict)` if the directory already exists.
pub fn create_store(name: &str) -> Result<(), KpioTestError> {
    let dir = instance_dir(name);
    if dir.exists() {
        return Err(KpioTestError::NameConflict {
            name: name.to_string(),
        });
    }
    fs::create_dir_all(&dir)?;
    fs::create_dir_all(screenshot_dir(name))?;
    Ok(())
}

/// Delete the Instance_Store directory and all its contents.
///
/// Returns `Err(InstanceNotFound)` if the directory does not exist.
pub fn delete_store(name: &str) -> Result<(), KpioTestError> {
    let dir = instance_dir(name);
    if !dir.exists() {
        return Err(KpioTestError::InstanceNotFound {
            name: name.to_string(),
        });
    }
    fs::remove_dir_all(&dir)?;
    Ok(())
}

/// List all instance names by enumerating subdirectories under the base dir.
///
/// Returns an empty vec if the base directory does not exist.
pub fn list_instances() -> Result<Vec<String>, KpioTestError> {
    let base = base_dir();
    if !base.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(&base)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Read and deserialize the `state.json` for the given instance.
///
/// Returns `Err(InstanceNotFound)` if the instance directory or state file
/// does not exist.
pub fn read_state(name: &str) -> Result<InstanceState, KpioTestError> {
    let path = state_path(name);
    if !path.exists() {
        return Err(KpioTestError::InstanceNotFound {
            name: name.to_string(),
        });
    }
    let contents = fs::read_to_string(&path)?;
    let state: InstanceState = serde_json::from_str(&contents)?;
    Ok(state)
}

/// Serialize and write `state.json` atomically.
///
/// Writes to a temporary file first, then renames to avoid partial writes.
pub fn write_state(state: &InstanceState) -> Result<(), KpioTestError> {
    let path = state_path(&state.name);
    let dir = path.parent().ok_or_else(|| {
        KpioTestError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no parent directory for state file",
        ))
    })?;

    // Write to a temp file in the same directory, then rename for atomicity.
    let tmp_path = dir.join("state.json.tmp");
    let json = serde_json::to_string_pretty(state)?;
    fs::write(&tmp_path, json)?;
    fs::rename(&tmp_path, &path)?;
    Ok(())
}

/// Check whether an instance with the given name exists.
pub fn instance_exists(name: &str) -> bool {
    instance_dir(name).exists()
}

/// Return the instance directory rooted at a custom base path.
///
/// This is useful for testing with temporary directories.
pub fn instance_dir_with_base(base: &Path, name: &str) -> PathBuf {
    base.join(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{InstanceConfig, InstanceStatus};
    use std::path::PathBuf;

    fn sample_state(name: &str) -> InstanceState {
        InstanceState {
            name: name.to_string(),
            pid: 1234,
            status: InstanceStatus::Running,
            qmp_socket: qmp_socket_path(name),
            serial_log: serial_log_path(name),
            qemu_log: qemu_log_path(name),
            screenshot_dir: screenshot_dir(name),
            created_at: "2024-01-15T10:30:00Z".to_string(),
            timeout_deadline: "2024-01-15T10:32:00Z".to_string(),
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

    #[test]
    fn path_helpers_are_consistent() {
        let name = "my-instance";
        assert_eq!(instance_dir(name), PathBuf::from("target/qemu-instances/my-instance"));
        assert_eq!(state_path(name), PathBuf::from("target/qemu-instances/my-instance/state.json"));
        assert_eq!(serial_log_path(name), PathBuf::from("target/qemu-instances/my-instance/serial.log"));
        assert_eq!(qemu_log_path(name), PathBuf::from("target/qemu-instances/my-instance/qemu.log"));
        assert_eq!(qmp_socket_path(name), PathBuf::from("target/qemu-instances/my-instance/qmp.sock"));
        assert_eq!(screenshot_dir(name), PathBuf::from("target/qemu-instances/my-instance/screenshots"));
    }

    #[test]
    fn create_and_delete_store() {
        let name = "test-create-delete-store";
        // Clean up from any previous failed run
        let _ = fs::remove_dir_all(instance_dir(name));

        create_store(name).expect("create_store should succeed");
        assert!(instance_dir(name).exists());
        assert!(screenshot_dir(name).exists());

        // Creating again should fail with NameConflict
        let err = create_store(name).unwrap_err();
        assert!(matches!(err, KpioTestError::NameConflict { .. }));

        delete_store(name).expect("delete_store should succeed");
        assert!(!instance_dir(name).exists());
    }

    #[test]
    fn delete_nonexistent_store_fails() {
        let err = delete_store("nonexistent-instance-xyz").unwrap_err();
        assert!(matches!(err, KpioTestError::InstanceNotFound { .. }));
    }

    #[test]
    fn write_and_read_state() {
        let name = "test-write-read-state";
        let _ = fs::remove_dir_all(instance_dir(name));
        create_store(name).unwrap();

        let state = sample_state(name);
        write_state(&state).expect("write_state should succeed");

        let read_back = read_state(name).expect("read_state should succeed");
        assert_eq!(state, read_back);

        delete_store(name).unwrap();
    }

    #[test]
    fn read_state_nonexistent_fails() {
        let err = read_state("nonexistent-read-state-xyz").unwrap_err();
        assert!(matches!(err, KpioTestError::InstanceNotFound { .. }));
    }

    #[test]
    fn list_instances_returns_sorted_names() {
        let names = ["test-list-c", "test-list-a", "test-list-b"];
        for name in &names {
            let _ = fs::remove_dir_all(instance_dir(name));
            create_store(name).unwrap();
        }

        let listed = list_instances().unwrap();
        // Our test names should be in the list (there may be others from parallel tests)
        for name in &names {
            assert!(listed.contains(&name.to_string()), "missing {name}");
        }
        // Verify sorted
        let mut sorted = listed.clone();
        sorted.sort();
        assert_eq!(listed, sorted);

        for name in &names {
            delete_store(name).unwrap();
        }
    }

    #[test]
    fn instance_exists_check() {
        let name = "test-exists-check";
        let _ = fs::remove_dir_all(instance_dir(name));

        assert!(!instance_exists(name));
        create_store(name).unwrap();
        assert!(instance_exists(name));
        delete_store(name).unwrap();
        assert!(!instance_exists(name));
    }
}
