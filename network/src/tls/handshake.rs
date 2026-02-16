//! TLS handshake protocol implementation.
//!
//! This module implements the TLS handshake protocol for both
//! TLS 1.2 and TLS 1.3.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::{CipherSuite, TlsError, TlsVersion};

/// Handshake message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HandshakeType {
    ClientHello = 1,
    ServerHello = 2,
    NewSessionTicket = 4,
    EndOfEarlyData = 5,
    EncryptedExtensions = 8,
    Certificate = 11,
    CertificateRequest = 13,
    CertificateVerify = 15,
    Finished = 20,
    KeyUpdate = 24,
    MessageHash = 254,
}

impl HandshakeType {
    /// Parse from byte.
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            1 => Some(HandshakeType::ClientHello),
            2 => Some(HandshakeType::ServerHello),
            4 => Some(HandshakeType::NewSessionTicket),
            5 => Some(HandshakeType::EndOfEarlyData),
            8 => Some(HandshakeType::EncryptedExtensions),
            11 => Some(HandshakeType::Certificate),
            13 => Some(HandshakeType::CertificateRequest),
            15 => Some(HandshakeType::CertificateVerify),
            20 => Some(HandshakeType::Finished),
            24 => Some(HandshakeType::KeyUpdate),
            254 => Some(HandshakeType::MessageHash),
            _ => None,
        }
    }
}

/// TLS extension type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ExtensionType {
    ServerName = 0,
    MaxFragmentLength = 1,
    StatusRequest = 5,
    SupportedGroups = 10,
    SignatureAlgorithms = 13,
    UseSrtp = 14,
    Heartbeat = 15,
    ApplicationLayerProtocolNegotiation = 16,
    SignedCertificateTimestamp = 18,
    ClientCertificateType = 19,
    ServerCertificateType = 20,
    Padding = 21,
    PreSharedKey = 41,
    EarlyData = 42,
    SupportedVersions = 43,
    Cookie = 44,
    PskKeyExchangeModes = 45,
    CertificateAuthorities = 47,
    OidFilters = 48,
    PostHandshakeAuth = 49,
    SignatureAlgorithmsCert = 50,
    KeyShare = 51,
}

impl ExtensionType {
    /// Parse from u16.
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0 => Some(ExtensionType::ServerName),
            1 => Some(ExtensionType::MaxFragmentLength),
            5 => Some(ExtensionType::StatusRequest),
            10 => Some(ExtensionType::SupportedGroups),
            13 => Some(ExtensionType::SignatureAlgorithms),
            14 => Some(ExtensionType::UseSrtp),
            15 => Some(ExtensionType::Heartbeat),
            16 => Some(ExtensionType::ApplicationLayerProtocolNegotiation),
            18 => Some(ExtensionType::SignedCertificateTimestamp),
            19 => Some(ExtensionType::ClientCertificateType),
            20 => Some(ExtensionType::ServerCertificateType),
            21 => Some(ExtensionType::Padding),
            41 => Some(ExtensionType::PreSharedKey),
            42 => Some(ExtensionType::EarlyData),
            43 => Some(ExtensionType::SupportedVersions),
            44 => Some(ExtensionType::Cookie),
            45 => Some(ExtensionType::PskKeyExchangeModes),
            47 => Some(ExtensionType::CertificateAuthorities),
            48 => Some(ExtensionType::OidFilters),
            49 => Some(ExtensionType::PostHandshakeAuth),
            50 => Some(ExtensionType::SignatureAlgorithmsCert),
            51 => Some(ExtensionType::KeyShare),
            _ => None,
        }
    }
}

/// Named group (elliptic curves and DH groups).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum NamedGroup {
    // ECDH curves
    Secp256r1 = 0x0017,
    Secp384r1 = 0x0018,
    Secp521r1 = 0x0019,
    X25519 = 0x001D,
    X448 = 0x001E,

    // FFDH groups
    Ffdhe2048 = 0x0100,
    Ffdhe3072 = 0x0101,
    Ffdhe4096 = 0x0102,
    Ffdhe6144 = 0x0103,
    Ffdhe8192 = 0x0104,
}

