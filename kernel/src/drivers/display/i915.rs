//! Intel i915 Graphics Driver
//!
//! Driver for Intel integrated graphics (Gen 3+).

use super::{Display, DisplayInfo, DisplayMode, DisplayError, PixelFormat, DisplayConnection,
            CursorInfo, HardwareCursor};
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

/// Intel GPU generations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntelGeneration {
    /// Gen 3 (GMA 900/950)
    Gen3,
    /// Gen 4 (GMA X3000/X3100)
    Gen4,
    /// Gen 5 (Ironlake)
    Gen5,
    /// Gen 6 (Sandy Bridge)
    Gen6,
    /// Gen 7 (Ivy Bridge/Haswell)
    Gen7,
    /// Gen 8 (Broadwell)
    Gen8,
    /// Gen 9 (Skylake)
    Gen9,
    /// Gen 11 (Ice Lake)
    Gen11,
    /// Gen 12 (Tiger Lake)
    Gen12,
    /// Xe (Alder Lake+)
    Xe,
}

/// MMIO register offsets
mod regs {
    // Graphics Memory Range
    pub const GMADR: u32 = 0x2020;
    pub const GTTADR: u32 = 0x2024;

    // Pipe registers
    pub const PIPEA_CONF: u32 = 0x70008;
    pub const PIPEB_CONF: u32 = 0x71008;
    pub const PIPEA_TIMING: u32 = 0x60000;
    pub const PIPEB_TIMING: u32 = 0x61000;

    // Display registers
    pub const DSPASURF: u32 = 0x7019C;
    pub const DSPBSURF: u32 = 0x7119C;
    pub const DSPACNTR: u32 = 0x70180;
    pub const DSPBCNTR: u32 = 0x71180;
    pub const DSPASIZE: u32 = 0x70190;
    pub const DSPBSIZE: u32 = 0x71190;
    pub const DSPASTRIDE: u32 = 0x70188;
    pub const DSPBSTRIDE: u32 = 0x71188;
    pub const DSPAPOS: u32 = 0x7018C;
    pub const DSPBPOS: u32 = 0x7118C;

    // Cursor registers
    pub const CURACNTR: u32 = 0x70080;
    pub const CURBCNTR: u32 = 0x700C0;
    pub const CURAPOS: u32 = 0x70088;
    pub const CURBPOS: u32 = 0x700C8;
    pub const CURABASE: u32 = 0x70084;
    pub const CURBBASE: u32 = 0x700C4;

    // VGA control
    pub const VGACNTRL: u32 = 0x71400;

    // Interrupt registers
    pub const IIR: u32 = 0x44028;
    pub const IER: u32 = 0x4402C;
    pub const IMR: u32 = 0x44024;

    // Fence registers (tiling)
    pub const FENCE_REG_BASE: u32 = 0x2000;

    // GMCH controls
    pub const GMCH_CTRL: u32 = 0x52;

    // Display timing
    pub const HTOTAL_A: u32 = 0x60000;
    pub const HBLANK_A: u32 = 0x60004;
    pub const HSYNC_A: u32 = 0x60008;
    pub const VTOTAL_A: u32 = 0x6000C;
    pub const VBLANK_A: u32 = 0x60010;
    pub const VSYNC_A: u32 = 0x60014;
    pub const PIPEASRC: u32 = 0x6001C;

    // PLL registers
    pub const DPLL_A: u32 = 0x06014;
    pub const DPLL_B: u32 = 0x06018;
    pub const FPA0: u32 = 0x06040;
    pub const FPA1: u32 = 0x06044;
    pub const FPB0: u32 = 0x06048;
    pub const FPB1: u32 = 0x0604C;

    // Backlight
    pub const BLC_PWM_CTL: u32 = 0x61254;
    pub const BLC_PWM_CTL2: u32 = 0x61250;
}

/// Display plane control bits
mod plane_ctrl {
    pub const ENABLE: u32 = 1 << 31;
    pub const GAMMA_ENABLE: u32 = 1 << 30;
    pub const FORMAT_MASK: u32 = 0xF << 26;
    pub const FORMAT_BGRX8888: u32 = 0x6 << 26;
    pub const FORMAT_RGBX8888: u32 = 0x7 << 26;
    pub const FORMAT_BGRX101010: u32 = 0x8 << 26;
    pub const TILED: u32 = 1 << 10;
}

