//! Host-guest file transfer via VirtIO-9p shared directory.
//!
//! The `copy-to` and `copy-from` subcommands transfer files between the host
//! and guest using a shared directory that was configured at instance creation
//! with `--shared-dir <path>`.
//!
//! Both operations work entirely on the host filesystem — the CLI places files
//! in (or reads files from) the shared directory. The guest-side 9p mount is
//! the guest OS's responsibility.

use std::path::PathBuf;

use serde::Serialize;

use crate::cli::{CopyFromArgs, CopyToArgs};
use crate::error::KpioTestError;
use crate::state::{InstanceState, InstanceStatus};
use crate::{store, watchdog};

// ── Output structs ───────────────────────────────────────────────────

/// Output returned by copy-to and copy-from operations.
#[derive(Serialize, Debug)]
pub struct FileTransferOutput {
    pub action: String,
    pub instance: String,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub bytes: u64,
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Validate that the instance was created with `--shared-dir`.
///
/// Returns the shared directory path on success, or `SharedDirRequired`
/// error if the instance has no shared directory configured.
pub fn ensure_shared_dir(state: &InstanceState) -> Result<PathBuf, KpioTestError> {
    match &state.config.shared_dir {
        Some(dir) => Ok(dir.clone()),
        None => Err(KpioTestError::SharedDirRequired),
    }
}

// ── Handlers ─────────────────────────────────────────────────────────

/// Copy a file from the host into the guest shared directory.
///
/// 1. Load instance state and enforce watchdog
/// 2. Verify instance is running
/// 3. Verify shared_dir is configured
/// 4. Verify source file exists on host
/// 5. Copy source file into `<shared_dir>/<dest>`
pub fn copy_to(args: CopyToArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    if state.status != InstanceStatus::Running {
        return Err(KpioTestError::InstanceNotRunning {
            name: args.name.clone(),
        });
    }

    let shared_dir = ensure_shared_dir(&state)?;

    // Validate source file exists
    if !args.src.exists() {
        return Err(KpioTestError::FileNotFound {
            path: args.src.clone(),
        });
    }

    // Destination inside the shared directory
    let dest_path = shared_dir.join(&args.dest);

    // Ensure parent directory exists inside shared dir
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let bytes = std::fs::copy(&args.src, &dest_path)?;

    let output = FileTransferOutput {
        action: "copy-to".to_string(),
        instance: args.name,
        source: args.src,
        destination: dest_path,
        bytes,
    };
    Ok(serde_json::to_value(output)?)
}

/// Copy a file from the guest shared directory to a host path.
///
/// 1. Load instance state and enforce watchdog
/// 2. Verify instance is running
/// 3. Verify shared_dir is configured
/// 4. Copy file from `<shared_dir>/<src>` to host destination
pub fn copy_from(args: CopyFromArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    if state.status != InstanceStatus::Running {
        return Err(KpioTestError::InstanceNotRunning {
            name: args.name.clone(),
        });
    }

    let shared_dir = ensure_shared_dir(&state)?;

    // Source inside the shared directory
    let src_path = shared_dir.join(&args.src);

    if !src_path.exists() {
        return Err(KpioTestError::FileNotFound {
            path: src_path,
        });
    }

    // Ensure parent directory of destination exists
    if let Some(parent) = args.dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let bytes = std::fs::copy(&src_path, &args.dest)?;

    let output = FileTransferOutput {
        action: "copy-from".to_string(),
        instance: args.name,
        source: src_path,
        destination: args.dest,
        bytes,
    };
    Ok(serde_json::to_value(output)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{InstanceConfig, InstanceStatus};

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

    #[test]
    fn ensure_shared_dir_passes_when_configured() {
        let state = make_state(Some(PathBuf::from("/tmp/share")));
        let result = ensure_shared_dir(&state);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/tmp/share"));
    }

    #[test]
    fn ensure_shared_dir_fails_when_none() {
        let state = make_state(None);
        let err = ensure_shared_dir(&state).unwrap_err();
        assert!(matches!(err, KpioTestError::SharedDirRequired));
    }

    #[test]
    fn shared_dir_required_has_exit_code_2() {
        let err = KpioTestError::SharedDirRequired;
        assert_eq!(err.exit_code(), std::process::ExitCode::from(2));
    }
}