impl NamedGroup {
    /// Parse from u16.
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0017 => Some(NamedGroup::Secp256r1),
            0x0018 => Some(NamedGroup::Secp384r1),
            0x0019 => Some(NamedGroup::Secp521r1),
            0x001D => Some(NamedGroup::X25519),
            0x001E => Some(NamedGroup::X448),
            0x0100 => Some(NamedGroup::Ffdhe2048),
            0x0101 => Some(NamedGroup::Ffdhe3072),
            0x0102 => Some(NamedGroup::Ffdhe4096),
            0x0103 => Some(NamedGroup::Ffdhe6144),
            0x0104 => Some(NamedGroup::Ffdhe8192),
            _ => None,
        }
    }

    /// Get key share size.
    pub fn key_share_size(&self) -> usize {
        match self {
            NamedGroup::Secp256r1 => 65,
            NamedGroup::Secp384r1 => 97,
            NamedGroup::Secp521r1 => 133,
            NamedGroup::X25519 => 32,
            NamedGroup::X448 => 56,
            _ => 0,
        }
    }
}

/// Signature scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum SignatureScheme {
    // RSASSA-PKCS1-v1_5
    RsaPkcs1Sha256 = 0x0401,
    RsaPkcs1Sha384 = 0x0501,
    RsaPkcs1Sha512 = 0x0601,

    // ECDSA
    EcdsaSecp256r1Sha256 = 0x0403,
    EcdsaSecp384r1Sha384 = 0x0503,
    EcdsaSecp521r1Sha512 = 0x0603,

    // RSASSA-PSS
    RsaPssRsaeSha256 = 0x0804,
    RsaPssRsaeSha384 = 0x0805,
    RsaPssRsaeSha512 = 0x0806,

    // EdDSA
    Ed25519 = 0x0807,
    Ed448 = 0x0808,

    // RSASSA-PSS with public key OID rsassa-pss
    RsaPssPssSha256 = 0x0809,
    RsaPssPssSha384 = 0x080A,
    RsaPssPssSha512 = 0x080B,
}

impl SignatureScheme {
    /// Parse from u16.
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0401 => Some(SignatureScheme::RsaPkcs1Sha256),
            0x0501 => Some(SignatureScheme::RsaPkcs1Sha384),
            0x0601 => Some(SignatureScheme::RsaPkcs1Sha512),
            0x0403 => Some(SignatureScheme::EcdsaSecp256r1Sha256),
            0x0503 => Some(SignatureScheme::EcdsaSecp384r1Sha384),
            0x0603 => Some(SignatureScheme::EcdsaSecp521r1Sha512),
            0x0804 => Some(SignatureScheme::RsaPssRsaeSha256),
            0x0805 => Some(SignatureScheme::RsaPssRsaeSha384),
            0x0806 => Some(SignatureScheme::RsaPssRsaeSha512),
            0x0807 => Some(SignatureScheme::Ed25519),
            0x0808 => Some(SignatureScheme::Ed448),
            0x0809 => Some(SignatureScheme::RsaPssPssSha256),
            0x080A => Some(SignatureScheme::RsaPssPssSha384),
            0x080B => Some(SignatureScheme::RsaPssPssSha512),
            _ => None,
        }
    }
}

/// ClientHello message.
#[derive(Debug, Clone)]
pub struct ClientHello {
    /// Legacy version (0x0303 for TLS 1.2).
    pub legacy_version: [u8; 2],
    /// Random bytes.
    pub random: [u8; 32],
    /// Session ID.
    pub session_id: Vec<u8>,
    /// Cipher suites.
    pub cipher_suites: Vec<CipherSuite>,
    /// Compression methods (always [0] for modern TLS).
    pub compression_methods: Vec<u8>,
    /// Extensions.
    pub extensions: Vec<Extension>,
}

