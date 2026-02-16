//! Local Account System
//!
//! Local-only user account management for KPIO Browser.
//! No cloud sync, no OAuth - simple offline profiles.

pub mod profile;

pub use profile::*;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

/// Account error types
#[derive(Debug, Clone)]
pub enum AccountError {
    /// Invalid credentials
    InvalidCredentials,
    /// Profile not found
    ProfileNotFound,
    /// Profile already exists
    ProfileExists,
    /// Storage error
    StorageError(String),
    /// Invalid name
    InvalidName,
    /// Too many profiles
    TooManyProfiles,
}

/// Maximum number of local profiles
pub const MAX_PROFILES: usize = 10;

/// Local user profile
#[derive(Debug, Clone)]
pub struct LocalProfile {
    /// Profile ID (auto-generated)
    pub id: u32,
    /// Profile name
    pub name: String,
    /// Avatar index (built-in avatars 0-15)
    pub avatar_id: u8,
    /// Profile color theme
    pub color: ProfileColor,
    /// Created timestamp
    pub created_at: u64,
    /// Last used timestamp
    pub last_used: Option<u64>,
    /// Is this the default profile?
    pub is_default: bool,
    /// Profile preferences
    pub preferences: ProfilePreferences,
}

impl LocalProfile {
    /// Create a new profile
    pub fn new(id: u32, name: String) -> Self {
        Self {
            id,
            name,
            avatar_id: 0,
            color: ProfileColor::Blue,
            created_at: 0,
            last_used: None,
            is_default: false,
            preferences: ProfilePreferences::default(),
        }
    }

    /// Create default profile
    pub fn default_profile() -> Self {
        let mut profile = Self::new(0, String::from("Default Profile"));
        profile.is_default = true;
        profile
    }

    /// Set avatar
    pub fn with_avatar(mut self, avatar_id: u8) -> Self {
        self.avatar_id = avatar_id.min(15);
        self
    }

    /// Set color
    pub fn with_color(mut self, color: ProfileColor) -> Self {
        self.color = color;
        self
    }

    /// Update last used time
    pub fn touch(&mut self, timestamp: u64) {
        self.last_used = Some(timestamp);
    }
}

/// Profile color theme
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileColor {
    Blue,
    Green,
    Purple,
    Orange,
    Pink,
    Teal,
    Red,
    Yellow,
}

impl ProfileColor {
    /// Get primary color RGB
    pub fn primary_rgb(&self) -> (u8, u8, u8) {
        match self {
            Self::Blue => (59, 130, 246),
            Self::Green => (34, 197, 94),
            Self::Purple => (168, 85, 247),
            Self::Orange => (249, 115, 22),
            Self::Pink => (236, 72, 153),
            Self::Teal => (20, 184, 166),
            Self::Red => (239, 68, 68),
            Self::Yellow => (234, 179, 8),
        }
    }

    /// Get accent color RGB (lighter variant)
    pub fn accent_rgb(&self) -> (u8, u8, u8) {
        match self {
            Self::Blue => (147, 197, 253),
            Self::Green => (134, 239, 172),
            Self::Purple => (216, 180, 254),
            Self::Orange => (253, 186, 116),
            Self::Pink => (249, 168, 212),
            Self::Teal => (94, 234, 212),
            Self::Red => (252, 165, 165),
            Self::Yellow => (253, 224, 71),
        }
    }
}

impl Default for ProfileColor {
    fn default() -> Self {
        Self::Blue
    }
}

/// Profile preferences (local only)
#[derive(Debug, Clone)]
pub struct ProfilePreferences {
    /// Theme preference
    pub theme: ThemePreference,
    /// Show bookmarks bar
    pub show_bookmarks_bar: bool,
    /// Homepage URL
    pub homepage: String,
    /// New tab page style
    pub new_tab_style: NewTabStyle,
    /// Enable download confirmations
    pub confirm_downloads: bool,
    /// Enable spell check
    pub spell_check: bool,
    /// Language code
    pub language: String,
}

