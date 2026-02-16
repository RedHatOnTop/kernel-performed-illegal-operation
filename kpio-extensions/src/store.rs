//! Extension Store Integration
//!
//! Handles CRX parsing, extension installation, and update management.

#![allow(dead_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use spin::RwLock;

use crate::manifest::Manifest;
use crate::{Extension, ExtensionId, ExtensionState};

/// CRX file magic number.
pub const CRX_MAGIC: [u8; 4] = [0x43, 0x72, 0x32, 0x34]; // "Cr24"

/// CRX format version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrxVersion {
    /// CRX2 (deprecated).
    Crx2,
    /// CRX3 (current).
    Crx3,
}

/// CRX file header.
#[derive(Debug, Clone)]
pub struct CrxHeader {
    /// Format version.
    pub version: CrxVersion,
    /// Header size (CRX3).
    pub header_size: u32,
    /// Public key (CRX2) or signed header (CRX3).
    pub signed_data: Vec<u8>,
    /// Signature.
    pub signature: Vec<u8>,
    /// ZIP archive offset.
    pub archive_offset: usize,
}

/// Parsed CRX file.
#[derive(Debug, Clone)]
pub struct CrxFile {
    /// Header information.
    pub header: CrxHeader,
    /// Manifest.
    pub manifest: Manifest,
    /// File entries.
    pub files: BTreeMap<String, Vec<u8>>,
    /// Extension ID (derived from public key).
    pub id: ExtensionId,
}

/// CRX parsing error.
#[derive(Debug, Clone)]
pub enum CrxError {
    /// Invalid magic number.
    InvalidMagic,
    /// Unsupported version.
    UnsupportedVersion,
    /// Invalid header.
    InvalidHeader,
    /// Invalid signature.
    InvalidSignature,
    /// Invalid ZIP archive.
    InvalidArchive,
    /// Missing manifest.
    MissingManifest,
    /// Invalid manifest.
    InvalidManifest(String),
}

impl core::fmt::Display for CrxError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidMagic => write!(f, "Invalid CRX magic number"),
            Self::UnsupportedVersion => write!(f, "Unsupported CRX version"),
            Self::InvalidHeader => write!(f, "Invalid CRX header"),
            Self::InvalidSignature => write!(f, "Invalid signature"),
            Self::InvalidArchive => write!(f, "Invalid ZIP archive"),
            Self::MissingManifest => write!(f, "Missing manifest.json"),
            Self::InvalidManifest(e) => write!(f, "Invalid manifest: {}", e),
        }
    }
}

/// Parse a CRX file.
pub fn parse_crx(data: &[u8]) -> Result<CrxFile, CrxError> {
    // Check minimum size
    if data.len() < 16 {
        return Err(CrxError::InvalidHeader);
    }

    // Check magic number
    if data[0..4] != CRX_MAGIC {
        return Err(CrxError::InvalidMagic);
    }

    // Read version
    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    let header = match version {
        2 => parse_crx2_header(data)?,
        3 => parse_crx3_header(data)?,
        _ => return Err(CrxError::UnsupportedVersion),
    };

    // Extract ZIP archive
    let archive_data = &data[header.archive_offset..];
    let files = parse_zip(archive_data)?;

    // Read manifest
    let manifest_data = files
        .get("manifest.json")
        .ok_or(CrxError::MissingManifest)?;
    let manifest_str = core::str::from_utf8(manifest_data)
        .map_err(|_| CrxError::InvalidManifest("Invalid UTF-8".to_string()))?;
    let manifest = crate::manifest::parse_manifest(manifest_str)
        .map_err(|e| CrxError::InvalidManifest(e.to_string()))?;

    // Derive extension ID from public key
    let id = derive_extension_id(&header.signed_data);

    Ok(CrxFile {
        header,
        manifest,
        files,
        id,
    })
}

