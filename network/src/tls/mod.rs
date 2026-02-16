//! TLS/SSL implementation for secure communication.
//!
//! This module provides TLS 1.2/1.3 support with certificate validation,
//! cipher negotiation, and secure key exchange.

#![allow(dead_code)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

pub mod certificate;
pub mod handshake;
pub mod record;

pub use certificate::*;
pub use handshake::*;
pub use record::*;

/// TLS version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsVersion {
    /// TLS 1.0 (deprecated).
    Tls10,
    /// TLS 1.1 (deprecated).
    Tls11,
    /// TLS 1.2.
    Tls12,
    /// TLS 1.3.
    Tls13,
}

impl TlsVersion {
    /// Get the protocol version bytes.
    pub fn to_bytes(&self) -> [u8; 2] {
        match self {
            TlsVersion::Tls10 => [0x03, 0x01],
            TlsVersion::Tls11 => [0x03, 0x02],
            TlsVersion::Tls12 => [0x03, 0x03],
            TlsVersion::Tls13 => [0x03, 0x04],
        }
    }

    /// Parse from bytes.
    pub fn from_bytes(bytes: [u8; 2]) -> Option<Self> {
        match bytes {
            [0x03, 0x01] => Some(TlsVersion::Tls10),
            [0x03, 0x02] => Some(TlsVersion::Tls11),
            [0x03, 0x03] => Some(TlsVersion::Tls12),
            [0x03, 0x04] => Some(TlsVersion::Tls13),
            _ => None,
        }
    }
}

/// TLS cipher suite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherSuite {
    // TLS 1.3 cipher suites
    /// TLS_AES_128_GCM_SHA256.
    Tls13Aes128GcmSha256,
    /// TLS_AES_256_GCM_SHA384.
    Tls13Aes256GcmSha384,
    /// TLS_CHACHA20_POLY1305_SHA256.
    Tls13Chacha20Poly1305Sha256,

    // TLS 1.2 cipher suites
    /// TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256.
    EcdheRsaAes128GcmSha256,
    /// TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384.
    EcdheRsaAes256GcmSha384,
    /// TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256.
    EcdheEcdsaAes128GcmSha256,
    /// TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384.
    EcdheEcdsaAes256GcmSha384,
    /// TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256.
    EcdheRsaChacha20Poly1305,
    /// TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256.
    EcdheEcdsaChacha20Poly1305,
}

impl CipherSuite {
    /// Get cipher suite ID.
    pub fn to_id(&self) -> u16 {
        match self {
            CipherSuite::Tls13Aes128GcmSha256 => 0x1301,
            CipherSuite::Tls13Aes256GcmSha384 => 0x1302,
            CipherSuite::Tls13Chacha20Poly1305Sha256 => 0x1303,
            CipherSuite::EcdheRsaAes128GcmSha256 => 0xC02F,
            CipherSuite::EcdheRsaAes256GcmSha384 => 0xC030,
            CipherSuite::EcdheEcdsaAes128GcmSha256 => 0xC02B,
            CipherSuite::EcdheEcdsaAes256GcmSha384 => 0xC02C,
            CipherSuite::EcdheRsaChacha20Poly1305 => 0xCCA8,
            CipherSuite::EcdheEcdsaChacha20Poly1305 => 0xCCA9,
        }
    }

    /// Parse from ID.
    pub fn from_id(id: u16) -> Option<Self> {
        match id {
            0x1301 => Some(CipherSuite::Tls13Aes128GcmSha256),
            0x1302 => Some(CipherSuite::Tls13Aes256GcmSha384),
            0x1303 => Some(CipherSuite::Tls13Chacha20Poly1305Sha256),
            0xC02F => Some(CipherSuite::EcdheRsaAes128GcmSha256),
            0xC030 => Some(CipherSuite::EcdheRsaAes256GcmSha384),
            0xC02B => Some(CipherSuite::EcdheEcdsaAes128GcmSha256),
            0xC02C => Some(CipherSuite::EcdheEcdsaAes256GcmSha384),
            0xCCA8 => Some(CipherSuite::EcdheRsaChacha20Poly1305),
            0xCCA9 => Some(CipherSuite::EcdheEcdsaChacha20Poly1305),
            _ => None,
        }
    }