/// Pipe configuration bits
mod pipe_conf {
    pub const ENABLE: u32 = 1 << 31;
    pub const STATE: u32 = 1 << 30;
    pub const INTERLACE_MASK: u32 = 0x3 << 21;
    pub const PROGRESSIVE: u32 = 0 << 21;
}

/// Intel i915 driver
pub struct I915Driver {
    /// MMIO base address
    mmio_base: u64,
    /// Graphics memory base
    gtt_base: u64,
    /// Framebuffer base
    fb_base: u64,
    /// GPU generation
    generation: IntelGeneration,
    /// Device ID
    device_id: u16,
    /// Current mode
    current_mode: DisplayMode,
    /// Available modes
    available_modes: Vec<DisplayMode>,
    /// Is enabled
    enabled: bool,
    /// Brightness (0-100)
    brightness: u8,
    /// Cursor enabled
    cursor_enabled: bool,
    /// Cursor X position
    cursor_x: u32,
    /// Cursor Y position
    cursor_y: u32,
    /// GTT entries
    gtt_size: usize,
    /// Stolen memory size
    stolen_size: usize,
}

impl I915Driver {
    /// Create a new i915 driver
    pub fn new(mmio_base: u64, gtt_base: u64, device_id: u16) -> Self {
        let generation = Self::detect_generation(device_id);

        Self {
            mmio_base,
            gtt_base,
            fb_base: 0,
            generation,
            device_id,
            current_mode: DisplayMode::MODE_1920X1080_60,
            available_modes: vec![
                DisplayMode::MODE_640X480_60,
                DisplayMode::MODE_800X600_60,
                DisplayMode::MODE_1024X768_60,
                DisplayMode::MODE_1280X720_60,
                DisplayMode::MODE_1280X1024_60,
                DisplayMode::MODE_1920X1080_60,
            ],
            enabled: false,
            brightness: 100,
            cursor_enabled: false,
            cursor_x: 0,
            cursor_y: 0,
            gtt_size: 0,
            stolen_size: 0,
        }
    }

    /// Detect GPU generation from device ID
    fn detect_generation(device_id: u16) -> IntelGeneration {
        match device_id {
            // Gen 3
            0x2582 | 0x258A | 0x2592 | 0x2772 | 0x27A2 | 0x27AE => IntelGeneration::Gen3,
            // Gen 4
            0x2972 | 0x2982 | 0x2992 | 0x29A2 | 0x2A02 | 0x2A12 => IntelGeneration::Gen4,
            // Gen 5 (Ironlake)
            0x0042 | 0x0046 => IntelGeneration::Gen5,
            // Gen 6 (Sandy Bridge)
            0x0102..=0x0126 => IntelGeneration::Gen6,
            // Gen 7 (Ivy Bridge)
            0x0152..=0x016A => IntelGeneration::Gen7,
            // Gen 8 (Broadwell)
            0x1602..=0x163E => IntelGeneration::Gen8,
            // Gen 9 (Skylake)
            0x1902..=0x193E => IntelGeneration::Gen9,
            // Gen 11 (Ice Lake)
            0x8A50..=0x8A5D => IntelGeneration::Gen11,
            // Gen 12 (Tiger Lake)
            0x9A40..=0x9A7F => IntelGeneration::Gen12,
            // Default to Gen 9
            _ => IntelGeneration::Gen9,
        }
    }

    /// Initialize the driver
    pub fn init(&mut self) -> Result<(), DisplayError> {
        // Disable VGA
        self.write_reg(regs::VGACNTRL, 0x80000000);

        // Detect stolen memory and GTT size
        self.detect_memory()?;

        // Setup GTT
        self.setup_gtt()?;

        // Enable display
        self.enable_display()?;

        self.enabled = true;
        Ok(())
    }

    /// Detect stolen memory configuration
    fn detect_memory(&mut self) -> Result<(), DisplayError> {
        // Read GMCH control register to determine stolen memory size
        // This is typically done via PCI config space

        // For now, assume reasonable defaults
        self.stolen_size = 32 * 1024 * 1024; // 32MB
        self.gtt_size = 2 * 1024 * 1024; // 2MB GTT

        Ok(())
    }

