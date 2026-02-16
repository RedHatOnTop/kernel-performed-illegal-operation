//! Screenshot comparison testing
//!
//! Provides visual regression testing through screenshot capture and comparison.

use alloc::string::String;
use alloc::vec::Vec;

/// A captured screenshot
#[derive(Debug, Clone)]
pub struct Screenshot {
    /// Screenshot name
    pub name: String,
    /// Image width
    pub width: u32,
    /// Image height
    pub height: u32,
    /// Raw RGBA pixel data
    pub data: Vec<u8>,
    /// Capture timestamp
    pub timestamp: u64,
}

impl Screenshot {
    /// Create a new screenshot
    pub fn new(name: &str, width: u32, height: u32) -> Self {
        let data_size = (width * height * 4) as usize;
        Self {
            name: String::from(name),
            width,
            height,
            data: alloc::vec![0u8; data_size],
            timestamp: 0,
        }
    }

    /// Create screenshot from raw data
    pub fn from_data(name: &str, width: u32, height: u32, data: Vec<u8>) -> Self {
        Self {
            name: String::from(name),
            width,
            height,
            data,
            timestamp: 0,
        }
    }

    /// Get pixel at (x, y)
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Rgba> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let offset = ((y * self.width + x) * 4) as usize;
        if offset + 3 >= self.data.len() {
            return None;
        }

        Some(Rgba {
            r: self.data[offset],
            g: self.data[offset + 1],
            b: self.data[offset + 2],
            a: self.data[offset + 3],
        })
    }

    /// Set pixel at (x, y)
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Rgba) {
        if x >= self.width || y >= self.height {
            return;
        }

        let offset = ((y * self.width + x) * 4) as usize;
        if offset + 3 >= self.data.len() {
            return;
        }

        self.data[offset] = color.r;
        self.data[offset + 1] = color.g;
        self.data[offset + 2] = color.b;
        self.data[offset + 3] = color.a;
    }

    /// Save screenshot to baseline
    pub fn save_as_baseline(&self, path: &str) -> Result<(), String> {
        // In real implementation, save to filesystem
        let _ = path;
        Ok(())
    }

    /// Compare with baseline screenshot
    pub fn compare_with_baseline(&self, baseline_path: &str) -> Result<ComparisonResult, String> {
        // Load baseline
        let baseline = load_baseline(baseline_path)?;
        compare_screenshots(self, &baseline)
    }

    /// Create a diff image
    pub fn create_diff(&self, other: &Screenshot) -> Result<Screenshot, String> {
        if self.width != other.width || self.height != other.height {
            return Err(String::from("Screenshot dimensions don't match"));
        }

        let mut diff = Screenshot::new(
            &alloc::format!("{}_diff", self.name),
            self.width,
            self.height,
        );

        for y in 0..self.height {
            for x in 0..self.width {
                let p1 = self.get_pixel(x, y).unwrap_or(Rgba::BLACK);
                let p2 = other.get_pixel(x, y).unwrap_or(Rgba::BLACK);

                if p1 != p2 {
                    // Highlight differences in red
                    diff.set_pixel(x, y, Rgba::RED);
                } else {
                    // Dim matching pixels
                    diff.set_pixel(
                        x,
                        y,
                        Rgba {
                            r: p1.r / 3,
                            g: p1.g / 3,
                            b: p1.b / 3,
                            a: p1.a,
                        },
                    );
                }
            }
        }

        Ok(diff)
    }

    /// Crop screenshot to region
    pub fn crop(&self, x: u32, y: u32, width: u32, height: u32) -> Result<Screenshot, String> {
        if x + width > self.width || y + height > self.height {
            return Err(String::from("Crop region out of bounds"));
        }

        let mut cropped = Screenshot::new(&alloc::format!("{}_cropped", self.name), width, height);

        for cy in 0..height {
            for cx in 0..width {
                if let Some(pixel) = self.get_pixel(x + cx, y + cy) {
                    cropped.set_pixel(cx, cy, pixel);
                }
            }
        }

        Ok(cropped)
    }
}

/// RGBA color
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const BLACK: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const WHITE: Rgba = Rgba {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const RED: Rgba = Rgba {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const GREEN: Rgba = Rgba {
        r: 0,
        g: 255,
        b: 0,
        a: 255,
    };
    pub const BLUE: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    };

    /// Calculate color distance using simple RGB difference
    pub fn distance(&self, other: &Rgba) -> u32 {
        let dr = (self.r as i32 - other.r as i32).abs() as u32;
        let dg = (self.g as i32 - other.g as i32).abs() as u32;
        let db = (self.b as i32 - other.b as i32).abs() as u32;
        dr + dg + db
    }

    /// Check if colors are similar within threshold
    pub fn is_similar(&self, other: &Rgba, threshold: u32) -> bool {
        self.distance(other) <= threshold
    }
}

/// Screenshot comparison result
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    /// Whether screenshots match
    pub matches: bool,
    /// Difference percentage (0.0 - 100.0)
    pub diff_percentage: f32,
    /// Number of different pixels
    pub diff_pixels: u64,
    /// Total pixels compared
    pub total_pixels: u64,
    /// Regions with most differences
    pub hot_spots: Vec<DiffRegion>,
}

impl ComparisonResult {
    /// Check if comparison passed threshold
    pub fn passed(&self, max_diff_percentage: f32) -> bool {
        self.diff_percentage <= max_diff_percentage
    }
}

/// Region with differences
#[derive(Debug, Clone)]
pub struct DiffRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub diff_count: u64,
}

