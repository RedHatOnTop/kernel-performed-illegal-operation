//! OAuth2 Implementation
//!
//! OAuth2 flow implementation for third-party authentication.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::OAuthProvider;

/// OAuth2 client configuration
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    /// Client ID
    pub client_id: String,
    /// Client secret (optional for PKCE)
    pub client_secret: Option<String>,
    /// Redirect URI
    pub redirect_uri: String,
    /// Scopes
    pub scopes: Vec<String>,
    /// Use PKCE
    pub use_pkce: bool,
}

impl OAuthConfig {
    /// Create new config
    pub fn new(client_id: String, redirect_uri: String) -> Self {
        Self {
            client_id,
            client_secret: None,
            redirect_uri,
            scopes: Vec::new(),
            use_pkce: true,
        }
    }

    /// Set client secret
    pub fn with_secret(mut self, secret: String) -> Self {
        self.client_secret = Some(secret);
        self
    }

    /// Add scope
    pub fn with_scope(mut self, scope: String) -> Self {
        self.scopes.push(scope);
        self
    }

    /// Set PKCE usage
    pub fn with_pkce(mut self, use_pkce: bool) -> Self {
        self.use_pkce = use_pkce;
        self
    }
}

/// OAuth2 authorization request
#[derive(Debug, Clone)]
pub struct AuthorizationRequest {
    /// Provider
    pub provider: OAuthProvider,
    /// Authorization URL
    pub auth_url: String,
    /// State parameter
    pub state: String,
    /// Code verifier (for PKCE)
    pub code_verifier: Option<String>,
}

impl AuthorizationRequest {
    /// Build authorization URL
    pub fn build(provider: OAuthProvider, config: &OAuthConfig) -> Self {
        let state = generate_state();
        let code_verifier = if config.use_pkce {
            Some(generate_code_verifier())
        } else {
            None
        };

        let mut url = alloc::format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&state={}",
            provider.auth_url(),
            url_encode(&config.client_id),
            url_encode(&config.redirect_uri),
            url_encode(&state),
        );

        if !config.scopes.is_empty() {
            let scope = config.scopes.join(" ");
            url.push_str("&scope=");
            url.push_str(&url_encode(&scope));
        }

        if let Some(ref verifier) = code_verifier {
            let challenge = generate_code_challenge(verifier);
            url.push_str("&code_challenge=");
            url.push_str(&url_encode(&challenge));
            url.push_str("&code_challenge_method=S256");
        }

        Self {
            provider,
            auth_url: url,
            state,
            code_verifier,
        }
    }
}

/// OAuth2 token response
#[derive(Debug, Clone)]
pub struct TokenResponse {
    /// Access token
    pub access_token: String,
    /// Token type
    pub token_type: String,
    /// Expires in (seconds)
    pub expires_in: Option<u64>,
    /// Refresh token
    pub refresh_token: Option<String>,
    /// Scope
    pub scope: Option<String>,
    /// ID token (for OpenID Connect)
    pub id_token: Option<String>,
}

impl TokenResponse {
    /// Parse from JSON
    pub fn parse(json: &str) -> Option<Self> {
        // Simple JSON parsing (would use serde_json in production)
        let access_token = extract_string(json, "access_token")?;
        let token_type = extract_string(json, "token_type").unwrap_or_else(|| "Bearer".to_string());
        let expires_in = extract_number(json, "expires_in");
        let refresh_token = extract_string(json, "refresh_token");
        let scope = extract_string(json, "scope");
        let id_token = extract_string(json, "id_token");

        Some(Self {
            access_token,
            token_type,
            expires_in,
            refresh_token,
            scope,
            id_token,
        })
    }
}

/// OAuth2 client
pub struct OAuthClient {
    /// Configurations by provider
    configs: BTreeMap<u8, OAuthConfig>,
    /// Pending authorization requests
    pending_requests: BTreeMap<String, AuthorizationRequest>,
}

impl OAuthClient {
    /// Create new client
    pub const fn new() -> Self {
        Self {
            configs: BTreeMap::new(),
            pending_requests: BTreeMap::new(),
        }
    }

    /// Configure provider
    pub fn configure(&mut self, provider: OAuthProvider, config: OAuthConfig) {
        self.configs.insert(provider as u8, config);
    }

