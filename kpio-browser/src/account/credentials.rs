//! Credential Storage
//!
//! Secure storage for user credentials.

use alloc::string::String;

/// Stored credential
#[derive(Debug, Clone)]
pub struct StoredCredential {
    /// Credential ID
    pub id: String,
    /// Site/service name
    pub name: String,
    /// URL
    pub url: Option<String>,
    /// Username
    pub username: String,
    /// Encrypted password
    encrypted_password: String,
    /// Created timestamp
    pub created_at: u64,
    /// Last used timestamp
    pub last_used: Option<u64>,
    /// Notes
    pub notes: Option<String>,
}

impl StoredCredential {
    /// Create new credential
    pub fn new(id: String, name: String, username: String, password: &str) -> Self {
        Self {
            id,
            name,
            url: None,
            username,
            encrypted_password: encrypt_password(password),
            created_at: 0,
            last_used: None,
            notes: None,
        }
    }

    /// Set URL
    pub fn with_url(mut self, url: String) -> Self {
        self.url = Some(url);
        self
    }

    /// Set notes
    pub fn with_notes(mut self, notes: String) -> Self {
        self.notes = Some(notes);
        self
    }

    /// Get decrypted password
    pub fn password(&self, master_key: &[u8]) -> Option<String> {
        decrypt_password(&self.encrypted_password, master_key)
    }

    /// Update password
    pub fn update_password(&mut self, password: &str) {
        self.encrypted_password = encrypt_password(password);
    }

    /// Mark as used
    pub fn mark_used(&mut self, timestamp: u64) {
        self.last_used = Some(timestamp);
    }
}

/// Credential vault
pub struct CredentialVault {
    /// Is unlocked
    unlocked: bool,
    /// Master key (cleared when locked)
    master_key: Option<[u8; 32]>,
    /// Lock timeout (seconds)
    lock_timeout: u64,
    /// Last activity timestamp
    last_activity: u64,
}

impl CredentialVault {
    /// Create new vault
    pub const fn new() -> Self {
        Self {
            unlocked: false,
            master_key: None,
            lock_timeout: 300, // 5 minutes
            last_activity: 0,
        }
    }

    /// Check if unlocked
    pub fn is_unlocked(&self) -> bool {
        self.unlocked
    }

    /// Unlock with master password
    pub fn unlock(&mut self, master_password: &str) -> bool {
        // Would verify master password hash
        // Derive master key from password
        let key = derive_key(master_password);
        self.master_key = Some(key);
        self.unlocked = true;
        true
    }

    /// Lock vault
    pub fn lock(&mut self) {
        self.unlocked = false;
        self.master_key = None;
    }

    /// Get master key (if unlocked)
    pub fn master_key(&self) -> Option<&[u8; 32]> {
        if self.unlocked {
            self.master_key.as_ref()
        } else {
            None
        }
    }

    /// Update activity timestamp
    pub fn update_activity(&mut self, timestamp: u64) {
        self.last_activity = timestamp;
    }

    /// Check if should auto-lock
    pub fn should_auto_lock(&self, current_time: u64) -> bool {
        if self.unlocked {
            current_time - self.last_activity > self.lock_timeout
        } else {
            false
        }
    }

    /// Set lock timeout
    pub fn set_lock_timeout(&mut self, seconds: u64) {
        self.lock_timeout = seconds;
    }
}

impl Default for CredentialVault {
    fn default() -> Self {
        Self::new()
    }
}

// Encryption helpers (simplified - would use proper crypto)

fn encrypt_password(password: &str) -> String {
    // Would use AES-256-GCM or similar
    // For now, just base64 encode (NOT SECURE - placeholder only)
    let bytes = password.as_bytes();
    let mut result = alloc::string::String::new();
    for byte in bytes {
        use core::fmt::Write;
        let _ = write!(result, "{:02x}", byte);
    }
    result
}

fn decrypt_password(encrypted: &str, _master_key: &[u8]) -> Option<String> {
    // Would use AES-256-GCM decryption
    // For now, just hex decode (NOT SECURE - placeholder only)
    let mut bytes = alloc::vec::Vec::new();
    let chars: alloc::vec::Vec<char> = encrypted.chars().collect();
    
    for chunk in chars.chunks(2) {
        if chunk.len() == 2 {
            let hex_str: alloc::string::String = chunk.iter().collect();
            if let Ok(byte) = u8::from_str_radix(&hex_str, 16) {
                bytes.push(byte);
            }
        }
    }
    
    alloc::string::String::from_utf8(bytes).ok()
}

fn derive_key(password: &str) -> [u8; 32] {
    // Would use PBKDF2 or Argon2
    // Placeholder: simple hash expansion
    let mut key = [0u8; 32];
    let bytes = password.as_bytes();
    
    for (i, byte) in bytes.iter().cycle().take(32).enumerate() {
        key[i] = *byte;
    }
    
    key
}

/// Password generator
pub struct PasswordGenerator {
    /// Length
    pub length: usize,
    /// Include lowercase
    pub lowercase: bool,
    /// Include uppercase
    pub uppercase: bool,
    /// Include digits
    pub digits: bool,
    /// Include symbols
    pub symbols: bool,
}

impl Default for PasswordGenerator {
    fn default() -> Self {
        Self {
            length: 16,
            lowercase: true,
            uppercase: true,
            digits: true,
            symbols: true,
        }
    }
}

impl PasswordGenerator {
    /// Create new generator
    pub fn new() -> Self {
        Self::default()
    }

    /// Set length
    pub fn with_length(mut self, length: usize) -> Self {
        self.length = length;
        self
    }

    /// Generate password
    pub fn generate(&self) -> alloc::string::String {
        use alloc::vec::Vec;
        
        let mut charset = Vec::new();
        
        if self.lowercase {
            charset.extend(b'a'..=b'z');
        }
        if self.uppercase {
            charset.extend(b'A'..=b'Z');
        }
        if self.digits {
            charset.extend(b'0'..=b'9');
        }
        if self.symbols {
            charset.extend(b"!@#$%^&*()-_=+[]{}|;:,.<>?".iter().copied());
        }
        
        if charset.is_empty() {
            return alloc::string::String::new();
        }
        
        // Would use secure random
        // Placeholder: deterministic sequence
        let mut password = alloc::string::String::new();
        for i in 0..self.length {
            let idx = (i * 7 + 13) % charset.len();
            password.push(charset[idx] as char);
        }
        
        password
    }
}
