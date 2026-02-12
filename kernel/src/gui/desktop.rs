//! Desktop Environment
//!
//! Flat desktop shell with subtle wallpaper and modern geometric icons.

use super::render::{Color, Renderer};
use super::theme::{Accent, IconColor, Radius, Size, Spacing, Surface, Text};
use alloc::string::String;
use alloc::vec::Vec;

/// Desktop icon
#[derive(Debug, Clone)]
pub struct DesktopIcon {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub icon_type: IconType,
    pub hovered: bool,
}

/// Icon types
#[derive(Debug, Clone)]
pub enum IconType {
    Files,
    Browser,
    Terminal,
    Settings,
    Trash,
    /// Dynamically installed app (PWA / WebApp)
    InstalledApp {
        app_id: u64,
        icon_data: Option<Vec<u8>>,
    },
}

impl IconType {
    /// Get icon pattern (32x32 bitmap — two u32 per row)
    pub fn get_pattern(&self) -> [u32; 32] {
        match self {
            IconType::Files => [
                // Folder icon — open folder shape
                0x00000000, 0x00000000, 0x1FFC0000, 0x20020000, 0x7FFE0000, 0x40010000, 0x40010000,
                0x40010000, 0x40010000, 0x40010000, 0x40010000, 0x40010000, 0x40010000, 0x40010000,
                0x40010000, 0x7FFE0000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000,
            ],
            IconType::Browser => [
                // Globe / browser icon
                0x00000000, 0x03FC0000, 0x0C030000, 0x13FC8000, 0x20004000, 0x2FF84000, 0x40002000,
                0x4FF82000, 0x40002000, 0x40002000, 0x40002000, 0x4FF82000, 0x40002000, 0x2FF84000,
                0x20004000, 0x13FC8000, 0x0C030000, 0x03FC0000, 0x00000000, 0x00000000, 0x00000000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000,
            ],
            IconType::Terminal => [
                // Terminal / console icon
                0x00000000, 0x7FFE0000, 0x40010000, 0x7FFE0000, 0x40010000, 0x40010000, 0x43010000,
                0x44810000, 0x48410000, 0x50210000, 0x48410000, 0x44810000, 0x43010000, 0x40010000,
                0x403E0000, 0x40010000, 0x40010000, 0x7FFE0000, 0x00000000, 0x00000000, 0x00000000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000,
            ],
            IconType::Settings => [
                // Gear / cog icon
                0x00000000, 0x01800000, 0x01800000, 0x0DB00000, 0x1FF80000, 0x38180000, 0x30080000,
                0x30080000, 0x38180000, 0x1FF80000, 0x0DB00000, 0x01800000, 0x01800000, 0x00000000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000,
            ],
            IconType::Trash => [
                // Trash can icon
                0x00000000, 0x07E00000, 0x02400000, 0x1FF80000, 0x10080000, 0x12480000, 0x12480000,
                0x12480000, 0x12480000, 0x12480000, 0x12480000, 0x12480000, 0x10080000, 0x0FF00000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000,
            ],
            IconType::InstalledApp { .. } => [
                // Generic app icon — rounded square with "A" letter shape
                0x00000000, 0x0FFF0000, 0x1FFF8000, 0x18018000, 0x18018000, 0x18018000, 0x18018000,
                0x18418000, 0x18E18000, 0x19B18000, 0x1B198000, 0x1E0F8000, 0x1FFF8000, 0x18018000,
                0x18018000, 0x18018000, 0x18018000, 0x0FFF0000, 0x00000000, 0x00000000, 0x00000000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
                0x00000000, 0x00000000, 0x00000000, 0x00000000,
            ],
        }
    }

    /// Get the accent colour for this icon type
    pub fn color(&self) -> Color {
        match self {
            IconType::Files => IconColor::FILES,
            IconType::Browser => IconColor::BROWSER,
            IconType::Terminal => IconColor::TERMINAL,
            IconType::Settings => IconColor::SETTINGS,
            IconType::Trash => IconColor::TRASH,
            IconType::InstalledApp { .. } => Accent::PRIMARY,
        }
    }
}

/// Desktop state
pub struct Desktop {
    pub width: u32,
    pub height: u32,
    pub icons: Vec<DesktopIcon>,
}

impl Desktop {
    /// Create new desktop
    pub fn new(width: u32, height: u32) -> Self {
        let gap = Size::DESKTOP_ICON_GAP as i32;
        let icons = alloc::vec![
            DesktopIcon {
                name: String::from("Files"),
                x: 24,
                y: 24,
                icon_type: IconType::Files,
                hovered: false
            },
            DesktopIcon {
                name: String::from("Browser"),
                x: 24,
                y: 24 + gap,
                icon_type: IconType::Browser,
                hovered: false
            },
            DesktopIcon {
                name: String::from("Terminal"),
                x: 24,
                y: 24 + gap * 2,
                icon_type: IconType::Terminal,
                hovered: false
            },
            DesktopIcon {
                name: String::from("Settings"),
                x: 24,
                y: 24 + gap * 3,
                icon_type: IconType::Settings,
                hovered: false
            },
            DesktopIcon {
                name: String::from("Trash"),
                x: 24,
                y: 24 + gap * 4,
                icon_type: IconType::Trash,
                hovered: false
            },
        ];
        Self {
            width,
            height,
            icons,
        }
    }

