//! Property 3: State file round-trip
//!
//! For any valid `InstanceState`, serialize to JSON and deserialize back
//! produces an equivalent value.
//!
//! Validates: Requirements 3.2

use kpio_test::state::{InstanceConfig, InstanceState, InstanceStatus};
use proptest::prelude::*;
use std::path::PathBuf;

/// Strategy for an arbitrary `InstanceStatus`.
fn arb_status() -> impl Strategy<Value = InstanceStatus> {
    prop_oneof![
        Just(InstanceStatus::Creating),
        Just(InstanceStatus::Running),
        Just(InstanceStatus::Stopped),
        Just(InstanceStatus::Crashed),
        Just(InstanceStatus::TimedOut),
    ]
}

/// Strategy for an arbitrary `InstanceConfig`.
fn arb_config() -> impl Strategy<Value = InstanceConfig> {
    (
        "[a-zA-Z0-9_/\\-\\.]{1,64}",           // image_path
        "(128M|256M|512M|1G|2G|4G)",            // memory
        any::<bool>(),                           // gui
        any::<bool>(),                           // virtio_net
        proptest::option::of("[a-zA-Z0-9_/\\-\\.]{1,32}"), // virtio_blk
        proptest::option::of("[a-zA-Z0-9_/\\-\\.]{1,32}"), // shared_dir
        proptest::collection::vec("[a-zA-Z0-9\\-]{1,16}", 0..4), // extra_args
    )
        .prop_map(|(img, mem, gui, vnet, vblk, sdir, args)| InstanceConfig {
            image_path: PathBuf::from(img),
            memory: mem,
            gui,
            virtio_net: vnet,
            virtio_blk: vblk.map(PathBuf::from),
            shared_dir: sdir.map(PathBuf::from),
            extra_args: args,
        })
}

/// Strategy for an arbitrary `InstanceState`.
///
/// Split into two nested tuples to stay within proptest's 12-element limit.
fn arb_instance_state() -> impl Strategy<Value = InstanceState> {
    let part1 = (
        "[a-zA-Z0-9_\\-]{1,32}",                // name
        1u32..65535,                             // pid
        arb_status(),
        "[a-zA-Z0-9_/\\-\\.]{1,64}",           // qmp_socket
        "[a-zA-Z0-9_/\\-\\.]{1,64}",           // serial_log
        "[a-zA-Z0-9_/\\-\\.]{1,64}",           // qemu_log
        "[a-zA-Z0-9_/\\-\\.]{1,64}",           // screenshot_dir
    );
    let part2 = (
        "[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", // created_at
        "[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", // timeout_deadline
        1u64..7200,                              // timeout_seconds
        proptest::option::of(-128i32..128),      // exit_code
        proptest::option::of("[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z"), // terminated_at
        arb_config(),
    );
    (part1, part2).prop_map(
        |((name, pid, status, qmp, serial, qlog, ssdir), (created, deadline, tsec, ec, term, cfg))| {
            InstanceState {
                name,
                pid,
                status,
                qmp_socket: PathBuf::from(qmp),
                serial_log: PathBuf::from(serial),
                qemu_log: PathBuf::from(qlog),
                screenshot_dir: PathBuf::from(ssdir),
                created_at: created,
                timeout_deadline: deadline,
                timeout_seconds: tsec,
                exit_code: ec,
                terminated_at: term,
                config: cfg,
            }
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Serialize any InstanceState to JSON and deserialize back — the result
    /// must equal the original.
    #[test]
    fn state_json_round_trip(state in arb_instance_state()) {
        let json = serde_json::to_string(&state)
            .expect("InstanceState should serialize to JSON");
        let back: InstanceState = serde_json::from_str(&json)
            .expect("InstanceState JSON should deserialize back");
        prop_assert_eq!(&state, &back);
    }

    /// Pretty-printed JSON also round-trips correctly.
    #[test]
    fn state_pretty_json_round_trip(state in arb_instance_state()) {
        let json = serde_json::to_string_pretty(&state)
            .expect("InstanceState should serialize to pretty JSON");
        let back: InstanceState = serde_json::from_str(&json)
            .expect("Pretty JSON should deserialize back");
        prop_assert_eq!(&state, &back);
    }

    /// InstanceConfig round-trips independently.
    #[test]
    fn config_json_round_trip(config in arb_config()) {
        let json = serde_json::to_string(&config)
            .expect("InstanceConfig should serialize to JSON");
        let back: InstanceConfig = serde_json::from_str(&json)
            .expect("InstanceConfig JSON should deserialize back");
        prop_assert_eq!(&config, &back);
    }

    /// InstanceStatus round-trips for all variants.
    #[test]
    fn status_json_round_trip(status in arb_status()) {
        let json = serde_json::to_string(&status)
            .expect("InstanceStatus should serialize");
        let back: InstanceStatus = serde_json::from_str(&json)
            .expect("InstanceStatus should deserialize");
        prop_assert_eq!(status, back);
    }
}