impl ClientHello {
    /// Create a new ClientHello.
    pub fn new(random: [u8; 32], cipher_suites: Vec<CipherSuite>) -> Self {
        Self {
            legacy_version: [0x03, 0x03],
            random,
            session_id: Vec::new(),
            cipher_suites,
            compression_methods: vec![0],
            extensions: Vec::new(),
        }
    }

    /// Add an extension.
    pub fn add_extension(&mut self, ext: Extension) {
        self.extensions.push(ext);
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Handshake type
        data.push(HandshakeType::ClientHello as u8);

        // Length placeholder
        let length_pos = data.len();
        data.extend_from_slice(&[0, 0, 0]);

        // Legacy version
        data.extend_from_slice(&self.legacy_version);

        // Random
        data.extend_from_slice(&self.random);

        // Session ID
        data.push(self.session_id.len() as u8);
        data.extend_from_slice(&self.session_id);

        // Cipher suites
        let cipher_bytes: Vec<u8> = self
            .cipher_suites
            .iter()
            .flat_map(|c| c.to_id().to_be_bytes())
            .collect();
        data.extend_from_slice(&(cipher_bytes.len() as u16).to_be_bytes());
        data.extend_from_slice(&cipher_bytes);

        // Compression methods
        data.push(self.compression_methods.len() as u8);
        data.extend_from_slice(&self.compression_methods);

        // Extensions
        let extensions = self.serialize_extensions();
        data.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
        data.extend_from_slice(&extensions);

        // Update length
        let length = data.len() - 4;
        data[length_pos] = ((length >> 16) & 0xFF) as u8;
        data[length_pos + 1] = ((length >> 8) & 0xFF) as u8;
        data[length_pos + 2] = (length & 0xFF) as u8;

        data
    }

    /// Serialize extensions.
    fn serialize_extensions(&self) -> Vec<u8> {
        let mut data = Vec::new();

        for ext in &self.extensions {
            let ext_bytes = ext.to_bytes();
            data.extend_from_slice(&ext_bytes);
        }

        data
    }

    /// Parse from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, TlsError> {
        if data.len() < 38 {
            return Err(TlsError::InvalidRecord);
        }

        // Skip handshake type and length
        let payload = &data[4..];

        let legacy_version = [payload[0], payload[1]];

        let mut random = [0u8; 32];
        random.copy_from_slice(&payload[2..34]);

        let session_id_len = payload[34] as usize;
        let session_id = payload[35..35 + session_id_len].to_vec();

        let mut offset = 35 + session_id_len;

        // Cipher suites
        let cipher_len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
        offset += 2;

        let mut cipher_suites = Vec::new();
        for i in (0..cipher_len).step_by(2) {
            let id = u16::from_be_bytes([payload[offset + i], payload[offset + i + 1]]);
            if let Some(suite) = CipherSuite::from_id(id) {
                cipher_suites.push(suite);
            }
        }
        offset += cipher_len;

        // Compression methods
        let comp_len = payload[offset] as usize;
        let compression_methods = payload[offset + 1..offset + 1 + comp_len].to_vec();
        offset += 1 + comp_len;

        // Extensions
        let extensions = if offset + 2 <= payload.len() {
            let ext_len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
            offset += 2;

            parse_extensions(&payload[offset..offset + ext_len])?
        } else {
            Vec::new()
        };

        Ok(ClientHello {
            legacy_version,
            random,
            session_id,
            cipher_suites,
            compression_methods,
            extensions,
        })
    }
}

/// ServerHello message.
#[derive(Debug, Clone)]
pub struct ServerHello {
    /// Legacy version.
    pub legacy_version: [u8; 2],
    /// Random bytes.
    pub random: [u8; 32],
    /// Session ID.
    pub session_id: Vec<u8>,
    /// Selected cipher suite.
    pub cipher_suite: CipherSuite,
    /// Compression method (always 0).
    pub compression_method: u8,
    /// Extensions.
    pub extensions: Vec<Extension>,
}

