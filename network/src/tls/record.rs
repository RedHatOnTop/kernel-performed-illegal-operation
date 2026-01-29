//! TLS record layer protocol.
//!
//! This module implements the TLS record layer for
//! encapsulating handshake, alert, and application data.

#![allow(dead_code)]

extern crate alloc;

use alloc::vec::Vec;
use alloc::vec;

use super::TlsError;

/// Content type for TLS records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ContentType {
    /// Change cipher spec (legacy).
    ChangeCipherSpec = 20,
    /// Alert.
    Alert = 21,
    /// Handshake.
    Handshake = 22,
    /// Application data.
    ApplicationData = 23,
}

impl ContentType {
    /// Parse from byte.
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            20 => Some(ContentType::ChangeCipherSpec),
            21 => Some(ContentType::Alert),
            22 => Some(ContentType::Handshake),
            23 => Some(ContentType::ApplicationData),
            _ => None,
        }
    }
}

/// TLS record.
#[derive(Debug, Clone)]
pub struct Record {
    /// Content type.
    pub content_type: ContentType,
    /// Protocol version.
    pub version: [u8; 2],
    /// Record payload.
    pub fragment: Vec<u8>,
}

impl Record {
    /// Maximum record size (16KB).
    pub const MAX_FRAGMENT_SIZE: usize = 16384;
    
    /// Create a new record.
    pub fn new(content_type: ContentType, fragment: Vec<u8>) -> Self {
        Self {
            content_type,
            version: [0x03, 0x03], // TLS 1.2
            fragment,
        }
    }
    
    /// Create a handshake record.
    pub fn handshake(data: Vec<u8>) -> Self {
        Self::new(ContentType::Handshake, data)
    }
    
    /// Create an application data record.
    pub fn application_data(data: Vec<u8>) -> Self {
        Self::new(ContentType::ApplicationData, data)
    }
    
    /// Create an alert record.
    pub fn alert(level: AlertLevel, description: u8) -> Self {
        Self::new(ContentType::Alert, vec![level as u8, description])
    }
    
    /// Serialize to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(5 + self.fragment.len());
        data.push(self.content_type as u8);
        data.extend_from_slice(&self.version);
        data.extend_from_slice(&(self.fragment.len() as u16).to_be_bytes());
        data.extend_from_slice(&self.fragment);
        data
    }
    
    /// Parse from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<(Self, usize), TlsError> {
        if data.len() < 5 {
            return Err(TlsError::InvalidRecord);
        }
        
        let content_type = ContentType::from_byte(data[0])
            .ok_or(TlsError::InvalidRecord)?;
        let version = [data[1], data[2]];
        let length = u16::from_be_bytes([data[3], data[4]]) as usize;
        
        if length > Self::MAX_FRAGMENT_SIZE {
            return Err(TlsError::InvalidRecord);
        }
        
        if data.len() < 5 + length {
            return Err(TlsError::InvalidRecord);
        }
        
        let fragment = data[5..5 + length].to_vec();
        
        Ok((
            Self {
                content_type,
                version,
                fragment,
            },
            5 + length,
        ))
    }
    
    /// Check if this is an encrypted record.
    pub fn is_encrypted(&self) -> bool {
        // In TLS 1.3, all application data and most handshake records after
        // ServerHello are encrypted
        self.content_type == ContentType::ApplicationData
    }
}

/// Alert level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AlertLevel {
    /// Warning.
    Warning = 1,
    /// Fatal.
    Fatal = 2,
}

impl AlertLevel {
    /// Parse from byte.
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            1 => Some(AlertLevel::Warning),
            2 => Some(AlertLevel::Fatal),
            _ => None,
        }
    }
}

/// Record layer state.
#[derive(Debug)]
pub struct RecordLayer {
    /// Read encryption state.
    read_state: CipherState,
    /// Write encryption state.
    write_state: CipherState,
    /// Maximum fragment size.
    max_fragment_size: usize,
}

