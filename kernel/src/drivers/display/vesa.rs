//! VESA/VBE Display Driver
//!
//! Driver for VESA BIOS Extensions framebuffer.

use super::{Display, DisplayConnection, DisplayError, DisplayInfo, DisplayMode, PixelFormat};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// VBE info block (returned by VBE function 00h)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct VbeInfoBlock {
    /// VBE signature ("VESA")
    pub signature: [u8; 4],
    /// VBE version
    pub version: u16,
    /// OEM string pointer
    pub oem_string_ptr: u32,
    /// Capabilities
    pub capabilities: u32,
    /// Video mode list pointer
    pub video_mode_ptr: u32,
    /// Total memory (64KB blocks)
    pub total_memory: u16,
    /// OEM software revision
    pub oem_software_rev: u16,
    /// OEM vendor name
    pub oem_vendor_name_ptr: u32,
    /// OEM product name
    pub oem_product_name_ptr: u32,
    /// OEM product revision
    pub oem_product_rev_ptr: u32,
    /// Reserved
    pub reserved: [u8; 222],
    /// OEM data
    pub oem_data: [u8; 256],
}

/// VBE mode info block (returned by VBE function 01h)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct VbeModeInfoBlock {
    // Mandatory info for all VBE revisions
    /// Mode attributes
    pub mode_attributes: u16,
    /// Window A attributes
    pub win_a_attributes: u8,
    /// Window B attributes
    pub win_b_attributes: u8,
    /// Window granularity (KB)
    pub win_granularity: u16,
    /// Window size (KB)
    pub win_size: u16,
    /// Window A segment
    pub win_a_segment: u16,
    /// Window B segment
    pub win_b_segment: u16,
    /// Window function pointer
    pub win_func_ptr: u32,
    /// Bytes per scanline
    pub bytes_per_scan_line: u16,

    // Mandatory info for VBE 1.2+
    /// Horizontal resolution
    pub x_resolution: u16,
    /// Vertical resolution
    pub y_resolution: u16,
    /// Character cell width
    pub x_char_size: u8,
    /// Character cell height
    pub y_char_size: u8,
    /// Number of memory planes
    pub number_of_planes: u8,
    /// Bits per pixel
    pub bits_per_pixel: u8,
    /// Number of banks
    pub number_of_banks: u8,
    /// Memory model type
    pub memory_model: u8,
    /// Bank size (KB)
    pub bank_size: u8,
    /// Number of image pages
    pub number_of_image_pages: u8,
    /// Reserved
    pub reserved1: u8,

    // Direct color fields
    /// Red mask size
    pub red_mask_size: u8,
    /// Red field position
    pub red_field_position: u8,
    /// Green mask size
    pub green_mask_size: u8,
    /// Green field position
    pub green_field_position: u8,
    /// Blue mask size
    pub blue_mask_size: u8,
    /// Blue field position
    pub blue_field_position: u8,
    /// Reserved mask size
    pub reserved_mask_size: u8,
    /// Reserved field position
    pub reserved_field_position: u8,
    /// Direct color mode info
    pub direct_color_mode_info: u8,

    // Mandatory info for VBE 2.0+
    /// Physical base address of linear framebuffer
    pub phys_base_ptr: u32,
    /// Reserved
    pub reserved2: u32,
    /// Reserved
    pub reserved3: u16,

    // Mandatory info for VBE 3.0+
    /// Bytes per scanline (linear modes)
    pub lin_bytes_per_scan_line: u16,
    /// Number of image pages (banked)
    pub bnk_number_of_image_pages: u8,
    /// Number of image pages (linear)
    pub lin_number_of_image_pages: u8,
    /// Red mask size (linear)
    pub lin_red_mask_size: u8,
    /// Red field position (linear)
    pub lin_red_field_position: u8,
    /// Green mask size (linear)
    pub lin_green_mask_size: u8,
    /// Green field position (linear)
    pub lin_green_field_position: u8,
    /// Blue mask size (linear)
    pub lin_blue_mask_size: u8,
    /// Blue field position (linear)
    pub lin_blue_field_position: u8,
    /// Reserved mask size (linear)
    pub lin_reserved_mask_size: u8,
    /// Reserved field position (linear)
    pub lin_reserved_field_position: u8,
    /// Max pixel clock (Hz)
    pub max_pixel_clock: u32,

    /// Reserved
    pub reserved4: [u8; 189],
}

