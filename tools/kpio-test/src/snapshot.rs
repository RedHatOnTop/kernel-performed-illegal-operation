//! VM snapshot save/restore/list/delete via QMP.
//!
//! All snapshot operations require the instance disk image to be in qcow2
//! format and the instance to be running. Snapshots are stored inside the
//! qcow2 image itself by QEMU.

use serde::Serialize;

use crate::cli::SnapshotArgs;
use crate::error::KpioTestError;
use crate::qmp::QmpClient;
use crate::state::InstanceStatus;
use crate::{store, watchdog};

// ── Output structs ───────────────────────────────────────────────────

#[derive(Serialize, Debug)]
struct SnapshotSaveOutput {
    name: String,
    tag: String,
    action: &'static str,
}

#[derive(Serialize, Debug)]
struct SnapshotRestoreOutput {
    name: String,
    tag: String,
    action: &'static str,
}

#[derive(Serialize, Debug)]
pub struct SnapshotEntry {
    pub id: String,
    pub tag: String,
    pub vm_size: String,
    pub date: String,
    pub vm_clock: String,
}

#[derive(Serialize, Debug)]
struct SnapshotListOutput {
    name: String,
    snapshots: Vec<SnapshotEntry>,
    action: &'static str,
}

#[derive(Serialize, Debug)]
struct SnapshotDeleteOutput {
    name: String,
    tag: String,
    action: &'static str,
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Check that the instance disk image path ends with `.qcow2`.
///
/// QEMU snapshots (savevm/loadvm) require a qcow2-format disk image.
fn validate_qcow2(image_path: &std::path::Path) -> Result<(), KpioTestError> {
    match image_path.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("qcow2") => Ok(()),
        _ => Err(KpioTestError::SnapshotRequiresQcow2),
    }
}

/// Parse the output of `info snapshots` from QMP human-monitor-command.
///
/// The output looks like:
/// ```text
/// List of snapshots present on all disks:
/// ID        TAG               VM SIZE                DATE     VM CLOCK     ICOUNT
/// --        init              524288 2024-01-15 10:30:00  00:00:01.000       0
/// ```
///
/// Lines that don't match the expected column layout are skipped.
pub fn parse_snapshot_list(output: &str) -> Vec<SnapshotEntry> {
    let mut entries = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        // Skip header lines, separator lines, and empty lines
        if trimmed.is_empty()
            || trimmed.starts_with("List of snapshots")
            || trimmed.starts_with("ID")
            || trimmed.starts_with("--")
            || trimmed.starts_with("No snapshots")
            || trimmed.starts_with("There is no")
        {
            continue;
        }

        // Columns are whitespace-separated. The format is:
        //   ID  TAG  VM_SIZE  DATE(yyyy-mm-dd)  TIME(hh:mm:ss)  VM_CLOCK  [ICOUNT]
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 6 {
            entries.push(SnapshotEntry {
                id: parts[0].to_string(),
                tag: parts[1].to_string(),
                vm_size: parts[2].to_string(),
                date: format!("{} {}", parts[3], parts[4]),
                vm_clock: parts[5].to_string(),
            });
        }
    }
    entries
}

// ── Main handler ─────────────────────────────────────────────────────

