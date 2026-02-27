//! Property 16: Wait-for immediate return on existing match
//!
//! For any serial log already containing the pattern, `wait-for` returns
//! immediately with the matching line.
//!
//! Validates: Requirements 20.1, 20.3, 20.4

use kpio_test::serial::find_match;
use proptest::prelude::*;

/// Strategy producing a multi-line log where at least one line contains a
/// known marker, plus the marker itself.
fn arb_log_with_marker() -> impl Strategy<Value = (String, String)> {
    (
        proptest::collection::vec("[a-zA-Z0-9 ]{0,30}", 0..10),
        "[a-zA-Z]{2,8}",
        proptest::collection::vec("[a-zA-Z0-9 ]{0,30}", 0..10),
    )
        .prop_map(|(before, marker, after)| {
            let mut lines = before;
            lines.push(format!("prefix {} suffix", marker));
            lines.extend(after);
            (lines.join("\n"), marker)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// When the pattern is already present as a literal substring,
    /// find_match returns Some immediately.
    #[test]
    fn find_match_literal_returns_immediately((content, marker) in arb_log_with_marker()) {
        let result = find_match(&content, &marker, false).unwrap();
        prop_assert!(
            result.is_some(),
            "find_match should find '{}' in content",
            marker
        );
        let line = result.unwrap();
        prop_assert!(
            line.contains(&marker),
            "matched line '{}' should contain '{}'",
            line, marker
        );
    }

    /// When the pattern is already present as a regex,
    /// find_match returns Some immediately.
    #[test]
    fn find_match_regex_returns_immediately((content, marker) in arb_log_with_marker()) {
        // Use the marker as a literal regex (safe chars only)
        let result = find_match(&content, &marker, true).unwrap();
        prop_assert!(
            result.is_some(),
            "find_match (regex) should find '{}' in content",
            marker
        );
    }

    /// When the pattern is NOT present, find_match returns None.
    #[test]
    fn find_match_absent_returns_none(
        lines in proptest::collection::vec("[a-z]{1,10}", 1..10),
    ) {
        let content = lines.join("\n");
        // Use a pattern guaranteed not to appear (digits in an all-alpha log)
        let result = find_match(&content, "99999", false).unwrap();
        prop_assert!(result.is_none());
    }

    /// find_match returns the FIRST matching line.
    #[test]
    fn find_match_returns_first_occurrence(marker in "[a-zA-Z]{2,6}") {
        let content = format!("first {} line\nsecond {} line\nthird line", marker, marker);
        let result = find_match(&content, &marker, false).unwrap();
        prop_assert!(result.is_some());
        let line = result.unwrap();
        prop_assert!(
            line.starts_with("first"),
            "should return first match, got: '{}'",
            line
        );
    }
}
