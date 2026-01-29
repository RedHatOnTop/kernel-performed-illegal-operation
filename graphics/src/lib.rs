//! KPIO Graphics Subsystem
//!
//! This crate provides the graphics stack for the KPIO operating system.
//! It uses Vulkan exclusively for GPU acceleration through wgpu and
//! Vello for 2D rendering and compositing.
//!
//! # Architecture
//!
//! The graphics subsystem is organized into:
//!
//! - `vulkan`: Low-level Vulkan interface through Mesa drivers
//! - `compositor`: Window compositor using Vello
//! - `surface`: Surface management for applications
//! - `buffer`: GPU buffer management
//! - `command`: Command buffer submission
//! - `render`: Rendering pipeline abstraction
//! - `browser`: Browser display list renderer (kpio-layout integration)
//! - `webrender`: WebRender-style GPU compositor with tile caching

#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

pub mod vulkan;
pub mod compositor;
pub mod surface;
pub mod buffer;
pub mod command;
pub mod render;
pub mod browser;
pub mod font;
pub mod animation;
pub mod webrender;

use alloc::string::String;
use alloc::vec::Vec;

/// Graphics error types.
#[derive(Debug, Clone)]
pub enum GraphicsError {
    /// No GPU found.
    NoGpuFound,
    /// GPU initialization failed.
    InitializationFailed(String),
    /// Surface creation failed.
    SurfaceCreationFailed(String),
    /// Buffer allocation failed.
    BufferAllocationFailed(String),
    /// Command submission failed.
    SubmissionFailed(String),
    /// Present failed.
    PresentFailed(String),
    /// Out of GPU memory.
    OutOfMemory,
    /// Device lost.
    DeviceLost,
    /// Invalid operation.
    InvalidOperation(String),
}

/// Display mode configuration.
#[derive(Debug, Clone, Copy)]
pub struct DisplayMode {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Refresh rate in Hz.
    pub refresh_rate: u32,
    /// Pixel format.
    pub format: PixelFormat,
}

/// Pixel formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// RGBA 8-bit per channel.
    Rgba8Unorm,
    /// RGBA 8-bit per channel, sRGB.
    Rgba8Srgb,
    /// BGRA 8-bit per channel.
    Bgra8Unorm,
    /// BGRA 8-bit per channel, sRGB.
    Bgra8Srgb,
    /// RGB 10-bit, Alpha 2-bit.
    Rgb10a2Unorm,
    /// RGBA 16-bit float per channel.
    Rgba16Float,
}

impl PixelFormat {
    /// Get bytes per pixel.
    pub fn bytes_per_pixel(&self) -> u32 {
        match self {
            PixelFormat::Rgba8Unorm | PixelFormat::Rgba8Srgb |
            PixelFormat::Bgra8Unorm | PixelFormat::Bgra8Srgb |
            PixelFormat::Rgb10a2Unorm => 4,
            PixelFormat::Rgba16Float => 8,
        }
    }
}

/// GPU information.
#[derive(Debug, Clone)]
pub struct GpuInfo {
    /// GPU name.
    pub name: String,
    /// Vendor ID.
    pub vendor_id: u32,
    /// Device ID.
    pub device_id: u32,
    /// GPU type.
    pub gpu_type: GpuType,
    /// Total VRAM in bytes.
    pub vram_size: u64,
    /// Vulkan API version.
    pub vulkan_version: (u32, u32, u32),
    /// Driver version.
    pub driver_version: u32,
}

/// GPU types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuType {
    /// Discrete GPU.
    Discrete,
    /// Integrated GPU.
    Integrated,
    /// Virtual GPU.
    Virtual,
    /// CPU (software rendering).
    Cpu,
    /// Unknown.
    Unknown,
}

/// Initialize the graphics subsystem.
pub fn init() -> Result<(), GraphicsError> {
    vulkan::init()?;
    compositor::init()?;
    Ok(())
}

/// Get information about available GPUs.
pub fn enumerate_gpus() -> Result<Vec<GpuInfo>, GraphicsError> {
    vulkan::enumerate_gpus()
}

/// Get the primary display mode.
pub fn get_display_mode() -> Option<DisplayMode> {
    compositor::get_display_mode()
}

/// Set the display mode.
pub fn set_display_mode(mode: DisplayMode) -> Result<(), GraphicsError> {
    compositor::set_display_mode(mode)
}
