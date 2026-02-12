//! Web Storage API
//!
//! Implements `localStorage` and `sessionStorage` per W3C Web Storage spec.
//!
//! - `localStorage`: persisted to VFS at `/apps/storage/{app_id}/local_storage.json`
//! - `sessionStorage`: in-memory only, cleared on app termination
//! - Per-origin quota: 5 MB (keys + values combined)

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

// ── Constants ───────────────────────────────────────────────

/// Maximum storage per origin (5 MB).
const MAX_STORAGE_SIZE: usize = 5 * 1024 * 1024;

// ── Types ───────────────────────────────────────────────────

/// Storage type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    /// Persistent — survives across sessions.
    Local,
    /// Ephemeral — cleared on app close.
    Session,
}

/// Error type for storage operations.
#[derive(Debug, Clone)]
pub enum StorageError {
    /// Key + value would exceed the 5 MB quota.
    QuotaExceeded,
    /// VFS I/O failure during persistence.
    IoError,
}

impl core::fmt::Display for StorageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StorageError::QuotaExceeded => write!(f, "QuotaExceededError"),
            StorageError::IoError => write!(f, "storage I/O error"),
        }
    }
}

/// A W3C Web Storage implementation.
pub struct WebStorage {
    /// Origin this storage belongs to.
    origin: String,
    /// App ID for VFS path.
    app_id: u64,
    /// Key → Value store.
    data: BTreeMap<String, String>,
    /// Local or Session.
    storage_type: StorageType,
    /// Current total size (keys + values in bytes).
    current_size: usize,
}

// ── Implementation ──────────────────────────────────────────

impl WebStorage {
    /// Create a new empty storage.
    pub fn new(origin: &str, app_id: u64, storage_type: StorageType) -> Self {
        Self {
            origin: String::from(origin),
            app_id,
            data: BTreeMap::new(),
            storage_type,
            current_size: 0,
        }
    }

    /// Get an item by key.
    pub fn get_item(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(|s| s.as_str())
    }

    /// Set an item.  Returns `QuotaExceeded` if the quota would be exceeded.
    pub fn set_item(&mut self, key: &str, value: &str) -> Result<(), StorageError> {
        let new_entry_size = key.len() + value.len();

        // Calculate size delta
        let old_entry_size = self
            .data
            .get(key)
            .map(|v| key.len() + v.len())
            .unwrap_or(0);
        let delta = new_entry_size as isize - old_entry_size as isize;
        let projected = (self.current_size as isize + delta) as usize;

        if projected > MAX_STORAGE_SIZE {
            return Err(StorageError::QuotaExceeded);
        }

        self.data.insert(String::from(key), String::from(value));
        self.current_size = projected;

        Ok(())
    }

    /// Remove an item.
    pub fn remove_item(&mut self, key: &str) {
        if let Some(value) = self.data.remove(key) {
            self.current_size = self
                .current_size
                .saturating_sub(key.len() + value.len());
        }
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.data.clear();
        self.current_size = 0;
    }

    /// Get the key at the given index (insertion order via BTreeMap sort order).
    pub fn key(&self, index: usize) -> Option<&str> {
        self.data.keys().nth(index).map(|s| s.as_str())
    }

    /// Number of items.
    pub fn length(&self) -> usize {
        self.data.len()
    }

    /// Current byte usage.
    pub fn size(&self) -> usize {
        self.current_size
    }

    /// Maximum byte capacity.
    pub fn max_size(&self) -> usize {
        MAX_STORAGE_SIZE
    }

    /// Origin string.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Storage type.
    pub fn storage_type(&self) -> StorageType {
        self.storage_type
    }

    // ── Persistence (localStorage only) ─────────────────────

    /// Serialize to a simple JSON string for VFS persistence.
    pub fn to_json(&self) -> String {
        let mut json = String::from("{");
        let mut first = true;
        for (k, v) in &self.data {
            if !first {
                json.push(',');
            }
            json.push('"');
            json.push_str(&escape_json(k));
            json.push_str("\":\"");
            json.push_str(&escape_json(v));
            json.push('"');
            first = false;
        }
        json.push('}');
        json
    }

    /// Load from a JSON string (simple `{"key":"value",...}` format).
    pub fn from_json(&mut self, json: &str) {
        self.data.clear();
        self.current_size = 0;

        // Minimal parser: extract "key":"value" pairs
        let trimmed = json.trim();
        if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
            return;
        }
        let inner = &trimmed[1..trimmed.len() - 1];

