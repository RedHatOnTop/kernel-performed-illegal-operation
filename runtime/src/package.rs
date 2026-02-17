//! `.kpioapp` package format handling.
//!
//! A `.kpioapp` file is a ZIP archive containing:
//! - `manifest.toml`  — app metadata (name, version, entry, permissions, etc.)
//! - `app.wasm`        — main WASM module binary
//! - `resources/`      — icons and other asset files (optional)
//! - `data/`           — initial data directory (optional)
//!
//! This module provides parsing, validation, and in-memory representation of
//! the package without relying on std or external ZIP libraries (the kernel
//! environment is `no_std`).

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

/// App manifest parsed from `manifest.toml`.
#[derive(Debug, Clone)]
pub struct AppManifest {
    /// Unique application identifier (reverse-domain style, e.g. `com.example.calc`).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// SemVer version string.
    pub version: String,
    /// One-line description.
    pub description: Option<String>,
    /// Author name / email.
    pub author: Option<String>,
    /// Relative path to icon inside the package (e.g. `resources/icon-192.png`).
    pub icon: Option<String>,
    /// Entry WASM module path inside the package (default: `app.wasm`).
    pub entry: String,
    /// Required permissions.
    pub permissions: ManifestPermissions,
    /// Minimum KPIO version required (SemVer).
    pub min_kpio_version: Option<String>,
}

/// Permissions declared in the manifest.
#[derive(Debug, Clone, Default)]
pub struct ManifestPermissions {
    /// Filesystem access mode.
    pub filesystem: FsPermission,
    /// Network access allowed.
    pub network: bool,
    /// GUI access (create windows / draw).
    pub gui: bool,
    /// Clipboard access.
    pub clipboard: bool,
    /// Notification access.
    pub notifications: bool,
}

/// Filesystem access level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsPermission {
    /// No filesystem access beyond the app's own data directory.
    None,
    /// Read-only access to user-selected paths.
    ReadOnly,
    /// Read-write access to user-selected paths.
    ReadWrite,
}

impl Default for FsPermission {
    fn default() -> Self {
        FsPermission::None
    }
}

// ---------------------------------------------------------------------------
// Package representation
// ---------------------------------------------------------------------------

/// In-memory representation of a validated `.kpioapp` package.
#[derive(Debug, Clone)]
pub struct KpioAppPackage {
    /// Parsed manifest.
    pub manifest: AppManifest,
    /// Raw bytes of the entry WASM module.
    pub wasm_bytes: Vec<u8>,
    /// Additional resource files: path → data.
    pub resources: BTreeMap<String, Vec<u8>>,
    /// Initial data files: path → data.
    pub data_files: BTreeMap<String, Vec<u8>>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors during package parsing / validation.
#[derive(Debug, Clone)]
pub enum PackageError {
    /// ZIP structure is invalid or unsupported.
    InvalidArchive(String),
    /// `manifest.toml` is missing.
    MissingManifest,
    /// `manifest.toml` could not be parsed.
    ManifestParseError(String),
    /// A required manifest field is missing or empty.
    MissingField(String),
    /// Entry WASM file specified in manifest not found inside the archive.
    MissingEntry(String),
    /// WASM magic number (`\0asm`) check failed.
    InvalidWasm,
    /// Package exceeds the total size limit.
    TooLarge { actual: usize, limit: usize },
    /// Generic I/O or processing error.
    IoError(String),
    /// Invalid version format.
    InvalidVersion(String),
    /// Invalid app ID format.
    InvalidAppId(String),
}

/// Validate a manifest's required fields and format.
///
/// Checks:
/// - `id` is non-empty and contains only alphanumeric, `.`, `-`, `_` chars
/// - `name` is non-empty
/// - `version` matches `major.minor.patch` format (digits + dots)
/// - `entry` ends with `.wasm`
pub fn validate_manifest(manifest: &AppManifest) -> Result<(), PackageError> {
    // Validate app ID format
    if manifest.id.is_empty() {
        return Err(PackageError::InvalidAppId(String::from("empty app id")));
    }
    for c in manifest.id.chars() {
        if !c.is_alphanumeric() && c != '.' && c != '-' && c != '_' {
            return Err(PackageError::InvalidAppId(alloc::format!(
                "invalid character '{}' in app id",
                c
            )));
        }
    }

    // Validate name
    if manifest.name.is_empty() {
        return Err(PackageError::MissingField(String::from("name")));
    }

    // Validate version format (digits + dots, at least "N.N.N")
    if manifest.version.is_empty() {
        return Err(PackageError::InvalidVersion(String::from("empty version")));
    }
    let parts: Vec<&str> = manifest.version.split('.').collect();
    if parts.len() < 2 {
        return Err(PackageError::InvalidVersion(alloc::format!(
            "version '{}' needs at least major.minor",
            manifest.version
        )));
    }
    for part in &parts {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return Err(PackageError::InvalidVersion(alloc::format!(
                "invalid version component '{}'",
                part
            )));
        }
    }