impl RecordLayer {
    /// Create a new record layer.
    pub fn new() -> Self {
        Self {
            read_state: CipherState::new(),
            write_state: CipherState::new(),
            max_fragment_size: Record::MAX_FRAGMENT_SIZE,
        }
    }
    
    /// Set the read key.
    pub fn set_read_key(&mut self, key: Vec<u8>, iv: Vec<u8>) {
        self.read_state.key = key;
        self.read_state.iv = iv;
        self.read_state.encrypted = true;
    }
    
    /// Set the write key.
    pub fn set_write_key(&mut self, key: Vec<u8>, iv: Vec<u8>) {
        self.write_state.key = key;
        self.write_state.iv = iv;
        self.write_state.encrypted = true;
    }
    
    /// Encrypt and wrap data in a record.
    pub fn encrypt(&mut self, content_type: ContentType, data: &[u8]) -> Result<Record, TlsError> {
        if !self.write_state.encrypted {
            return Ok(Record::new(content_type, data.to_vec()));
        }
        
        // Build inner plaintext (TLS 1.3)
        let mut plaintext = data.to_vec();
        plaintext.push(content_type as u8);
        
        // Add padding (optional, but good for security)
        // plaintext.extend_from_slice(&[0; 16]);
        
        // Encrypt
        let ciphertext = self.write_state.encrypt(&plaintext)?;
        
        // The outer content type is always ApplicationData for encrypted records
        let record = Record::new(ContentType::ApplicationData, ciphertext);
        
        Ok(record)
    }
    
    /// Decrypt a record.
    pub fn decrypt(&mut self, record: &Record) -> Result<(ContentType, Vec<u8>), TlsError> {
        if !self.read_state.encrypted {
            return Ok((record.content_type, record.fragment.clone()));
        }
        
        // Decrypt
        let plaintext = self.read_state.decrypt(&record.fragment)?;
        
        if plaintext.is_empty() {
            return Err(TlsError::DecryptionError);
        }
        
        // Find the real content type (last non-zero byte)
        let mut content_type_byte = None;
        for i in (0..plaintext.len()).rev() {
            if plaintext[i] != 0 {
                content_type_byte = Some(plaintext[i]);
                break;
            }
        }
        
        let content_type = content_type_byte
            .and_then(ContentType::from_byte)
            .ok_or(TlsError::InvalidRecord)?;
        
        // Remove content type and padding
        let data_end = plaintext.len() - 1 - plaintext.iter().rev().skip(1).take_while(|&&b| b == 0).count();
        let data = plaintext[..data_end].to_vec();
        
        Ok((content_type, data))
    }
    
    /// Fragment data into multiple records if needed.
    pub fn fragment(&self, content_type: ContentType, data: &[u8]) -> Vec<Record> {
        let mut records = Vec::new();
        
        for chunk in data.chunks(self.max_fragment_size) {
            records.push(Record::new(content_type, chunk.to_vec()));
        }
        
        if records.is_empty() {
            records.push(Record::new(content_type, Vec::new()));
        }
        
        records
    }
}

impl Default for RecordLayer {
    fn default() -> Self {
        Self::new()
    }
}

/// Cipher state for a direction.
#[derive(Debug, Clone)]
struct CipherState {
    /// Encryption key.
    key: Vec<u8>,
    /// IV/nonce.
    iv: Vec<u8>,
    /// Sequence number.
    seq: u64,
    /// Whether encryption is active.
    encrypted: bool,
}

impl CipherState {
    /// Create a new cipher state.
    fn new() -> Self {
        Self {
            key: Vec::new(),
            iv: Vec::new(),
            seq: 0,
            encrypted: false,
        }
    }
    
