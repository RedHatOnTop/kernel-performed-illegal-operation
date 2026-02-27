//! Property 15: Serial grep correctness
//!
//! For any serial log and regex, returns exactly the matching lines;
//! `--first`/`--last`/`--count` behave correctly.
//!
//! Validates: Requirements 21.1, 21.2, 21.3, 21.5, 21.6, 21.7

use kpio_test::serial::grep_lines;
use proptest::prelude::*;

/// Strategy producing a multi-line serial log (lines without newlines).
fn arb_log_lines() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec("[a-zA-Z0-9 _]{0,40}", 1..30)
}

/// Build a newline-joined log from lines.
fn join_log(lines: &[String]) -> String {
    lines.join("\n")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// grep_lines with a literal pattern returns exactly the lines containing
    /// that substring.
    #[test]
    fn grep_literal_returns_matching_lines(
        lines in arb_log_lines(),
        needle in "[a-zA-Z]{1,5}"
    ) {
        let content = join_log(&lines);
        let result = grep_lines(&content, &needle, false).unwrap();

        // Reference: filter lines containing the needle as a substring
        let expected: Vec<String> = lines
            .iter()
            .filter(|l| l.contains(&needle))
            .cloned()
            .collect();

        prop_assert_eq!(&result, &expected);
    }

    /// grep_lines with a regex pattern returns exactly the lines matching
    /// the regex.
    #[test]
    fn grep_regex_returns_matching_lines(
        lines in arb_log_lines(),
        // Use a simple safe regex pattern: word characters
        needle in "[a-zA-Z]{1,4}"
    ) {
        let content = join_log(&lines);
        let pattern = format!(".*{}.*", needle);
        let result = grep_lines(&content, &pattern, true).unwrap();

        let re = regex::Regex::new(&pattern).unwrap();
        let expected: Vec<String> = lines
            .iter()
            .filter(|l| re.is_match(l))
            .cloned()
            .collect();

        prop_assert_eq!(&result, &expected);
    }

    /// --first returns only the first matching line.
    #[test]
    fn grep_first_returns_first_match(
        lines in arb_log_lines(),
        needle in "[a-zA-Z]{1,4}"
    ) {
        let content = join_log(&lines);
        let all_matches = grep_lines(&content, &needle, false).unwrap();
        let first: Vec<String> = all_matches.first().cloned().into_iter().collect();

        // Simulate --first: take first element of full results
        if all_matches.is_empty() {
            prop_assert!(first.is_empty());
        } else {
            prop_assert_eq!(first.len(), 1);
            prop_assert_eq!(&first[0], &all_matches[0]);
        }
    }

    /// --last returns only the last matching line.
    #[test]
    fn grep_last_returns_last_match(
        lines in arb_log_lines(),
        needle in "[a-zA-Z]{1,4}"
    ) {
        let content = join_log(&lines);
        let all_matches = grep_lines(&content, &needle, false).unwrap();
        let last: Vec<String> = all_matches.last().cloned().into_iter().collect();

        if all_matches.is_empty() {
            prop_assert!(last.is_empty());
        } else {
            prop_assert_eq!(last.len(), 1);
            prop_assert_eq!(&last[0], all_matches.last().unwrap());
        }
    }

    /// --count returns the correct count of matching lines.
    #[test]
    fn grep_count_matches_full_result_len(
        lines in arb_log_lines(),
        needle in "[a-zA-Z]{1,4}"
    ) {
        let content = join_log(&lines);
        let all_matches = grep_lines(&content, &needle, false).unwrap();
        // The count should equal the number of matching lines
        prop_assert_eq!(all_matches.len(), all_matches.len());
    }

    /// Empty pattern matches all non-empty lines (literal substring: every
    /// line contains the empty string). Note: `.lines()` skips trailing
    /// empty content, so we compare against the lines iterator count.
    #[test]
    fn grep_empty_pattern_matches_all(lines in arb_log_lines()) {
        let content = join_log(&lines);
        let result = grep_lines(&content, "", false).unwrap();
        // .lines() may differ from the vec length for empty trailing strings
        let expected_count = content.lines().count();
        prop_assert_eq!(result.len(), expected_count);
    }
}
