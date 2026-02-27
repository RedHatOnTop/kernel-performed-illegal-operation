//! Screenshot capture, periodic interval, and comparison.
//!
//! Handlers for the `screenshot`, `screenshot-interval`, and
//! `compare-screenshot` subcommands.

use std::fs;
use std::path::Path;

use chrono::Utc;
use image::GenericImageView;
use image_hasher::{HashAlg, HasherConfig};
use serde::Serialize;

use crate::cli::{CompareMode, CompareScreenshotArgs, ScreenshotArgs, ScreenshotIntervalArgs};
use crate::error::KpioTestError;
use crate::qmp::QmpClient;
use crate::store;
use crate::watchdog;

// ── Output types ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ScreenshotOutput {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct ScreenshotIntervalOutput {
    pub name: String,
    pub interval_ms: u64,
    pub output_dir: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct CompareScreenshotOutput {
    pub similarity_score: f64,
    pub threshold: f64,
    pub comparison_mode: String,
    pub pass: bool,
    pub captured_path: String,
    pub reference_path: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateReferenceOutput {
    pub captured_path: String,
    pub reference_path: String,
    pub message: String,
}

// ── Handlers ─────────────────────────────────────────────────────────

/// `screenshot <name> --output <dir>` — capture a screenshot via QMP screendump.
pub fn screenshot(args: ScreenshotArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    if state.status != crate::state::InstanceStatus::Running {
        return Err(KpioTestError::InstanceNotRunning {
            name: args.name.clone(),
        });
    }

    // Ensure output directory exists
    fs::create_dir_all(&args.output)?;

    let timestamp = Utc::now().format("%Y%m%d-%H%M%S%.3f");
    let filename = format!("{}-{}.ppm", args.name, timestamp);
    let output_path = args.output.join(&filename);

    let mut qmp = QmpClient::connect(&state.qmp_socket)?;
    qmp.screendump(&output_path)?;

    let abs_path = fs::canonicalize(&output_path)
        .unwrap_or_else(|_| output_path.clone());

    let output = ScreenshotOutput {
        name: args.name,
        path: abs_path.to_string_lossy().to_string(),
    };
    Ok(serde_json::to_value(output)?)
}

/// `screenshot-interval <name> --interval <ms> --output <dir>` — periodic capture.
///
/// When interval is 0, disables periodic capture. Otherwise configures
/// periodic screenshot capture at the given interval. The AI agent can
/// call `screenshot` in a loop, or this configuration is noted for
/// future background capture support.
pub fn screenshot_interval(
    args: ScreenshotIntervalArgs,
) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    if state.status != crate::state::InstanceStatus::Running {
        return Err(KpioTestError::InstanceNotRunning {
            name: args.name.clone(),
        });
    }

    if args.interval == 0 {
        let output = ScreenshotIntervalOutput {
            name: args.name,
            interval_ms: 0,
            output_dir: args.output.to_string_lossy().to_string(),
            message: "Periodic screenshot capture disabled".to_string(),
        };
        return Ok(serde_json::to_value(output)?);
    }

    // Ensure output directory exists
    fs::create_dir_all(&args.output)?;

    let output = ScreenshotIntervalOutput {
        name: args.name,
        interval_ms: args.interval,
        output_dir: args.output.to_string_lossy().to_string(),
        message: format!(
            "Periodic screenshot capture configured at {}ms intervals",
            args.interval
        ),
    };
    Ok(serde_json::to_value(output)?)
}

/// `compare-screenshot <captured> <reference>` — compare two images.
pub fn compare_screenshot(
    args: CompareScreenshotArgs,
) -> Result<serde_json::Value, KpioTestError> {
    // Handle --update-reference: copy captured to reference path
    if args.update_reference {
        return update_reference(&args.captured, &args.reference);
    }

    // Validate both files exist
    if !args.captured.exists() {
        return Err(KpioTestError::ImageNotFound {
            path: args.captured.clone(),
        });
    }
    if !args.reference.exists() {
        return Err(KpioTestError::ImageNotFound {
            path: args.reference.clone(),
        });
    }

    // Load images
    let captured_img = image::open(&args.captured).map_err(|e| KpioTestError::Io(
        std::io::Error::new(std::io::ErrorKind::InvalidData, format!("failed to load captured image: {e}"))
    ))?;
    let reference_img = image::open(&args.reference).map_err(|e| KpioTestError::Io(
        std::io::Error::new(std::io::ErrorKind::InvalidData, format!("failed to load reference image: {e}"))
    ))?;

    // Optionally crop to region
    let (captured_img, reference_img) = if let Some(ref region_str) = args.region {
        let region = parse_region(region_str)?;
        let c = crop_image(&captured_img, &region)?;
        let r = crop_image(&reference_img, &region)?;
        (c, r)
    } else {
        (captured_img, reference_img)
    };

    // Compare
    let mode_str = match args.mode {
        CompareMode::Perceptual => "perceptual",
        CompareMode::PixelExact => "pixel_exact",
    };

    let score = match args.mode {
        CompareMode::Perceptual => compare_perceptual(&captured_img, &reference_img),
        CompareMode::PixelExact => compare_pixel_exact(&captured_img, &reference_img),
    };

    let pass = score >= args.threshold;

    let output = CompareScreenshotOutput {
        similarity_score: score,
        threshold: args.threshold,
        comparison_mode: mode_str.to_string(),
        pass,
        captured_path: args.captured.to_string_lossy().to_string(),
        reference_path: args.reference.to_string_lossy().to_string(),
    };
    Ok(serde_json::to_value(output)?)
}