impl VbeModeInfoBlock {
    /// Check if mode is supported
    pub fn is_supported(&self) -> bool {
        (self.mode_attributes & 0x01) != 0
    }

    /// Check if linear framebuffer is available
    pub fn has_linear_framebuffer(&self) -> bool {
        (self.mode_attributes & 0x80) != 0
    }

    /// Get pixel format
    pub fn pixel_format(&self) -> PixelFormat {
        match self.bits_per_pixel {
            32 => {
                if self.red_field_position == 16 {
                    PixelFormat::Argb8888
                } else {
                    PixelFormat::Abgr8888
                }
            }
            24 => {
                if self.red_field_position == 16 {
                    PixelFormat::Rgb888
                } else {
                    PixelFormat::Bgr888
                }
            }
            16 => {
                if self.red_field_position == 11 {
                    PixelFormat::Rgb565
                } else {
                    PixelFormat::Bgr565
                }
            }
            8 => PixelFormat::Indexed8,
            _ => PixelFormat::Argb8888,
        }
    }
}

/// VESA display driver
pub struct VesaDisplay {
    /// Mode info
    mode_info: VbeModeInfoBlock,
    /// Current mode number
    mode_number: u16,
    /// Available modes
    available_modes: Vec<(u16, DisplayMode)>,
    /// Is enabled
    enabled: bool,
    /// Brightness
    brightness: u8,
}

impl VesaDisplay {
    /// Create from bootloader-provided mode info
    pub fn from_mode_info(mode_info: VbeModeInfoBlock, mode_number: u16) -> Self {
        let current_mode = DisplayMode {
            width: mode_info.x_resolution as u32,
            height: mode_info.y_resolution as u32,
            refresh_rate: 60, // VBE doesn't provide refresh rate
            format: mode_info.pixel_format(),
        };

        Self {
            mode_info,
            mode_number,
            available_modes: vec![(mode_number, current_mode)],
            enabled: true,
            brightness: 100,
        }
    }

    /// Get framebuffer info
    pub fn framebuffer_info(&self) -> (u64, u32, u32, u32) {
        (
            self.mode_info.phys_base_ptr as u64,
            self.mode_info.x_resolution as u32,
            self.mode_info.y_resolution as u32,
            self.mode_info.bytes_per_scan_line as u32,
        )
    }
}

impl Display for VesaDisplay {
    fn info(&self) -> DisplayInfo {
        DisplayInfo {
            name: String::from("VESA VBE Display"),
            manufacturer: String::from("Standard VESA"),
            physical_width_mm: 0,
            physical_height_mm: 0,
            current_mode: DisplayMode {
                width: self.mode_info.x_resolution as u32,
                height: self.mode_info.y_resolution as u32,
                refresh_rate: 60,
                format: self.mode_info.pixel_format(),
            },
            available_modes: self.available_modes.iter().map(|(_, m)| *m).collect(),
            is_primary: true,
            connection: DisplayConnection::Internal,
        }
    }

    fn current_mode(&self) -> DisplayMode {
        DisplayMode {
            width: self.mode_info.x_resolution as u32,
            height: self.mode_info.y_resolution as u32,
            refresh_rate: 60,
            format: self.mode_info.pixel_format(),
        }
    }

    fn set_mode(&mut self, mode: DisplayMode) -> Result<(), DisplayError> {
        // Find matching mode
        for (num, m) in &self.available_modes {
            if m.width == mode.width && m.height == mode.height {
                self.mode_number = *num;
                // Would call VBE function 02h here
                return Ok(());
            }
        }
        Err(DisplayError::ModeNotSupported)
    }

    fn available_modes(&self) -> Vec<DisplayMode> {
        self.available_modes.iter().map(|(_, m)| *m).collect()
    }

    fn framebuffer_address(&self) -> u64 {
        self.mode_info.phys_base_ptr as u64
    }

    fn framebuffer_size(&self) -> usize {
        (self.mode_info.bytes_per_scan_line as usize) * (self.mode_info.y_resolution as usize)
    }

    fn stride(&self) -> u32 {
        self.mode_info.bytes_per_scan_line as u32
    }

    fn enable(&mut self) -> Result<(), DisplayError> {
        self.enabled = true;
        Ok(())
    }