    /// Check if this is a TLS 1.3 cipher suite.
    pub fn is_tls13(&self) -> bool {
        matches!(
            self,
            CipherSuite::Tls13Aes128GcmSha256
                | CipherSuite::Tls13Aes256GcmSha384
                | CipherSuite::Tls13Chacha20Poly1305Sha256
        )
    }

    /// Get the key length in bytes.
    pub fn key_length(&self) -> usize {
        match self {
            CipherSuite::Tls13Aes128GcmSha256
            | CipherSuite::EcdheRsaAes128GcmSha256
            | CipherSuite::EcdheEcdsaAes128GcmSha256 => 16,
            CipherSuite::Tls13Aes256GcmSha384
            | CipherSuite::EcdheRsaAes256GcmSha384
            | CipherSuite::EcdheEcdsaAes256GcmSha384 => 32,
            CipherSuite::Tls13Chacha20Poly1305Sha256
            | CipherSuite::EcdheRsaChacha20Poly1305
            | CipherSuite::EcdheEcdsaChacha20Poly1305 => 32,
        }
    }

    /// Get the MAC length in bytes.
    pub fn mac_length(&self) -> usize {
        match self {
            CipherSuite::Tls13Aes128GcmSha256
            | CipherSuite::EcdheRsaAes128GcmSha256
            | CipherSuite::EcdheEcdsaAes128GcmSha256 => 16,
            CipherSuite::Tls13Aes256GcmSha384
            | CipherSuite::EcdheRsaAes256GcmSha384
            | CipherSuite::EcdheEcdsaAes256GcmSha384 => 16,
            CipherSuite::Tls13Chacha20Poly1305Sha256
            | CipherSuite::EcdheRsaChacha20Poly1305
            | CipherSuite::EcdheEcdsaChacha20Poly1305 => 16,
        }
    }
}

/// TLS error types.
#[derive(Debug, Clone)]
pub enum TlsError {
    /// Handshake failure.
    HandshakeFailure,
    /// Certificate error.
    CertificateError(CertificateError),
    /// Protocol version not supported.
    UnsupportedVersion,
    /// Cipher suite not supported.
    UnsupportedCipherSuite,
    /// Invalid record.
    InvalidRecord,
    /// Decryption error.
    DecryptionError,
    /// Authentication error.
    AuthenticationError,
    /// Connection closed.
    ConnectionClosed,
    /// Alert received.
    AlertReceived(AlertDescription),
    /// IO error.
    IoError,
    /// Bad certificate.
    BadCertificate,
    /// Certificate expired.
    CertificateExpired,
    /// Certificate revoked.
    CertificateRevoked,
    /// Unknown CA.
    UnknownCa,
    /// Hostname mismatch.
    HostnameMismatch,
}

impl fmt::Display for TlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TlsError::HandshakeFailure => write!(f, "Handshake failure"),
            TlsError::CertificateError(e) => write!(f, "Certificate error: {:?}", e),
            TlsError::UnsupportedVersion => write!(f, "Unsupported TLS version"),
            TlsError::UnsupportedCipherSuite => write!(f, "Unsupported cipher suite"),
            TlsError::InvalidRecord => write!(f, "Invalid record"),
            TlsError::DecryptionError => write!(f, "Decryption error"),
            TlsError::AuthenticationError => write!(f, "Authentication error"),
            TlsError::ConnectionClosed => write!(f, "Connection closed"),
            TlsError::AlertReceived(desc) => write!(f, "Alert received: {:?}", desc),
            TlsError::IoError => write!(f, "I/O error"),
            TlsError::BadCertificate => write!(f, "Bad certificate"),
            TlsError::CertificateExpired => write!(f, "Certificate expired"),
            TlsError::CertificateRevoked => write!(f, "Certificate revoked"),
            TlsError::UnknownCa => write!(f, "Unknown CA"),
            TlsError::HostnameMismatch => write!(f, "Hostname mismatch"),
        }
    }
}

/// TLS alert description.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AlertDescription {
    CloseNotify = 0,
    UnexpectedMessage = 10,
    BadRecordMac = 20,
    RecordOverflow = 22,
    HandshakeFailure = 40,
    BadCertificate = 42,
    UnsupportedCertificate = 43,
    CertificateRevoked = 44,
    CertificateExpired = 45,
    CertificateUnknown = 46,
    IllegalParameter = 47,
    UnknownCa = 48,
    AccessDenied = 49,
    DecodeError = 50,
    DecryptError = 51,
    ProtocolVersion = 70,
    InsufficientSecurity = 71,
    InternalError = 80,
    InappropriateFallback = 86,
    UserCanceled = 90,
    MissingExtension = 109,
    UnsupportedExtension = 110,
    UnrecognizedName = 112,
    BadCertificateStatusResponse = 113,
    UnknownPskIdentity = 115,
    CertificateRequired = 116,
    NoApplicationProtocol = 120,
}

