//! Settings Application
//!
//! System settings and preferences panel.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Settings app state
#[derive(Debug, Clone)]
pub struct SettingsApp {
    /// Current section
    pub current_section: SettingsSection,
    /// Search query
    pub search_query: Option<String>,
    /// Search results
    pub search_results: Vec<SettingSearchResult>,
}

impl SettingsApp {
    /// Create new settings app
    pub fn new() -> Self {
        Self {
            current_section: SettingsSection::System,
            search_query: None,
            search_results: Vec::new(),
        }
    }

    /// Navigate to section
    pub fn navigate(&mut self, section: SettingsSection) {
        self.current_section = section;
        self.search_query = None;
        self.search_results.clear();
    }

    /// Search settings
    pub fn search(&mut self, query: &str) {
        self.search_query = Some(query.to_string());
        // In real implementation, would search all settings
        self.search_results.clear();
    }

    /// Get all sections
    pub fn sections() -> Vec<SettingsSectionInfo> {
        alloc::vec![
            SettingsSectionInfo {
                section: SettingsSection::System,
                name: String::from("System"),
                description: String::from("Display, notifications, power"),
                icon: String::from("monitor"),
            },
            SettingsSectionInfo {
                section: SettingsSection::Devices,
                name: String::from("Devices"),
                description: String::from("Bluetooth, printers, mouse"),
                icon: String::from("monitor"),
            },
            SettingsSectionInfo {
                section: SettingsSection::Network,
                name: String::from("Network"),
                description: String::from("Wi-Fi, VPN, proxy"),
                icon: String::from("wifi"),
            },
            SettingsSectionInfo {
                section: SettingsSection::Personalization,
                name: String::from("Personalization"),
                description: String::from("Background, colors, themes"),
                icon: String::from("palette"),
            },
            SettingsSectionInfo {
                section: SettingsSection::Apps,
                name: String::from("Apps"),
                description: String::from("Installed apps, default apps"),
                icon: String::from("grid"),
            },
            SettingsSectionInfo {
                section: SettingsSection::Accounts,
                name: String::from("Accounts"),
                description: String::from("Profile, sign-in options"),
                icon: String::from("user"),
            },
            SettingsSectionInfo {
                section: SettingsSection::Privacy,
                name: String::from("Privacy"),
                description: String::from("Location, camera, microphone"),
                icon: String::from("shield"),
            },
            SettingsSectionInfo {
                section: SettingsSection::Accessibility,
                name: String::from("Accessibility"),
                description: String::from("Screen reader, display, audio"),
                icon: String::from("accessibility"),
            },
            SettingsSectionInfo {
                section: SettingsSection::DateTime,
                name: String::from("Date & Time"),
                description: String::from("Timezone, format"),
                icon: String::from("clock"),
            },
            SettingsSectionInfo {
                section: SettingsSection::Language,
                name: String::from("Language & Region"),
                description: String::from("Language, input, regional format"),
                icon: String::from("globe"),
            },
            SettingsSectionInfo {
                section: SettingsSection::Update,
                name: String::from("Update"),
                description: String::from("System update, recovery"),
                icon: String::from("refresh-cw"),
            },
            SettingsSectionInfo {
                section: SettingsSection::About,
                name: String::from("About"),
                description: String::from("System information, specifications"),
                icon: String::from("info"),
            },
        ]
    }
}

impl Default for SettingsApp {
    fn default() -> Self {
        Self::new()
    }
}

/// Settings sections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsSection {
    #[default]
    System,
    Devices,
    Network,
    Personalization,
    Apps,
    Accounts,
    Privacy,
    Accessibility,
    DateTime,
    Language,
    Update,
    About,
}

/// Settings section info
#[derive(Debug, Clone)]
pub struct SettingsSectionInfo {
    pub section: SettingsSection,
    pub name: String,
    pub description: String,
    pub icon: String,
}

/// Setting search result
#[derive(Debug, Clone)]
pub struct SettingSearchResult {
    pub section: SettingsSection,
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
}

// =============================================================================
// System Settings
// =============================================================================

/// Display settings
#[derive(Debug, Clone)]
pub struct DisplaySettings {
    /// Screen resolution
    pub resolution: Resolution,
    /// Available resolutions
    pub available_resolutions: Vec<Resolution>,
    /// Refresh rate
    pub refresh_rate: u32,
    /// Scale factor (100 = 100%)
    pub scale: u32,
    /// Brightness (0-100)
    pub brightness: u8,
    /// Night light enabled
    pub night_light: bool,
    /// Night light strength (0-100)
    pub night_light_strength: u8,
    /// Orientation
    pub orientation: Orientation,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            resolution: Resolution {
                width: 1920,
                height: 1080,
            },
            available_resolutions: alloc::vec![
                Resolution {
                    width: 1920,
                    height: 1080
                },
                Resolution {
                    width: 1600,
                    height: 900
                },
                Resolution {
                    width: 1366,
                    height: 768
                },
                Resolution {
                    width: 1280,
                    height: 720
                },
            ],
            refresh_rate: 60,
            scale: 100,
            brightness: 80,
            night_light: false,
            night_light_strength: 50,
            orientation: Orientation::Landscape,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    Landscape,
    Portrait,
    LandscapeFlipped,
    PortraitFlipped,
}