    // Validate entry ends with .wasm
    if !manifest.entry.ends_with(".wasm") {
        return Err(PackageError::MissingEntry(alloc::format!(
            "entry '{}' must end with .wasm",
            manifest.entry
        )));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Minimal TOML parser (no_std)
// ---------------------------------------------------------------------------

/// Very small TOML-subset parser supporting `[section]`, `key = "value"`,
/// `key = true/false`.  This is intentionally minimal; real TOML edge-cases
/// (multi-line strings, inline tables, etc.) are *not* handled.
fn parse_minimal_toml(input: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut current_section = String::from("__root__");

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Section header
        if line.starts_with('[') && line.ends_with(']') {
            current_section = String::from(&line[1..line.len() - 1]);
            sections
                .entry(current_section.clone())
                .or_insert_with(BTreeMap::new);
            continue;
        }

        // key = value
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim();
            let value_raw = line[eq_pos + 1..].trim();
            // Strip surrounding quotes
            let value = if (value_raw.starts_with('"') && value_raw.ends_with('"'))
                || (value_raw.starts_with('\'') && value_raw.ends_with('\''))
            {
                &value_raw[1..value_raw.len() - 1]
            } else {
                value_raw
            };
            sections
                .entry(current_section.clone())
                .or_insert_with(BTreeMap::new)
                .insert(String::from(key), String::from(value));
        }
    }

    sections
}

// ---------------------------------------------------------------------------
// Minimal ZIP reader (no_std, store-only)
// ---------------------------------------------------------------------------