impl ServerHello {
    /// Create a new ServerHello.
    pub fn new(random: [u8; 32], cipher_suite: CipherSuite) -> Self {
        Self {
            legacy_version: [0x03, 0x03],
            random,
            session_id: Vec::new(),
            cipher_suite,
            compression_method: 0,
            extensions: Vec::new(),
        }
    }

    /// Add an extension.
    pub fn add_extension(&mut self, ext: Extension) {
        self.extensions.push(ext);
    }

    /// Check if this is a HelloRetryRequest.
    pub fn is_hello_retry_request(&self) -> bool {
        // HelloRetryRequest has a special random value
        const HELLO_RETRY_REQUEST_RANDOM: [u8; 32] = [
            0xCF, 0x21, 0xAD, 0x74, 0xE5, 0x9A, 0x61, 0x11, 0xBE, 0x1D, 0x8C, 0x02, 0x1E, 0x65,
            0xB8, 0x91, 0xC2, 0xA2, 0x11, 0x16, 0x7A, 0xBB, 0x8C, 0x5E, 0x07, 0x9E, 0x09, 0xE2,
            0xC8, 0xA8, 0x33, 0x9C,
        ];
        self.random == HELLO_RETRY_REQUEST_RANDOM
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Handshake type
        data.push(HandshakeType::ServerHello as u8);

        // Length placeholder
        let length_pos = data.len();
        data.extend_from_slice(&[0, 0, 0]);

        // Legacy version
        data.extend_from_slice(&self.legacy_version);

        // Random
        data.extend_from_slice(&self.random);

        // Session ID
        data.push(self.session_id.len() as u8);
        data.extend_from_slice(&self.session_id);

        // Cipher suite
        data.extend_from_slice(&self.cipher_suite.to_id().to_be_bytes());

        // Compression method
        data.push(self.compression_method);

        // Extensions
        let mut ext_data = Vec::new();
        for ext in &self.extensions {
            ext_data.extend_from_slice(&ext.to_bytes());
        }
        data.extend_from_slice(&(ext_data.len() as u16).to_be_bytes());
        data.extend_from_slice(&ext_data);

        // Update length
        let length = data.len() - 4;
        data[length_pos] = ((length >> 16) & 0xFF) as u8;
        data[length_pos + 1] = ((length >> 8) & 0xFF) as u8;
        data[length_pos + 2] = (length & 0xFF) as u8;

        data
    }

    /// Parse from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, TlsError> {
        if data.len() < 38 {
            return Err(TlsError::InvalidRecord);
        }

        let payload = &data[4..];

        let legacy_version = [payload[0], payload[1]];

        let mut random = [0u8; 32];
        random.copy_from_slice(&payload[2..34]);

        let session_id_len = payload[34] as usize;
        let session_id = payload[35..35 + session_id_len].to_vec();

        let mut offset = 35 + session_id_len;

        let cipher_id = u16::from_be_bytes([payload[offset], payload[offset + 1]]);
        let cipher_suite =
            CipherSuite::from_id(cipher_id).ok_or(TlsError::UnsupportedCipherSuite)?;
        offset += 2;

        let compression_method = payload[offset];
        offset += 1;

        let extensions = if offset + 2 <= payload.len() {
            let ext_len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;
            offset += 2;
            parse_extensions(&payload[offset..offset + ext_len])?
        } else {
            Vec::new()
        };

        Ok(ServerHello {
            legacy_version,
            random,
            session_id,
            cipher_suite,
            compression_method,
            extensions,
        })
    }
}

/// TLS extension.
#[derive(Debug, Clone)]
pub struct Extension {
    /// Extension type.
    pub extension_type: u16,
    /// Extension data.
    pub data: Vec<u8>,
}

impl Extension {
    /// Create a new extension.
    pub fn new(extension_type: u16, data: Vec<u8>) -> Self {
        Self {
            extension_type,
            data,
        }
    }

