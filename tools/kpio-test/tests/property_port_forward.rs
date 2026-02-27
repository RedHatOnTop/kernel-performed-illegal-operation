//! Property 21: Port forwarding prerequisite check
//!
//! For any instance with `virtio_net = false`, port-forward fails with
//! `NetworkDeviceRequired`; with `virtio_net = true`, prerequisite passes.
//! Also verifies that `parse_usernet_output` correctly extracts forwarding
//! rules from generated `info usernet` output.
//!
//! **Validates: Requirements 24.5, 24.6**

use std::path::PathBuf;
use std::process::ExitCode;

use kpio_test::error::KpioTestError;
use kpio_test::network::{ensure_virtio_net, normalise_protocol, parse_usernet_output};
use kpio_test::state::{InstanceConfig, InstanceState, InstanceStatus};
use proptest::prelude::*;

// ── Helpers ──────────────────────────────────────────────────────────

fn make_state(virtio_net: bool) -> InstanceState {
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

/// Build a synthetic `info usernet` output block from a list of rules.
fn build_usernet_output(rules: &[(String, u16, u16)]) -> String {
    let mut out = String::from(
        "VLAN -1 (net0):\n  Protocol[State]    FD  Source Address  Port   Dest. Address  Port RecvQ SendQ\n",
    );
    for (proto, host_port, guest_port) in rules {
        let proto_upper = proto.to_ascii_uppercase();
        out.push_str(&format!(
            "  {proto_upper}[HOST_FORWARD]  12  0.0.0.0        {host_port}         10.0.2.15   {guest_port}     0     0\n"
        ));
    }
    out
}

// ── Strategies ───────────────────────────────────────────────────────

fn arb_protocol() -> impl Strategy<Value = String> {
    prop_oneof![Just("tcp".to_string()), Just("udp".to_string())]
}

fn arb_port() -> impl Strategy<Value = u16> {
    1u16..=65535u16
}

fn arb_rule() -> impl Strategy<Value = (String, u16, u16)> {
    (arb_protocol(), arb_port(), arb_port())
}

fn arb_rules() -> impl Strategy<Value = Vec<(String, u16, u16)>> {
    proptest::collection::vec(arb_rule(), 0..=10)
}

/// Arbitrary protocol string in mixed case for normalisation testing.
fn arb_protocol_mixed_case() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("tcp".to_string()),
        Just("TCP".to_string()),
        Just("Tcp".to_string()),
        Just("tCp".to_string()),
        Just("udp".to_string()),
        Just("UDP".to_string()),
        Just("Udp".to_string()),
        Just("uDp".to_string()),
    ]
}

// ── Property tests ───────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    // Feature: qemu-boot-testing-infrastructure, Property 21: Port forwarding prerequisite check

    /// For any instance with `virtio_net = false`, `ensure_virtio_net` fails
    /// with `NetworkDeviceRequired` (exit code 2).
    #[test]
    fn virtio_net_disabled_fails_prerequisite(pid in 1u32..100_000) {
        let mut state = make_state(false);
        state.pid = pid;
        let err = ensure_virtio_net(&state).unwrap_err();
        prop_assert!(
            matches!(err, KpioTestError::NetworkDeviceRequired),
            "expected NetworkDeviceRequired, got: {err:?}"
        );
        prop_assert_eq!(err.exit_code(), ExitCode::from(2));
    }

    /// For any instance with `virtio_net = true`, `ensure_virtio_net` passes.
    #[test]
    fn virtio_net_enabled_passes_prerequisite(pid in 1u32..100_000) {
        let mut state = make_state(true);
        state.pid = pid;
        prop_assert!(ensure_virtio_net(&state).is_ok());
    }

    /// For any set of generated forwarding rules in the expected `info usernet`
    /// format, `parse_usernet_output` extracts exactly those rules with correct
    /// protocol, host_port, and guest_port.
    #[test]
    fn parse_usernet_extracts_all_generated_rules(rules in arb_rules()) {
        let output = build_usernet_output(&rules);
        let parsed = parse_usernet_output(&output);

        prop_assert_eq!(
            parsed.len(),
            rules.len(),
            "expected {} rules, got {}",
            rules.len(),
            parsed.len()
        );

        for (i, ((proto, hp, gp), parsed_rule)) in rules.iter().zip(parsed.iter()).enumerate() {
            prop_assert_eq!(
                &parsed_rule.protocol, proto,
                "rule {}: protocol mismatch", i
            );
            prop_assert_eq!(
                parsed_rule.host_port, *hp,
                "rule {}: host_port mismatch", i
            );
            prop_assert_eq!(
                parsed_rule.guest_port, *gp,
                "rule {}: guest_port mismatch", i
            );
        }
    }

    /// For any valid protocol string (tcp/udp in any case), `normalise_protocol`
    /// returns the lowercase form.
    #[test]
    fn protocol_normalisation_preserves_semantics(proto in arb_protocol_mixed_case()) {
        let normalised = normalise_protocol(&proto).unwrap();
        let expected = proto.to_ascii_lowercase();
        prop_assert_eq!(normalised, expected);
    }
}