    /// Encrypt plaintext.
    fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, TlsError> {
        // Build nonce from IV and sequence number
        let nonce = self.build_nonce();
        
        // In a real implementation, this would use AES-GCM or ChaCha20-Poly1305
        // For now, just return the plaintext (insecure placeholder)
        let mut ciphertext = plaintext.to_vec();
        
        // Append authentication tag (16 bytes for GCM/Poly1305)
        ciphertext.extend_from_slice(&[0u8; 16]);
        
        self.seq += 1;
        
        Ok(ciphertext)
    }
    
    /// Decrypt ciphertext.
    fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, TlsError> {
        if ciphertext.len() < 16 {
            return Err(TlsError::DecryptionError);
        }
        
        let nonce = self.build_nonce();
        
        // In a real implementation, verify and strip the authentication tag
        let plaintext = ciphertext[..ciphertext.len() - 16].to_vec();
        
        self.seq += 1;
        
        Ok(plaintext)
    }
    
    /// Build nonce from IV and sequence number.
    fn build_nonce(&self) -> Vec<u8> {
        let mut nonce = self.iv.clone();
        
        // XOR the sequence number into the last 8 bytes of the IV
        if nonce.len() >= 8 {
            let seq_bytes = self.seq.to_be_bytes();
            let start = nonce.len() - 8;
            for i in 0..8 {
                nonce[start + i] ^= seq_bytes[i];
            }
        }
        
        nonce
    }
}

/// Record buffer for accumulating incomplete records.
#[derive(Debug, Clone)]
pub struct RecordBuffer {
    /// Buffered data.
    buffer: Vec<u8>,
}

impl RecordBuffer {
    /// Create a new record buffer.
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }
    
    /// Append data to the buffer.
    pub fn append(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }
    
    /// Try to extract a complete record.
    pub fn try_read_record(&mut self) -> Result<Option<Record>, TlsError> {
        if self.buffer.len() < 5 {
            return Ok(None);
        }
        
        let length = u16::from_be_bytes([self.buffer[3], self.buffer[4]]) as usize;
        
        if length > Record::MAX_FRAGMENT_SIZE {
            return Err(TlsError::InvalidRecord);
        }
        
        if self.buffer.len() < 5 + length {
            return Ok(None);
        }
        
        let (record, consumed) = Record::from_bytes(&self.buffer)?;
        self.buffer.drain(..consumed);
        
        Ok(Some(record))
    }
    
    /// Check if the buffer has a complete record.
    pub fn has_complete_record(&self) -> bool {
        if self.buffer.len() < 5 {
            return false;
        }
        
        let length = u16::from_be_bytes([self.buffer[3], self.buffer[4]]) as usize;
        self.buffer.len() >= 5 + length
    }
    
    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
    
    /// Get buffer length.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }
    
    /// Check if buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

impl Default for RecordBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_record_roundtrip() {
        let original = Record::new(ContentType::Handshake, vec![1, 2, 3, 4]);
        let bytes = original.to_bytes();
        let (parsed, consumed) = Record::from_bytes(&bytes).unwrap();
        
        assert_eq!(consumed, bytes.len());
        assert_eq!(parsed.content_type, ContentType::Handshake);
        assert_eq!(parsed.fragment, vec![1, 2, 3, 4]);
    }
    
    #[test]
    fn test_record_buffer() {
        let mut buffer = RecordBuffer::new();
        
        // Append partial record
        buffer.append(&[22, 0x03, 0x03, 0, 4]);
        assert!(!buffer.has_complete_record());
        
        // Complete the record
        buffer.append(&[1, 2, 3, 4]);
        assert!(buffer.has_complete_record());
        
        let record = buffer.try_read_record().unwrap().unwrap();
        assert_eq!(record.content_type, ContentType::Handshake);
        assert_eq!(record.fragment, vec![1, 2, 3, 4]);
    }
    
    #[test]
    fn test_alert_record() {
        let alert = Record::alert(AlertLevel::Fatal, 40);
        assert_eq!(alert.content_type, ContentType::Alert);
        assert_eq!(alert.fragment, vec![2, 40]);
    }
}