    /// Setup Graphics Translation Table
    fn setup_gtt(&mut self) -> Result<(), DisplayError> {
        // GTT maps graphics addresses to physical memory
        // For each page (4KB), we need a GTT entry

        let fb_size = self.current_mode.width * self.current_mode.height * 4;
        let pages = (fb_size as usize + 4095) / 4096;

        // Allocate framebuffer in stolen memory
        self.fb_base = self.gtt_base;

        // Map framebuffer pages in GTT
        for i in 0..pages {
            let gtt_entry = (self.fb_base as u64 + (i * 4096) as u64) | 1; // Present bit
            self.write_gtt_entry(i, gtt_entry);
        }

        Ok(())
    }

    /// Write GTT entry
    fn write_gtt_entry(&mut self, index: usize, value: u64) {
        let gtt_addr = self.mmio_base + 0x10000 + (index * 8) as u64;
        unsafe {
            core::ptr::write_volatile(gtt_addr as *mut u64, value);
        }
    }

    /// Enable display output
    fn enable_display(&mut self) -> Result<(), DisplayError> {
        // Configure display timing
        self.set_display_timing()?;

        // Enable pipe
        let pipe_conf = self.read_reg(regs::PIPEA_CONF);
        self.write_reg(regs::PIPEA_CONF, pipe_conf | pipe_conf::ENABLE);

        // Wait for pipe to enable
        for _ in 0..1000 {
            if self.read_reg(regs::PIPEA_CONF) & pipe_conf::STATE != 0 {
                break;
            }
        }

        // Enable plane
        let plane_ctrl = plane_ctrl::ENABLE | plane_ctrl::FORMAT_BGRX8888;
        self.write_reg(regs::DSPACNTR, plane_ctrl);

        // Set stride
        let stride = self.current_mode.width * 4;
        self.write_reg(regs::DSPASTRIDE, stride);

        // Set surface address
        self.write_reg(regs::DSPASURF, self.fb_base as u32);

        Ok(())
    }

    /// Set display timing for current mode
    fn set_display_timing(&mut self) -> Result<(), DisplayError> {
        // Get timing parameters for current mode
        let (htotal, hblank, hsync, vtotal, vblank, vsync) = self.get_timing_params();

        self.write_reg(regs::HTOTAL_A, htotal);
        self.write_reg(regs::HBLANK_A, hblank);
        self.write_reg(regs::HSYNC_A, hsync);
        self.write_reg(regs::VTOTAL_A, vtotal);
        self.write_reg(regs::VBLANK_A, vblank);
        self.write_reg(regs::VSYNC_A, vsync);

        // Set pipe source size
        let src = ((self.current_mode.width - 1) << 16) | (self.current_mode.height - 1);
        self.write_reg(regs::PIPEASRC, src);

        Ok(())
    }

    /// Get timing parameters for current mode
    fn get_timing_params(&self) -> (u32, u32, u32, u32, u32, u32) {
        match (self.current_mode.width, self.current_mode.height) {
            (1920, 1080) => (
                (2199 << 16) | 1919,  // HTOTAL: total | active
                (2112 << 16) | 1919,  // HBLANK
                (2051 << 16) | 2007,  // HSYNC
                (1124 << 16) | 1079,  // VTOTAL
                (1120 << 16) | 1079,  // VBLANK
                (1088 << 16) | 1083,  // VSYNC
            ),
            (1280, 720) => (
                (1649 << 16) | 1279,
                (1520 << 16) | 1279,
                (1459 << 16) | 1355,
                (749 << 16) | 719,
                (745 << 16) | 719,
                (728 << 16) | 723,
            ),
            (1024, 768) => (
                (1343 << 16) | 1023,
                (1295 << 16) | 1023,
                (1183 << 16) | 1063,
                (805 << 16) | 767,
                (802 << 16) | 767,
                (776 << 16) | 769,
            ),
            _ => (
                (799 << 16) | 639,
                (703 << 16) | 639,
                (695 << 16) | 655,
                (524 << 16) | 479,
                (521 << 16) | 479,
                (491 << 16) | 489,
            ),
        }
    }

    /// Read MMIO register
    fn read_reg(&self, offset: u32) -> u32 {
        unsafe {
            core::ptr::read_volatile((self.mmio_base + offset as u64) as *const u32)
        }
    }

    /// Write MMIO register
    fn write_reg(&self, offset: u32, value: u32) {
        unsafe {
            core::ptr::write_volatile((self.mmio_base + offset as u64) as *mut u32, value);
        }
    }

