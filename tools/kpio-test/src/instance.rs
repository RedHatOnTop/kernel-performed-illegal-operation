//! Instance lifecycle handlers — create, destroy, destroy-all, status, list.
//!
//! `create` spawns QEMU as a background process and writes state.json.
//! `destroy` kills the process and removes the Instance_Store.
//! `status` and `list` enforce the watchdog before reporting.

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;

use crate::cli::CreateArgs;
use crate::error::KpioTestError;
use crate::health;
use crate::state::{InstanceConfig, InstanceState, InstanceStatus};
use crate::store;
use crate::watchdog;

// ── Output types ─────────────────────────────────────────────────────

#[derive(Serialize, Debug)]
pub struct CreateOutput {
    pub name: String,
    pub pid: u32,
    pub qmp_socket: String,
    pub status: String,
}

#[derive(Serialize, Debug)]
pub struct StatusOutput {
    pub name: String,
    pub status: String,
    pub pid: u32,
    pub uptime_seconds: Option<i64>,
    pub exit_code: Option<i32>,
    pub serial_tail: Vec<String>,
}

#[derive(Serialize, Debug)]
pub struct ListEntry {
    pub name: String,
    pub status: String,
    pub pid: u32,
    pub uptime_seconds: Option<i64>,
}

#[derive(Serialize, Debug)]
pub struct ListOutput {
    pub instances: Vec<ListEntry>,
}

#[derive(Serialize, Debug)]
pub struct DestroyOutput {
    pub destroyed: String,
}

#[derive(Serialize, Debug)]
pub struct DestroyAllOutput {
    pub destroyed: Vec<String>,
}

// ── Handlers ─────────────────────────────────────────────────────────

/// Create a new QEMU instance as a background process.
pub fn create(args: CreateArgs) -> Result<serde_json::Value, KpioTestError> {
    let name = &args.name;

    // Resolve image path
    let image_path = args
        .image
        .clone()
        .unwrap_or_else(|| default_image_path());

    // Run health check
    let report = health::check(Some(&image_path));
    health::validate(&report)?;

    // Create instance store (fails if name already exists)
    store::create_store(name)?;

    let config = InstanceConfig {
        image_path: image_path.clone(),
        memory: args.memory.clone(),
        gui: args.gui,
        virtio_net: args.virtio_net,
        virtio_blk: args.virtio_blk.clone(),
        shared_dir: args.shared_dir.clone(),
        extra_args: args.extra_args.clone(),
    };

    // Build QEMU command-line arguments
    let qemu_args = build_qemu_args(name, &config);

    // Spawn QEMU as a background process
    let qemu_log_path = store::qemu_log_path(name);
    let qemu_log_file = std::fs::File::create(&qemu_log_path)?;
    let qemu_err_file = qemu_log_file.try_clone()?;

    let child = spawn_qemu_background(&qemu_args, qemu_log_file, qemu_err_file)?;
    let pid = child.id();

    // Write state.json
    let now = Utc::now();
    let created_at = now.to_rfc3339();
    let timeout_deadline = watchdog::compute_deadline(&created_at, args.timeout)?;

    let state = InstanceState {
        name: name.clone(),
        pid,
        status: InstanceStatus::Running,
        qmp_socket: store::qmp_socket_path(name),
        serial_log: store::serial_log_path(name),
        qemu_log: qemu_log_path,
        screenshot_dir: store::screenshot_dir(name),
        created_at,
        timeout_deadline,
        timeout_seconds: args.timeout,
        exit_code: None,
        terminated_at: None,
        config,
    };
    store::write_state(&state)?;

    let output = CreateOutput {
        name: name.clone(),
        pid,
        qmp_socket: state.qmp_socket.to_string_lossy().to_string(),
        status: "running".to_string(),
    };
    Ok(serde_json::to_value(output)?)
}

/// Query the status of a specific instance.
pub fn status(name: &str) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(name)?;
    watchdog::enforce(&mut state)?;

    let uptime = compute_uptime(&state);
    let serial_tail = read_serial_tail(&state.serial_log, 10);

    let output = StatusOutput {
        name: state.name,
        status: state.status.to_string(),
        pid: state.pid,
        uptime_seconds: uptime,
        exit_code: state.exit_code,
        serial_tail,
    };
    Ok(serde_json::to_value(output)?)
}