/// Read files from a ZIP archive that uses Store (no compression) or
/// Deflate method = 0.  This is a minimal implementation for the kernel; it
/// reads the central directory and extracts entries.
fn read_zip_entries(data: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, PackageError> {
    // Find End-of-Central-Directory record (EOCD).
    // Signature: 0x06054b50
    let eocd_sig: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
    let eocd_pos = find_signature(data, &eocd_sig)
        .ok_or_else(|| PackageError::InvalidArchive(String::from("EOCD not found")))?;

    if eocd_pos + 22 > data.len() {
        return Err(PackageError::InvalidArchive(String::from("EOCD truncated")));
    }

    let cd_size = read_u32_le(data, eocd_pos + 12) as usize;
    let cd_offset = read_u32_le(data, eocd_pos + 16) as usize;
    let total_entries = read_u16_le(data, eocd_pos + 10) as usize;

    if cd_offset + cd_size > data.len() {
        return Err(PackageError::InvalidArchive(String::from(
            "Central directory out of bounds",
        )));
    }

    let mut entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let cd_sig: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
    let mut pos = cd_offset;

    for _ in 0..total_entries {
        if pos + 46 > data.len() {
            break;
        }
        if data[pos..pos + 4] != cd_sig {
            break;
        }

        let compression = read_u16_le(data, pos + 10);
        let compressed_size = read_u32_le(data, pos + 20) as usize;
        let uncompressed_size = read_u32_le(data, pos + 24) as usize;
        let name_len = read_u16_le(data, pos + 28) as usize;
        let extra_len = read_u16_le(data, pos + 30) as usize;
        let comment_len = read_u16_le(data, pos + 32) as usize;
        let local_header_offset = read_u32_le(data, pos + 42) as usize;

        if pos + 46 + name_len > data.len() {
            break;
        }
        let name_bytes = &data[pos + 46..pos + 46 + name_len];
        let name = core::str::from_utf8(name_bytes)
            .unwrap_or("")
            .trim_end_matches('/');

        // Skip directories
        if !name.is_empty() && uncompressed_size > 0 && compression == 0 {
            // Read from local file header
            let lh_sig: [u8; 4] = [0x50, 0x4b, 0x03, 0x04];
            if local_header_offset + 30 <= data.len()
                && data[local_header_offset..local_header_offset + 4] == lh_sig
            {
                let lh_name_len = read_u16_le(data, local_header_offset + 26) as usize;
                let lh_extra_len = read_u16_le(data, local_header_offset + 28) as usize;
                let file_data_offset = local_header_offset + 30 + lh_name_len + lh_extra_len;
                if file_data_offset + uncompressed_size <= data.len() {
                    let file_bytes =
                        data[file_data_offset..file_data_offset + uncompressed_size].to_vec();
                    entries.insert(String::from(name), file_bytes);
                }
            }
        } else if !name.is_empty() && compression == 0 {
            // Could be an empty file or directory — register with empty vec
            if !name.contains('/') || uncompressed_size == 0 {
                entries.insert(String::from(name), Vec::new());
            }
        }

        pos += 46 + name_len + extra_len + comment_len;
    }

    Ok(entries)
}

fn find_signature(data: &[u8], sig: &[u8; 4]) -> Option<usize> {
    if data.len() < 4 {
        return None;
    }
    // Search backwards from the end (EOCD is near the end)
    let start = if data.len() > 65535 + 22 {
        data.len() - 65535 - 22
    } else {
        0
    };
    for i in (start..data.len() - 3).rev() {
        if data[i] == sig[0]
            && data[i + 1] == sig[1]
            && data[i + 2] == sig[2]
            && data[i + 3] == sig[3]
        {
            return Some(i);
        }
    }
    None
}

fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

// ---------------------------------------------------------------------------
// WASM magic check
// ---------------------------------------------------------------------------

const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d]; // \0asm

fn is_valid_wasm(data: &[u8]) -> bool {
    data.len() >= 8 && data[0..4] == WASM_MAGIC
}

// ---------------------------------------------------------------------------
// Package parsing & validation
// ---------------------------------------------------------------------------

/// Default maximum package size in bytes (50 MB).
pub const MAX_PACKAGE_SIZE: usize = 50 * 1024 * 1024;