/// Dispatch snapshot operations based on CLI args.
///
/// Exactly one of `--save`, `--restore`, `--list`, or `--delete` must be
/// provided. The function validates qcow2 format and running status before
/// issuing QMP commands.
pub fn snapshot(args: SnapshotArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    // All snapshot operations require a running instance
    if state.status != InstanceStatus::Running {
        return Err(KpioTestError::InstanceNotRunning {
            name: args.name.clone(),
        });
    }

    // All snapshot operations require qcow2 format
    validate_qcow2(&state.config.image_path)?;

    if let Some(ref tag) = args.save {
        let mut qmp = QmpClient::connect(&state.qmp_socket)?;
        qmp.savevm(tag)?;
        let output = SnapshotSaveOutput {
            name: args.name,
            tag: tag.clone(),
            action: "save",
        };
        Ok(serde_json::to_value(output)?)
    } else if let Some(ref tag) = args.restore {
        let mut qmp = QmpClient::connect(&state.qmp_socket)?;
        // Check if the snapshot exists by listing first
        let list_output = qmp.human_monitor_command("info snapshots")?;
        let snapshots = parse_snapshot_list(&list_output);
        if !snapshots.iter().any(|s| s.tag == *tag) {
            return Err(KpioTestError::SnapshotNotFound { tag: tag.clone() });
        }
        qmp.loadvm(tag)?;
        let output = SnapshotRestoreOutput {
            name: args.name,
            tag: tag.clone(),
            action: "restore",
        };
        Ok(serde_json::to_value(output)?)
    } else if args.list {
        let mut qmp = QmpClient::connect(&state.qmp_socket)?;
        let list_output = qmp.human_monitor_command("info snapshots")?;
        let snapshots = parse_snapshot_list(&list_output);
        let output = SnapshotListOutput {
            name: args.name,
            snapshots,
            action: "list",
        };
        Ok(serde_json::to_value(output)?)
    } else if let Some(ref tag) = args.delete {
        let mut qmp = QmpClient::connect(&state.qmp_socket)?;
        // Check if the snapshot exists by listing first
        let list_output = qmp.human_monitor_command("info snapshots")?;
        let snapshots = parse_snapshot_list(&list_output);
        if !snapshots.iter().any(|s| s.tag == *tag) {
            return Err(KpioTestError::SnapshotNotFound { tag: tag.clone() });
        }
        qmp.human_monitor_command(&format!("delvm {tag}"))?;
        let output = SnapshotDeleteOutput {
            name: args.name,
            tag: tag.clone(),
            action: "delete",
        };
        Ok(serde_json::to_value(output)?)
    } else {
        Err(KpioTestError::QmpError {
            desc: "snapshot requires one of --save, --restore, --list, or --delete".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ── validate_qcow2 tests ─────────────────────────────────────────

    #[test]
    fn validate_qcow2_accepts_qcow2_extension() {
        assert!(validate_qcow2(&PathBuf::from("disk.qcow2")).is_ok());
    }

    #[test]
    fn validate_qcow2_accepts_uppercase() {
        assert!(validate_qcow2(&PathBuf::from("disk.QCOW2")).is_ok());
    }

    #[test]
    fn validate_qcow2_rejects_raw() {
        let err = validate_qcow2(&PathBuf::from("disk.raw")).unwrap_err();
        assert!(matches!(err, KpioTestError::SnapshotRequiresQcow2));
    }

    #[test]
    fn validate_qcow2_rejects_img() {
        let err = validate_qcow2(&PathBuf::from("disk.img")).unwrap_err();
        assert!(matches!(err, KpioTestError::SnapshotRequiresQcow2));
    }

    #[test]
    fn validate_qcow2_rejects_no_extension() {
        let err = validate_qcow2(&PathBuf::from("disk")).unwrap_err();
        assert!(matches!(err, KpioTestError::SnapshotRequiresQcow2));
    }

    // ── parse_snapshot_list tests ────────────────────────────────────

    #[test]
    fn parse_empty_output() {
        assert!(parse_snapshot_list("").is_empty());
    }

    #[test]
    fn parse_no_snapshots_message() {
        let output = "No snapshots available.\n";
        assert!(parse_snapshot_list(output).is_empty());
    }

    #[test]
    fn parse_single_snapshot() {
        let output = "\
List of snapshots present on all disks:
ID        TAG               VM SIZE                DATE     VM CLOCK     ICOUNT
--        --                ------                 ----     --------     ------
1         init              524288 2024-01-15 10:30:00  00:00:01.000       0
";
        let entries = parse_snapshot_list(output);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "1");
        assert_eq!(entries[0].tag, "init");
        assert_eq!(entries[0].vm_size, "524288");
        assert_eq!(entries[0].date, "2024-01-15 10:30:00");
        assert_eq!(entries[0].vm_clock, "00:00:01.000");
    }

    #[test]
    fn parse_multiple_snapshots() {
        let output = "\
List of snapshots present on all disks:
ID        TAG               VM SIZE                DATE     VM CLOCK     ICOUNT
--        --                ------                 ----     --------     ------
1         init              524288 2024-01-15 10:30:00  00:00:01.000       0
2         after-boot        1048576 2024-01-15 10:31:00  00:00:05.500       0
3         test-ready        2097152 2024-01-15 10:32:00  00:00:10.200       0
";
        let entries = parse_snapshot_list(output);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].tag, "init");
        assert_eq!(entries[1].tag, "after-boot");
        assert_eq!(entries[2].tag, "test-ready");
    }

    #[test]
    fn parse_header_only_no_snapshots() {
        let output = "\
List of snapshots present on all disks:
ID        TAG               VM SIZE                DATE     VM CLOCK     ICOUNT
--        --                ------                 ----     --------     ------
";
        let entries = parse_snapshot_list(output);
        assert!(entries.is_empty());
    }
}
