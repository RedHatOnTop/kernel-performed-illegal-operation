//! Profile Storage
//!
//! Local profile data persistence.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{LocalProfile, ProfileColor, ProfilePreferences, AccountError};

/// Profile storage interface
pub trait ProfileStorage {
    /// Load all profiles
    fn load_all(&self) -> Result<Vec<LocalProfile>, AccountError>;
    
    /// Save profile
    fn save(&self, profile: &LocalProfile) -> Result<(), AccountError>;
    
    /// Delete profile data
    fn delete(&self, id: u32) -> Result<(), AccountError>;
    
    /// Load profile preferences
    fn load_preferences(&self, id: u32) -> Result<ProfilePreferences, AccountError>;
    
    /// Save profile preferences
    fn save_preferences(&self, id: u32, prefs: &ProfilePreferences) -> Result<(), AccountError>;
}

/// Profile data directory structure
/// 
/// /profiles/
///   ├── profiles.dat        - Profile list metadata
///   ├── 0/                   - Default profile
///   │   ├── preferences.dat
///   │   ├── bookmarks.dat
///   │   ├── history.dat
///   │   └── settings.dat
///   └── 1/                   - Additional profile
///       └── ...
pub struct ProfileDirectory {
    /// Base path
    pub base_path: String,
}

impl ProfileDirectory {
    /// Create new profile directory manager
    pub fn new(base_path: impl Into<String>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Get profile path
    pub fn profile_path(&self, id: u32) -> String {
        alloc::format!("{}/profiles/{}", self.base_path, id)
    }

    /// Get preferences path
    pub fn preferences_path(&self, id: u32) -> String {
        alloc::format!("{}/profiles/{}/preferences.dat", self.base_path, id)
    }

    /// Get bookmarks path
    pub fn bookmarks_path(&self, id: u32) -> String {
        alloc::format!("{}/profiles/{}/bookmarks.dat", self.base_path, id)
    }

    /// Get history path
    pub fn history_path(&self, id: u32) -> String {
        alloc::format!("{}/profiles/{}/history.dat", self.base_path, id)
    }
}

/// Profile data serializer
pub struct ProfileSerializer;

impl ProfileSerializer {
    /// Serialize profile to bytes
    pub fn serialize(profile: &LocalProfile) -> Vec<u8> {
        let mut data = Vec::new();
        
        // Magic header
        data.extend_from_slice(b"KPRF");
        
        // Version
        data.push(1);
        
        // Profile ID (4 bytes)
        data.extend_from_slice(&profile.id.to_le_bytes());
        
        // Name length + name
        let name_bytes = profile.name.as_bytes();
        data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        data.extend_from_slice(name_bytes);
        
        // Avatar ID
        data.push(profile.avatar_id);
        
        // Color
        data.push(Self::color_to_u8(&profile.color));
        
        // Timestamps
        data.extend_from_slice(&profile.created_at.to_le_bytes());
        data.extend_from_slice(&profile.last_used.unwrap_or(0).to_le_bytes());
        
        // Is default
        data.push(if profile.is_default { 1 } else { 0 });
        
        data
    }

    /// Deserialize profile from bytes
    pub fn deserialize(data: &[u8]) -> Result<LocalProfile, AccountError> {
        if data.len() < 22 {
            return Err(AccountError::StorageError(String::from("Invalid data")));
        }

        // Check magic header
        if &data[0..4] != b"KPRF" {
            return Err(AccountError::StorageError(String::from("Invalid format")));
        }

        // Version
        let _version = data[4];

        // Profile ID
        let id = u32::from_le_bytes([data[5], data[6], data[7], data[8]]);

        // Name
        let name_len = u16::from_le_bytes([data[9], data[10]]) as usize;
        if data.len() < 11 + name_len + 10 {
            return Err(AccountError::StorageError(String::from("Truncated data")));
        }
        let name = String::from_utf8(data[11..11 + name_len].to_vec())
            .map_err(|_| AccountError::StorageError(String::from("Invalid name")))?;

        let offset = 11 + name_len;

        // Avatar ID
        let avatar_id = data[offset];

        // Color
        let color = Self::u8_to_color(data[offset + 1]);

        // Timestamps
        let created_at = u64::from_le_bytes([
            data[offset + 2], data[offset + 3], data[offset + 4], data[offset + 5],
            data[offset + 6], data[offset + 7], data[offset + 8], data[offset + 9],
        ]);

        let last_used_raw = u64::from_le_bytes([
            data[offset + 10], data[offset + 11], data[offset + 12], data[offset + 13],
            data[offset + 14], data[offset + 15], data[offset + 16], data[offset + 17],
        ]);
        let last_used = if last_used_raw == 0 { None } else { Some(last_used_raw) };

        let is_default = data[offset + 18] != 0;

        Ok(LocalProfile {
            id,
            name,
            avatar_id,
            color,
            created_at,
            last_used,
            is_default,
            preferences: ProfilePreferences::default(),
        })
    }

    fn color_to_u8(color: &ProfileColor) -> u8 {
        match color {
            ProfileColor::Blue => 0,
            ProfileColor::Green => 1,
            ProfileColor::Purple => 2,
            ProfileColor::Orange => 3,
            ProfileColor::Pink => 4,
            ProfileColor::Teal => 5,
            ProfileColor::Red => 6,
            ProfileColor::Yellow => 7,
        }
    }

    fn u8_to_color(byte: u8) -> ProfileColor {
        match byte {
            0 => ProfileColor::Blue,
            1 => ProfileColor::Green,
            2 => ProfileColor::Purple,
            3 => ProfileColor::Orange,
            4 => ProfileColor::Pink,
            5 => ProfileColor::Teal,
            6 => ProfileColor::Red,
            7 => ProfileColor::Yellow,
            _ => ProfileColor::Blue,
        }
    }
}
