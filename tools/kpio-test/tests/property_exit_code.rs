//! Property 2: Exit code contract
//!
//! For any `KpioTestError` variant, `exit_code()` returns 1 or 2.
//! Infrastructure errors map to 2, operational errors map to 1.
//!
//! Validates: Requirements 1.4, 1.7, 5.4, 7.4, 12.4, 14.9, 15.7, 17.3,
//!            22.5, 22.8, 25.4, 27.6, 29.7

use kpio_test::error::KpioTestError;
use proptest::prelude::*;
use std::path::PathBuf;
use std::process::ExitCode;

/// Strategy that produces an arbitrary `KpioTestError` variant.
///
/// We tag each variant with an index (0..=20) and generate random payloads.
fn arb_kpio_test_error() -> impl Strategy<Value = KpioTestError> {
    // Reusable leaf strategies
    let arb_string = "[a-zA-Z0-9_ /\\-\\.]{0,64}";
    let arb_path = arb_string.prop_map(PathBuf::from);

    (0..=20u8, arb_string, arb_path, 1..3600u64, 0..1000usize).prop_map(
        |(tag, s, p, secs, count)| match tag {
            // Infrastructure errors (exit code 2)
            0 => KpioTestError::QemuNotFound {
                hint: s.to_string(),
            },
            1 => KpioTestError::OvmfNotFound {
                hint: s.to_string(),
            },
            2 => KpioTestError::ImageNotFound { path: p },
            3 => KpioTestError::ManifestParseError(s.to_string()),
            4 => KpioTestError::OcrNotAvailable {
                hint: s.to_string(),
            },
            5 => KpioTestError::SnapshotRequiresQcow2,
            6 => KpioTestError::SharedDirRequired,
            7 => KpioTestError::NetworkDeviceRequired,
            8 => KpioTestError::UnknownSubcommand {
                name: s.to_string(),
            },
            // Io and Json are infrastructure but hard to generate arbitrarily;
            // we test them explicitly below.

            // Operational errors (exit code 1)
            9 => KpioTestError::InstanceNotFound {
                name: s.to_string(),
            },
            10 => KpioTestError::InstanceNotRunning {
                name: s.to_string(),
            },
            11 => KpioTestError::NameConflict {
                name: s.to_string(),
            },
            12 => KpioTestError::QmpError {
                desc: s.to_string(),
            },
            13 => KpioTestError::QmpTimeout { seconds: secs },
            14 => KpioTestError::WaitForTimeout { seconds: secs },
            15 => KpioTestError::VerificationFailed { fail_count: count },
            16 => KpioTestError::BuildFailed {
                message: s.to_string(),
            },
            17 => KpioTestError::SnapshotNotFound {
                tag: s.to_string(),
            },
            18 => KpioTestError::FileNotFound { path: p },
            // Wrap around to cover more infrastructure variants
            19 => KpioTestError::QemuNotFound {
                hint: s.to_string(),
            },
            _ => KpioTestError::OvmfNotFound {
                hint: s.to_string(),
            },
        },
    )
}

/// Returns true if the variant is classified as an infrastructure error.
fn is_infrastructure(err: &KpioTestError) -> bool {
    matches!(
        err,
        KpioTestError::QemuNotFound { .. }
            | KpioTestError::OvmfNotFound { .. }
            | KpioTestError::ImageNotFound { .. }
            | KpioTestError::ManifestParseError(_)
            | KpioTestError::OcrNotAvailable { .. }
            | KpioTestError::SnapshotRequiresQcow2
            | KpioTestError::SharedDirRequired
            | KpioTestError::NetworkDeviceRequired
            | KpioTestError::UnknownSubcommand { .. }
            | KpioTestError::Io(_)
            | KpioTestError::Json(_)
    )
}

/// Returns true if the variant is classified as an operational error.
fn is_operational(err: &KpioTestError) -> bool {
    matches!(
        err,
        KpioTestError::InstanceNotFound { .. }
            | KpioTestError::InstanceNotRunning { .. }
            | KpioTestError::NameConflict { .. }
            | KpioTestError::QmpError { .. }
            | KpioTestError::QmpTimeout { .. }
            | KpioTestError::WaitForTimeout { .. }
            | KpioTestError::VerificationFailed { .. }
            | KpioTestError::BuildFailed { .. }
            | KpioTestError::SnapshotNotFound { .. }
            | KpioTestError::FileNotFound { .. }
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// For any generated KpioTestError, exit_code() is 1 or 2 (never 0).
    #[test]
    fn exit_code_is_1_or_2(err in arb_kpio_test_error()) {
        let code = err.exit_code();
        prop_assert!(
            code == ExitCode::from(1) || code == ExitCode::from(2),
            "exit_code() must be 1 or 2, got {:?} for {:?}", code, err
        );
    }

    /// Infrastructure errors always map to exit code 2.
    #[test]
    fn infrastructure_errors_map_to_2(err in arb_kpio_test_error()) {
        if is_infrastructure(&err) {
            prop_assert_eq!(
                err.exit_code(),
                ExitCode::from(2),
                "Infrastructure error should have exit code 2: {:?}", err
            );
        }
    }

    /// Operational errors always map to exit code 1.
    #[test]
    fn operational_errors_map_to_1(err in arb_kpio_test_error()) {
        if is_operational(&err) {
            prop_assert_eq!(
                err.exit_code(),
                ExitCode::from(1),
                "Operational error should have exit code 1: {:?}", err
            );
        }
    }

    /// Every variant is either infrastructure or operational (exhaustive classification).
    #[test]
    fn every_variant_is_classified(err in arb_kpio_test_error()) {
        prop_assert!(
            is_infrastructure(&err) || is_operational(&err),
            "Variant must be infrastructure or operational: {:?}", err
        );
    }
}

/// Explicit tests for Io and Json variants that are hard to generate via proptest.
#[test]
fn io_error_is_infrastructure_exit_2() {
    let err = KpioTestError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "test",
    ));
    assert!(is_infrastructure(&err));
    assert_eq!(err.exit_code(), ExitCode::from(2));
}

#[test]
fn json_error_is_infrastructure_exit_2() {
    // Produce a real serde_json::Error by parsing invalid JSON
    let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
    let err = KpioTestError::Json(json_err);
    assert!(is_infrastructure(&err));
    assert_eq!(err.exit_code(), ExitCode::from(2));
}