impl Default for ProfilePreferences {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            show_bookmarks_bar: true,
            homepage: String::from("kpio://newtab"),
            new_tab_style: NewTabStyle::Default,
            confirm_downloads: true,
            spell_check: true,
            language: String::from("ko-KR"),
        }
    }
}

/// Theme preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemePreference {
    Light,
    Dark,
    #[default]
    System,
}

/// New tab page style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NewTabStyle {
    #[default]
    Default,
    Minimal,
    Blank,
    Custom,
}

/// Profile manager for local profiles
pub struct ProfileManager {
    /// All profiles
    profiles: RwLock<Vec<LocalProfile>>,
    /// Currently active profile ID
    active_id: RwLock<u32>,
    /// Next profile ID
    next_id: RwLock<u32>,
}

impl ProfileManager {
    /// Create new manager
    pub fn new() -> Self {
        let default_profile = LocalProfile::default_profile();

        Self {
            profiles: RwLock::new(alloc::vec![default_profile]),
            active_id: RwLock::new(0),
            next_id: RwLock::new(1),
        }
    }

    /// Get active profile
    pub fn active(&self) -> Option<LocalProfile> {
        let profiles = self.profiles.read();
        let active_id = *self.active_id.read();

        profiles.iter().find(|p| p.id == active_id).cloned()
    }

    /// Get all profiles
    pub fn list(&self) -> Vec<LocalProfile> {
        self.profiles.read().clone()
    }

    /// Get profile by ID
    pub fn get(&self, id: u32) -> Option<LocalProfile> {
        self.profiles.read().iter().find(|p| p.id == id).cloned()
    }

    /// Create new profile
    pub fn create(&self, name: String) -> Result<LocalProfile, AccountError> {
        // Validate name
        let name = name.trim().to_string();
        if name.is_empty() || name.len() > 50 {
            return Err(AccountError::InvalidName);
        }

        let mut profiles = self.profiles.write();

        // Check limit
        if profiles.len() >= MAX_PROFILES {
            return Err(AccountError::TooManyProfiles);
        }

        // Check duplicate name
        if profiles.iter().any(|p| p.name == name) {
            return Err(AccountError::ProfileExists);
        }

        // Create profile
        let mut next_id = self.next_id.write();
        let id = *next_id;
        *next_id += 1;

        let profile = LocalProfile::new(id, name);
        profiles.push(profile.clone());

        Ok(profile)
    }

    /// Update profile
    pub fn update(&self, profile: LocalProfile) -> Result<(), AccountError> {
        let mut profiles = self.profiles.write();

        if let Some(existing) = profiles.iter_mut().find(|p| p.id == profile.id) {
            *existing = profile;
            Ok(())
        } else {
            Err(AccountError::ProfileNotFound)
        }
    }

    /// Delete profile
    pub fn delete(&self, id: u32) -> Result<(), AccountError> {
        let mut profiles = self.profiles.write();

        // Find profile
        let idx = profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or(AccountError::ProfileNotFound)?;

        // Can't delete default profile
        if profiles[idx].is_default {
            return Err(AccountError::InvalidName);
        }

        profiles.remove(idx);

        // Switch to default if deleted active
        let active_id = *self.active_id.read();
        if active_id == id {
            if let Some(default) = profiles.iter().find(|p| p.is_default) {
                *self.active_id.write() = default.id;
            }
        }

        Ok(())
    }

    /// Switch active profile
    pub fn switch(&self, id: u32) -> Result<(), AccountError> {
        let profiles = self.profiles.read();

        if !profiles.iter().any(|p| p.id == id) {
            return Err(AccountError::ProfileNotFound);
        }

        *self.active_id.write() = id;
        Ok(())
    }

    /// Get profile count
    pub fn count(&self) -> usize {
        self.profiles.read().len()
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global profile manager
pub static PROFILE_MANAGER: RwLock<Option<ProfileManager>> = RwLock::new(None);

/// Initialize the profile manager
pub fn init_profile_manager() {
    *PROFILE_MANAGER.write() = Some(ProfileManager::new());
}