    /// Set backlight level
    fn set_backlight(&mut self, level: u8) {
        // Read current PWM configuration
        let pwm_ctl = self.read_reg(regs::BLC_PWM_CTL);
        let max_level = (pwm_ctl >> 16) & 0xFFFF;

        if max_level > 0 {
            let new_level = ((level as u32) * max_level) / 100;
            self.write_reg(regs::BLC_PWM_CTL, (pwm_ctl & 0xFFFF0000) | new_level);
        }
    }
}

impl Display for I915Driver {
    fn info(&self) -> DisplayInfo {
        DisplayInfo {
            name: String::from("Intel HD Graphics"),
            manufacturer: String::from("Intel Corporation"),
            physical_width_mm: 0,
            physical_height_mm: 0,
            current_mode: self.current_mode,
            available_modes: self.available_modes.clone(),
            is_primary: true,
            connection: DisplayConnection::Internal,
        }
    }

    fn current_mode(&self) -> DisplayMode {
        self.current_mode
    }

    fn set_mode(&mut self, mode: DisplayMode) -> Result<(), DisplayError> {
        if !self.available_modes.contains(&mode) {
            return Err(DisplayError::ModeNotSupported);
        }

        // Disable plane and pipe
        self.write_reg(regs::DSPACNTR, 0);
        let pipe_conf = self.read_reg(regs::PIPEA_CONF);
        self.write_reg(regs::PIPEA_CONF, pipe_conf & !pipe_conf::ENABLE);

        // Wait for pipe to disable
        for _ in 0..1000 {
            if self.read_reg(regs::PIPEA_CONF) & pipe_conf::STATE == 0 {
                break;
            }
        }

        // Update mode
        self.current_mode = mode;

        // Re-setup and enable
        self.setup_gtt()?;
        self.enable_display()?;

        Ok(())
    }

    fn available_modes(&self) -> Vec<DisplayMode> {
        self.available_modes.clone()
    }

    fn framebuffer_address(&self) -> u64 {
        self.fb_base
    }

    fn framebuffer_size(&self) -> usize {
        (self.current_mode.width * self.current_mode.height * 4) as usize
    }

    fn stride(&self) -> u32 {
        self.current_mode.width * 4
    }

    fn enable(&mut self) -> Result<(), DisplayError> {
        if !self.enabled {
            self.enable_display()?;
            self.enabled = true;
        }
        Ok(())
    }

    fn disable(&mut self) -> Result<(), DisplayError> {
        if self.enabled {
            // Disable plane
            self.write_reg(regs::DSPACNTR, 0);
            
            // Disable pipe
            let pipe_conf = self.read_reg(regs::PIPEA_CONF);
            self.write_reg(regs::PIPEA_CONF, pipe_conf & !pipe_conf::ENABLE);

            self.enabled = false;
        }
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn wait_vsync(&self) {
        // Clear vsync interrupt
        self.write_reg(regs::IIR, 0x00000002);
        
        // Wait for vsync
        for _ in 0..100000 {
            if self.read_reg(regs::IIR) & 0x00000002 != 0 {
                break;
            }
        }
    }

    fn set_brightness(&mut self, brightness: u8) -> Result<(), DisplayError> {
        self.brightness = brightness.min(100);
        self.set_backlight(self.brightness);
        Ok(())
    }

    fn brightness(&self) -> u8 {
        self.brightness
    }
}

impl HardwareCursor for I915Driver {
    fn hardware_cursor_supported(&self) -> bool {
        true
    }

    fn set_cursor_image(&mut self, cursor: &CursorInfo) -> Result<(), DisplayError> {
        if cursor.width > 64 || cursor.height > 64 {
            return Err(DisplayError::InvalidParameters);
        }

        // Cursor image should be 64x64 ARGB
        // Would allocate cursor buffer and setup here
        Ok(())
    }

    fn show_cursor(&mut self) {
        self.cursor_enabled = true;
        self.write_reg(regs::CURACNTR, 0x27); // ARGB cursor enabled
    }

    fn hide_cursor(&mut self) {
        self.cursor_enabled = false;
        self.write_reg(regs::CURACNTR, 0);
    }

    fn move_cursor(&mut self, x: u32, y: u32) {
        self.cursor_x = x;
        self.cursor_y = y;
        
        // Pack position (can be negative for partial visibility)
        let pos = ((y as i16 as u16 as u32) << 16) | (x as i16 as u16 as u32);
        self.write_reg(regs::CURAPOS, pos);
    }
}