    /// Hit-test: returns which icon (if any) is at the given pixel
    pub fn icon_at(&self, mx: i32, my: i32) -> Option<usize> {
        let area = Size::ICON_AREA;
        for (i, icon) in self.icons.iter().enumerate() {
            if mx >= icon.x - 4
                && mx < icon.x + area as i32 + 4
                && my >= icon.y - 4
                && my < icon.y + area as i32 + 20
            {
                return Some(i);
            }
        }
        None
    }

    /// Set hover state for desktop icons
    pub fn set_hover(&mut self, idx: Option<usize>) {
        for (i, icon) in self.icons.iter_mut().enumerate() {
            icon.hovered = Some(i) == idx;
        }
    }

    /// Render desktop with subtle flat wallpaper
    pub fn render(&self, renderer: &mut Renderer) {
        // Very subtle vertical gradient (flat wallpaper)
        renderer.fill_gradient_v(
            0,
            0,
            self.width,
            self.height,
            Surface::DESKTOP_TOP,
            Surface::DESKTOP_BOTTOM,
        );

        // Render desktop icons
        for icon in &self.icons {
            self.render_icon(renderer, icon);
        }
    }

    /// Render a single flat desktop icon
    fn render_icon(&self, renderer: &mut Renderer, icon: &DesktopIcon) {
        let icon_area = Size::ICON_AREA;
        let pattern = icon.icon_type.get_pattern();
        let tint = icon.icon_type.color();

        // Hover highlight — subtle translucent pill
        if icon.hovered {
            renderer.fill_rounded_rect_aa(
                icon.x - 4,
                icon.y - 4,
                icon_area + 8,
                icon_area + 24,
                Radius::ICON,
                Color::rgba(255, 255, 255, 16),
            );
        }

        // Draw the 32×32 icon bitmap at 2× scale (using top 16 rows of the 32-entry pattern)
        let scale = 2i32;
        let ix = icon.x + (icon_area as i32 - 16 * scale) / 2;
        let iy = icon.y + 4;

        // The pattern is 32 u32 entries but actually represents 16 rows of 16-bit data
        // stored in the upper 16 bits of each u32 (to keep the bitmap simple).
        // We'll render the first 16 non-zero rows we find.
        for (row, &bits) in pattern.iter().enumerate() {
            if row >= 16 {
                break;
            } // Only use first 16 rows for the icon bitmap
            for col in 0..16i32 {
                if (bits >> (31 - col)) & 1 == 1 {
                    let bx = ix + col * scale;
                    let by = iy + row as i32 * scale;
                    renderer.fill_rect(bx, by, scale as u32, scale as u32, tint);
                }
            }
        }

        // Icon label (centered below)
        let name_len = icon.name.len() as i32 * 8;
        let name_x = icon.x + (icon_area as i32 - name_len) / 2;
        let name_y = icon.y + icon_area as i32;
        // Text shadow for readability on dark wallpaper
        renderer.draw_text(
            name_x + 1,
            name_y + 1,
            &icon.name,
            Color::rgba(0, 0, 0, 100),
        );
        renderer.draw_text(name_x, name_y, &icon.name, Text::ON_DARK);
    }

    /// Refresh installed-app icons from the kernel app registry.
    ///
    /// Keeps the five system icons, then appends one icon per registered
    /// `WebApp` type application.  Icons are arranged in a single column
    /// beneath the system icons.
    pub fn refresh_app_icons(&mut self) {
        use crate::app::registry::{self, APP_REGISTRY};

        // Remove existing InstalledApp icons
        self.icons.retain(|icon| !matches!(icon.icon_type, IconType::InstalledApp { .. }));

        let gap = Size::DESKTOP_ICON_GAP as i32;
        let system_count = self.icons.len() as i32; // normally 5

        // Query the registry for all WebApp-type apps
        let reg = APP_REGISTRY.lock();
        let apps = reg.list();
        let mut app_idx: i32 = 0;
        for desc in apps {
            // Only add WebApp-type apps to the desktop
            if matches!(desc.app_type, registry::KernelAppType::WebApp { .. }) {
                let icon = DesktopIcon {
                    name: desc.name.clone(),
                    x: 24,
                    y: 24 + gap * (system_count + app_idx),
                    icon_type: IconType::InstalledApp {
                        app_id: desc.id.0,
                        icon_data: desc.icon_data.clone(),
                    },
                    hovered: false,
                };
                self.icons.push(icon);
                app_idx += 1;
            }
        }
    }
}
