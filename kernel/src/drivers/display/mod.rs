//! Display Driver Module
//!
//! Provides display hardware abstraction and driver implementations.

pub mod framebuffer;
pub mod i915;
pub mod vesa;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// Display error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayError {
    /// Display not found
    NotFound,
    /// Invalid mode
    InvalidMode,
    /// Mode not supported
    ModeNotSupported,
    /// Hardware error
    HardwareError,
    /// Out of memory
    OutOfMemory,
    /// Invalid parameters
    InvalidParameters,
    /// Already initialized
    AlreadyInitialized,
    /// Not initialized
    NotInitialized,
}

/// Pixel format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 32-bit ARGB (8 bits per channel)
    Argb8888,
    /// 32-bit XRGB (RGB + padding)
    Xrgb8888,
    /// 32-bit ABGR
    Abgr8888,
    /// 24-bit RGB
    Rgb888,
    /// 24-bit BGR
    Bgr888,
    /// 16-bit RGB (5-6-5)
    Rgb565,
    /// 16-bit BGR (5-6-5)
    Bgr565,
    /// 8-bit indexed
    Indexed8,
}

impl PixelFormat {
    /// Get bits per pixel
    pub fn bits_per_pixel(&self) -> u8 {
        match self {
            PixelFormat::Argb8888 => 32,
            PixelFormat::Xrgb8888 => 32,
            PixelFormat::Abgr8888 => 32,
            PixelFormat::Rgb888 => 24,
            PixelFormat::Bgr888 => 24,
            PixelFormat::Rgb565 => 16,
            PixelFormat::Bgr565 => 16,
            PixelFormat::Indexed8 => 8,
        }
    }

    /// Get bytes per pixel
    pub fn bytes_per_pixel(&self) -> u8 {
        (self.bits_per_pixel() + 7) / 8
    }
}

/// Display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayMode {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Refresh rate in Hz
    pub refresh_rate: u32,
    /// Pixel format
    pub format: PixelFormat,
}

impl DisplayMode {
    /// Common display modes
    pub const MODE_640X480_60: DisplayMode = DisplayMode {
        width: 640,
        height: 480,
        refresh_rate: 60,
        format: PixelFormat::Xrgb8888,
    };

    pub const MODE_800X600_60: DisplayMode = DisplayMode {
        width: 800,
        height: 600,
        refresh_rate: 60,
        format: PixelFormat::Xrgb8888,
    };

    pub const MODE_1024X768_60: DisplayMode = DisplayMode {
        width: 1024,
        height: 768,
        refresh_rate: 60,
        format: PixelFormat::Xrgb8888,
    };

    pub const MODE_1280X720_60: DisplayMode = DisplayMode {
        width: 1280,
        height: 720,
        refresh_rate: 60,
        format: PixelFormat::Xrgb8888,
    };

    pub const MODE_1280X1024_60: DisplayMode = DisplayMode {
        width: 1280,
        height: 1024,
        refresh_rate: 60,
        format: PixelFormat::Xrgb8888,
    };

    pub const MODE_1920X1080_60: DisplayMode = DisplayMode {
        width: 1920,
        height: 1080,
        refresh_rate: 60,
        format: PixelFormat::Xrgb8888,
    };

    pub const MODE_2560X1440_60: DisplayMode = DisplayMode {
        width: 2560,
        height: 1440,
        refresh_rate: 60,
        format: PixelFormat::Xrgb8888,
    };

    pub const MODE_3840X2160_60: DisplayMode = DisplayMode {
        width: 3840,
        height: 2160,
        refresh_rate: 60,
        format: PixelFormat::Xrgb8888,
    };
}

/// Display information
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    /// Display name
    pub name: String,
    /// Manufacturer
    pub manufacturer: String,
    /// Physical width in mm
    pub physical_width_mm: u32,
    /// Physical height in mm
    pub physical_height_mm: u32,
    /// Current mode
    pub current_mode: DisplayMode,
    /// Available modes
    pub available_modes: Vec<DisplayMode>,
    /// Is primary display
    pub is_primary: bool,
    /// Connection type
    pub connection: DisplayConnection,
}

/// Display connection type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayConnection {
    /// Internal/built-in display
    Internal,
    /// VGA connector
    Vga,
    /// DVI connector
    Dvi,
    /// HDMI connector
    Hdmi,
    /// DisplayPort connector
    DisplayPort,
    /// USB-C/Thunderbolt
    UsbC,
    /// Unknown
    Unknown,
}

/// Display trait for all display drivers
pub trait Display: Send + Sync {
    /// Get display info
    fn info(&self) -> DisplayInfo;

    /// Get current mode
    fn current_mode(&self) -> DisplayMode;

    /// Set display mode
    fn set_mode(&mut self, mode: DisplayMode) -> Result<(), DisplayError>;

    /// Get available modes
    fn available_modes(&self) -> Vec<DisplayMode>;

    /// Get framebuffer address
    fn framebuffer_address(&self) -> u64;