impl AlertDescription {
    /// Parse from byte.
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(AlertDescription::CloseNotify),
            10 => Some(AlertDescription::UnexpectedMessage),
            20 => Some(AlertDescription::BadRecordMac),
            22 => Some(AlertDescription::RecordOverflow),
            40 => Some(AlertDescription::HandshakeFailure),
            42 => Some(AlertDescription::BadCertificate),
            43 => Some(AlertDescription::UnsupportedCertificate),
            44 => Some(AlertDescription::CertificateRevoked),
            45 => Some(AlertDescription::CertificateExpired),
            46 => Some(AlertDescription::CertificateUnknown),
            47 => Some(AlertDescription::IllegalParameter),
            48 => Some(AlertDescription::UnknownCa),
            49 => Some(AlertDescription::AccessDenied),
            50 => Some(AlertDescription::DecodeError),
            51 => Some(AlertDescription::DecryptError),
            70 => Some(AlertDescription::ProtocolVersion),
            71 => Some(AlertDescription::InsufficientSecurity),
            80 => Some(AlertDescription::InternalError),
            86 => Some(AlertDescription::InappropriateFallback),
            90 => Some(AlertDescription::UserCanceled),
            109 => Some(AlertDescription::MissingExtension),
            110 => Some(AlertDescription::UnsupportedExtension),
            112 => Some(AlertDescription::UnrecognizedName),
            113 => Some(AlertDescription::BadCertificateStatusResponse),
            115 => Some(AlertDescription::UnknownPskIdentity),
            116 => Some(AlertDescription::CertificateRequired),
            120 => Some(AlertDescription::NoApplicationProtocol),
            _ => None,
        }
    }
}

/// TLS connection configuration.
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Minimum TLS version to accept.
    pub min_version: TlsVersion,
    /// Maximum TLS version to use.
    pub max_version: TlsVersion,
    /// Allowed cipher suites (in order of preference).
    pub cipher_suites: Vec<CipherSuite>,
    /// Server name for SNI.
    pub server_name: Option<String>,
    /// Whether to verify certificates.
    pub verify_certificates: bool,
    /// Whether to require client certificates.
    pub require_client_cert: bool,
    /// ALPN protocols.
    pub alpn_protocols: Vec<String>,
    /// Session resumption enabled.
    pub session_resumption: bool,
    /// Maximum session cache size.
    pub session_cache_size: usize,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            min_version: TlsVersion::Tls12,
            max_version: TlsVersion::Tls13,
            cipher_suites: vec![
                CipherSuite::Tls13Aes256GcmSha384,
                CipherSuite::Tls13Aes128GcmSha256,
                CipherSuite::Tls13Chacha20Poly1305Sha256,
                CipherSuite::EcdheEcdsaAes256GcmSha384,
                CipherSuite::EcdheRsaAes256GcmSha384,
                CipherSuite::EcdheEcdsaAes128GcmSha256,
                CipherSuite::EcdheRsaAes128GcmSha256,
            ],
            server_name: None,
            verify_certificates: true,
            require_client_cert: false,
            alpn_protocols: Vec::new(),
            session_resumption: true,
            session_cache_size: 256,
        }
    }
}

/// TLS session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsState {
    /// Initial state.
    Initial,
    /// Client hello sent.
    ClientHelloSent,
    /// Server hello received.
    ServerHelloReceived,
    /// Certificate received.
    CertificateReceived,
    /// Key exchange done.
    KeyExchangeDone,
    /// Handshake complete.
    Connected,
    /// Connection closing.
    Closing,
    /// Connection closed.
    Closed,
    /// Error state.
    Error,
}