/// Parse CRX2 header.
fn parse_crx2_header(data: &[u8]) -> Result<CrxHeader, CrxError> {
    if data.len() < 16 {
        return Err(CrxError::InvalidHeader);
    }

    let public_key_length = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
    let signature_length = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;

    let expected_size = 16 + public_key_length + signature_length;
    if data.len() < expected_size {
        return Err(CrxError::InvalidHeader);
    }

    let public_key = data[16..16 + public_key_length].to_vec();
    let signature =
        data[16 + public_key_length..16 + public_key_length + signature_length].to_vec();

    Ok(CrxHeader {
        version: CrxVersion::Crx2,
        header_size: expected_size as u32,
        signed_data: public_key,
        signature,
        archive_offset: expected_size,
    })
}

/// Parse CRX3 header.
fn parse_crx3_header(data: &[u8]) -> Result<CrxHeader, CrxError> {
    if data.len() < 12 {
        return Err(CrxError::InvalidHeader);
    }

    let header_size = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;

    let archive_offset = 12 + header_size;
    if data.len() < archive_offset {
        return Err(CrxError::InvalidHeader);
    }

    // CRX3 uses protobuf for signed header, simplified here
    let signed_header = data[12..archive_offset].to_vec();

    Ok(CrxHeader {
        version: CrxVersion::Crx3,
        header_size: header_size as u32,
        signed_data: signed_header.clone(),
        signature: Vec::new(), // Embedded in signed_header for CRX3
        archive_offset,
    })
}

/// Simple ZIP parser (for CRX contents).
fn parse_zip(data: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, CrxError> {
    let mut files = BTreeMap::new();
    let mut pos = 0;

    // ZIP local file header signature
    const LOCAL_FILE_HEADER: [u8; 4] = [0x50, 0x4b, 0x03, 0x04];

    while pos + 30 <= data.len() {
        // Check for local file header
        if data[pos..pos + 4] != LOCAL_FILE_HEADER {
            // May be central directory or end of archive
            break;
        }

        // Read header fields
        let compressed_size = u32::from_le_bytes([
            data[pos + 18],
            data[pos + 19],
            data[pos + 20],
            data[pos + 21],
        ]) as usize;
        let uncompressed_size = u32::from_le_bytes([
            data[pos + 22],
            data[pos + 23],
            data[pos + 24],
            data[pos + 25],
        ]) as usize;
        let file_name_length = u16::from_le_bytes([data[pos + 26], data[pos + 27]]) as usize;
        let extra_field_length = u16::from_le_bytes([data[pos + 28], data[pos + 29]]) as usize;

        // Read file name
        let name_start = pos + 30;
        let name_end = name_start + file_name_length;
        if name_end > data.len() {
            return Err(CrxError::InvalidArchive);
        }

        let file_name = core::str::from_utf8(&data[name_start..name_end])
            .map_err(|_| CrxError::InvalidArchive)?
            .to_string();

        // Read file data
        let data_start = name_end + extra_field_length;
        let data_end = data_start + compressed_size;
        if data_end > data.len() {
            return Err(CrxError::InvalidArchive);
        }

        // For now, assume STORE compression (no compression)
        let compression_method = u16::from_le_bytes([data[pos + 8], data[pos + 9]]);
        let file_data = if compression_method == 0 {
            // STORE
            data[data_start..data_end].to_vec()
        } else {
            // DEFLATE - would need actual decompression
            // For now, return raw data
            data[data_start..data_end].to_vec()
        };

        if !file_name.ends_with('/') {
            files.insert(file_name, file_data);
        }

        pos = data_end;

        // Handle data descriptor if present
        let flags = u16::from_le_bytes([
            data[pos - compressed_size - extra_field_length - file_name_length - 24],
            data[pos - compressed_size - extra_field_length - file_name_length - 23],
        ]);
        if flags & 0x08 != 0 {
            // Skip data descriptor
            pos += 12;
            if pos + 4 <= data.len() && data[pos..pos + 4] == [0x50, 0x4b, 0x07, 0x08] {
                pos += 4; // Skip optional signature
            }
        }
    }

    Ok(files)
}

/// Derive extension ID from public key.
fn derive_extension_id(public_key: &[u8]) -> ExtensionId {
    // SHA256 hash of public key, first 16 bytes, base16 with a-p alphabet
    // Simplified implementation
    let hash = simple_sha256(public_key);
    let mut id = String::with_capacity(32);

    for byte in &hash[0..16] {
        let lo = byte & 0x0f;
        let hi = (byte >> 4) & 0x0f;
        id.push((b'a' + hi) as char);
        id.push((b'a' + lo) as char);
    }

    ExtensionId::new(&id)
}

/// Simple SHA256 (stub implementation).
fn simple_sha256(data: &[u8]) -> [u8; 32] {
    // Would use actual SHA256
    // For now, simple hash
    let mut result = [0u8; 32];
    for (i, byte) in data.iter().enumerate() {
        result[i % 32] ^= byte;
        result[(i + 1) % 32] = result[(i + 1) % 32].wrapping_add(*byte);
    }
    result
}

/// Extension update status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStatus {
    /// No update available.
    NoUpdate,
    /// Update available.
    UpdateAvailable,
    /// Downloading update.
    Downloading,
    /// Installing update.
    Installing,
    /// Update complete.
    Complete,
    /// Update failed.
    Failed,
}