/// List all managed instances.
pub fn list() -> Result<serde_json::Value, KpioTestError> {
    let names = store::list_instances()?;
    let mut entries = Vec::new();

    for name in &names {
        match store::read_state(name) {
            Ok(mut state) => {
                let _ = watchdog::enforce(&mut state);
                let uptime = compute_uptime(&state);
                entries.push(ListEntry {
                    name: state.name,
                    status: state.status.to_string(),
                    pid: state.pid,
                    uptime_seconds: uptime,
                });
            }
            Err(_) => {
                // Orphaned directory without valid state — skip
                entries.push(ListEntry {
                    name: name.clone(),
                    status: "unknown".to_string(),
                    pid: 0,
                    uptime_seconds: None,
                });
            }
        }
    }

    let output = ListOutput { instances: entries };
    Ok(serde_json::to_value(output)?)
}

/// Destroy a specific instance.
pub fn destroy(name: &str) -> Result<serde_json::Value, KpioTestError> {
    // Try to kill the process if state exists and it's running
    if let Ok(state) = store::read_state(name) {
        if state.status == InstanceStatus::Running {
            let _ = kill_process(state.pid);
        }
    }

    store::delete_store(name)?;

    let output = DestroyOutput {
        destroyed: name.to_string(),
    };
    Ok(serde_json::to_value(output)?)
}

/// Destroy all managed instances.
pub fn destroy_all() -> Result<serde_json::Value, KpioTestError> {
    let names = store::list_instances()?;
    let mut destroyed = Vec::new();

    for name in &names {
        if let Ok(state) = store::read_state(name) {
            if state.status == InstanceStatus::Running {
                let _ = kill_process(state.pid);
            }
        }
        if store::delete_store(name).is_ok() {
            destroyed.push(name.clone());
        }
    }

    let output = DestroyAllOutput { destroyed };
    Ok(serde_json::to_value(output)?)
}

// ── QEMU argument generation ─────────────────────────────────────────

/// Build the QEMU command-line arguments from an InstanceConfig.
pub fn build_qemu_args(name: &str, config: &InstanceConfig) -> Vec<String> {
    let mut args = Vec::new();

    // Memory
    args.extend(["-m".to_string(), config.memory.clone()]);

    // Display
    if config.gui {
        args.extend(["-display".to_string(), "gtk".to_string()]);
    } else {
        args.extend(["-display".to_string(), "none".to_string()]);
    }

    // UEFI firmware
    if let Some(ovmf) = health::find_ovmf() {
        args.extend([
            "-drive".to_string(),
            format!(
                "if=pflash,format=raw,readonly=on,file={}",
                ovmf.display()
            ),
        ]);
    }

    // Boot disk image
    args.extend([
        "-drive".to_string(),
        format!(
            "format=raw,file={}",
            config.image_path.display()
        ),
    ]);

    // isa-debug-exit device for kernel exit codes
    args.extend([
        "-device".to_string(),
        "isa-debug-exit,iobase=0xf4,iosize=0x04".to_string(),
    ]);

    // Serial output to file
    let serial_path = store::serial_log_path(name);
    args.extend([
        "-serial".to_string(),
        format!("file:{}", serial_path.display()),
    ]);

    // QMP socket
    let qmp_path = store::qmp_socket_path(name);
    #[cfg(unix)]
    args.extend([
        "-qmp".to_string(),
        format!("unix:{},server,nowait", qmp_path.display()),
    ]);
    #[cfg(windows)]
    args.extend([
        "-qmp".to_string(),
        format!("pipe:{}", qmp_path.display()),
    ]);

    // VirtIO network
    if config.virtio_net {
        args.extend([
            "-device".to_string(),
            "virtio-net-pci,netdev=net0".to_string(),
            "-netdev".to_string(),
            "user,id=net0".to_string(),
        ]);
    }

    // VirtIO block device
    if let Some(ref blk_path) = config.virtio_blk {
        args.extend([
            "-drive".to_string(),
            format!("file={},if=none,id=drive0", blk_path.display()),
            "-device".to_string(),
            "virtio-blk-pci,drive=drive0".to_string(),
        ]);
    }

    // VirtIO-9p shared directory
    if let Some(ref shared) = config.shared_dir {
        args.extend([
            "-fsdev".to_string(),
            format!("local,id=fsdev0,path={},security_model=mapped", shared.display()),
            "-device".to_string(),
            "virtio-9p-pci,fsdev=fsdev0,mount_tag=hostshare".to_string(),
        ]);
    }

    // Extra args
    args.extend(config.extra_args.clone());

    args
}

// ── Process management ───────────────────────────────────────────────