/// TLS session.
pub struct TlsSession {
    /// Configuration.
    config: TlsConfig,
    /// Current state.
    state: TlsState,
    /// Negotiated TLS version.
    version: Option<TlsVersion>,
    /// Negotiated cipher suite.
    cipher_suite: Option<CipherSuite>,
    /// Is this a client session.
    is_client: bool,
    /// Client random (32 bytes).
    client_random: [u8; 32],
    /// Server random (32 bytes).
    server_random: [u8; 32],
    /// Master secret (48 bytes).
    master_secret: [u8; 48],
    /// Session ID.
    session_id: Vec<u8>,
    /// Verified peer certificate chain.
    peer_certificates: Vec<Certificate>,
    /// Selected ALPN protocol.
    alpn_protocol: Option<String>,
    /// Sequence number for sending.
    send_seq: u64,
    /// Sequence number for receiving.
    recv_seq: u64,
    /// Traffic secrets for TLS 1.3.
    client_traffic_secret: Vec<u8>,
    /// Traffic secrets for TLS 1.3.
    server_traffic_secret: Vec<u8>,
}

impl TlsSession {
    /// Create a new client session.
    pub fn new_client(config: TlsConfig) -> Self {
        let mut client_random = [0u8; 32];
        // In a real implementation, use a CSPRNG
        for (i, byte) in client_random.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(17).wrapping_add(42);
        }