/// Extension update info.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// Extension ID.
    pub extension_id: ExtensionId,
    /// Current version.
    pub current_version: String,
    /// New version.
    pub new_version: Option<String>,
    /// Update status.
    pub status: UpdateStatus,
    /// Download progress (0-100).
    pub progress: u8,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Extension store configuration.
#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// Chrome Web Store URL.
    pub webstore_url: String,
    /// Update check interval (seconds).
    pub update_interval: u64,
    /// Allow external extensions.
    pub allow_external: bool,
    /// Allow developer mode.
    pub allow_developer_mode: bool,
    /// Block list.
    pub blocked_extensions: Vec<ExtensionId>,
    /// Allow list (if empty, all are allowed).
    pub allowed_extensions: Vec<ExtensionId>,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            webstore_url: "https://clients2.google.com/service/update2/crx".to_string(),
            update_interval: 5 * 60 * 60, // 5 hours
            allow_external: false,
            allow_developer_mode: true,
            blocked_extensions: Vec::new(),
            allowed_extensions: Vec::new(),
        }
    }
}

/// Extension store.
pub struct ExtensionStore {
    /// Configuration.
    config: RwLock<StoreConfig>,
    /// Installed extensions.
    installed: RwLock<BTreeMap<ExtensionId, InstalledExtension>>,
    /// Pending updates.
    pending_updates: RwLock<Vec<UpdateInfo>>,
    /// Last update check timestamp.
    last_update_check: RwLock<u64>,
}

/// Installed extension info.
#[derive(Debug, Clone)]
pub struct InstalledExtension {
    /// Extension ID.
    pub id: ExtensionId,
    /// Manifest.
    pub manifest: Manifest,
    /// Install location.
    pub location: InstallLocation,
    /// Install timestamp.
    pub install_time: u64,
    /// Update timestamp.
    pub update_time: u64,
    /// Files.
    pub files: BTreeMap<String, Vec<u8>>,
}

/// Install location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallLocation {
    /// User installed from web store.
    WebStore,
    /// Installed by policy.
    Policy,
    /// Unpacked development extension.
    Unpacked,
    /// External extension.
    External,
    /// Component extension.
    Component,
}

impl ExtensionStore {
    /// Create a new extension store.
    pub fn new() -> Self {
        Self {
            config: RwLock::new(StoreConfig::default()),
            installed: RwLock::new(BTreeMap::new()),
            pending_updates: RwLock::new(Vec::new()),
            last_update_check: RwLock::new(0),
        }
    }

    /// Install extension from CRX data.
    pub fn install_from_crx(
        &self,
        crx_data: &[u8],
        location: InstallLocation,
    ) -> Result<ExtensionId, CrxError> {
        let crx = parse_crx(crx_data)?;

        // Check if blocked
        let config = self.config.read();
        if config.blocked_extensions.contains(&crx.id) {
            return Err(CrxError::InvalidSignature); // Using as generic error
        }

        // Check allow list
        if !config.allowed_extensions.is_empty() && !config.allowed_extensions.contains(&crx.id) {
            return Err(CrxError::InvalidSignature);
        }
        drop(config);

        let installed = InstalledExtension {
            id: crx.id.clone(),
            manifest: crx.manifest,
            location,
            install_time: 0, // Would use current timestamp
            update_time: 0,
            files: crx.files,
        };

        self.installed.write().insert(crx.id.clone(), installed);

        Ok(crx.id)
    }

