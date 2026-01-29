//! User Account System
//!
//! Provides user authentication, OAuth2 integration, and session management.

pub mod credentials;
pub mod oauth;
pub mod session;

pub use credentials::*;
pub use oauth::*;
pub use session::*;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::RwLock;

/// Account error types
#[derive(Debug, Clone)]
pub enum AccountError {
    /// Invalid credentials
    InvalidCredentials,
    /// Account not found
    NotFound,
    /// Account locked
    Locked,
    /// Session expired
    SessionExpired,
    /// Network error
    NetworkError(String),
    /// OAuth error
    OAuthError(String),
    /// Storage error
    StorageError(String),
}

/// User account
#[derive(Debug, Clone)]
pub struct UserAccount {
    /// User ID
    pub id: String,
    /// Email address
    pub email: String,
    /// Display name
    pub display_name: Option<String>,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// Account status
    pub status: AccountStatus,
    /// Created timestamp
    pub created_at: u64,
    /// Last login timestamp
    pub last_login: Option<u64>,
    /// Account preferences
    pub preferences: AccountPreferences,
    /// Linked OAuth providers
    pub linked_providers: Vec<OAuthProvider>,
}

impl UserAccount {
    /// Create new account
    pub fn new(id: String, email: String) -> Self {
        Self {
            id,
            email,
            display_name: None,
            avatar_url: None,
            status: AccountStatus::Active,
            created_at: 0, // Would get current time
            last_login: None,
            preferences: AccountPreferences::default(),
            linked_providers: Vec::new(),
        }
    }

    /// Check if account is active
    pub fn is_active(&self) -> bool {
        self.status == AccountStatus::Active
    }

    /// Get display name or email
    pub fn name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.email)
    }

    /// Check if provider is linked
    pub fn has_provider(&self, provider: OAuthProvider) -> bool {
        self.linked_providers.contains(&provider)
    }

    /// Add OAuth provider
    pub fn add_provider(&mut self, provider: OAuthProvider) {
        if !self.has_provider(provider) {
            self.linked_providers.push(provider);
        }
    }

    /// Remove OAuth provider
    pub fn remove_provider(&mut self, provider: OAuthProvider) {
        self.linked_providers.retain(|p| *p != provider);
    }
}

/// Account status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountStatus {
    /// Active
    Active,
    /// Pending verification
    PendingVerification,
    /// Suspended
    Suspended,
    /// Deleted
    Deleted,
}

/// Account preferences
#[derive(Debug, Clone)]
pub struct AccountPreferences {
    /// Sync enabled
    pub sync_enabled: bool,
    /// Sync bookmarks
    pub sync_bookmarks: bool,
    /// Sync history
    pub sync_history: bool,
    /// Sync settings
    pub sync_settings: bool,
    /// Sync extensions
    pub sync_extensions: bool,
    /// Sync open tabs
    pub sync_tabs: bool,
    /// Two-factor authentication enabled
    pub two_factor_enabled: bool,
}

impl Default for AccountPreferences {
    fn default() -> Self {
        Self {
            sync_enabled: true,
            sync_bookmarks: true,
            sync_history: true,
            sync_settings: true,
            sync_extensions: true,
            sync_tabs: true,
            two_factor_enabled: false,
        }
    }
}

/// OAuth provider
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthProvider {
    /// Google
    Google,
    /// GitHub
    GitHub,
    /// Microsoft
    Microsoft,
    /// Apple
    Apple,
}

impl OAuthProvider {
    /// Get provider name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::GitHub => "GitHub",
            Self::Microsoft => "Microsoft",
            Self::Apple => "Apple",
        }
    }

    /// Get authorization URL base
    pub fn auth_url(&self) -> &'static str {
        match self {
            Self::Google => "https://accounts.google.com/o/oauth2/v2/auth",
            Self::GitHub => "https://github.com/login/oauth/authorize",
            Self::Microsoft => "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            Self::Apple => "https://appleid.apple.com/auth/authorize",
        }
    }

    /// Get token URL
    pub fn token_url(&self) -> &'static str {
        match self {
            Self::Google => "https://oauth2.googleapis.com/token",
            Self::GitHub => "https://github.com/login/oauth/access_token",
            Self::Microsoft => "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            Self::Apple => "https://appleid.apple.com/auth/token",
        }
    }
}

/// Account manager
pub struct AccountManager {
    /// Current logged in account
    current_account: Option<UserAccount>,
    /// Current session
    current_session: Option<Session>,
    /// Stored credentials
    stored_credentials: BTreeMap<String, StoredCredential>,
}

impl AccountManager {
    /// Create new account manager
    pub const fn new() -> Self {
        Self {
            current_account: None,
            current_session: None,
            stored_credentials: BTreeMap::new(),
        }
    }

    /// Check if logged in
    pub fn is_logged_in(&self) -> bool {
        if let Some(ref session) = self.current_session {
            !session.is_expired()
        } else {
            false
        }
    }

    /// Get current account
    pub fn current_account(&self) -> Option<&UserAccount> {
        if self.is_logged_in() {
            self.current_account.as_ref()
        } else {
            None
        }
    }

    /// Get current session
    pub fn current_session(&self) -> Option<&Session> {
        if self.is_logged_in() {
            self.current_session.as_ref()
        } else {
            None
        }
    }

    /// Login with email and password
    pub fn login(&mut self, email: &str, password: &str) -> Result<&UserAccount, AccountError> {
        // Would verify credentials against server
        // For now, create mock account
        let account = UserAccount::new(
            generate_id(),
            email.to_string(),
        );

        let session = Session::new(
            generate_session_token(),
            account.id.clone(),
        );

        self.current_account = Some(account);
        self.current_session = Some(session);

        Ok(self.current_account.as_ref().unwrap())
    }

    /// Login with OAuth
    pub fn login_oauth(&mut self, provider: OAuthProvider, access_token: &str) 
        -> Result<&UserAccount, AccountError> 
    {
        // Would exchange token for user info
        let account = UserAccount::new(
            generate_id(),
            "oauth@example.com".to_string(),
        );

        let mut account = account;
        account.add_provider(provider);

        let session = Session::new(
            generate_session_token(),
            account.id.clone(),
        );

        self.current_account = Some(account);
        self.current_session = Some(session);

        Ok(self.current_account.as_ref().unwrap())
    }

    /// Logout
    pub fn logout(&mut self) {
        self.current_account = None;
        self.current_session = None;
    }

    /// Refresh session
    pub fn refresh_session(&mut self) -> Result<(), AccountError> {
        if let Some(ref mut session) = self.current_session {
            session.refresh();
            Ok(())
        } else {
            Err(AccountError::SessionExpired)
        }
    }

    /// Store credential
    pub fn store_credential(&mut self, credential: StoredCredential) {
        self.stored_credentials.insert(credential.id.clone(), credential);
    }

    /// Get stored credential
    pub fn get_credential(&self, id: &str) -> Option<&StoredCredential> {
        self.stored_credentials.get(id)
    }

    /// Remove stored credential
    pub fn remove_credential(&mut self, id: &str) {
        self.stored_credentials.remove(id);
    }

    /// List stored credentials
    pub fn list_credentials(&self) -> Vec<&StoredCredential> {
        self.stored_credentials.values().collect()
    }
}

impl Default for AccountManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global account manager
pub static ACCOUNT_MANAGER: RwLock<AccountManager> = RwLock::new(AccountManager::new());

// Helper functions

fn generate_id() -> String {
    // Would generate UUID
    "user_12345678".to_string()
}

fn generate_session_token() -> String {
    // Would generate secure token
    "session_abcdefgh12345678".to_string()
}