/// Comparison options
#[derive(Debug, Clone)]
pub struct ComparisonOptions {
    /// Color threshold for pixel matching
    pub color_threshold: u32,
    /// Maximum allowed difference percentage
    pub max_diff_percentage: f32,
    /// Ignore anti-aliasing differences
    pub ignore_antialiasing: bool,
    /// Ignore alpha channel
    pub ignore_alpha: bool,
    /// Regions to ignore (x, y, width, height)
    pub ignore_regions: Vec<(u32, u32, u32, u32)>,
}

impl Default for ComparisonOptions {
    fn default() -> Self {
        Self {
            color_threshold: 10,
            max_diff_percentage: 1.0,
            ignore_antialiasing: true,
            ignore_alpha: false,
            ignore_regions: Vec::new(),
        }
    }
}

/// Load a baseline screenshot
pub fn load_baseline(path: &str) -> Result<Screenshot, String> {
    // In real implementation, load from filesystem
    let _ = path;
    Ok(Screenshot::new("baseline", 1920, 1080))
}

/// Compare two screenshots
pub fn compare_screenshots(a: &Screenshot, b: &Screenshot) -> Result<ComparisonResult, String> {
    compare_screenshots_with_options(a, b, &ComparisonOptions::default())
}

/// Compare two screenshots with options
pub fn compare_screenshots_with_options(
    a: &Screenshot,
    b: &Screenshot,
    options: &ComparisonOptions,
) -> Result<ComparisonResult, String> {
    if a.width != b.width || a.height != b.height {
        return Ok(ComparisonResult {
            matches: false,
            diff_percentage: 100.0,
            diff_pixels: (a.width * a.height) as u64,
            total_pixels: (a.width * a.height) as u64,
            hot_spots: Vec::new(),
        });
    }

    let mut diff_pixels: u64 = 0;
    let total_pixels = (a.width * a.height) as u64;

    for y in 0..a.height {
        for x in 0..a.width {
            // Check if in ignore region
            let in_ignore = options
                .ignore_regions
                .iter()
                .any(|(rx, ry, rw, rh)| x >= *rx && x < rx + rw && y >= *ry && y < ry + rh);

            if in_ignore {
                continue;
            }

            let p1 = a.get_pixel(x, y).unwrap_or(Rgba::BLACK);
            let p2 = b.get_pixel(x, y).unwrap_or(Rgba::BLACK);

            if !p1.is_similar(&p2, options.color_threshold) {
                diff_pixels += 1;
            }
        }
    }

    let diff_percentage = if total_pixels > 0 {
        (diff_pixels as f32 / total_pixels as f32) * 100.0
    } else {
        0.0
    };

    Ok(ComparisonResult {
        matches: diff_percentage <= options.max_diff_percentage,
        diff_percentage,
        diff_pixels,
        total_pixels,
        hot_spots: Vec::new(), // Would need more analysis for hot spots
    })
}

/// Screenshot manager for managing baselines
pub struct ScreenshotManager {
    /// Baseline directory
    baseline_dir: String,
    /// Current test run directory
    current_dir: String,
    /// Comparison options
    options: ComparisonOptions,
}

impl ScreenshotManager {
    /// Create a new screenshot manager
    pub fn new(baseline_dir: &str, current_dir: &str) -> Self {
        Self {
            baseline_dir: String::from(baseline_dir),
            current_dir: String::from(current_dir),
            options: ComparisonOptions::default(),
        }
    }

    /// Set comparison options
    pub fn set_options(&mut self, options: ComparisonOptions) {
        self.options = options;
    }

    /// Verify screenshot against baseline
    pub fn verify(&self, screenshot: &Screenshot) -> Result<ComparisonResult, String> {
        let baseline_path = alloc::format!("{}/{}.png", self.baseline_dir, screenshot.name);
        let baseline = load_baseline(&baseline_path)?;
        compare_screenshots_with_options(screenshot, &baseline, &self.options)
    }

    /// Update baseline with new screenshot
    pub fn update_baseline(&self, screenshot: &Screenshot) -> Result<(), String> {
        let baseline_path = alloc::format!("{}/{}.png", self.baseline_dir, screenshot.name);
        screenshot.save_as_baseline(&baseline_path)
    }

    /// Save current screenshot
    pub fn save_current(&self, screenshot: &Screenshot) -> Result<(), String> {
        let current_path = alloc::format!("{}/{}.png", self.current_dir, screenshot.name);
        screenshot.save_as_baseline(&current_path)
    }

    /// Generate diff report
    pub fn generate_diff_report(&self, screenshot: &Screenshot) -> Result<DiffReport, String> {
        let baseline_path = alloc::format!("{}/{}.png", self.baseline_dir, screenshot.name);
        let baseline = load_baseline(&baseline_path)?;
        let comparison = compare_screenshots_with_options(screenshot, &baseline, &self.options)?;

        let diff_image = if !comparison.matches {
            Some(screenshot.create_diff(&baseline)?)
        } else {
            None
        };

        Ok(DiffReport {
            screenshot_name: screenshot.name.clone(),
            comparison,
            diff_image,
        })
    }
}

/// Diff report containing comparison results and diff image
#[derive(Debug)]
pub struct DiffReport {
    /// Screenshot name
    pub screenshot_name: String,
    /// Comparison result
    pub comparison: ComparisonResult,
    /// Diff image (if there are differences)
    pub diff_image: Option<Screenshot>,
}

impl DiffReport {
    /// Check if passed
    pub fn passed(&self) -> bool {
        self.comparison.matches
    }
}
