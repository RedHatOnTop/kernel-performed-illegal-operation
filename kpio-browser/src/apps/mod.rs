//! System Applications Module
//!
//! Built-in system applications for KPIO OS.
//! These apps provide core desktop functionality.

pub mod desktop;
pub mod taskbar;
pub mod file_explorer;
pub mod settings;
pub mod terminal;
pub mod text_editor;
pub mod media_viewer;
pub mod calculator;
pub mod app_launcher;

pub use desktop::*;
pub use taskbar::*;
pub use file_explorer::*;
pub use settings::*;
pub use terminal::*;
pub use text_editor::*;
pub use media_viewer::*;
pub use calculator::*;
pub use app_launcher::*;

use alloc::string::String;
use alloc::vec::Vec;

/// Application identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppId(pub String);

impl AppId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Application metadata
#[derive(Debug, Clone)]
pub struct AppInfo {
    /// Application ID
    pub id: AppId,
    /// Display name
    pub name: String,
    /// Localized name
    pub localized_name: Option<String>,
    /// Description
    pub description: String,
    /// Icon name
    pub icon: String,
    /// Application category
    pub category: AppCategory,
    /// Is system app
    pub system: bool,
    /// Can be uninstalled
    pub removable: bool,
    /// Version
    pub version: String,
}

/// Application category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCategory {
    /// System utilities
    System,
    /// Productivity apps
    Productivity,
    /// Media applications
    Media,
    /// Internet/Web
    Internet,
    /// Games
    Games,
    /// Development tools
    Development,
    /// Accessories
    Accessories,
    /// Settings/Preferences
    Settings,
}

impl AppCategory {
    pub fn name(&self) -> &'static str {
        match self {
            Self::System => "시스템",
            Self::Productivity => "생산성",
            Self::Media => "미디어",
            Self::Internet => "인터넷",
            Self::Games => "게임",
            Self::Development => "개발",
            Self::Accessories => "보조프로그램",
            Self::Settings => "설정",
        }
    }
}

/// Application state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// Not running
    Stopped,
    /// Starting up
    Starting,
    /// Running normally
    Running,
    /// Suspended (background)
    Suspended,
    /// Not responding
    NotResponding,
    /// Closing
    Closing,
}

/// Running application instance
#[derive(Debug, Clone)]
pub struct AppInstance {
    /// Instance ID
    pub instance_id: u64,
    /// Application info
    pub app: AppInfo,
    /// Current state
    pub state: AppState,
    /// Window IDs
    pub windows: Vec<u64>,
    /// Memory usage (bytes)
    pub memory_usage: u64,
    /// CPU usage (percentage)
    pub cpu_usage: f32,
    /// Started at timestamp
    pub started_at: u64,
}

/// System applications registry
pub fn system_apps() -> Vec<AppInfo> {
    alloc::vec![
        AppInfo {
            id: AppId::new("kpio.files"),
            name: String::from("Files"),
            localized_name: Some(String::from("파일")),
            description: String::from("파일 및 폴더 탐색"),
            icon: String::from("folder"),
            category: AppCategory::System,
            system: true,
            removable: false,
            version: String::from("1.0.0"),
        },
        AppInfo {
            id: AppId::new("kpio.settings"),
            name: String::from("Settings"),
            localized_name: Some(String::from("설정")),
            description: String::from("시스템 설정"),
            icon: String::from("settings"),
            category: AppCategory::Settings,
            system: true,
            removable: false,
            version: String::from("1.0.0"),
        },
        AppInfo {
            id: AppId::new("kpio.terminal"),
            name: String::from("Terminal"),
            localized_name: Some(String::from("터미널")),
            description: String::from("명령줄 인터페이스"),
            icon: String::from("terminal"),
            category: AppCategory::Development,
            system: true,
            removable: false,
            version: String::from("1.0.0"),
        },
        AppInfo {
            id: AppId::new("kpio.editor"),
            name: String::from("Text Editor"),
            localized_name: Some(String::from("텍스트 편집기")),
            description: String::from("텍스트 파일 편집"),
            icon: String::from("file-text"),
            category: AppCategory::Productivity,
            system: true,
            removable: false,
            version: String::from("1.0.0"),
        },
        AppInfo {
            id: AppId::new("kpio.photos"),
            name: String::from("Photos"),
            localized_name: Some(String::from("사진")),
            description: String::from("이미지 보기 및 관리"),
            icon: String::from("image"),
            category: AppCategory::Media,
            system: true,
            removable: false,
            version: String::from("1.0.0"),
        },
        AppInfo {
            id: AppId::new("kpio.videos"),
            name: String::from("Videos"),
            localized_name: Some(String::from("동영상")),
            description: String::from("비디오 재생"),
            icon: String::from("video"),
            category: AppCategory::Media,
            system: true,
            removable: false,
            version: String::from("1.0.0"),
        },
        AppInfo {
            id: AppId::new("kpio.calculator"),
            name: String::from("Calculator"),
            localized_name: Some(String::from("계산기")),
            description: String::from("계산기"),
            icon: String::from("calculator"),
            category: AppCategory::Accessories,
            system: true,
            removable: false,
            version: String::from("1.0.0"),
        },
        AppInfo {
            id: AppId::new("kpio.browser"),
            name: String::from("Browser"),
            localized_name: Some(String::from("브라우저")),
            description: String::from("웹 브라우저"),
            icon: String::from("globe"),
            category: AppCategory::Internet,
            system: true,
            removable: false,
            version: String::from("1.0.0"),
        },
    ]
}