    /// Get framebuffer size
    fn framebuffer_size(&self) -> usize;

    /// Get stride (bytes per row)
    fn stride(&self) -> u32;

    /// Enable display
    fn enable(&mut self) -> Result<(), DisplayError>;

    /// Disable display
    fn disable(&mut self) -> Result<(), DisplayError>;

    /// Check if enabled
    fn is_enabled(&self) -> bool;

    /// Wait for vertical blank
    fn wait_vsync(&self);

    /// Set backlight brightness (0-100)
    fn set_brightness(&mut self, brightness: u8) -> Result<(), DisplayError>;

    /// Get backlight brightness
    fn brightness(&self) -> u8;
}

/// Cursor information
#[derive(Debug, Clone)]
pub struct CursorInfo {
    /// Cursor width
    pub width: u32,
    /// Cursor height
    pub height: u32,
    /// Hot spot X
    pub hotspot_x: u32,
    /// Hot spot Y
    pub hotspot_y: u32,
    /// Cursor data (ARGB)
    pub data: Vec<u32>,
}

/// Hardware cursor support
pub trait HardwareCursor {
    /// Check if hardware cursor is supported
    fn hardware_cursor_supported(&self) -> bool;

    /// Set cursor image
    fn set_cursor_image(&mut self, cursor: &CursorInfo) -> Result<(), DisplayError>;

    /// Show cursor
    fn show_cursor(&mut self);

    /// Hide cursor
    fn hide_cursor(&mut self);

    /// Move cursor
    fn move_cursor(&mut self, x: u32, y: u32);
}

/// Display manager
pub struct DisplayManager {
    /// Connected displays
    displays: Vec<Box<dyn Display>>,
    /// Primary display index
    primary_index: Option<usize>,
}

impl DisplayManager {
    /// Create a new display manager
    pub fn new() -> Self {
        Self {
            displays: Vec::new(),
            primary_index: None,
        }
    }

    /// Add a display
    pub fn add_display(&mut self, display: Box<dyn Display>) {
        let is_primary = display.info().is_primary;
        let index = self.displays.len();
        self.displays.push(display);

        if is_primary || self.primary_index.is_none() {
            self.primary_index = Some(index);
        }
    }

    /// Get number of displays
    pub fn display_count(&self) -> usize {
        self.displays.len()
    }

    /// Get display by index
    pub fn display(&self, index: usize) -> Option<&dyn Display> {
        self.displays.get(index).map(|d| d.as_ref())
    }

    /// Get mutable display by index
    pub fn display_mut(&mut self, index: usize) -> Option<&mut dyn Display> {
        match self.displays.get_mut(index) {
            Some(d) => Some(d.as_mut()),
            None => None,
        }
    }

    /// Get primary display
    pub fn primary(&self) -> Option<&dyn Display> {
        self.primary_index.and_then(|i| self.display(i))
    }

    /// Get mutable primary display
    pub fn primary_mut(&mut self) -> Option<&mut dyn Display> {
        match self.primary_index {
            Some(idx) => match self.displays.get_mut(idx) {
                Some(d) => Some(d.as_mut()),
                None => None,
            },
            None => None,
        }
    }

    /// Enumerate all displays
    pub fn enumerate(&self) -> impl Iterator<Item = (usize, &dyn Display)> {
        self.displays
            .iter()
            .enumerate()
            .map(|(i, d)| (i, d.as_ref()))
    }
}

impl Default for DisplayManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Color utilities
pub mod color {
    /// ARGB color
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Color {
        pub a: u8,
        pub r: u8,
        pub g: u8,
        pub b: u8,
    }

    impl Color {
        /// Create a new color
        pub const fn new(r: u8, g: u8, b: u8) -> Self {
            Self { a: 255, r, g, b }
        }

        /// Create a color with alpha
        pub const fn with_alpha(a: u8, r: u8, g: u8, b: u8) -> Self {
            Self { a, r, g, b }
        }

        /// To ARGB u32
        pub const fn to_argb(&self) -> u32 {
            ((self.a as u32) << 24)
                | ((self.r as u32) << 16)
                | ((self.g as u32) << 8)
                | (self.b as u32)
        }

        /// From ARGB u32
        pub const fn from_argb(value: u32) -> Self {
            Self {
                a: (value >> 24) as u8,
                r: (value >> 16) as u8,
                g: (value >> 8) as u8,
                b: value as u8,
            }
        }

        /// Common colors
        pub const BLACK: Color = Color::new(0, 0, 0);
        pub const WHITE: Color = Color::new(255, 255, 255);
        pub const RED: Color = Color::new(255, 0, 0);
        pub const GREEN: Color = Color::new(0, 255, 0);
        pub const BLUE: Color = Color::new(0, 0, 255);
        pub const YELLOW: Color = Color::new(255, 255, 0);
        pub const CYAN: Color = Color::new(0, 255, 255);
        pub const MAGENTA: Color = Color::new(255, 0, 255);
        pub const TRANSPARENT: Color = Color::with_alpha(0, 0, 0, 0);
    }
}