    fn disable(&mut self) -> Result<(), DisplayError> {
        self.enabled = false;
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn wait_vsync(&self) {
        // Wait for vertical retrace using VGA port
        unsafe {
            // Wait for not in vertical retrace
            while (inb(0x3DA) & 0x08) != 0 {}
            // Wait for vertical retrace
            while (inb(0x3DA) & 0x08) == 0 {}
        }
    }

    fn set_brightness(&mut self, brightness: u8) -> Result<(), DisplayError> {
        self.brightness = brightness.min(100);
        // VBE doesn't support backlight control
        Ok(())
    }

    fn brightness(&self) -> u8 {
        self.brightness
    }
}

/// Read from I/O port
#[inline]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        core::arch::asm!(
            "in al, dx",
            out("al") value,
            in("dx") port,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

/// EDID (Extended Display Identification Data) parser
#[derive(Debug, Clone)]
pub struct Edid {
    /// Manufacturer ID
    pub manufacturer: [char; 3],
    /// Product code
    pub product_code: u16,
    /// Serial number
    pub serial_number: u32,
    /// Week of manufacture
    pub week: u8,
    /// Year of manufacture
    pub year: u16,
    /// EDID version
    pub version: u8,
    /// EDID revision
    pub revision: u8,
    /// Horizontal screen size (cm)
    pub screen_width_cm: u8,
    /// Vertical screen size (cm)
    pub screen_height_cm: u8,
    /// Preferred timing mode
    pub preferred_mode: Option<DisplayMode>,
    /// Display name
    pub name: String,
}

impl Edid {
    /// Parse EDID data
    pub fn parse(data: &[u8; 128]) -> Option<Self> {
        // Check header
        if data[0..8] != [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00] {
            return None;
        }

        // Manufacturer ID (3 characters packed in 2 bytes)
        let mfg_id = u16::from_be_bytes([data[8], data[9]]);
        let manufacturer = [
            ((mfg_id >> 10) & 0x1F) as u8 + b'A' - 1,
            ((mfg_id >> 5) & 0x1F) as u8 + b'A' - 1,
            (mfg_id & 0x1F) as u8 + b'A' - 1,
        ];
        let manufacturer = [
            manufacturer[0] as char,
            manufacturer[1] as char,
            manufacturer[2] as char,
        ];

        let product_code = u16::from_le_bytes([data[10], data[11]]);
        let serial_number = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let week = data[16];
        let year = 1990 + data[17] as u16;
        let version = data[18];
        let revision = data[19];
        let screen_width_cm = data[21];
        let screen_height_cm = data[22];

        // Parse preferred timing (first detailed timing descriptor)
        let preferred_mode = Self::parse_detailed_timing(&data[54..72]);

        // Parse display name from descriptor blocks
        let mut name = String::new();
        for i in 0..4 {
            let block_start = 54 + i * 18;
            if data[block_start] == 0
                && data[block_start + 1] == 0
                && data[block_start + 2] == 0
                && data[block_start + 3] == 0xFC
            {
                // This is a display name descriptor
                for j in 5..18 {
                    let c = data[block_start + j];
                    if c == 0x0A || c == 0x00 {
                        break;
                    }
                    name.push(c as char);
                }
            }
        }

        Some(Self {
            manufacturer,
            product_code,
            serial_number,
            week,
            year,
            version,
            revision,
            screen_width_cm,
            screen_height_cm,
            preferred_mode,
            name,
        })
    }

    /// Parse detailed timing descriptor
    fn parse_detailed_timing(data: &[u8]) -> Option<DisplayMode> {
        let pixel_clock = u16::from_le_bytes([data[0], data[1]]) as u32 * 10000;
        if pixel_clock == 0 {
            return None;
        }

        let h_active = ((data[4] as u32 & 0xF0) << 4) | data[2] as u32;
        let v_active = ((data[7] as u32 & 0xF0) << 4) | data[5] as u32;
        let h_total = h_active + (((data[4] as u32 & 0x0F) << 8) | data[3] as u32);
        let v_total = v_active + (((data[7] as u32 & 0x0F) << 8) | data[6] as u32);

        let refresh_rate = if h_total > 0 && v_total > 0 {
            pixel_clock / (h_total * v_total)
        } else {
            60
        };

        Some(DisplayMode {
            width: h_active,
            height: v_active,
            refresh_rate,
            format: PixelFormat::Argb8888,
        })
    }
}
