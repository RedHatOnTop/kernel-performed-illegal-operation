//! Property 20: QEMU argument generation from config.
//!
//! For any valid `InstanceConfig`, generated args include correct `-m`,
//! `-display`, `-device isa-debug-exit`, `-serial file:`, `-qmp unix:`,
//! VirtIO flags.
//!
//! Validates: Requirements 3.4, 3.5, 3.6, 3.9, 3.12, 17.1, 17.2

use std::path::PathBuf;

use proptest::prelude::*;

use kpio_test::instance::build_qemu_args;
use kpio_test::state::InstanceConfig;

/// Strategy for generating valid InstanceConfig values.
fn arb_config() -> impl Strategy<Value = InstanceConfig> {
    (
        "[a-z][a-z0-9_-]{0,15}\\.img",       // image_path
        prop_oneof!["512M", "1G", "2G", "256M"], // memory
        any::<bool>(),                         // gui
        any::<bool>(),                         // virtio_net
        any::<bool>(),                         // has virtio_blk
        any::<bool>(),                         // has shared_dir
    )
        .prop_map(
            |(img, mem, gui, vnet, has_blk, has_shared)| InstanceConfig {
                image_path: PathBuf::from(img),
                memory: mem.to_string(),
                gui,
                virtio_net: vnet,
                virtio_blk: if has_blk {
                    Some(PathBuf::from("extra.img"))
                } else {
                    None
                },
                shared_dir: if has_shared {
                    Some(PathBuf::from("/tmp/share"))
                } else {
                    None
                },
                extra_args: vec![],
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Memory flag is always present and matches config.
    #[test]
    fn args_contain_memory(config in arb_config()) {
        let args = build_qemu_args("test", &config);
        let idx = args.iter().position(|a| a == "-m").expect("-m flag missing");
        prop_assert_eq!(&args[idx + 1], &config.memory);
    }

    /// Display flag matches gui setting.
    #[test]
    fn args_contain_display(config in arb_config()) {
        let args = build_qemu_args("test", &config);
        let idx = args.iter().position(|a| a == "-display").expect("-display flag missing");
        if config.gui {
            prop_assert_eq!(&args[idx + 1], "gtk");
        } else {
            prop_assert_eq!(&args[idx + 1], "none");
        }
    }

    /// isa-debug-exit device is always present.
    #[test]
    fn args_contain_isa_debug_exit(config in arb_config()) {
        let args = build_qemu_args("test", &config);
        prop_assert!(
            args.iter().any(|a| a.contains("isa-debug-exit")),
            "isa-debug-exit device missing"
        );
    }

    /// Serial output redirect is always present.
    #[test]
    fn args_contain_serial(config in arb_config()) {
        let args = build_qemu_args("test", &config);
        prop_assert!(
            args.iter().any(|a| a.starts_with("file:") && a.contains("serial.log")),
            "serial file redirect missing"
        );
    }

    /// QMP socket is always present.
    #[test]
    fn args_contain_qmp(config in arb_config()) {
        let args = build_qemu_args("test", &config);
        prop_assert!(
            args.iter().any(|a| a.contains("qmp")),
            "QMP socket missing"
        );
    }

    /// VirtIO net flags present iff virtio_net is true.
    #[test]
    fn args_virtio_net_matches_config(config in arb_config()) {
        let args = build_qemu_args("test", &config);
        let has_net = args.iter().any(|a| a.contains("virtio-net-pci"));
        prop_assert_eq!(has_net, config.virtio_net);
    }

    /// VirtIO blk flags present iff virtio_blk is Some.
    #[test]
    fn args_virtio_blk_matches_config(config in arb_config()) {
        let args = build_qemu_args("test", &config);
        let has_blk = args.iter().any(|a| a.contains("virtio-blk-pci"));
        prop_assert_eq!(has_blk, config.virtio_blk.is_some());
    }

    /// VirtIO-9p shared dir flags present iff shared_dir is Some.
    #[test]
    fn args_shared_dir_matches_config(config in arb_config()) {
        let args = build_qemu_args("test", &config);
        let has_9p = args.iter().any(|a| a.contains("virtio-9p-pci"));
        prop_assert_eq!(has_9p, config.shared_dir.is_some());
    }
}