/// Spawn QEMU as a detached background process.
fn spawn_qemu_background(
    args: &[String],
    stdout_file: std::fs::File,
    stderr_file: std::fs::File,
) -> Result<std::process::Child, KpioTestError> {
    let qemu_bin = crate::health::find_qemu().ok_or_else(|| KpioTestError::QemuNotFound {
        hint: "qemu-system-x86_64 not found on PATH or in common install locations".to_string(),
    })?;

    let mut cmd = std::process::Command::new(&qemu_bin);
    cmd.args(args)
        .stdout(stdout_file)
        .stderr(stderr_file);

    // Detach from the parent process group
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            KpioTestError::QemuNotFound {
                hint: format!("QEMU binary at {} could not be executed", qemu_bin.display()),
            }
        } else {
            KpioTestError::Io(e)
        }
    })?;

    Ok(child)
}

/// Kill a process by PID.
#[cfg(unix)]
fn kill_process(pid: u32) -> Result<(), std::io::Error> {
    use std::process::Command;
    let _ = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output();
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

// ── Helpers ──────────────────────────────────────────────────────────

/// Default disk image path (debug build output).
fn default_image_path() -> PathBuf {
    PathBuf::from("target/x86_64-unknown-none/debug/kpio-uefi.img")
}

/// Compute uptime in seconds for a running instance, or None if not running.
fn compute_uptime(state: &InstanceState) -> Option<i64> {
    if state.status == InstanceStatus::Running {
        if let Ok(created) = chrono::DateTime::parse_from_rfc3339(&state.created_at) {
            let elapsed = Utc::now() - created.with_timezone(&Utc);
            return Some(elapsed.num_seconds());
        }
    }
    None
}

/// Read the last N lines from a serial log file.
fn read_serial_tail(path: &Path, n: usize) -> Vec<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = lines.len().saturating_sub(n);
            lines[start..].iter().map(|s| s.to_string()).collect()
        }
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::InstanceConfig;

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

    #[test]
    fn build_qemu_args_includes_memory() {
        let args = build_qemu_args("test", &sample_config());
        let idx = args.iter().position(|a| a == "-m").unwrap();
        assert_eq!(args[idx + 1], "512M");
    }

    #[test]
    fn build_qemu_args_headless_by_default() {
        let args = build_qemu_args("test", &sample_config());
        let idx = args.iter().position(|a| a == "-display").unwrap();
        assert_eq!(args[idx + 1], "none");
    }

    #[test]
    fn build_qemu_args_gui_mode() {
        let mut config = sample_config();
        config.gui = true;
        let args = build_qemu_args("test", &config);
        let idx = args.iter().position(|a| a == "-display").unwrap();
        assert_eq!(args[idx + 1], "gtk");
    }

    #[test]
    fn build_qemu_args_includes_isa_debug_exit() {
        let args = build_qemu_args("test", &sample_config());
        assert!(args.iter().any(|a| a.contains("isa-debug-exit")));
    }

    #[test]
    fn build_qemu_args_includes_serial() {
        let args = build_qemu_args("test", &sample_config());
        assert!(args.iter().any(|a| a.starts_with("file:") && a.contains("serial.log")));
    }

    #[test]
    fn build_qemu_args_includes_qmp() {
        let args = build_qemu_args("test", &sample_config());
        assert!(args.iter().any(|a| a.contains("qmp")));
    }

    #[test]
    fn build_qemu_args_virtio_net() {
        let mut config = sample_config();
        config.virtio_net = true;
        let args = build_qemu_args("test", &config);
        assert!(args.iter().any(|a| a.contains("virtio-net-pci")));
        assert!(args.iter().any(|a| a.contains("user,id=net0")));
    }

    #[test]
    fn build_qemu_args_virtio_blk() {
        let mut config = sample_config();
        config.virtio_blk = Some(PathBuf::from("extra.img"));
        let args = build_qemu_args("test", &config);
        assert!(args.iter().any(|a| a.contains("virtio-blk-pci")));
        assert!(args.iter().any(|a| a.contains("extra.img")));
    }

    #[test]
    fn build_qemu_args_shared_dir() {
        let mut config = sample_config();
        config.shared_dir = Some(PathBuf::from("/tmp/share"));
        let args = build_qemu_args("test", &config);
        assert!(args.iter().any(|a| a.contains("virtio-9p-pci")));
        assert!(args.iter().any(|a| a.contains("/tmp/share")));
    }

    #[test]
    fn build_qemu_args_extra_args() {
        let mut config = sample_config();
        config.extra_args = vec!["-cpu".to_string(), "host".to_string()];
        let args = build_qemu_args("test", &config);
        assert!(args.contains(&"-cpu".to_string()));
        assert!(args.contains(&"host".to_string()));
    }
}
