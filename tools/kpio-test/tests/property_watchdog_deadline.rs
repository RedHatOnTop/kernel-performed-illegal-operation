//! Property 4: Watchdog deadline calculation
//!
//! For any timeout T and creation timestamp C, deadline equals C + T seconds;
//! default T is 120.
//!
//! Validates: Requirements 3.7, 3.8

use chrono::{DateTime, Duration, Utc};
use kpio_test::watchdog;
use proptest::prelude::*;

/// Strategy for a random creation timestamp within a reasonable range.
fn arb_created_at() -> impl Strategy<Value = DateTime<Utc>> {
    // Generate timestamps between 2020 and 2030
    (2020i32..=2030, 1u32..=12, 1u32..=28, 0u32..=23, 0u32..=59, 0u32..=59).prop_map(
        |(year, month, day, hour, min, sec)| {
            chrono::NaiveDate::from_ymd_opt(year, month, day)
                .unwrap()
                .and_hms_opt(hour, min, sec)
                .unwrap()
                .and_utc()
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// For any creation timestamp C and timeout T, compute_deadline returns
    /// C + T seconds.
    #[test]
    fn deadline_equals_created_plus_timeout(
        created in arb_created_at(),
        timeout in 1u64..7200
    ) {
        let created_str = created.to_rfc3339();
        let deadline_str = watchdog::compute_deadline(&created_str, timeout)
            .expect("compute_deadline should succeed");
        let deadline = DateTime::parse_from_rfc3339(&deadline_str)
            .expect("deadline should be valid RFC 3339");

        let expected = created + Duration::seconds(timeout as i64);
        prop_assert_eq!(
            deadline.timestamp(),
            expected.timestamp(),
            "deadline should be created_at + timeout seconds"
        );
    }

    /// Default timeout is 120 seconds.
    #[test]
    fn default_timeout_is_120(created in arb_created_at()) {
        let created_str = created.to_rfc3339();
        let deadline_str = watchdog::compute_deadline(&created_str, watchdog::DEFAULT_TIMEOUT)
            .expect("compute_deadline should succeed");
        let deadline = DateTime::parse_from_rfc3339(&deadline_str)
            .expect("deadline should be valid RFC 3339");

        let expected = created + Duration::seconds(120);
        prop_assert_eq!(
            deadline.timestamp(),
            expected.timestamp(),
            "default deadline should be created_at + 120s"
        );
    }
}
