//! Port forwarding via QMP human-monitor-command.
//!
//! All port-forward operations require the instance to have been created
//! with `--virtio-net` (user-mode networking). Operations use QMP's
//! `human-monitor-command` to execute HMP commands:
//!
//! - **Add**: `hostfwd_add <proto>::<host_port>-:<guest_port>`
//! - **Remove**: `hostfwd_remove <proto>::<host_port>`
//! - **List**: `info usernet` — parsed to extract active forwarding rules

use serde::Serialize;

use crate::cli::PortForwardArgs;
use crate::error::KpioTestError;
use crate::qmp::QmpClient;
use crate::state::{InstanceState, InstanceStatus};
use crate::{store, watchdog};

// ── Output structs ───────────────────────────────────────────────────

/// A single port forwarding rule.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct PortForwardRule {
    pub protocol: String,
    pub host_port: u16,
    pub guest_port: u16,
}

/// Output returned by all port-forward operations.
#[derive(Serialize, Debug)]
pub struct PortForwardOutput {
    pub action: String,
    pub instance: String,
    pub rules: Vec<PortForwardRule>,
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Validate that the instance was created with `--virtio-net`.
pub fn ensure_virtio_net(state: &InstanceState) -> Result<(), KpioTestError> {
    if !state.config.virtio_net {
        return Err(KpioTestError::NetworkDeviceRequired);
    }
    Ok(())
}

/// Normalise the protocol string to lowercase and validate it.
pub fn normalise_protocol(proto: &str) -> Result<String, KpioTestError> {
    let lower = proto.to_ascii_lowercase();
    match lower.as_str() {
        "tcp" | "udp" => Ok(lower),
        _ => Err(KpioTestError::QmpError {
            desc: format!("unsupported protocol: {proto} (expected tcp or udp)"),
        }),
    }
}

/// Parse the output of `info usernet` to extract active forwarding rules.
///
/// The relevant section looks like:
/// ```text
/// Protocol[State]    FD  Source Address  Port   Dest. Address  Port RecvQ SendQ
/// TCP[HOST_FORWARD]  12  0.0.0.0        8080         10.0.2.15   80     0     0
/// UDP[HOST_FORWARD]   5  0.0.0.0        5353         10.0.2.15   53     0     0
/// ```
///
/// We look for lines containing `[HOST_FORWARD]` and extract the protocol,
/// host port (source port), and guest port (dest port).
pub fn parse_usernet_output(output: &str) -> Vec<PortForwardRule> {
    let mut rules = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("[HOST_FORWARD]") {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        // Expected columns:
        //   0: Protocol[State]  e.g. "TCP[HOST_FORWARD]"
        //   1: FD
        //   2: Source Address
        //   3: Host Port
        //   4: Dest Address
        //   5: Guest Port
        if parts.len() < 6 {
            continue;
        }
        let proto = parts[0]
            .split('[')
            .next()
            .unwrap_or("tcp")
            .to_ascii_lowercase();
        let host_port = match parts[3].parse::<u16>() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let guest_port = match parts[5].parse::<u16>() {
            Ok(p) => p,
            Err(_) => continue,
        };
        rules.push(PortForwardRule {
            protocol: proto,
            host_port,
            guest_port,
        });
    }
    rules
}

// ── Main handler ─────────────────────────────────────────────────────

/// Dispatch port-forward operations based on CLI args.
///
/// The handler checks that the instance is running and has `virtio_net`
/// enabled before issuing any QMP commands.
pub fn port_forward(args: PortForwardArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    // Must be running
    if state.status != InstanceStatus::Running {
        return Err(KpioTestError::InstanceNotRunning {
            name: args.name.clone(),
        });
    }

    // Must have virtio_net
    ensure_virtio_net(&state)?;

    let proto = normalise_protocol(&args.protocol)?;

    if args.list {
        // List active forwarding rules
        let mut qmp = QmpClient::connect(&state.qmp_socket)?;
        let raw = qmp.human_monitor_command("info usernet")?;
        let rules = parse_usernet_output(&raw);
        let output = PortForwardOutput {
            action: "list".to_string(),
            instance: args.name,
            rules,
        };
        return Ok(serde_json::to_value(output)?);
    }

    if args.remove {
        // Remove a forwarding rule by host port
        let host_port = args.host.ok_or_else(|| KpioTestError::QmpError {
            desc: "--host is required when removing a port forwarding rule".to_string(),
        })?;
        let cmd = format!("hostfwd_remove {proto}::{host_port}");
        let mut qmp = QmpClient::connect(&state.qmp_socket)?;
        qmp.human_monitor_command(&cmd)?;
        let rule = PortForwardRule {
            protocol: proto,
            host_port,
            guest_port: 0, // not known on remove
        };
        let output = PortForwardOutput {
            action: "removed".to_string(),
            instance: args.name,
            rules: vec![rule],
        };
        return Ok(serde_json::to_value(output)?);
    }

    // Default: add a forwarding rule
    let host_port = args.host.ok_or_else(|| KpioTestError::QmpError {
        desc: "--host is required when adding a port forwarding rule".to_string(),
    })?;
    let guest_port = args.guest.ok_or_else(|| KpioTestError::QmpError {
        desc: "--guest is required when adding a port forwarding rule".to_string(),
    })?;
    let cmd = format!("hostfwd_add {proto}::{host_port}-:{guest_port}");
    let mut qmp = QmpClient::connect(&state.qmp_socket)?;
    qmp.human_monitor_command(&cmd)?;
    let rule = PortForwardRule {
        protocol: proto,
        host_port,
        guest_port,
    };
    let output = PortForwardOutput {
        action: "added".to_string(),
        instance: args.name,
        rules: vec![rule],
    };
    Ok(serde_json::to_value(output)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_usernet_output tests ───────────────────────────────────

    #[test]
    fn parse_empty_output() {
        assert!(parse_usernet_output("").is_empty());
    }

    #[test]
    fn parse_no_forwarding_rules() {
        let output = "\
VLAN -1 (net0):
  Protocol[State]    FD  Source Address  Port   Dest. Address  Port RecvQ SendQ
";
        assert!(parse_usernet_output(output).is_empty());
    }

    #[test]
    fn parse_single_tcp_rule() {
        let output = "\
VLAN -1 (net0):
  Protocol[State]    FD  Source Address  Port   Dest. Address  Port RecvQ SendQ
  TCP[HOST_FORWARD]  12  0.0.0.0        8080         10.0.2.15   80     0     0
";
        let rules = parse_usernet_output(output);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].protocol, "tcp");
        assert_eq!(rules[0].host_port, 8080);
        assert_eq!(rules[0].guest_port, 80);
    }

