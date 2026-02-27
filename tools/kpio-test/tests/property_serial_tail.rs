//! Property 9: Serial tail correctness
//!
//! For any serial log with N lines and tail count T, returns exactly
//! `min(T, N)` last lines in order.
//!
//! Validates: Requirements 6.1, 6.2

use kpio_test::serial::tail_lines;
use proptest::prelude::*;

/// Strategy producing a non-empty vec of lines (no embedded newlines).
fn arb_lines() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec("[^\n\r]{0,80}", 1..50)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// tail_lines returns exactly min(T, N) lines.
    #[test]
    fn tail_returns_correct_count(lines in arb_lines(), t in 0usize..100) {
        let result = tail_lines(&lines, t);
        let expected_len = t.min(lines.len());
        prop_assert_eq!(
            result.len(),
            expected_len,
            "tail({}) on {} lines should return {}",
            t, lines.len(), expected_len
        );
    }

    /// tail_lines returns the *last* T lines, in original order.
    #[test]
    fn tail_returns_last_lines_in_order(lines in arb_lines(), t in 1usize..100) {
        let result = tail_lines(&lines, t);
        let n = lines.len();
        let start = n.saturating_sub(t);
        let expected: Vec<String> = lines[start..].to_vec();
        prop_assert_eq!(result, expected);
    }

    /// Requesting 0 lines always returns empty.
    #[test]
    fn tail_zero_returns_empty(lines in arb_lines()) {
        let result = tail_lines(&lines, 0);
        prop_assert!(result.is_empty());
    }

    /// Requesting more lines than exist returns all lines.
    #[test]
    fn tail_exceeding_returns_all(lines in arb_lines()) {
        let big = lines.len() + 100;
        let result = tail_lines(&lines, big);
        prop_assert_eq!(result, lines);
    }
}
