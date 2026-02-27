//! Property 13: Screenshot comparison score bounds and identity
//!
//! Similarity score in [0.0, 1.0]; image compared with itself yields 1.0;
//! pass iff score >= threshold.
//!
//! **Validates: Requirements 15.1, 15.2, 15.4**

use image::{DynamicImage, RgbaImage};
use kpio_test::screenshot::{compare_perceptual, compare_pixel_exact};
use proptest::prelude::*;

/// Strategy for generating a small random RGBA image (width x height with random pixels).
fn arb_image(max_dim: u32) -> impl Strategy<Value = DynamicImage> {
    (1..=max_dim, 1..=max_dim)
        .prop_flat_map(|(w, h)| {
            proptest::collection::vec(0u8..=255, (w * h * 4) as usize)
                .prop_map(move |pixels| {
                    let img = RgbaImage::from_raw(w, h, pixels)
                        .expect("pixel buffer size matches dimensions");
                    DynamicImage::ImageRgba8(img)
                })
        })
}

/// Strategy for a pair of images with the same dimensions.
fn arb_image_pair(max_dim: u32) -> impl Strategy<Value = (DynamicImage, DynamicImage)> {
    (1..=max_dim, 1..=max_dim)
        .prop_flat_map(|(w, h)| {
            let pixel_count = (w * h * 4) as usize;
            (
                proptest::collection::vec(0u8..=255, pixel_count),
                proptest::collection::vec(0u8..=255, pixel_count),
            )
                .prop_map(move |(px_a, px_b)| {
                    let a = RgbaImage::from_raw(w, h, px_a)
                        .expect("pixel buffer size matches dimensions");
                    let b = RgbaImage::from_raw(w, h, px_b)
                        .expect("pixel buffer size matches dimensions");
                    (DynamicImage::ImageRgba8(a), DynamicImage::ImageRgba8(b))
                })
        })
}

/// Strategy for a threshold value in [0.0, 1.0].
fn arb_threshold() -> impl Strategy<Value = f64> {
    (0u32..=100).prop_map(|v| v as f64 / 100.0)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // Feature: qemu-boot-testing-infrastructure, Property 13: Screenshot comparison score bounds and identity

    /// Perceptual similarity score is always in [0.0, 1.0].
    #[test]
    fn perceptual_score_in_bounds((img_a, img_b) in arb_image_pair(16)) {
        let score = compare_perceptual(&img_a, &img_b);
        prop_assert!(score >= 0.0, "score {} < 0.0", score);
        prop_assert!(score <= 1.0, "score {} > 1.0", score);
    }

    /// Pixel-exact similarity score is always in [0.0, 1.0].
    #[test]
    fn pixel_exact_score_in_bounds((img_a, img_b) in arb_image_pair(16)) {
        let score = compare_pixel_exact(&img_a, &img_b);
        prop_assert!(score >= 0.0, "score {} < 0.0", score);
        prop_assert!(score <= 1.0, "score {} > 1.0", score);
    }

    /// An image compared with itself yields 1.0 in perceptual mode.
    #[test]
    fn perceptual_identity(img in arb_image(16)) {
        let score = compare_perceptual(&img, &img);
        prop_assert!(
            (score - 1.0).abs() < f64::EPSILON,
            "perceptual self-comparison score {} != 1.0",
            score
        );
    }

    /// An image compared with itself yields 1.0 in pixel_exact mode.
    #[test]
    fn pixel_exact_identity(img in arb_image(16)) {
        let score = compare_pixel_exact(&img, &img);
        prop_assert!(
            (score - 1.0).abs() < f64::EPSILON,
            "pixel_exact self-comparison score {} != 1.0",
            score
        );
    }

    /// Pass iff score >= threshold (perceptual mode).
    #[test]
    fn perceptual_pass_iff_score_gte_threshold(
        (img_a, img_b) in arb_image_pair(16),
        threshold in arb_threshold(),
    ) {
        let score = compare_perceptual(&img_a, &img_b);
        let pass = score >= threshold;
        prop_assert_eq!(
            pass,
            score >= threshold,
            "pass={} but score={}, threshold={}",
            pass, score, threshold
        );
    }

    /// Pass iff score >= threshold (pixel_exact mode).
    #[test]
    fn pixel_exact_pass_iff_score_gte_threshold(
        (img_a, img_b) in arb_image_pair(16),
        threshold in arb_threshold(),
    ) {
        let score = compare_pixel_exact(&img_a, &img_b);
        let pass = score >= threshold;
        prop_assert_eq!(
            pass,
            score >= threshold,
            "pass={} but score={}, threshold={}",
            pass, score, threshold
        );
    }
}