        Self {
            config,
            state: TlsState::Initial,
            version: None,
            cipher_suite: None,
            is_client: true,
            client_random,
            server_random: [0u8; 32],
            master_secret: [0u8; 48],
            session_id: Vec::new(),
            peer_certificates: Vec::new(),
            alpn_protocol: None,
            send_seq: 0,
            recv_seq: 0,
            client_traffic_secret: Vec::new(),
            server_traffic_secret: Vec::new(),
        }
    }

    /// Create a new server session.
    pub fn new_server(config: TlsConfig) -> Self {
        let mut server_random = [0u8; 32];
        for (i, byte) in server_random.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(23).wrapping_add(17);
        }

        Self {
            config,
            state: TlsState::Initial,
            version: None,
            cipher_suite: None,
            is_client: false,
            client_random: [0u8; 32],
            server_random,
            master_secret: [0u8; 48],
            session_id: Vec::new(),
            peer_certificates: Vec::new(),
            alpn_protocol: None,
            send_seq: 0,
            recv_seq: 0,
            client_traffic_secret: Vec::new(),
            server_traffic_secret: Vec::new(),
        }
    }

    /// Get current state.
    pub fn state(&self) -> TlsState {
        self.state
    }

    /// Check if handshake is complete.
    pub fn is_connected(&self) -> bool {
        self.state == TlsState::Connected
    }

    /// Get negotiated version.
    pub fn version(&self) -> Option<TlsVersion> {
        self.version
    }

    /// Get negotiated cipher suite.
    pub fn cipher_suite(&self) -> Option<CipherSuite> {
        self.cipher_suite
    }

    /// Get selected ALPN protocol.
    pub fn alpn_protocol(&self) -> Option<&str> {
        self.alpn_protocol.as_deref()
    }

    /// Get peer certificates.
    pub fn peer_certificates(&self) -> &[Certificate] {
        &self.peer_certificates
    }

    /// Process handshake data.
    pub fn process_handshake(&mut self, data: &[u8]) -> Result<Vec<u8>, TlsError> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        // Parse record layer
        if data.len() < 5 {
            return Err(TlsError::InvalidRecord);
        }

        let content_type = data[0];
        let _version = [data[1], data[2]];
        let length = u16::from_be_bytes([data[3], data[4]]) as usize;

        if data.len() < 5 + length {
            return Err(TlsError::InvalidRecord);
        }

        let payload = &data[5..5 + length];

        match content_type {
            22 => self.process_handshake_message(payload),
            21 => self.process_alert(payload),
            23 => {
                // Application data - should not happen during handshake
                Err(TlsError::HandshakeFailure)
            }
            _ => Err(TlsError::InvalidRecord),
        }
    }

    /// Process handshake message.
    fn process_handshake_message(&mut self, data: &[u8]) -> Result<Vec<u8>, TlsError> {
        if data.is_empty() {
            return Err(TlsError::InvalidRecord);
        }

        let msg_type = data[0];

        match (self.is_client, self.state, msg_type) {
            // Client waiting for ServerHello
            (true, TlsState::ClientHelloSent, 2) => {
                self.process_server_hello(data)?;
                self.state = TlsState::ServerHelloReceived;
                Ok(Vec::new())
            }
            // Client waiting for Certificate
            (true, TlsState::ServerHelloReceived, 11) => {
                self.process_certificate(data)?;
                self.state = TlsState::CertificateReceived;
                Ok(Vec::new())
            }
            // Server waiting for ClientHello
            (false, TlsState::Initial, 1) => {
                self.process_client_hello(data)?;
                self.build_server_response()
            }
            _ => {
                // For now, handle other cases
                Ok(Vec::new())
            }
        }
    }

    /// Process alert.
    fn process_alert(&mut self, data: &[u8]) -> Result<Vec<u8>, TlsError> {
        if data.len() < 2 {
            return Err(TlsError::InvalidRecord);
        }

        let level = data[0];
        let description = AlertDescription::from_byte(data[1]);

        if level == 2 {
            // Fatal alert
            self.state = TlsState::Error;
            if let Some(desc) = description {
                return Err(TlsError::AlertReceived(desc));
            }
        }

        if description == Some(AlertDescription::CloseNotify) {
            self.state = TlsState::Closing;
        }

        Ok(Vec::new())
    }

    /// Build ClientHello message.
    pub fn build_client_hello(&mut self) -> Result<Vec<u8>, TlsError> {
        let mut hello = Vec::new();

        // Handshake type: ClientHello
        hello.push(1);

        // Length placeholder (3 bytes)
        let length_pos = hello.len();
        hello.extend_from_slice(&[0, 0, 0]);

        // Legacy version (TLS 1.2)
        hello.extend_from_slice(&[0x03, 0x03]);

        // Client random
        hello.extend_from_slice(&self.client_random);

        // Session ID (empty for now)
        hello.push(0);

        // Cipher suites
        let cipher_bytes: Vec<u8> = self
            .config
            .cipher_suites
            .iter()
            .flat_map(|c| c.to_id().to_be_bytes())
            .collect();
        hello.extend_from_slice(&(cipher_bytes.len() as u16).to_be_bytes());
        hello.extend_from_slice(&cipher_bytes);

        // Compression methods (null compression only)
        hello.push(1);
        hello.push(0);

        // Extensions
        let extensions = self.build_extensions();
        hello.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
        hello.extend_from_slice(&extensions);

        // Update length
        let length = hello.len() - 4;
        hello[length_pos] = ((length >> 16) & 0xFF) as u8;
        hello[length_pos + 1] = ((length >> 8) & 0xFF) as u8;
        hello[length_pos + 2] = (length & 0xFF) as u8;

        // Wrap in record
        let record = self.wrap_record(22, &hello);

        self.state = TlsState::ClientHelloSent;

        Ok(record)
    }

    /// Build extensions.
    fn build_extensions(&self) -> Vec<u8> {
        let mut extensions = Vec::new();

        // SNI extension
        if let Some(ref server_name) = self.config.server_name {
            extensions.extend_from_slice(&[0x00, 0x00]); // Extension type
            let sni_data_len = server_name.len() + 5;
            extensions.extend_from_slice(&(sni_data_len as u16).to_be_bytes());
            extensions.extend_from_slice(&((server_name.len() + 3) as u16).to_be_bytes());
            extensions.push(0); // Host name type
            extensions.extend_from_slice(&(server_name.len() as u16).to_be_bytes());
            extensions.extend_from_slice(server_name.as_bytes());
        }

        // Supported versions extension
        extensions.extend_from_slice(&[0x00, 0x2B]); // Extension type
        let versions: Vec<u8> = if self.config.max_version == TlsVersion::Tls13 {
            vec![0x03, 0x04, 0x03, 0x03] // TLS 1.3 and 1.2
        } else {
            vec![0x03, 0x03] // TLS 1.2 only
        };
        extensions.extend_from_slice(&((versions.len() + 1) as u16).to_be_bytes());
        extensions.push(versions.len() as u8);
        extensions.extend_from_slice(&versions);

        // Signature algorithms extension
        extensions.extend_from_slice(&[0x00, 0x0D]); // Extension type
        let sig_algs: [u8; 8] = [
            0x04, 0x03, // ECDSA-SECP256r1-SHA256
            0x05, 0x03, // ECDSA-SECP384r1-SHA384
            0x04, 0x01, // RSA-PKCS1-SHA256
            0x05, 0x01, // RSA-PKCS1-SHA384
        ];
        extensions.extend_from_slice(&((sig_algs.len() + 2) as u16).to_be_bytes());
        extensions.extend_from_slice(&(sig_algs.len() as u16).to_be_bytes());
        extensions.extend_from_slice(&sig_algs);

        // Supported groups extension
        extensions.extend_from_slice(&[0x00, 0x0A]); // Extension type
        let groups: [u8; 6] = [
            0x00, 0x1D, // x25519
            0x00, 0x17, // secp256r1
            0x00, 0x18, // secp384r1
        ];
        extensions.extend_from_slice(&((groups.len() + 2) as u16).to_be_bytes());
        extensions.extend_from_slice(&(groups.len() as u16).to_be_bytes());
        extensions.extend_from_slice(&groups);

        // ALPN extension
        if !self.config.alpn_protocols.is_empty() {
            extensions.extend_from_slice(&[0x00, 0x10]); // Extension type
            let mut alpn_data = Vec::new();
            for proto in &self.config.alpn_protocols {
                alpn_data.push(proto.len() as u8);
                alpn_data.extend_from_slice(proto.as_bytes());
            }
            extensions.extend_from_slice(&((alpn_data.len() + 2) as u16).to_be_bytes());
            extensions.extend_from_slice(&(alpn_data.len() as u16).to_be_bytes());
            extensions.extend_from_slice(&alpn_data);
        }

        extensions
    }

    /// Wrap data in a TLS record.
    fn wrap_record(&self, content_type: u8, data: &[u8]) -> Vec<u8> {
        let mut record = Vec::with_capacity(5 + data.len());
        record.push(content_type);
        record.extend_from_slice(&[0x03, 0x03]); // TLS 1.2
        record.extend_from_slice(&(data.len() as u16).to_be_bytes());
        record.extend_from_slice(data);
        record
    }

    /// Process ServerHello.
    fn process_server_hello(&mut self, data: &[u8]) -> Result<(), TlsError> {
        if data.len() < 38 {
            return Err(TlsError::InvalidRecord);
        }

        // Skip handshake header (4 bytes)
        let payload = &data[4..];

        // Legacy version
        let _version = [payload[0], payload[1]];

        // Server random
        self.server_random.copy_from_slice(&payload[2..34]);

        // Session ID
        let session_id_len = payload[34] as usize;
        if payload.len() < 35 + session_id_len + 3 {
            return Err(TlsError::InvalidRecord);
        }

        self.session_id = payload[35..35 + session_id_len].to_vec();

        // Cipher suite
        let offset = 35 + session_id_len;
        let cipher_id = u16::from_be_bytes([payload[offset], payload[offset + 1]]);
        self.cipher_suite = CipherSuite::from_id(cipher_id);

        if self.cipher_suite.is_none() {
            return Err(TlsError::UnsupportedCipherSuite);
        }

        // Determine version from extensions or cipher suite
        if self.cipher_suite.unwrap().is_tls13() {
            self.version = Some(TlsVersion::Tls13);
        } else {
            self.version = Some(TlsVersion::Tls12);
        }

        Ok(())
    }

    /// Process Certificate message.
    fn process_certificate(&mut self, data: &[u8]) -> Result<(), TlsError> {
        if data.len() < 7 {
            return Err(TlsError::InvalidRecord);
        }

        // Skip handshake header (4 bytes)
        let payload = &data[4..];

        // Certificate list length (3 bytes)
        let list_len =
            ((payload[0] as usize) << 16) | ((payload[1] as usize) << 8) | (payload[2] as usize);

        if payload.len() < 3 + list_len {
            return Err(TlsError::InvalidRecord);
        }

        let mut offset = 3;
        while offset < 3 + list_len {
            // Certificate length (3 bytes)
            let cert_len = ((payload[offset] as usize) << 16)
                | ((payload[offset + 1] as usize) << 8)
                | (payload[offset + 2] as usize);
            offset += 3;

            if offset + cert_len > payload.len() {
                return Err(TlsError::InvalidRecord);
            }

            // Parse certificate
            let cert_data = &payload[offset..offset + cert_len];
            let cert =
                Certificate::from_der(cert_data).map_err(|e| TlsError::CertificateError(e))?;

            self.peer_certificates.push(cert);
            offset += cert_len;
        }

        // Verify certificate chain
        if self.config.verify_certificates {
            self.verify_certificate_chain()?;
        }

        Ok(())
    }

    /// Process ClientHello.
    fn process_client_hello(&mut self, data: &[u8]) -> Result<(), TlsError> {
        if data.len() < 38 {
            return Err(TlsError::InvalidRecord);
        }

        // Skip handshake header
        let payload = &data[4..];

        // Client random
        self.client_random.copy_from_slice(&payload[2..34]);

        // Session ID
        let session_id_len = payload[34] as usize;
        self.session_id = payload[35..35 + session_id_len].to_vec();

        // Find cipher suite
        let offset = 35 + session_id_len;
        let cipher_len = u16::from_be_bytes([payload[offset], payload[offset + 1]]) as usize;

        for i in (0..cipher_len).step_by(2) {
            let cipher_id =
                u16::from_be_bytes([payload[offset + 2 + i], payload[offset + 2 + i + 1]]);

            if let Some(suite) = CipherSuite::from_id(cipher_id) {
                if self.config.cipher_suites.contains(&suite) {
                    self.cipher_suite = Some(suite);
                    break;
                }
            }
        }

        if self.cipher_suite.is_none() {
            return Err(TlsError::UnsupportedCipherSuite);
        }

        // Determine version
        if self.cipher_suite.unwrap().is_tls13() {
            self.version = Some(TlsVersion::Tls13);
        } else {
            self.version = Some(TlsVersion::Tls12);
        }

        Ok(())
    }

    /// Build server response.
    fn build_server_response(&mut self) -> Result<Vec<u8>, TlsError> {
        let mut response = Vec::new();

        // ServerHello
        let server_hello = self.build_server_hello()?;
        response.extend_from_slice(&self.wrap_record(22, &server_hello));

        // Certificate (if we have one)
        // For now, skip certificate

        self.state = TlsState::ServerHelloReceived;

        Ok(response)
    }

    /// Build ServerHello.
    fn build_server_hello(&self) -> Result<Vec<u8>, TlsError> {
        let mut hello = Vec::new();

        // Handshake type: ServerHello
        hello.push(2);

        // Length placeholder
        let length_pos = hello.len();
        hello.extend_from_slice(&[0, 0, 0]);

        // Legacy version
        hello.extend_from_slice(&[0x03, 0x03]);

        // Server random
        hello.extend_from_slice(&self.server_random);

        // Session ID
        hello.push(self.session_id.len() as u8);
        hello.extend_from_slice(&self.session_id);

        // Cipher suite
        if let Some(suite) = self.cipher_suite {
            hello.extend_from_slice(&suite.to_id().to_be_bytes());
        } else {
            return Err(TlsError::UnsupportedCipherSuite);
        }

        // Compression method
        hello.push(0);

        // Extensions (minimal)
        hello.extend_from_slice(&[0, 0]); // No extensions

        // Update length
        let length = hello.len() - 4;
        hello[length_pos] = ((length >> 16) & 0xFF) as u8;
        hello[length_pos + 1] = ((length >> 8) & 0xFF) as u8;
        hello[length_pos + 2] = (length & 0xFF) as u8;

        Ok(hello)
    }

    /// Verify certificate chain.
    fn verify_certificate_chain(&self) -> Result<(), TlsError> {
        if self.peer_certificates.is_empty() {
            return Err(TlsError::BadCertificate);
        }

        // Verify hostname
        if let Some(ref server_name) = self.config.server_name {
            let leaf = &self.peer_certificates[0];
            if !leaf.verify_hostname(server_name) {
                return Err(TlsError::HostnameMismatch);
            }
        }

        // Verify expiration
        for cert in &self.peer_certificates {
            if cert.is_expired() {
                return Err(TlsError::CertificateExpired);
            }
        }

        // Verify chain (simplified - would need root store)
        // For now, just check basic validity
        for i in 0..self.peer_certificates.len() - 1 {
            let cert = &self.peer_certificates[i];
            let issuer = &self.peer_certificates[i + 1];

            if !cert.verify_signature(issuer) {
                return Err(TlsError::BadCertificate);
            }
        }

        Ok(())
    }

    /// Encrypt application data.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, TlsError> {
        if self.state != TlsState::Connected {
            return Err(TlsError::HandshakeFailure);
        }

        // In a real implementation, this would encrypt using the negotiated cipher
        // For now, just wrap in a record
        let record = self.wrap_record(23, plaintext);
        self.send_seq += 1;

        Ok(record)
    }

    /// Decrypt application data.
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, TlsError> {
        if self.state != TlsState::Connected {
            return Err(TlsError::HandshakeFailure);
        }

        if ciphertext.len() < 5 {
            return Err(TlsError::InvalidRecord);
        }

        let content_type = ciphertext[0];
        if content_type != 23 {
            return Err(TlsError::InvalidRecord);
        }

        let length = u16::from_be_bytes([ciphertext[3], ciphertext[4]]) as usize;
        if ciphertext.len() < 5 + length {
            return Err(TlsError::InvalidRecord);
        }

        // In a real implementation, this would decrypt using the negotiated cipher
        let plaintext = ciphertext[5..5 + length].to_vec();
        self.recv_seq += 1;

        Ok(plaintext)
    }

    /// Close the session.
    pub fn close(&mut self) -> Result<Vec<u8>, TlsError> {
        self.state = TlsState::Closing;

        // Build close_notify alert
        let alert = vec![1, 0]; // Warning level, close_notify
        let record = self.wrap_record(21, &alert);

        Ok(record)
    }
}