/// Sound settings
#[derive(Debug, Clone)]
pub struct SoundSettings {
    /// Master volume (0-100)
    pub master_volume: u8,
    /// System sounds
    pub system_sounds: bool,
    /// Output device
    pub output_device: Option<String>,
    /// Input device
    pub input_device: Option<String>,
    /// Input volume (0-100)
    pub input_volume: u8,
    /// Mono audio
    pub mono_audio: bool,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self {
            master_volume: 50,
            system_sounds: true,
            output_device: None,
            input_device: None,
            input_volume: 50,
            mono_audio: false,
        }
    }
}

/// Notification settings
#[derive(Debug, Clone)]
pub struct NotificationSettings {
    /// Enable notifications
    pub enabled: bool,
    /// Show on lock screen
    pub show_on_lock_screen: bool,
    /// Show banners
    pub show_banners: bool,
    /// Play sounds
    pub play_sounds: bool,
    /// Do not disturb
    pub do_not_disturb: bool,
    /// DND schedule
    pub dnd_schedule: Option<DndSchedule>,
    /// Per-app settings
    pub app_settings: Vec<AppNotificationSettings>,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            show_on_lock_screen: true,
            show_banners: true,
            play_sounds: true,
            do_not_disturb: false,
            dnd_schedule: None,
            app_settings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DndSchedule {
    pub start_hour: u8,
    pub start_minute: u8,
    pub end_hour: u8,
    pub end_minute: u8,
}

#[derive(Debug, Clone)]
pub struct AppNotificationSettings {
    pub app_id: String,
    pub enabled: bool,
    pub show_banners: bool,
    pub play_sounds: bool,
}

/// Power settings
#[derive(Debug, Clone)]
pub struct PowerSettings {
    /// Power mode
    pub power_mode: PowerMode,
    /// Screen off timeout (seconds)
    pub screen_off_timeout: u32,
    /// Sleep timeout (seconds)
    pub sleep_timeout: u32,
    /// Battery saver threshold
    pub battery_saver_threshold: u8,
    /// Auto battery saver
    pub auto_battery_saver: bool,
}

impl Default for PowerSettings {
    fn default() -> Self {
        Self {
            power_mode: PowerMode::Balanced,
            screen_off_timeout: 300,
            sleep_timeout: 1800,
            battery_saver_threshold: 20,
            auto_battery_saver: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerMode {
    PowerSaver,
    #[default]
    Balanced,
    Performance,
}

// =============================================================================
// Personalization Settings
// =============================================================================

/// Theme settings
#[derive(Debug, Clone)]
pub struct ThemeSettings {
    /// Theme mode
    pub mode: ThemeMode,
    /// Accent color
    pub accent_color: String,
    /// Custom accent colors
    pub custom_colors: Vec<String>,
    /// Transparency effects
    pub transparency: bool,
    /// Animation effects
    pub animations: bool,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            mode: ThemeMode::System,
            accent_color: String::from("#3B82F6"),
            custom_colors: Vec::new(),
            transparency: true,
            animations: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

/// Wallpaper settings
#[derive(Debug, Clone)]
pub struct WallpaperSettings {
    /// Current wallpaper
    pub wallpaper_type: WallpaperType,
    /// Solid color
    pub solid_color: String,
    /// Image path
    pub image_path: Option<String>,
    /// Slideshow folder
    pub slideshow_folder: Option<String>,
    /// Slideshow interval (minutes)
    pub slideshow_interval: u32,
    /// Fit mode
    pub fit: WallpaperFit,
}

impl Default for WallpaperSettings {
    fn default() -> Self {
        Self {
            wallpaper_type: WallpaperType::SolidColor,
            solid_color: String::from("#1e293b"),
            image_path: None,
            slideshow_folder: None,
            slideshow_interval: 30,
            fit: WallpaperFit::Fill,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WallpaperType {
    #[default]
    SolidColor,
    Image,
    Slideshow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WallpaperFit {
    #[default]
    Fill,
    Fit,
    Stretch,
    Tile,
    Center,
}

/// Taskbar settings
#[derive(Debug, Clone)]
pub struct TaskbarSettings {
    /// Position
    pub position: TaskbarPosition,
    /// Auto-hide
    pub auto_hide: bool,
    /// Show labels
    pub show_labels: bool,
    /// Small icons
    pub small_icons: bool,
    /// Center icons
    pub center_icons: bool,
    /// Show search
    pub show_search: bool,
    /// Show task view
    pub show_task_view: bool,
}

impl Default for TaskbarSettings {
    fn default() -> Self {
        Self {
            position: TaskbarPosition::Bottom,
            auto_hide: false,
            show_labels: true,
            small_icons: false,
            center_icons: false,
            show_search: true,
            show_task_view: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskbarPosition {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

// =============================================================================
// Privacy Settings
// =============================================================================

/// Privacy settings
#[derive(Debug, Clone)]
pub struct PrivacySettings {
    /// Location access
    pub location: PermissionSetting,
    /// Camera access
    pub camera: PermissionSetting,
    /// Microphone access
    pub microphone: PermissionSetting,
    /// Clear browsing data on close
    pub clear_on_close: bool,
    /// Send crash reports
    pub crash_reports: bool,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            location: PermissionSetting::default(),
            camera: PermissionSetting::default(),
            microphone: PermissionSetting::default(),
            clear_on_close: false,
            crash_reports: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PermissionSetting {
    /// Global enable
    pub enabled: bool,
    /// Per-app permissions
    pub app_permissions: Vec<AppPermission>,
}

impl Default for PermissionSetting {
    fn default() -> Self {
        Self {
            enabled: true,
            app_permissions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppPermission {
    pub app_id: String,
    pub allowed: bool,
}

// =============================================================================
// Accessibility Settings
// =============================================================================

/// Accessibility settings
#[derive(Debug, Clone)]
pub struct AccessibilitySettings {
    /// High contrast
    pub high_contrast: bool,
    /// Reduce motion
    pub reduce_motion: bool,
    /// Reduce transparency
    pub reduce_transparency: bool,
    /// Text scaling (100 = 100%)
    pub text_scale: u32,
    /// Screen reader
    pub screen_reader: bool,
    /// Cursor size
    pub cursor_size: CursorSize,
    /// Keyboard settings
    pub keyboard: KeyboardAccessibility,
}

impl Default for AccessibilitySettings {
    fn default() -> Self {
        Self {
            high_contrast: false,
            reduce_motion: false,
            reduce_transparency: false,
            text_scale: 100,
            screen_reader: false,
            cursor_size: CursorSize::Normal,
            keyboard: KeyboardAccessibility::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorSize {
    Small,
    #[default]
    Normal,
    Large,
    ExtraLarge,
}

#[derive(Debug, Clone)]
pub struct KeyboardAccessibility {
    /// Sticky keys
    pub sticky_keys: bool,
    /// Filter keys
    pub filter_keys: bool,
    /// Toggle keys (sound for caps/num lock)
    pub toggle_keys: bool,
}

impl Default for KeyboardAccessibility {
    fn default() -> Self {
        Self {
            sticky_keys: false,
            filter_keys: false,
            toggle_keys: false,
        }
    }
}

// =============================================================================
// Date/Time Settings
// =============================================================================

/// Date and time settings
#[derive(Debug, Clone)]
pub struct DateTimeSettings {
    /// Automatic time
    pub automatic_time: bool,
    /// Automatic timezone
    pub automatic_timezone: bool,
    /// Timezone
    pub timezone: String,
    /// 24-hour format
    pub use_24_hour: bool,
    /// Date format
    pub date_format: DateFormat,
    /// First day of week
    pub first_day_of_week: DayOfWeek,
}

impl Default for DateTimeSettings {
    fn default() -> Self {
        Self {
            automatic_time: true,
            automatic_timezone: true,
            timezone: String::from("Asia/Seoul"),
            use_24_hour: true,
            date_format: DateFormat::YearMonthDay,
            first_day_of_week: DayOfWeek::Sunday,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateFormat {
    #[default]
    YearMonthDay,
    MonthDayYear,
    DayMonthYear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DayOfWeek {
    #[default]
    Sunday,
    Monday,
    Saturday,
}

// =============================================================================
// Language Settings
// =============================================================================

/// Language settings
#[derive(Debug, Clone)]
pub struct LanguageSettings {
    /// Display language
    pub display_language: String,
    /// Installed languages
    pub installed_languages: Vec<String>,
    /// Input methods
    pub input_methods: Vec<InputMethod>,
    /// Active input method
    pub active_input_method: Option<String>,
}

impl Default for LanguageSettings {
    fn default() -> Self {
        Self {
            display_language: String::from("ko-KR"),
            installed_languages: alloc::vec![String::from("ko-KR"), String::from("en-US"),],
            input_methods: alloc::vec![
                InputMethod {
                    id: String::from("hangul"),
                    name: String::from("Hangul"),
                    language: String::from("ko-KR"),
                },
                InputMethod {
                    id: String::from("english"),
                    name: String::from("English"),
                    language: String::from("en-US"),
                },
            ],
            active_input_method: Some(String::from("hangul")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InputMethod {
    pub id: String,
    pub name: String,
    pub language: String,
}

// =============================================================================
// About Settings
// =============================================================================

/// System info
#[derive(Debug, Clone)]
pub struct SystemInfo {
    /// OS name
    pub os_name: String,
    /// OS version
    pub os_version: String,
    /// Build number
    pub build_number: String,
    /// Device name
    pub device_name: String,
    /// Processor
    pub processor: String,
    /// RAM
    pub ram_gb: u32,
    /// Storage total
    pub storage_total_gb: u64,
    /// Storage used
    pub storage_used_gb: u64,
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self {
            os_name: String::from("KPIO OS"),
            os_version: String::from("1.0.0"),
            build_number: String::from("1000"),
            device_name: String::from("KPIO Desktop"),
            processor: String::from("Unknown"),
            ram_gb: 8,
            storage_total_gb: 256,
            storage_used_gb: 64,
        }
    }
}