    /// Create Server Name Indication extension.
    pub fn server_name(hostname: &str) -> Self {
        let mut data = Vec::new();

        // Server name list length
        let list_len = hostname.len() + 3;
        data.extend_from_slice(&(list_len as u16).to_be_bytes());

        // Name type (host_name = 0)
        data.push(0);

        // Name length
        data.extend_from_slice(&(hostname.len() as u16).to_be_bytes());

        // Name
        data.extend_from_slice(hostname.as_bytes());

        Self::new(ExtensionType::ServerName as u16, data)
    }

    /// Create Supported Versions extension.
    pub fn supported_versions(versions: &[TlsVersion]) -> Self {
        let mut data = Vec::new();

        // Versions length
        data.push((versions.len() * 2) as u8);

        // Versions
        for v in versions {
            data.extend_from_slice(&v.to_bytes());
        }

        Self::new(ExtensionType::SupportedVersions as u16, data)
    }

    /// Create Supported Groups extension.
    pub fn supported_groups(groups: &[NamedGroup]) -> Self {
        let mut data = Vec::new();

        // Groups length
        data.extend_from_slice(&((groups.len() * 2) as u16).to_be_bytes());

        // Groups
        for g in groups {
            data.extend_from_slice(&(*g as u16).to_be_bytes());
        }

        Self::new(ExtensionType::SupportedGroups as u16, data)
    }

    /// Create Signature Algorithms extension.
    pub fn signature_algorithms(schemes: &[SignatureScheme]) -> Self {
        let mut data = Vec::new();

        // Schemes length
        data.extend_from_slice(&((schemes.len() * 2) as u16).to_be_bytes());

        // Schemes
        for s in schemes {
            data.extend_from_slice(&(*s as u16).to_be_bytes());
        }

        Self::new(ExtensionType::SignatureAlgorithms as u16, data)
    }

    /// Create Key Share extension.
    pub fn key_share(group: NamedGroup, public_key: &[u8]) -> Self {
        let mut data = Vec::new();

        // Key share entries length
        let entry_len = 4 + public_key.len();
        data.extend_from_slice(&(entry_len as u16).to_be_bytes());

        // Named group
        data.extend_from_slice(&(group as u16).to_be_bytes());

        // Key exchange length
        data.extend_from_slice(&(public_key.len() as u16).to_be_bytes());

        // Key exchange
        data.extend_from_slice(public_key);

        Self::new(ExtensionType::KeyShare as u16, data)
    }

    /// Create ALPN extension.
    pub fn alpn(protocols: &[&str]) -> Self {
        let mut data = Vec::new();

        // Protocol names
        let mut protocols_data = Vec::new();
        for proto in protocols {
            protocols_data.push(proto.len() as u8);
            protocols_data.extend_from_slice(proto.as_bytes());
        }

        // Length
        data.extend_from_slice(&(protocols_data.len() as u16).to_be_bytes());
        data.extend_from_slice(&protocols_data);

        Self::new(
            ExtensionType::ApplicationLayerProtocolNegotiation as u16,
            data,
        )
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.extension_type.to_be_bytes());
        data.extend_from_slice(&(self.data.len() as u16).to_be_bytes());
        data.extend_from_slice(&self.data);
        data
    }

    /// Parse from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, TlsError> {
        if data.len() < 4 {
            return Err(TlsError::InvalidRecord);
        }

        let extension_type = u16::from_be_bytes([data[0], data[1]]);
        let length = u16::from_be_bytes([data[2], data[3]]) as usize;

        if data.len() < 4 + length {
            return Err(TlsError::InvalidRecord);
        }

        Ok(Self {
            extension_type,
            data: data[4..4 + length].to_vec(),
        })
    }
}

/// Key share entry.
#[derive(Debug, Clone)]
pub struct KeyShareEntry {
    /// Named group.
    pub group: NamedGroup,
    /// Key exchange data.
    pub key_exchange: Vec<u8>,
}

