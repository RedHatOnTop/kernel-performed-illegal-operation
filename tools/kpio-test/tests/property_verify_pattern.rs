//! Property 10: Verify pattern matching engine
//!
//! For any serial log and test checks, literal patterns match as substrings,
//! regex patterns match correctly, present/absent expectations evaluated
//! correctly, overall_pass iff all pass.
//!
//! Validates: Requirements 14.1, 14.3, 14.4, 14.6, 14.7, 14.8

use kpio_test::manifest::{
    evaluate_check, evaluate_suite, pattern_matches, CheckExpectation, TestCheck,
};
use proptest::prelude::*;

/// Strategy producing a multi-line serial log.
fn arb_log_lines() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec("[a-zA-Z0-9 _]{0,40}", 1..20)
}

fn join_log(lines: &[String]) -> String {
    lines.join("\n")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Literal pattern_matches returns true iff any line contains the needle.
    #[test]
    fn literal_match_iff_substring_present(
        lines in arb_log_lines(),
        needle in "[a-zA-Z]{1,5}"
    ) {
        let content = join_log(&lines);
        let result = pattern_matches(&content, &needle, false).unwrap();
        let expected = lines.iter().any(|l| l.contains(&needle));
        prop_assert_eq!(result, expected);
    }

    /// Regex pattern_matches returns true iff any line matches the regex.
    #[test]
    fn regex_match_iff_regex_matches(
        lines in arb_log_lines(),
        needle in "[a-zA-Z]{1,4}"
    ) {
        let content = join_log(&lines);
        let pattern = format!(".*{}.*", needle);
        let result = pattern_matches(&content, &pattern, true).unwrap();
        let re = regex::Regex::new(&pattern).unwrap();
        let expected = lines.iter().any(|l| re.is_match(l));
        prop_assert_eq!(result, expected);
    }

    /// Present expectation: check passes iff pattern is found.
    #[test]
    fn present_check_passes_iff_found(
        lines in arb_log_lines(),
        needle in "[a-zA-Z]{1,5}"
    ) {
        let content = join_log(&lines);
        let check = TestCheck {
            pattern: needle.clone(),
            label: "test".to_string(),
            regex: false,
            expect: CheckExpectation::Present,
        };
        let result = evaluate_check(&check, &content).unwrap();
        let found = lines.iter().any(|l| l.contains(&needle));
        if found {
            prop_assert_eq!(result.status.as_str(), "pass");
        } else {
            prop_assert_eq!(result.status.as_str(), "fail");
        }
    }

    /// Absent expectation: check passes iff pattern is NOT found.
    #[test]
    fn absent_check_passes_iff_not_found(
        lines in arb_log_lines(),
        needle in "[a-zA-Z]{1,5}"
    ) {
        let content = join_log(&lines);
        let check = TestCheck {
            pattern: needle.clone(),
            label: "test".to_string(),
            regex: false,
            expect: CheckExpectation::Absent,
        };
        let result = evaluate_check(&check, &content).unwrap();
        let found = lines.iter().any(|l| l.contains(&needle));
        if found {
            prop_assert_eq!(result.status.as_str(), "fail");
        } else {
            prop_assert_eq!(result.status.as_str(), "pass");
        }
    }

    /// overall_pass is true iff all checks pass (fail_count == 0).
    #[test]
    fn overall_pass_iff_all_checks_pass(
        lines in arb_log_lines(),
        needles in proptest::collection::vec("[a-zA-Z]{1,4}", 1..6),
    ) {
        let content = join_log(&lines);
        let checks: Vec<TestCheck> = needles
            .iter()
            .map(|n| TestCheck {
                pattern: n.clone(),
                label: format!("check-{n}"),
                regex: false,
                expect: CheckExpectation::Present,
            })
            .collect();

        let (results, pass_count, fail_count) = evaluate_suite(&checks, &content).unwrap();

        // overall_pass iff fail_count == 0
        let overall_pass = fail_count == 0;
        prop_assert_eq!(
            overall_pass,
            results.iter().all(|r| r.status == "pass")
        );
        prop_assert_eq!(pass_count + fail_count, checks.len());
    }

    /// pass_count + fail_count always equals the number of checks.
    #[test]
    fn pass_fail_counts_sum_to_total(
        lines in arb_log_lines(),
        needles in proptest::collection::vec("[a-zA-Z]{1,4}", 1..8),
        expect_absent in proptest::collection::vec(proptest::bool::ANY, 1..8),
    ) {
        let content = join_log(&lines);
        let len = needles.len().min(expect_absent.len());
        let checks: Vec<TestCheck> = needles[..len]
            .iter()
            .zip(expect_absent[..len].iter())
            .map(|(n, &absent)| TestCheck {
                pattern: n.clone(),
                label: format!("check-{n}"),
                regex: false,
                expect: if absent {
                    CheckExpectation::Absent
                } else {
                    CheckExpectation::Present
                },
            })
            .collect();

        let (_results, pass_count, fail_count) = evaluate_suite(&checks, &content).unwrap();
        prop_assert_eq!(pass_count + fail_count, checks.len());
    }
}