    /// Get config for provider
    pub fn get_config(&self, provider: OAuthProvider) -> Option<&OAuthConfig> {
        self.configs.get(&(provider as u8))
    }

    /// Start authorization flow
    pub fn authorize(&mut self, provider: OAuthProvider) -> Option<&AuthorizationRequest> {
        let config = self.get_config(provider)?;
        let request = AuthorizationRequest::build(provider, config);
        let state = request.state.clone();
        self.pending_requests.insert(state.clone(), request);
        self.pending_requests.get(&state)
    }

    /// Handle authorization callback
    pub fn handle_callback(&mut self, state: &str, code: &str) 
        -> Result<TokenExchangeRequest, OAuthError> 
    {
        let request = self.pending_requests.remove(state)
            .ok_or(OAuthError::InvalidState)?;

        let config = self.get_config(request.provider)
            .ok_or(OAuthError::ProviderNotConfigured)?;

        let exchange = TokenExchangeRequest {
            provider: request.provider,
            code: code.to_string(),
            redirect_uri: config.redirect_uri.clone(),
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
            code_verifier: request.code_verifier,
        };

        Ok(exchange)
    }

    /// Clear pending request
    pub fn cancel(&mut self, state: &str) {
        self.pending_requests.remove(state);
    }
}

impl Default for OAuthClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Token exchange request
#[derive(Debug, Clone)]
pub struct TokenExchangeRequest {
    /// Provider
    pub provider: OAuthProvider,
    /// Authorization code
    pub code: String,
    /// Redirect URI
    pub redirect_uri: String,
    /// Client ID
    pub client_id: String,
    /// Client secret
    pub client_secret: Option<String>,
    /// Code verifier (for PKCE)
    pub code_verifier: Option<String>,
}

impl TokenExchangeRequest {
    /// Build request body
    pub fn to_form_data(&self) -> String {
        let mut params = alloc::vec![
            alloc::format!("grant_type=authorization_code"),
            alloc::format!("code={}", url_encode(&self.code)),
            alloc::format!("redirect_uri={}", url_encode(&self.redirect_uri)),
            alloc::format!("client_id={}", url_encode(&self.client_id)),
        ];

        if let Some(ref secret) = self.client_secret {
            params.push(alloc::format!("client_secret={}", url_encode(secret)));
        }

        if let Some(ref verifier) = self.code_verifier {
            params.push(alloc::format!("code_verifier={}", url_encode(verifier)));
        }

        params.join("&")
    }

    /// Get token URL
    pub fn token_url(&self) -> &'static str {
        self.provider.token_url()
    }
}

/// OAuth error
#[derive(Debug, Clone)]
pub enum OAuthError {
    /// Invalid state parameter
    InvalidState,
    /// Provider not configured
    ProviderNotConfigured,
    /// Token exchange failed
    TokenExchangeFailed(String),
    /// Invalid token response
    InvalidTokenResponse,
    /// Access denied
    AccessDenied,
    /// Network error
    NetworkError(String),
}

// Helper functions

fn generate_state() -> String {
    // Would use secure random
    "state_abcdef123456".to_string()
}

fn generate_code_verifier() -> String {
    // Would generate 43-128 character random string
    "code_verifier_1234567890abcdefghijklmnopqrstuvwxyz12345678".to_string()
}

fn generate_code_challenge(verifier: &str) -> String {
    // Would compute SHA256 and base64url encode
    // Placeholder
    alloc::format!("challenge_{}", &verifier[..10])
}

fn url_encode(s: &str) -> String {
    let mut encoded = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push('%');
                use core::fmt::Write;
                let _ = write!(encoded, "{:02X}", byte);
            }
        }
    }
    encoded
}

fn extract_string(json: &str, field: &str) -> Option<String> {
    let pattern = alloc::format!("\"{}\"", field);
    let pos = json.find(&pattern)?;
    let rest = &json[pos + pattern.len()..];
    let rest = rest.trim_start();
    if !rest.starts_with(':') {
        return None;
    }
    let rest = rest[1..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_number(json: &str, field: &str) -> Option<u64> {
    let pattern = alloc::format!("\"{}\"", field);
    let pos = json.find(&pattern)?;
    let rest = &json[pos + pattern.len()..];
    let rest = rest.trim_start();
    if !rest.starts_with(':') {
        return None;
    }
    let rest = rest[1..].trim_start();
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}
