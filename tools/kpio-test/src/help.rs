//! Help subcommand — list subcommands or show detailed help for one.
//!
//! Supports both human-readable and JSON output modes.

use serde::Serialize;

use crate::cli::HelpArgs;
use crate::error::KpioTestError;

// ── Output types ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct HelpOverview {
    pub subcommands: Vec<SubcommandSummary>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SubcommandSummary {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct SubcommandHelp {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ParameterInfo>,
    pub exit_codes: Vec<ExitCodeInfo>,
    pub examples: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ParameterInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub required: bool,
    pub default: Option<String>,
    pub description: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ExitCodeInfo {
    pub code: u8,
    pub meaning: String,
}

// ── Subcommand registry ──────────────────────────────────────────────

fn standard_exit_codes() -> Vec<ExitCodeInfo> {
    vec![
        ExitCodeInfo { code: 0, meaning: "Success".to_string() },
        ExitCodeInfo { code: 1, meaning: "Operational failure".to_string() },
        ExitCodeInfo { code: 2, meaning: "Infrastructure error".to_string() },
    ]
}

fn all_subcommands() -> Vec<SubcommandSummary> {
    vec![
        SubcommandSummary { name: "create".into(), description: "Create a new QEMU instance as a background process".into() },
        SubcommandSummary { name: "list".into(), description: "List all managed instances".into() },
        SubcommandSummary { name: "status".into(), description: "Query the status of a specific instance".into() },
        SubcommandSummary { name: "serial".into(), description: "Read serial console output from an instance".into() },
        SubcommandSummary { name: "serial-grep".into(), description: "Search serial log for lines matching a regex pattern".into() },
        SubcommandSummary { name: "wait-for".into(), description: "Block until a serial pattern appears or timeout elapses".into() },
        SubcommandSummary { name: "send-command".into(), description: "Send a text command to the serial console".into() },
        SubcommandSummary { name: "screenshot".into(), description: "Capture a screenshot of a running instance".into() },
        SubcommandSummary { name: "screenshot-interval".into(), description: "Configure periodic screenshot capture".into() },
        SubcommandSummary { name: "compare-screenshot".into(), description: "Compare a captured screenshot against a reference image".into() },
        SubcommandSummary { name: "screen-ocr".into(), description: "Extract text from the guest display via OCR".into() },
        SubcommandSummary { name: "send-key".into(), description: "Send keyboard key press/release events to an instance".into() },
        SubcommandSummary { name: "type-text".into(), description: "Type a text string as sequential key events".into() },
        SubcommandSummary { name: "mouse-click".into(), description: "Send a mouse click at specific coordinates".into() },
        SubcommandSummary { name: "mouse-move".into(), description: "Move the mouse to specific coordinates".into() },
        SubcommandSummary { name: "snapshot".into(), description: "Save, restore, list, or delete VM state snapshots".into() },
        SubcommandSummary { name: "guest-info".into(), description: "Query guest VM configuration and runtime information".into() },
        SubcommandSummary { name: "port-forward".into(), description: "Configure host-guest port forwarding".into() },
        SubcommandSummary { name: "copy-to".into(), description: "Copy a file from the host into the guest shared directory".into() },
        SubcommandSummary { name: "copy-from".into(), description: "Copy a file from the guest shared directory to the host".into() },
        SubcommandSummary { name: "logs".into(), description: "Retrieve QEMU process log output".into() },
        SubcommandSummary { name: "build".into(), description: "Build the kernel and create the UEFI disk image".into() },
        SubcommandSummary { name: "verify".into(), description: "Verify test results against a manifest".into() },
        SubcommandSummary { name: "health".into(), description: "Run pre-flight health checks without creating an instance".into() },
        SubcommandSummary { name: "destroy-all".into(), description: "Destroy all managed instances".into() },
        SubcommandSummary { name: "destroy".into(), description: "Destroy a specific instance and clean up resources".into() },
        SubcommandSummary { name: "help".into(), description: "Show help for a subcommand".into() },
    ]
}

fn subcommand_detail(name: &str) -> Option<SubcommandHelp> {
    let exit_codes = standard_exit_codes();
    match name {
        "create" => Some(SubcommandHelp {
            name: "create".into(),
            description: "Create a new QEMU instance as a background process".into(),
            parameters: vec![
                ParameterInfo { name: "name".into(), param_type: "string".into(), required: true, default: None, description: "Instance name".into() },
                ParameterInfo { name: "--image".into(), param_type: "path".into(), required: false, default: None, description: "Disk image path".into() },
                ParameterInfo { name: "--memory".into(), param_type: "string".into(), required: false, default: Some("512M".into()), description: "Memory size".into() },
                ParameterInfo { name: "--timeout".into(), param_type: "u64".into(), required: false, default: Some("120".into()), description: "Watchdog timeout in seconds".into() },
                ParameterInfo { name: "--gui".into(), param_type: "bool".into(), required: false, default: Some("false".into()), description: "Enable VNC display".into() },
                ParameterInfo { name: "--virtio-net".into(), param_type: "bool".into(), required: false, default: Some("false".into()), description: "Attach VirtIO network device".into() },
                ParameterInfo { name: "--virtio-blk".into(), param_type: "path".into(), required: false, default: None, description: "Attach VirtIO block device".into() },
                ParameterInfo { name: "--shared-dir".into(), param_type: "path".into(), required: false, default: None, description: "VirtIO-9p shared directory".into() },
            ],
            exit_codes,
            examples: vec![
                "kpio-test create boot-test".into(),
                "kpio-test create boot-test --memory 1G --timeout 60".into(),
                "kpio-test create io-test --virtio-net --shared-dir /tmp/share".into(),
            ],
        }),
        "destroy" => Some(SubcommandHelp {
            name: "destroy".into(),
            description: "Destroy a specific instance and clean up resources".into(),
            parameters: vec![
                ParameterInfo { name: "name".into(), param_type: "string".into(), required: true, default: None, description: "Instance name".into() },
            ],
            exit_codes,
            examples: vec!["kpio-test destroy boot-test".into()],
        }),
        "verify" => Some(SubcommandHelp {
            name: "verify".into(),
            description: "Verify test results against a manifest".into(),
            parameters: vec![
                ParameterInfo { name: "name".into(), param_type: "string".into(), required: true, default: None, description: "Instance name".into() },
                ParameterInfo { name: "--manifest".into(), param_type: "path".into(), required: true, default: None, description: "Test manifest TOML path".into() },
                ParameterInfo { name: "--mode".into(), param_type: "string".into(), required: false, default: None, description: "Test suite name to evaluate".into() },
            ],
            exit_codes,
            examples: vec![
                "kpio-test verify boot-test --manifest tests/manifests/boot.toml".into(),
                "kpio-test verify boot-test --manifest tests/manifests/default.toml --mode smoke".into(),
            ],
        }),
        "wait-for" => Some(SubcommandHelp {
            name: "wait-for".into(),
            description: "Block until a serial pattern appears or timeout elapses".into(),
            parameters: vec![
                ParameterInfo { name: "name".into(), param_type: "string".into(), required: true, default: None, description: "Instance name".into() },
                ParameterInfo { name: "--pattern".into(), param_type: "string".into(), required: true, default: None, description: "Pattern to wait for".into() },
                ParameterInfo { name: "--timeout".into(), param_type: "u64".into(), required: true, default: None, description: "Timeout in seconds".into() },
                ParameterInfo { name: "--regex".into(), param_type: "bool".into(), required: false, default: Some("false".into()), description: "Interpret pattern as regex".into() },
            ],
            exit_codes,
            examples: vec![
                "kpio-test wait-for boot-test --pattern \"Heap initialized\" --timeout 30".into(),
            ],
        }),
        _ => {
            // Generic help for subcommands without detailed entries
            let subs = all_subcommands();
            subs.iter().find(|s| s.name == name).map(|s| SubcommandHelp {
                name: s.name.clone(),
                description: s.description.clone(),
                parameters: vec![],
                exit_codes,
                examples: vec![],
            })
        }
    }
}

// ── Handler ──────────────────────────────────────────────────────────

pub fn show(args: HelpArgs) -> Result<serde_json::Value, KpioTestError> {
    match args.subcommand {
        None => {
            let output = HelpOverview {
                subcommands: all_subcommands(),
            };
            serde_json::to_value(&output).map_err(KpioTestError::Json)
        }
        Some(name) => {
            match subcommand_detail(&name) {
                Some(detail) => serde_json::to_value(&detail).map_err(KpioTestError::Json),
                None => Err(KpioTestError::UnknownSubcommand { name }),
            }
        }
    }
}

/// Return the list of all subcommand names (for property testing).
pub fn subcommand_names() -> Vec<String> {
    all_subcommands().iter().map(|s| s.name.clone()).collect()
}
