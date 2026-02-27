//! Property 12: Test manifest TOML round-trip
//!
//! For any valid `TestManifest`, serialize to TOML and deserialize back
//! produces equivalent struct; invalid TOML produces parse error.
//!
//! Validates: Requirements 16.1, 16.2, 16.3, 16.4, 14.9

use kpio_test::manifest::{
    parse_manifest_str, CheckExpectation, ManifestMetadata, TestCheck, TestManifest, TestSuite,
};
use proptest::prelude::*;

/// Strategy for a valid CheckExpectation.
fn arb_expectation() -> impl Strategy<Value = CheckExpectation> {
    prop_oneof![
        Just(CheckExpectation::Present),
        Just(CheckExpectation::Absent),
    ]
}

/// Strategy for a valid TestCheck (literal only — regex patterns are harder
/// to round-trip safely through arbitrary generation).
fn arb_check() -> impl Strategy<Value = TestCheck> {
    (
        "[a-zA-Z0-9 ]{1,20}",  // pattern
        "[a-zA-Z ]{1,15}",     // label
        arb_expectation(),
    )
        .prop_map(|(pattern, label, expect)| TestCheck {
            pattern,
            label,
            regex: false,
            expect,
        })
}

/// Strategy for a valid TestSuite.
fn arb_suite() -> impl Strategy<Value = TestSuite> {
    (
        proptest::option::of(30u64..300u64),
        proptest::option::of(prop_oneof![Just("512M".to_string()), Just("1G".to_string())]),
        proptest::option::of(proptest::bool::ANY),
        proptest::collection::vec(arb_check(), 1..5),
    )
        .prop_map(|(timeout, memory, virtio_net, checks)| TestSuite {
            timeout,
            memory,
            virtio_net,
            virtio_blk: None,
            extra_args: None,
            checks,
        })
}

/// Strategy for a valid TestManifest with 1-3 suites.
fn arb_manifest() -> impl Strategy<Value = TestManifest> {
    (
        "[a-zA-Z ]{1,20}",  // metadata name
        "[0-9]\\.[0-9]",    // metadata version
        proptest::collection::hash_map("[a-z]{2,8}", arb_suite(), 1..4),
    )
        .prop_map(|(name, version, suites)| TestManifest {
            metadata: ManifestMetadata { name, version },
            suites,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Serialize to TOML and deserialize back produces an equivalent manifest.
    #[test]
    fn toml_round_trip(manifest in arb_manifest()) {
        let toml_str = toml::to_string(&manifest).unwrap();
        let back: TestManifest = toml::from_str(&toml_str).unwrap();
        prop_assert_eq!(&manifest, &back);
    }

    /// parse_manifest_str on valid TOML succeeds and matches the original.
    #[test]
    fn parse_manifest_str_round_trip(manifest in arb_manifest()) {
        let toml_str = toml::to_string(&manifest).unwrap();
        let back = parse_manifest_str(&toml_str).unwrap();
        prop_assert_eq!(&manifest, &back);
    }

    /// Invalid TOML always produces a ManifestParseError.
    #[test]
    fn invalid_toml_produces_error(garbage in "[^\\[\\]=\"]{5,30}") {
        // Prepend something that's definitely not valid manifest TOML
        let bad = format!("{{{{ {garbage} }}}}");
        let result = parse_manifest_str(&bad);
        prop_assert!(result.is_err(), "expected error for invalid TOML");
    }

    /// A manifest with suites containing checks preserves check order.
    #[test]
    fn check_order_preserved(manifest in arb_manifest()) {
        let toml_str = toml::to_string(&manifest).unwrap();
        let back = parse_manifest_str(&toml_str).unwrap();
        for (suite_name, suite) in &manifest.suites {
            let back_suite = &back.suites[suite_name];
            prop_assert_eq!(suite.checks.len(), back_suite.checks.len());
            for (orig, rt) in suite.checks.iter().zip(back_suite.checks.iter()) {
                prop_assert_eq!(&orig.pattern, &rt.pattern);
                prop_assert_eq!(&orig.label, &rt.label);
                prop_assert_eq!(orig.expect, rt.expect);
            }
        }
    }
}