/// TLS connection builder.
pub struct TlsConnector {
    config: TlsConfig,
}

impl TlsConnector {
    /// Create a new TLS connector with default config.
    pub fn new() -> Self {
        Self {
            config: TlsConfig::default(),
        }
    }

    /// Create with custom config.
    pub fn with_config(config: TlsConfig) -> Self {
        Self { config }
    }

    /// Set server name for SNI.
    pub fn server_name(mut self, name: &str) -> Self {
        self.config.server_name = Some(name.to_string());
        self
    }

    /// Set whether to verify certificates.
    pub fn verify_certificates(mut self, verify: bool) -> Self {
        self.config.verify_certificates = verify;
        self
    }

    /// Set minimum TLS version.
    pub fn min_version(mut self, version: TlsVersion) -> Self {
        self.config.min_version = version;
        self
    }

    /// Add ALPN protocol.
    pub fn alpn_protocol(mut self, protocol: &str) -> Self {
        self.config.alpn_protocols.push(protocol.to_string());
        self
    }

    /// Create a client session.
    pub fn connect(self) -> TlsSession {
        TlsSession::new_client(self.config)
    }
}

impl Default for TlsConnector {
    fn default() -> Self {
        Self::new()
    }
}

/// TLS acceptor for servers.
pub struct TlsAcceptor {
    config: TlsConfig,
    certificate: Option<Certificate>,
    private_key: Option<Vec<u8>>,
}

