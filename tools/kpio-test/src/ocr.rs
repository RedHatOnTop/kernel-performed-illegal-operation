//! OCR integration — extract text from screenshots via Tesseract.
//!
//! Uses the `leptess` crate (Tesseract C FFI bindings) when the `ocr`
//! feature is enabled. When disabled, all OCR operations return an
//! `OcrNotAvailable` error with installation instructions.

use std::path::Path;

use serde::Serialize;

use crate::cli::ScreenOcrArgs;
use crate::error::KpioTestError;
use crate::screenshot::{parse_region, Region};
use crate::state::InstanceStatus;
use crate::store;
use crate::watchdog;

// ── Output types ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct OcrOutput {
    pub name: String,
    pub text: String,
    pub region: Option<String>,
}

// ── Handler ──────────────────────────────────────────────────────────

/// `screen-ocr <name> [--region x,y,w,h]` — capture screenshot and run OCR.
pub fn screen_ocr(args: ScreenOcrArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    if state.status != InstanceStatus::Running {
        return Err(KpioTestError::InstanceNotRunning {
            name: args.name.clone(),
        });
    }

    // Parse region if provided
    let region = if let Some(ref region_str) = args.region {
        Some(parse_region(region_str)?)
    } else {
        None
    };

    // Capture a screenshot first
    let screenshot_dir = store::screenshot_dir(&args.name);
    std::fs::create_dir_all(&screenshot_dir)?;
    let screenshot_path = screenshot_dir.join("ocr-capture.ppm");

    let mut qmp = crate::qmp::QmpClient::connect(&state.qmp_socket)?;
    qmp.screendump(&screenshot_path)?;

    // Run OCR on the captured image
    let text = extract_text(&screenshot_path, region.as_ref())?;

    let output = OcrOutput {
        name: args.name,
        text,
        region: args.region,
    };
    Ok(serde_json::to_value(output)?)
}

// ── OCR engine (feature-gated) ───────────────────────────────────────

/// Extract text from an image file using Tesseract OCR.
///
/// Optionally crops to a region before running OCR.
#[cfg(feature = "ocr")]
pub fn extract_text(
    image_path: &Path,
    region: Option<&Region>,
) -> Result<String, KpioTestError> {
    use image::GenericImageView;

    // Load the image
    let img = image::open(image_path).map_err(|e| {
        KpioTestError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to load image for OCR: {e}"),
        ))
    })?;

    // Optionally crop to region
    let img = if let Some(r) = region {
        let (w, h) = img.dimensions();
        if r.x + r.width > w || r.y + r.height > h {
            return Err(KpioTestError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "OCR region ({},{},{},{}) exceeds image dimensions ({}x{})",
                    r.x, r.y, r.width, r.height, w, h
                ),
            )));
        }
        img.crop_imm(r.x, r.y, r.width, r.height)
    } else {
        img
    };

    // Convert to grayscale for better OCR results
    let gray = img.to_luma8();

    // Write grayscale image to a temporary file for Tesseract
    let tmp_dir = tempfile::tempdir().map_err(|e| KpioTestError::Io(e))?;
    let tmp_path = tmp_dir.path().join("ocr-input.png");
    gray.save(&tmp_path).map_err(|e| {
        KpioTestError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("failed to save grayscale image: {e}"),
        ))
    })?;

    // Run Tesseract via leptess
    let mut lt = leptess::LepTess::new(None, "eng").map_err(|e| {
        KpioTestError::OcrNotAvailable {
            hint: format!(
                "Tesseract initialization failed: {}. Install Tesseract: \
                 apt install tesseract-ocr (Linux), brew install tesseract (macOS)",
                e
            ),
        }
    })?;

    lt.set_image(tmp_path.to_str().ok_or_else(|| {
        KpioTestError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "temporary path contains non-UTF8 characters",
        ))
    })?)
    .map_err(|e| {
        KpioTestError::OcrNotAvailable {
            hint: format!("failed to set image for OCR: {}", e),
        }
    })?;

    let text = lt.get_utf8_text().map_err(|e| {
        KpioTestError::OcrNotAvailable {
            hint: format!("OCR text extraction failed: {}", e),
        }
    })?;

    Ok(text.trim().to_string())
}

/// Extract text from an image file — stub when OCR feature is disabled.
#[cfg(not(feature = "ocr"))]
pub fn extract_text(
    _image_path: &Path,
    _region: Option<&Region>,
) -> Result<String, KpioTestError> {
    Err(KpioTestError::OcrNotAvailable {
        hint: "OCR support is not compiled in. Rebuild with `--features ocr` and \
               ensure Tesseract is installed: apt install tesseract-ocr (Linux), \
               brew install tesseract (macOS)"
            .to_string(),
    })
}

/// Check if the OCR engine is available (used by health check).
pub fn is_available() -> bool {
    #[cfg(feature = "ocr")]
    {
        // Try to initialize leptess — if it succeeds, Tesseract is available
        leptess::LepTess::new(None, "eng").is_ok()
    }
    #[cfg(not(feature = "ocr"))]
    {
        false
    }
}