impl KeyShareEntry {
    /// Create a new key share entry.
    pub fn new(group: NamedGroup, key_exchange: Vec<u8>) -> Self {
        Self {
            group,
            key_exchange,
        }
    }

    /// Parse from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, TlsError> {
        if data.len() < 4 {
            return Err(TlsError::InvalidRecord);
        }

        let group_id = u16::from_be_bytes([data[0], data[1]]);
        let group = NamedGroup::from_u16(group_id).ok_or(TlsError::UnsupportedCipherSuite)?;

        let len = u16::from_be_bytes([data[2], data[3]]) as usize;
        if data.len() < 4 + len {
            return Err(TlsError::InvalidRecord);
        }

        Ok(Self {
            group,
            key_exchange: data[4..4 + len].to_vec(),
        })
    }
}

/// Finished message.
#[derive(Debug, Clone)]
pub struct Finished {
    /// Verify data.
    pub verify_data: Vec<u8>,
}

impl Finished {
    /// Create a new Finished message.
    pub fn new(verify_data: Vec<u8>) -> Self {
        Self { verify_data }
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(HandshakeType::Finished as u8);

        let len = self.verify_data.len();
        data.push(((len >> 16) & 0xFF) as u8);
        data.push(((len >> 8) & 0xFF) as u8);
        data.push((len & 0xFF) as u8);

        data.extend_from_slice(&self.verify_data);
        data
    }
}

/// Parse extensions from bytes.
fn parse_extensions(data: &[u8]) -> Result<Vec<Extension>, TlsError> {
    let mut extensions = Vec::new();
    let mut offset = 0;

    while offset + 4 <= data.len() {
        let ext_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let ext_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
        offset += 4;

        if offset + ext_len > data.len() {
            return Err(TlsError::InvalidRecord);
        }

        extensions.push(Extension {
            extension_type: ext_type,
            data: data[offset..offset + ext_len].to_vec(),
        });

        offset += ext_len;
    }

    Ok(extensions)
}

/// Handshake hash for key derivation.
#[derive(Debug, Clone)]
pub struct HandshakeHash {
    /// Accumulated handshake messages.
    messages: Vec<u8>,
}

impl HandshakeHash {
    /// Create a new handshake hash.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    /// Add a handshake message.
    pub fn add(&mut self, message: &[u8]) {
        self.messages.extend_from_slice(message);
    }

    /// Get the current transcript hash (SHA-256).
    pub fn transcript_hash(&self) -> [u8; 32] {
        // Would compute SHA-256 of messages
        [0u8; 32]
    }

    /// Get the current transcript hash (SHA-384).
    pub fn transcript_hash_384(&self) -> [u8; 48] {
        // Would compute SHA-384 of messages
        [0u8; 48]
    }
}

impl Default for HandshakeHash {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_hello() {
        let random = [0u8; 32];
        let cipher_suites = vec![
            CipherSuite::Tls13Aes256GcmSha384,
            CipherSuite::Tls13Aes128GcmSha256,
        ];

        let mut hello = ClientHello::new(random, cipher_suites);
        hello.add_extension(Extension::server_name("example.com"));

        let bytes = hello.to_bytes();
        assert!(!bytes.is_empty());
        assert_eq!(bytes[0], HandshakeType::ClientHello as u8);
    }

    #[test]
    fn test_extension() {
        let ext = Extension::server_name("example.com");
        let bytes = ext.to_bytes();

        assert_eq!(bytes[0], 0);
        assert_eq!(bytes[1], 0); // ServerName = 0
    }

    #[test]
    fn test_supported_versions() {
        let ext = Extension::supported_versions(&[TlsVersion::Tls13, TlsVersion::Tls12]);
        let bytes = ext.to_bytes();

        assert_eq!(
            u16::from_be_bytes([bytes[0], bytes[1]]),
            ExtensionType::SupportedVersions as u16
        );
    }
}
