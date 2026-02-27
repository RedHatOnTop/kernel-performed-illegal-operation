//! Property 14: Reference image update is a faithful copy
//!
//! After `--update-reference`, reference file is byte-for-byte identical
//! to captured file.
//!
//! **Validates: Requirements 15.8**

use std::fs;

use kpio_test::screenshot::update_reference;
use proptest::prelude::*;

/// Strategy for arbitrary file content (simulating an image file).
fn arb_file_content() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 1..1024)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // Feature: qemu-boot-testing-infrastructure, Property 14: Reference image update is a faithful copy

    /// After update_reference, the reference file is byte-for-byte identical
    /// to the captured file.
    #[test]
    fn reference_is_faithful_copy(content in arb_file_content()) {
        let tmp = tempfile::tempdir().unwrap();
        let captured_path = tmp.path().join("captured.ppm");
        let reference_path = tmp.path().join("refs").join("reference.ppm");

        // Write the captured file
        fs::write(&captured_path, &content).unwrap();

        // Call update_reference
        let result = update_reference(&captured_path, &reference_path);
        prop_assert!(result.is_ok(), "update_reference failed: {:?}", result.err());

        // Verify byte-for-byte identity
        let ref_content = fs::read(&reference_path).unwrap();
        prop_assert_eq!(
            &content,
            &ref_content,
            "reference file content differs from captured file"
        );
    }

    /// update_reference creates parent directories if they don't exist.
    #[test]
    fn reference_creates_parent_dirs(content in arb_file_content()) {
        let tmp = tempfile::tempdir().unwrap();
        let captured_path = tmp.path().join("captured.ppm");
        let reference_path = tmp.path().join("deep").join("nested").join("ref.ppm");

        fs::write(&captured_path, &content).unwrap();

        let result = update_reference(&captured_path, &reference_path);
        prop_assert!(result.is_ok(), "update_reference failed: {:?}", result.err());
        prop_assert!(reference_path.exists(), "reference file was not created");

        let ref_content = fs::read(&reference_path).unwrap();
        prop_assert_eq!(&content, &ref_content);
    }

    /// update_reference with a nonexistent captured file returns an error.
    #[test]
    fn nonexistent_captured_returns_error(suffix in "[a-z]{3,8}") {
        let tmp = tempfile::tempdir().unwrap();
        let captured_path = tmp.path().join(format!("nonexistent-{}.ppm", suffix));
        let reference_path = tmp.path().join("ref.ppm");

        let result = update_reference(&captured_path, &reference_path);
        prop_assert!(result.is_err(), "expected error for nonexistent captured file");
    }
}