impl KpioAppPackage {
    /// Parse and validate a `.kpioapp` package from raw bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, PackageError> {
        Self::from_bytes_with_limit(data, MAX_PACKAGE_SIZE)
    }

    /// Parse with a custom size limit.
    pub fn from_bytes_with_limit(data: &[u8], max_size: usize) -> Result<Self, PackageError> {
        // Size check
        if data.len() > max_size {
            return Err(PackageError::TooLarge {
                actual: data.len(),
                limit: max_size,
            });
        }

        // Unpack ZIP
        let entries = read_zip_entries(data)?;

        // Locate and parse manifest
        let manifest_data = entries
            .get("manifest.toml")
            .ok_or(PackageError::MissingManifest)?;
        let manifest_str = core::str::from_utf8(manifest_data)
            .map_err(|_| PackageError::ManifestParseError(String::from("invalid UTF-8")))?;
        let manifest = Self::parse_manifest(manifest_str)?;

        // Locate WASM entry
        let wasm_bytes = entries
            .get(manifest.entry.as_str())
            .ok_or_else(|| PackageError::MissingEntry(manifest.entry.clone()))?
            .clone();

        // Validate WASM magic
        if !is_valid_wasm(&wasm_bytes) {
            return Err(PackageError::InvalidWasm);
        }

        // Collect resources & data
        let mut resources = BTreeMap::new();
        let mut data_files = BTreeMap::new();
        for (path, content) in &entries {
            if path.starts_with("resources/") {
                resources.insert(path.clone(), content.clone());
            } else if path.starts_with("data/") {
                data_files.insert(path.clone(), content.clone());
            }
        }

        Ok(KpioAppPackage {
            manifest,
            wasm_bytes,
            resources,
            data_files,
        })
    }

    /// Parse manifest TOML string into `AppManifest`.
    fn parse_manifest(input: &str) -> Result<AppManifest, PackageError> {
        let sections = parse_minimal_toml(input);

        let app = sections
            .get("app")
            .ok_or_else(|| PackageError::ManifestParseError(String::from("missing [app] section")))?;

        let id = app
            .get("id")
            .cloned()
            .unwrap_or_else(|| {
                // Derive ID from name if not provided
                app.get("name")
                    .map(|n| {
                        let mut id = String::from("app.");
                        for c in n.chars() {
                            if c.is_alphanumeric() {
                                id.push(c.to_ascii_lowercase());
                            } else if c == ' ' {
                                id.push('-');
                            }
                        }
                        id
                    })
                    .unwrap_or_else(|| String::from("app.unknown"))
            });

        let name = app
            .get("name")
            .cloned()
            .ok_or_else(|| PackageError::MissingField(String::from("name")))?;

        if name.is_empty() {
            return Err(PackageError::MissingField(String::from("name")));
        }

        let version = app
            .get("version")
            .cloned()
            .ok_or_else(|| PackageError::MissingField(String::from("version")))?;

        let entry = app
            .get("entry")
            .cloned()
            .unwrap_or_else(|| String::from("app.wasm"));

        let description = app.get("description").cloned();
        let author = app.get("author").cloned();
        let icon = app.get("icon").cloned();
        let min_kpio_version = app.get("min_kpio_version").cloned();

        // Parse permissions
        let perms_section = sections.get("permissions");
        let permissions = Self::parse_permissions(perms_section);

        Ok(AppManifest {
            id,
            name,
            version,
            description,
            author,
            icon,
            entry,
            permissions,
            min_kpio_version,
        })
    }

    /// Parse the `[permissions]` section.
    fn parse_permissions(
        section: Option<&BTreeMap<String, String>>,
    ) -> ManifestPermissions {
        let mut perms = ManifestPermissions::default();
        if let Some(sec) = section {
            if let Some(fs) = sec.get("filesystem") {
                perms.filesystem = match fs.as_str() {
                    "read-only" | "readonly" => FsPermission::ReadOnly,
                    "read-write" | "readwrite" => FsPermission::ReadWrite,
                    _ => FsPermission::None,
                };
            }
            if let Some(v) = sec.get("network") {
                perms.network = v == "true";
            }
            if let Some(v) = sec.get("gui") {
                perms.gui = v == "true";
            }
            if let Some(v) = sec.get("clipboard") {
                perms.clipboard = v == "true";
            }
            if let Some(v) = sec.get("notifications") {
                perms.notifications = v == "true";
            }
        }
        perms
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_parse_minimal_toml_basic() {
        let toml = r#"
[app]
name = "Hello World"
version = "1.0.0"
entry = "app.wasm"

[permissions]
gui = "true"
network = "false"
"#;
        let sections = parse_minimal_toml(toml);
        let app = sections.get("app").unwrap();
        assert_eq!(app.get("name").unwrap(), "Hello World");
        assert_eq!(app.get("version").unwrap(), "1.0.0");
        let perms = sections.get("permissions").unwrap();
        assert_eq!(perms.get("gui").unwrap(), "true");
    }

    #[test]
    fn test_manifest_missing_name() {
        let toml = r#"
[app]
version = "1.0.0"
"#;
        let result = KpioAppPackage::parse_manifest(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_manifest_parse_permissions() {
        let toml = r#"
[app]
name = "Test"
version = "0.1.0"
entry = "app.wasm"

[permissions]
filesystem = "read-only"
gui = "true"
clipboard = "true"
network = "false"
"#;
        let manifest = KpioAppPackage::parse_manifest(toml).unwrap();
        assert_eq!(manifest.permissions.filesystem, FsPermission::ReadOnly);
        assert!(manifest.permissions.gui);
        assert!(manifest.permissions.clipboard);
        assert!(!manifest.permissions.network);
    }

    #[test]
    fn test_wasm_magic_check() {
        assert!(is_valid_wasm(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]));
        assert!(!is_valid_wasm(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]));
        assert!(!is_valid_wasm(&[0x00, 0x61]));
    }

    #[test]
    fn test_too_large_package() {
        let data = vec![0u8; 100];
        let result = KpioAppPackage::from_bytes_with_limit(&data, 50);
        match result {
            Err(PackageError::TooLarge { actual: 100, limit: 50 }) => {}
            _ => panic!("expected TooLarge error"),
        }
    }

    #[test]
    fn test_invalid_archive() {
        let data = vec![0u8; 100];
        let result = KpioAppPackage::from_bytes(&data);
        assert!(matches!(result, Err(PackageError::InvalidArchive(_))));
    }

    // ── Manifest validation tests ───────────────────────────────────

    fn make_test_manifest(id: &str, name: &str, version: &str, entry: &str) -> AppManifest {
        AppManifest {
            id: String::from(id),
            name: String::from(name),
            version: String::from(version),
            description: None,
            author: None,
            icon: None,
            entry: String::from(entry),
            permissions: ManifestPermissions::default(),
            min_kpio_version: None,
        }
    }

    #[test]
    fn test_validate_manifest_valid() {
        let m = make_test_manifest("com.example.app", "My App", "1.0.0", "app.wasm");
        assert!(validate_manifest(&m).is_ok());
    }

    #[test]
    fn test_validate_manifest_empty_id() {
        let m = make_test_manifest("", "App", "1.0.0", "app.wasm");
        assert!(matches!(validate_manifest(&m), Err(PackageError::InvalidAppId(_))));
    }

    #[test]
    fn test_validate_manifest_invalid_id_chars() {
        let m = make_test_manifest("com/example/app", "App", "1.0.0", "app.wasm");
        assert!(matches!(validate_manifest(&m), Err(PackageError::InvalidAppId(_))));
    }

    #[test]
    fn test_validate_manifest_empty_name() {
        let m = make_test_manifest("com.test", "", "1.0.0", "app.wasm");
        assert!(matches!(validate_manifest(&m), Err(PackageError::MissingField(_))));
    }

    #[test]
    fn test_validate_manifest_empty_version() {
        let m = make_test_manifest("com.test", "App", "", "app.wasm");
        assert!(matches!(validate_manifest(&m), Err(PackageError::InvalidVersion(_))));
    }

    #[test]
    fn test_validate_manifest_bad_version_format() {
        let m = make_test_manifest("com.test", "App", "abc", "app.wasm");
        assert!(matches!(validate_manifest(&m), Err(PackageError::InvalidVersion(_))));
    }

    #[test]
    fn test_validate_manifest_single_number_version() {
        let m = make_test_manifest("com.test", "App", "1", "app.wasm");
        assert!(matches!(validate_manifest(&m), Err(PackageError::InvalidVersion(_))));
    }

    #[test]
    fn test_validate_manifest_two_part_version_ok() {
        let m = make_test_manifest("com.test", "App", "1.0", "app.wasm");
        assert!(validate_manifest(&m).is_ok());
    }

    #[test]
    fn test_validate_manifest_bad_entry() {
        let m = make_test_manifest("com.test", "App", "1.0.0", "app.txt");
        assert!(matches!(validate_manifest(&m), Err(PackageError::MissingEntry(_))));
    }

    #[test]
    fn test_validate_manifest_valid_id_with_hyphens_underscores() {
        let m = make_test_manifest("com.my-app_v2", "App", "1.0.0", "app.wasm");
        assert!(validate_manifest(&m).is_ok());
    }
}