        let mut pos = 0;
        while pos < inner.len() {
            // Find key start
            if let Some(ks) = inner[pos..].find('"') {
                let key_start = pos + ks + 1;
                if let Some(ke) = inner[key_start..].find('"') {
                    let key = unescape_json(&inner[key_start..key_start + ke]);

                    // Find value start (after ":")
                    let after_key = key_start + ke + 1;
                    if let Some(colon) = inner[after_key..].find(':') {
                        let after_colon = after_key + colon + 1;
                        if let Some(vs) = inner[after_colon..].find('"') {
                            let val_start = after_colon + vs + 1;
                            if let Some(ve) = inner[val_start..].find('"') {
                                let value = unescape_json(&inner[val_start..val_start + ve]);
                                self.current_size += key.len() + value.len();
                                self.data.insert(key, value);
                                pos = val_start + ve + 1;
                                continue;
                            }
                        }
                    }
                }
            }
            break;
        }
    }

    /// VFS path for this storage.
    pub fn vfs_path(&self) -> String {
        alloc::format!("/apps/storage/{}/local_storage.json", self.app_id)
    }
}

// ── Helpers ─────────────────────────────────────────────────

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

fn unescape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut escape = false;
    for c in s.chars() {
        if escape {
            match c {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                _ => {
                    out.push('\\');
                    out.push(c);
                }
            }
            escape = false;
        } else if c == '\\' {
            escape = true;
        } else {
            out.push(c);
        }
    }
    out
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_item() {
        let mut s = WebStorage::new("https://example.com", 1, StorageType::Local);
        s.set_item("key", "value").unwrap();
        assert_eq!(s.get_item("key"), Some("value"));
    }

    #[test]
    fn remove_item() {
        let mut s = WebStorage::new("https://example.com", 1, StorageType::Local);
        s.set_item("k", "v").unwrap();
        s.remove_item("k");
        assert_eq!(s.get_item("k"), None);
    }

    #[test]
    fn clear() {
        let mut s = WebStorage::new("https://example.com", 1, StorageType::Local);
        s.set_item("a", "1").unwrap();
        s.set_item("b", "2").unwrap();
        s.clear();
        assert_eq!(s.length(), 0);
        assert_eq!(s.size(), 0);
    }

    #[test]
    fn key_by_index() {
        let mut s = WebStorage::new("https://example.com", 1, StorageType::Local);
        s.set_item("alpha", "1").unwrap();
        s.set_item("beta", "2").unwrap();
        // BTreeMap sorts alphabetically
        assert_eq!(s.key(0), Some("alpha"));
        assert_eq!(s.key(1), Some("beta"));
        assert_eq!(s.key(2), None);
    }

    #[test]
    fn quota_exceeded() {
        let mut s = WebStorage::new("https://example.com", 1, StorageType::Local);
        // Create a string just under 5MB
        let big = "x".repeat(MAX_STORAGE_SIZE - 10);
        s.set_item("big", &big).unwrap();

        // Adding more should fail
        let result = s.set_item("extra", "too much data");
        assert!(matches!(result, Err(StorageError::QuotaExceeded)));
    }

    #[test]
    fn overwrite_same_key_no_leak() {
        let mut s = WebStorage::new("https://example.com", 1, StorageType::Local);
        s.set_item("k", "short").unwrap();
        let size1 = s.size();
        s.set_item("k", "longer value").unwrap();
        let size2 = s.size();
        // Size should reflect the new value, not accumulate
        assert!(size2 > size1);
        assert_eq!(s.length(), 1);
    }

    #[test]
    fn json_roundtrip() {
        let mut s = WebStorage::new("https://example.com", 1, StorageType::Local);
        s.set_item("name", "KPIO").unwrap();
        s.set_item("version", "1.0").unwrap();

        let json = s.to_json();

        let mut s2 = WebStorage::new("https://example.com", 1, StorageType::Local);
        s2.from_json(&json);

        assert_eq!(s2.get_item("name"), Some("KPIO"));
        assert_eq!(s2.get_item("version"), Some("1.0"));
        assert_eq!(s2.length(), 2);
    }

    #[test]
    fn json_escape_special_chars() {
        let mut s = WebStorage::new("https://example.com", 1, StorageType::Local);
        s.set_item("quote", "he said \"hello\"").unwrap();

        let json = s.to_json();
        assert!(json.contains("\\\"hello\\\""));

        let mut s2 = WebStorage::new("https://example.com", 1, StorageType::Local);
        s2.from_json(&json);
        assert_eq!(s2.get_item("quote"), Some("he said \"hello\""));
    }

    #[test]
    fn length_and_size() {
        let mut s = WebStorage::new("https://example.com", 1, StorageType::Local);
        assert_eq!(s.length(), 0);
        assert_eq!(s.size(), 0);

        s.set_item("ab", "cd").unwrap();
        assert_eq!(s.length(), 1);
        assert_eq!(s.size(), 4); // "ab" + "cd" = 4 bytes
    }
}