impl TlsAcceptor {
    /// Create a new TLS acceptor.
    pub fn new(certificate: Certificate, private_key: Vec<u8>) -> Self {
        Self {
            config: TlsConfig::default(),
            certificate: Some(certificate),
            private_key: Some(private_key),
        }
    }

    /// Set whether to require client certificates.
    pub fn require_client_cert(mut self, require: bool) -> Self {
        self.config.require_client_cert = require;
        self
    }

    /// Add ALPN protocol.
    pub fn alpn_protocol(mut self, protocol: &str) -> Self {
        self.config.alpn_protocols.push(protocol.to_string());
        self
    }

    /// Accept a client connection.
    pub fn accept(self) -> TlsSession {
        TlsSession::new_server(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_version() {
        assert_eq!(TlsVersion::Tls13.to_bytes(), [0x03, 0x04]);
        assert_eq!(
            TlsVersion::from_bytes([0x03, 0x03]),
            Some(TlsVersion::Tls12)
        );
    }

    #[test]
    fn test_cipher_suite() {
        assert_eq!(CipherSuite::Tls13Aes256GcmSha384.to_id(), 0x1302);
        assert!(CipherSuite::Tls13Aes256GcmSha384.is_tls13());
        assert!(!CipherSuite::EcdheRsaAes256GcmSha384.is_tls13());
    }

    #[test]
    fn test_default_config() {
        let config = TlsConfig::default();
        assert_eq!(config.min_version, TlsVersion::Tls12);
        assert_eq!(config.max_version, TlsVersion::Tls13);
        assert!(config.verify_certificates);
    }

    #[test]
    fn test_connector_builder() {
        let connector = TlsConnector::new()
            .server_name("example.com")
            .verify_certificates(true)
            .alpn_protocol("h2")
            .alpn_protocol("http/1.1");

        let session = connector.connect();
        assert!(session.is_client);
        assert_eq!(session.state(), TlsState::Initial);
    }
}
