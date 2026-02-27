//! Property 8: Instance listing completeness
//!
//! For any set of Instance_Store directories, `list_instances()` returns
//! exactly that set of names.
//!
//! Validates: Requirements 5.1

use kpio_test::store;
use proptest::prelude::*;
use std::collections::BTreeSet;
use std::fs;

/// Strategy that generates a set of 0–5 unique valid instance names.
///
/// Names are prefixed with `prop8-` to avoid collisions with other tests.
fn arb_instance_names() -> impl Strategy<Value = BTreeSet<String>> {
    proptest::collection::btree_set("[a-z][a-z0-9\\-]{0,11}", 0..=5)
        .prop_map(|set| set.into_iter().map(|n| format!("prop8-{n}")).collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Create a random set of instance directories, then verify
    /// `list_instances()` returns at least those names.
    #[test]
    fn listing_contains_all_created_instances(names in arb_instance_names()) {
        // Setup: ensure base dir exists and create instance dirs
        let base = store::base_dir();
        let _ = fs::create_dir_all(&base);

        // Clean up any leftover prop8- dirs from previous runs
        if let Ok(entries) = fs::read_dir(&base) {
            for entry in entries.flatten() {
                if let Some(n) = entry.file_name().to_str() {
                    if n.starts_with("prop8-") {
                        let _ = fs::remove_dir_all(entry.path());
                    }
                }
            }
        }

        // Create the instance directories
        for name in &names {
            store::create_store(name).expect("create_store should succeed");
        }

        // List and verify
        let listed: BTreeSet<String> = store::list_instances()
            .expect("list_instances should succeed")
            .into_iter()
            .filter(|n| n.starts_with("prop8-"))
            .collect();

        prop_assert_eq!(
            &names, &listed,
            "Listed instances should match created instances"
        );

        // Cleanup
        for name in &names {
            let _ = store::delete_store(name);
        }
    }
}