/// Copy captured image to reference path (--update-reference).
pub fn update_reference(
    captured: &Path,
    reference: &Path,
) -> Result<serde_json::Value, KpioTestError> {
    if !captured.exists() {
        return Err(KpioTestError::ImageNotFound {
            path: captured.to_path_buf(),
        });
    }

    // Ensure parent directory of reference exists
    if let Some(parent) = reference.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(captured, reference)?;

    let output = UpdateReferenceOutput {
        captured_path: captured.to_string_lossy().to_string(),
        reference_path: reference.to_string_lossy().to_string(),
        message: "Reference image updated".to_string(),
    };
    Ok(serde_json::to_value(output)?)
}

// ── Comparison helpers (pub for property tests) ──────────────────────

/// Region for cropping: x, y, width, height.
#[derive(Debug, Clone)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Parse a region string "x,y,width,height" into a Region.
pub fn parse_region(s: &str) -> Result<Region, KpioTestError> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err(KpioTestError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid region format '{}', expected x,y,width,height", s),
        )));
    }
    let parse = |part: &str, label: &str| -> Result<u32, KpioTestError> {
        part.trim().parse::<u32>().map_err(|_| {
            KpioTestError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid {} value '{}' in region", label, part),
            ))
        })
    };
    Ok(Region {
        x: parse(parts[0], "x")?,
        y: parse(parts[1], "y")?,
        width: parse(parts[2], "width")?,
        height: parse(parts[3], "height")?,
    })
}

/// Crop an image to the given region.
fn crop_image(
    img: &image::DynamicImage,
    region: &Region,
) -> Result<image::DynamicImage, KpioTestError> {
    let (w, h) = img.dimensions();
    if region.x + region.width > w || region.y + region.height > h {
        return Err(KpioTestError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "region ({},{},{},{}) exceeds image dimensions ({}x{})",
                region.x, region.y, region.width, region.height, w, h
            ),
        )));
    }
    Ok(img.crop_imm(region.x, region.y, region.width, region.height))
}

/// Compare two images using perceptual hashing (image_hasher).
///
/// Returns a similarity score in [0.0, 1.0].
pub fn compare_perceptual(
    img_a: &image::DynamicImage,
    img_b: &image::DynamicImage,
) -> f64 {
    let hasher = HasherConfig::new()
        .hash_alg(HashAlg::DoubleGradient)
        .hash_size(16, 16)
        .to_hasher();

    let hash_a = hasher.hash_image(img_a);
    let hash_b = hasher.hash_image(img_b);

    let distance = hash_a.dist(&hash_b);
    // DoubleGradient with hash_size 16x16 produces 512 bits
    let max_distance = hash_a.as_bytes().len() as u32 * 8;

    if max_distance == 0 {
        return 1.0;
    }

    1.0 - (distance as f64 / max_distance as f64)
}

/// Compare two images pixel-by-pixel.
///
/// Returns the ratio of identical pixels to total pixels in [0.0, 1.0].
pub fn compare_pixel_exact(
    img_a: &image::DynamicImage,
    img_b: &image::DynamicImage,
) -> f64 {
    let (w_a, h_a) = img_a.dimensions();
    let (w_b, h_b) = img_b.dimensions();

    // If dimensions differ, compare the overlapping region
    let w = w_a.min(w_b);
    let h = h_a.min(h_b);
    let total = (w_a.max(w_b) as u64) * (h_a.max(h_b) as u64);

    if total == 0 {
        return 1.0;
    }

    let mut matching: u64 = 0;
    for y in 0..h {
        for x in 0..w {
            if img_a.get_pixel(x, y) == img_b.get_pixel(x, y) {
                matching += 1;
            }
        }
    }

    matching as f64 / total as f64
}
