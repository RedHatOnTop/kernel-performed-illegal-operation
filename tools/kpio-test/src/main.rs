pub mod build;
pub mod cli;
pub mod error;
pub mod health;
pub mod help;
pub mod input;
pub mod instance;
pub mod manifest;
pub mod network;
pub mod ocr;
pub mod output;
pub mod qmp;
pub mod screenshot;
pub mod serial;
pub mod snapshot;
pub mod state;
pub mod store;
pub mod transfer;
pub mod watchdog;

use std::process::ExitCode;

use clap::Parser;
use serde::Serialize;

use crate::cli::{Cli, Command};
use crate::error::KpioTestError;

// ── Guest-info output ────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct GuestInfoOutput {
    memory_size: String,
    cpu_count: u32,
    block_devices: serde_json::Value,
    network_devices: serde_json::Value,
    display: serde_json::Value,
    qemu_version: String,
    machine_type: String,
    kvm_enabled: bool,
    vm_status: String,
}

/// Query guest VM information via QMP.
fn guest_info(name: &str) -> Result<serde_json::Value, KpioTestError> {
    let mut st = store::read_state(name)?;
    watchdog::enforce(&mut st)?;

    if st.status != state::InstanceStatus::Running {
        return Err(KpioTestError::InstanceNotRunning {
            name: name.to_string(),
        });
    }

    let mut qmp = qmp::QmpClient::connect(&st.qmp_socket)?;

    // Query status
    let status: serde_json::Value = qmp
        .execute("query-status", None)
        .unwrap_or(serde_json::json!({"status": "unknown"}));
    let vm_status = status["status"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    // Query block devices (graceful degradation)
    let block_devices: serde_json::Value = qmp
        .execute("query-block", None)
        .unwrap_or(serde_json::json!([]));

    // Query PCI devices (graceful degradation)
    let pci_devices: serde_json::Value = qmp
        .execute("query-pci", None)
        .unwrap_or(serde_json::json!([]));

    // Query VNC info (graceful degradation)
    let display: serde_json::Value = qmp
        .execute("query-vnc", None)
        .unwrap_or(serde_json::json!(null));

    // Query QEMU version (graceful degradation)
    let version_info: serde_json::Value = qmp
        .execute("query-version", None)
        .unwrap_or(serde_json::json!({"qemu": {"major": 0, "minor": 0, "micro": 0}}));
    let qemu_version = if let Some(qemu) = version_info.get("qemu") {
        format!(
            "{}.{}.{}",
            qemu.get("major").and_then(|v| v.as_u64()).unwrap_or(0),
            qemu.get("minor").and_then(|v| v.as_u64()).unwrap_or(0),
            qemu.get("micro").and_then(|v| v.as_u64()).unwrap_or(0),
        )
    } else {
        "unknown".to_string()
    };

    // Query KVM status (graceful degradation)
    let kvm_info: serde_json::Value = qmp
        .execute("query-kvm", None)
        .unwrap_or(serde_json::json!({"enabled": false}));
    let kvm_enabled = kvm_info
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let output = GuestInfoOutput {
        memory_size: st.config.memory.clone(),
        cpu_count: 1, // default; QEMU doesn't expose this easily via QMP
        block_devices,
        network_devices: pci_devices,
        display,
        qemu_version,
        machine_type: "q35".to_string(), // default machine type
        kvm_enabled,
        vm_status,
    };

    serde_json::to_value(&output).map_err(KpioTestError::Json)
}

// ── Logs handler ─────────────────────────────────────────────────────

/// Return QEMU process log (stderr/stdout) for an instance.
fn logs(args: cli::LogsArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut st = store::read_state(&args.name)?;
    watchdog::enforce(&mut st)?;

    let log_path = store::qemu_log_path(&args.name);
    let content = if log_path.exists() {
        std::fs::read_to_string(&log_path)?
    } else {
        String::new()
    };

    let lines: Vec<&str> = content.lines().collect();
    let output_lines: Vec<&str> = if let Some(n) = args.tail {
        lines.iter().rev().take(n).rev().copied().collect()
    } else {
        lines
    };

    Ok(serde_json::json!({
        "name": args.name,
        "lines": output_lines,
        "total_lines": content.lines().count(),
    }))
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result: Result<serde_json::Value, error::KpioTestError> = match cli.command {
        Command::Create(args) => instance::create(args),
        Command::List => instance::list(),
        Command::Status(args) => instance::status(&args.name),
        Command::Serial(args) => serial::read(args),
        Command::SerialGrep(args) => serial::grep(args),
        Command::WaitFor(args) => serial::wait_for(args),
        Command::SendCommand(args) => serial::send_command(args),
        Command::Screenshot(args) => screenshot::screenshot(args),
        Command::ScreenshotInterval(args) => screenshot::screenshot_interval(args),
        Command::CompareScreenshot(args) => screenshot::compare_screenshot(args),
        Command::ScreenOcr(args) => ocr::screen_ocr(args),
        Command::SendKey(args) => input::send_key(args),
        Command::TypeText(args) => input::type_text(args),
        Command::MouseClick(args) => input::mouse_click(args),
        Command::MouseMove(args) => input::mouse_move(args),
        Command::Snapshot(args) => snapshot::snapshot(args),
        Command::GuestInfo(args) => guest_info(&args.name),
        Command::PortForward(args) => network::port_forward(args),
        Command::CopyTo(args) => transfer::copy_to(args),
        Command::CopyFrom(args) => transfer::copy_from(args),
        Command::Logs(args) => logs(args),
        Command::Build(args) => build::build(args),
        Command::Verify(args) => manifest::verify(args),
        Command::Health => {
            let report = health::check(None);
            serde_json::to_value(&report).map_err(|e| error::KpioTestError::Json(e))
        }
        Command::DestroyAll => instance::destroy_all(),
        Command::Destroy(args) => instance::destroy(&args.name),
        Command::HelpCmd(args) => help::show(args),
    };

    match result {
        Ok(output) => {
            let _ = crate::output::emit(cli.output, &output);
            ExitCode::SUCCESS
        }
        Err(e) => {
            let code = e.exit_code();
            crate::output::emit_error(cli.output, exit_code_to_u8(&code), &e.to_string());
            code
        }
    }
}

fn exit_code_to_u8(code: &ExitCode) -> u8 {
    // ExitCode doesn't expose its inner value directly.
    // We compare against known values.
    if *code == ExitCode::from(2) {
        2
    } else if *code == ExitCode::from(1) {
        1
    } else {
        0
    }
}