    /// Install unpacked extension.
    pub fn install_unpacked(
        &self,
        manifest: Manifest,
        files: BTreeMap<String, Vec<u8>>,
    ) -> Result<ExtensionId, CrxError> {
        let config = self.config.read();
        if !config.allow_developer_mode {
            return Err(CrxError::InvalidSignature);
        }
        drop(config);

        // Generate ID from manifest name
        let id = derive_extension_id(manifest.name.as_bytes());

        let installed = InstalledExtension {
            id: id.clone(),
            manifest,
            location: InstallLocation::Unpacked,
            install_time: 0,
            update_time: 0,
            files,
        };

        self.installed.write().insert(id.clone(), installed);

        Ok(id)
    }

    /// Uninstall extension.
    pub fn uninstall(&self, id: &ExtensionId) -> bool {
        self.installed.write().remove(id).is_some()
    }

    /// Get installed extension.
    pub fn get(&self, id: &ExtensionId) -> Option<InstalledExtension> {
        self.installed.read().get(id).cloned()
    }

    /// List installed extensions.
    pub fn list(&self) -> Vec<ExtensionId> {
        self.installed.read().keys().cloned().collect()
    }

    /// Get file from extension.
    pub fn get_file(&self, id: &ExtensionId, path: &str) -> Option<Vec<u8>> {
        self.installed
            .read()
            .get(id)
            .and_then(|ext| ext.files.get(path).cloned())
    }

    /// Check for updates.
    pub fn check_updates(&self) -> Vec<UpdateInfo> {
        let installed = self.installed.read();
        let mut updates = Vec::new();

        for (id, ext) in installed.iter() {
            // Would make HTTP request to check for updates
            updates.push(UpdateInfo {
                extension_id: id.clone(),
                current_version: ext.manifest.version.clone(),
                new_version: None,
                status: UpdateStatus::NoUpdate,
                progress: 0,
                error: None,
            });
        }

        *self.last_update_check.write() = 0; // Would use current timestamp
        *self.pending_updates.write() = updates.clone();

        updates
    }

    /// Update extension.
    pub fn update(&self, id: &ExtensionId, crx_data: &[u8]) -> Result<(), CrxError> {
        let crx = parse_crx(crx_data)?;

        if crx.id != *id {
            return Err(CrxError::InvalidSignature);
        }

        let mut installed = self.installed.write();
        if let Some(ext) = installed.get_mut(id) {
            ext.manifest = crx.manifest;
            ext.files = crx.files;
            ext.update_time = 0; // Would use current timestamp
            Ok(())
        } else {
            Err(CrxError::InvalidArchive)
        }
    }

    /// Get update URL for extension.
    pub fn get_update_url(&self, id: &ExtensionId) -> Option<String> {
        self.installed
            .read()
            .get(id)
            .and_then(|ext| ext.manifest.update_url.clone())
    }

    /// Configure store.
    pub fn configure(&self, config: StoreConfig) {
        *self.config.write() = config;
    }
}

impl Default for ExtensionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ManifestVersion;

    #[test]
    fn test_extension_id_derivation() {
        let key = b"test public key";
        let id = derive_extension_id(key);
        assert_eq!(id.as_str().len(), 32);
    }

    #[test]
    fn test_store_unpacked_install() {
        let store = ExtensionStore::new();

        let manifest = Manifest {
            manifest_version: ManifestVersion::V3,
            name: "Test Extension".to_string(),
            version: "1.0.0".to_string(),
            ..Default::default()
        };

        let mut files = BTreeMap::new();
        files.insert("manifest.json".to_string(), b"{}".to_vec());

        let id = store.install_unpacked(manifest, files).unwrap();

        assert!(store.get(&id).is_some());
        assert_eq!(store.list().len(), 1);

        assert!(store.uninstall(&id));
        assert!(store.get(&id).is_none());
    }
}