    #[test]
    fn parse_multiple_rules() {
        let output = "\
VLAN -1 (net0):
  Protocol[State]    FD  Source Address  Port   Dest. Address  Port RecvQ SendQ
  TCP[HOST_FORWARD]  12  0.0.0.0        8080         10.0.2.15   80     0     0
  UDP[HOST_FORWARD]   5  0.0.0.0        5353         10.0.2.15   53     0     0
  TCP[HOST_FORWARD]  14  0.0.0.0        2222         10.0.2.15   22     0     0
";
        let rules = parse_usernet_output(output);
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].protocol, "tcp");
        assert_eq!(rules[0].host_port, 8080);
        assert_eq!(rules[0].guest_port, 80);
        assert_eq!(rules[1].protocol, "udp");
        assert_eq!(rules[1].host_port, 5353);
        assert_eq!(rules[1].guest_port, 53);
        assert_eq!(rules[2].protocol, "tcp");
        assert_eq!(rules[2].host_port, 2222);
        assert_eq!(rules[2].guest_port, 22);
    }

    #[test]
    fn parse_ignores_non_forward_lines() {
        let output = "\
VLAN -1 (net0):
  Protocol[State]    FD  Source Address  Port   Dest. Address  Port RecvQ SendQ
  TCP[ESTABLISHED]   10  10.0.2.15      12345        93.184.216.34  80     0     0
  TCP[HOST_FORWARD]  12  0.0.0.0        8080         10.0.2.15   80     0     0
";
        let rules = parse_usernet_output(output);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].host_port, 8080);
    }

    // ── ensure_virtio_net tests ──────────────────────────────────────

    #[test]
    fn ensure_virtio_net_passes_when_enabled() {
        let state = make_state(true);
        assert!(ensure_virtio_net(&state).is_ok());
    }

    #[test]
    fn ensure_virtio_net_fails_when_disabled() {
        let state = make_state(false);
        let err = ensure_virtio_net(&state).unwrap_err();
        assert!(matches!(err, KpioTestError::NetworkDeviceRequired));
    }

    // ── normalise_protocol tests ─────────────────────────────────────

    #[test]
    fn normalise_tcp() {
        assert_eq!(normalise_protocol("tcp").unwrap(), "tcp");
        assert_eq!(normalise_protocol("TCP").unwrap(), "tcp");
        assert_eq!(normalise_protocol("Tcp").unwrap(), "tcp");
    }

    #[test]
    fn normalise_udp() {
        assert_eq!(normalise_protocol("udp").unwrap(), "udp");
        assert_eq!(normalise_protocol("UDP").unwrap(), "udp");
    }

    #[test]
    fn normalise_invalid_protocol() {
        assert!(normalise_protocol("sctp").is_err());
    }

    // ── Test helpers ─────────────────────────────────────────────────

    fn make_state(virtio_net: bool) -> InstanceState {
        use crate::state::{InstanceConfig, InstanceStatus};
        use std::path::PathBuf;
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
                virtio_net,
                virtio_blk: None,
                shared_dir: None,
                extra_args: vec![],
            },
        }
    }
}
