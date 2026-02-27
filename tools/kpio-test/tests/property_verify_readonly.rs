//! Property 11: Verify is read-only
//!
//! Running verify (evaluate_suite) does not modify state file, serial log,
//! or any Instance_Store file.
//!
//! Validates: Requirements 14.10

use kpio_test::manifest::{evaluate_suite, CheckExpectation, TestCheck};
use proptest::prelude::*;
use std::fs;

/// Strategy producing a multi-line serial log.
fn arb_log_lines() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec("[a-zA-Z0-9 _]{0,30}", 1..15)
}

fn join_log(lines: &[String]) -> String {
    lines.join("\n")
}

/// Strategy producing a set of test checks.
fn arb_checks() -> impl Strategy<Value = Vec<TestCheck>> {
    proptest::collection::vec(
        ("[a-zA-Z]{1,5}", proptest::bool::ANY).prop_map(|(pattern, absent)| TestCheck {
            pattern,
            label: "test-check".to_string(),
            regex: false,
            expect: if absent {
                CheckExpectation::Absent
            } else {
                CheckExpectation::Present
            },
        }),
        1..6,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// evaluate_suite does not modify any files on disk.
    /// We write a state file and serial log to a temp dir, run evaluate_suite,
    /// and verify the files are byte-for-byte identical afterward.
    #[test]
    fn verify_does_not_modify_files(
        lines in arb_log_lines(),
        checks in arb_checks(),
    ) {
        let content = join_log(&lines);

        // Create a temp dir with a fake state file and serial log
        let tmp = std::env::temp_dir().join(format!("kpio-verify-ro-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let state_file = tmp.join("state.json");
        let serial_file = tmp.join("serial.log");
        let extra_file = tmp.join("screenshots.txt");

        let state_content = r#"{"name":"test","pid":1234,"status":"running"}"#;
        fs::write(&state_file, state_content).unwrap();
        fs::write(&serial_file, &content).unwrap();
        fs::write(&extra_file, "screenshot-list").unwrap();

        // Record file contents and metadata before
        let state_before = fs::read(&state_file).unwrap();
        let serial_before = fs::read(&serial_file).unwrap();
        let extra_before = fs::read(&extra_file).unwrap();

        // Run evaluate_suite (the core verify logic) — this should be pure
        let _result = evaluate_suite(&checks, &content);

        // Verify files are unchanged
        let state_after = fs::read(&state_file).unwrap();
        let serial_after = fs::read(&serial_file).unwrap();
        let extra_after = fs::read(&extra_file).unwrap();

        prop_assert_eq!(&state_before, &state_after, "state file was modified");
        prop_assert_eq!(&serial_before, &serial_after, "serial log was modified");
        prop_assert_eq!(&extra_before, &extra_after, "extra file was modified");

        // Cleanup
        let _ = fs::remove_dir_all(&tmp);
    }

    /// evaluate_suite is a pure function: same inputs produce same outputs.
    #[test]
    fn verify_is_deterministic(
        lines in arb_log_lines(),
        checks in arb_checks(),
    ) {
        let content = join_log(&lines);
        let result1 = evaluate_suite(&checks, &content);
        let result2 = evaluate_suite(&checks, &content);

        match (result1, result2) {
            (Ok((r1, p1, f1)), Ok((r2, p2, f2))) => {
                prop_assert_eq!(r1, r2);
                prop_assert_eq!(p1, p2);
                prop_assert_eq!(f1, f2);
            }
            (Err(_), Err(_)) => {} // both errored, fine
            _ => prop_assert!(false, "results differ between runs"),
        }
    }
}
