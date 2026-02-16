//! Tracking Protection
//!
//! Block trackers and enhance privacy.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Tracking protection manager
#[derive(Debug, Clone)]
pub struct TrackingProtection {
    /// Settings
    pub settings: TrackingSettings,
    /// Blocked trackers (domain -> count)
    pub blocked_count: BTreeMap<String, u64>,
    /// Total blocked
    pub total_blocked: u64,
    /// Custom block list
    pub custom_blocklist: Vec<String>,
    /// Custom allow list
    pub custom_allowlist: Vec<String>,
    /// Per-site settings
    pub site_settings: BTreeMap<String, SiteTrackingSettings>,
}

impl TrackingProtection {
    /// Create new tracking protection
    pub fn new() -> Self {
        Self {
            settings: TrackingSettings::default(),
            blocked_count: BTreeMap::new(),
            total_blocked: 0,
            custom_blocklist: Vec::new(),
            custom_allowlist: Vec::new(),
            site_settings: BTreeMap::new(),
        }
    }

    /// Check if request should be blocked
    pub fn should_block(&self, request: &TrackingRequest) -> BlockResult {
        // Check if protection is enabled
        if !self.settings.enabled {
            return BlockResult::Allow;
        }

        // Check site-specific settings
        if let Some(site_settings) = self.site_settings.get(&request.site_domain) {
            if !site_settings.enabled {
                return BlockResult::Allow;
            }
        }

        // Check custom allowlist
        if self.is_in_allowlist(&request.url) {
            return BlockResult::Allow;
        }

        // Check custom blocklist
        if self.is_in_blocklist(&request.url) {
            return BlockResult::Block(BlockReason::CustomBlocklist);
        }

        // Check tracker lists based on protection level
        match self.settings.level {
            ProtectionLevel::Basic => self.check_basic(request),
            ProtectionLevel::Standard => self.check_standard(request),
            ProtectionLevel::Strict => self.check_strict(request),
            ProtectionLevel::Custom => self.check_custom(request),
        }
    }

    fn check_basic(&self, request: &TrackingRequest) -> BlockResult {
        // Only block known cryptominers and fingerprinters
        if self.is_cryptominer(&request.url) {
            return BlockResult::Block(BlockReason::Cryptominer);
        }
        if self.is_fingerprinter(&request.url) {
            return BlockResult::Block(BlockReason::Fingerprinting);
        }
        BlockResult::Allow
    }

    fn check_standard(&self, request: &TrackingRequest) -> BlockResult {
        // Basic + known trackers
        let basic = self.check_basic(request);
        if matches!(basic, BlockResult::Block(_)) {
            return basic;
        }

        if self.is_tracker(&request.url) {
            return BlockResult::Block(BlockReason::Tracker);
        }

        // Block third-party cookies
        if request.is_third_party && self.settings.block_third_party_cookies {
            if request.request_type == RequestType::Cookie {
                return BlockResult::Block(BlockReason::ThirdPartyCookie);
            }
        }

        BlockResult::Allow
    }

    fn check_strict(&self, request: &TrackingRequest) -> BlockResult {
        // Standard + social trackers + all third-party
        let standard = self.check_standard(request);
        if matches!(standard, BlockResult::Block(_)) {
            return standard;
        }

        if self.is_social_tracker(&request.url) {
            return BlockResult::Block(BlockReason::SocialTracker);
        }

        // Block all third-party requests in strict mode
        if request.is_third_party && self.settings.block_all_third_party {
            return BlockResult::Block(BlockReason::ThirdParty);
        }

        BlockResult::Allow
    }

    fn check_custom(&self, request: &TrackingRequest) -> BlockResult {
        // Apply individual settings
        if self.settings.block_trackers && self.is_tracker(&request.url) {
            return BlockResult::Block(BlockReason::Tracker);
        }
        if self.settings.block_cryptominers && self.is_cryptominer(&request.url) {
            return BlockResult::Block(BlockReason::Cryptominer);
        }
        if self.settings.block_fingerprinters && self.is_fingerprinter(&request.url) {
            return BlockResult::Block(BlockReason::Fingerprinting);
        }
        if self.settings.block_social && self.is_social_tracker(&request.url) {
            return BlockResult::Block(BlockReason::SocialTracker);
        }
        if self.settings.block_third_party_cookies
            && request.is_third_party
            && request.request_type == RequestType::Cookie
        {
            return BlockResult::Block(BlockReason::ThirdPartyCookie);
        }
        BlockResult::Allow
    }

    /// Record blocked request
    pub fn record_blocked(&mut self, domain: &str) {
        *self.blocked_count.entry(domain.to_string()).or_insert(0) += 1;
        self.total_blocked += 1;
    }

    /// Get blocked count for domain
    pub fn get_blocked_count(&self, domain: &str) -> u64 {
        self.blocked_count.get(domain).copied().unwrap_or(0)
    }

    /// Get top blocked domains
    pub fn top_blocked(&self, limit: usize) -> Vec<(&String, &u64)> {
        let mut entries: Vec<_> = self.blocked_count.iter().collect();
        entries.sort_by(|a, b| b.1.cmp(a.1));
        entries.into_iter().take(limit).collect()
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.blocked_count.clear();
        self.total_blocked = 0;
    }

    /// Add to custom blocklist
    pub fn add_blocklist(&mut self, pattern: &str) {
        if !self.custom_blocklist.contains(&pattern.to_string()) {
            self.custom_blocklist.push(pattern.to_string());
        }
    }

    /// Remove from custom blocklist
    pub fn remove_blocklist(&mut self, pattern: &str) {
        self.custom_blocklist.retain(|p| p != pattern);
    }

    /// Add to custom allowlist
    pub fn add_allowlist(&mut self, pattern: &str) {
        if !self.custom_allowlist.contains(&pattern.to_string()) {
            self.custom_allowlist.push(pattern.to_string());
        }
    }

    /// Remove from custom allowlist
    pub fn remove_allowlist(&mut self, pattern: &str) {
        self.custom_allowlist.retain(|p| p != pattern);
    }

    /// Set site-specific settings
    pub fn set_site_settings(&mut self, domain: &str, settings: SiteTrackingSettings) {
        self.site_settings.insert(domain.to_string(), settings);
    }

    /// Remove site-specific settings
    pub fn remove_site_settings(&mut self, domain: &str) {
        self.site_settings.remove(domain);
    }

    // Tracker detection (simplified - real implementation would use filter lists)

    fn is_in_blocklist(&self, url: &str) -> bool {
        self.custom_blocklist
            .iter()
            .any(|p| Self::match_pattern(p, url))
    }

    fn is_in_allowlist(&self, url: &str) -> bool {
        self.custom_allowlist
            .iter()
            .any(|p| Self::match_pattern(p, url))
    }

    fn is_tracker(&self, url: &str) -> bool {
        // Known tracker domains
        let trackers = [
            "google-analytics.com",
            "googletagmanager.com",
            "doubleclick.net",
            "googlesyndication.com",
            "facebook.net",
            "fbcdn.net",
            "analytics.",
            "tracker.",
            "tracking.",
            "pixel.",
            "beacon.",
            "ad.",
            "ads.",
            "adserver.",
            "scorecardresearch.com",
            "quantserve.com",
            "hotjar.com",
            "mixpanel.com",
            "segment.io",
            "amplitude.com",
            "optimizely.com",
            "crazyegg.com",
            "mouseflow.com",
            "fullstory.com",
            "clarity.ms",
        ];
        trackers.iter().any(|t| url.contains(t))
    }

    fn is_cryptominer(&self, url: &str) -> bool {
        let miners = [
            "coinhive.com",
            "coin-hive.com",
            "cryptoloot.pro",
            "crypto-loot.com",
            "minero.cc",
            "webminepool.com",
            "authedmine.com",
        ];
        miners.iter().any(|m| url.contains(m))
    }

    fn is_fingerprinter(&self, url: &str) -> bool {
        let fingerprinters = [
            "fingerprintjs.com",
            "maxmind.com",
            "threatmetrix.com",
            "iovation.com",
            "bluecava.com",
        ];
        fingerprinters.iter().any(|f| url.contains(f))
    }

    fn is_social_tracker(&self, url: &str) -> bool {
        let social = [
            "facebook.com/tr",
            "facebook.net",
            "twitter.com/i/",
            "platform.twitter.com",
            "linkedin.com/px",
            "snap.licdn.com",
            "connect.facebook.net",
            "platform.linkedin.com",
            "pinterest.com/v3",
            "tiktok.com/i18n",
        ];
        social.iter().any(|s| url.contains(s))
    }

    fn match_pattern(pattern: &str, url: &str) -> bool {
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            let mut pos = 0;
            for (i, part) in parts.iter().enumerate() {
                if part.is_empty() {
                    continue;
                }
                if let Some(found) = url[pos..].find(part) {
                    if i == 0 && found != 0 {
                        return false;
                    }
                    pos += found + part.len();
                } else {
                    return false;
                }
            }
            true
        } else {
            url.contains(pattern)
        }
    }
}

impl Default for TrackingProtection {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracking protection settings
#[derive(Debug, Clone)]
pub struct TrackingSettings {
    /// Enable tracking protection
    pub enabled: bool,
    /// Protection level
    pub level: ProtectionLevel,
    /// Block known trackers
    pub block_trackers: bool,
    /// Block cryptominers
    pub block_cryptominers: bool,
    /// Block fingerprinters
    pub block_fingerprinters: bool,
    /// Block social media trackers
    pub block_social: bool,
    /// Block third-party cookies
    pub block_third_party_cookies: bool,
    /// Block all third-party requests
    pub block_all_third_party: bool,
    /// Send Do Not Track header
    pub send_dnt: bool,
    /// Send Global Privacy Control header
    pub send_gpc: bool,
    /// Block tracking parameters in URLs
    pub strip_tracking_params: bool,
    /// Tracking parameters to strip
    pub tracking_params: Vec<String>,
}

impl Default for TrackingSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            level: ProtectionLevel::Standard,
            block_trackers: true,
            block_cryptominers: true,
            block_fingerprinters: true,
            block_social: false,
            block_third_party_cookies: true,
            block_all_third_party: false,
            send_dnt: true,
            send_gpc: true,
            strip_tracking_params: true,
            tracking_params: alloc::vec![
                String::from("utm_source"),
                String::from("utm_medium"),
                String::from("utm_campaign"),
                String::from("utm_content"),
                String::from("utm_term"),
                String::from("fbclid"),
                String::from("gclid"),
                String::from("msclkid"),
                String::from("mc_eid"),
                String::from("igshid"),
                String::from("ref_"),
                String::from("__twitter_impression"),
            ],
        }
    }
}

/// Protection level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProtectionLevel {
    /// Basic: cryptominers and fingerprinters only
    Basic,
    /// Standard: + known trackers and third-party cookies
    #[default]
    Standard,
    /// Strict: + social trackers and all third-party
    Strict,
    /// Custom settings
    Custom,
}

/// Tracking request
#[derive(Debug, Clone)]
pub struct TrackingRequest {
    /// Request URL
    pub url: String,
    /// Site domain (first party)
    pub site_domain: String,
    /// Request domain (may be third party)
    pub request_domain: String,
    /// Is third-party request
    pub is_third_party: bool,
    /// Request type
    pub request_type: RequestType,
}

/// Request type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestType {
    Document,
    Script,
    Image,
    Stylesheet,
    Font,
    XmlHttpRequest,
    Cookie,
    WebSocket,
    Other,
}

/// Block result
#[derive(Debug, Clone)]
pub enum BlockResult {
    /// Allow request
    Allow,
    /// Block request
    Block(BlockReason),
}

/// Block reason
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    Tracker,
    Cryptominer,
    Fingerprinting,
    SocialTracker,
    ThirdPartyCookie,
    ThirdParty,
    CustomBlocklist,
}

impl BlockReason {
    pub fn description(&self) -> &'static str {
        match self {
            Self::Tracker => "Tracker",
            Self::Cryptominer => "Cryptominer",
            Self::Fingerprinting => "Fingerprinting",
            Self::SocialTracker => "Social media tracker",
            Self::ThirdPartyCookie => "Third-party cookie",
            Self::ThirdParty => "Third-party request",
            Self::CustomBlocklist => "Custom blocklist",
        }
    }
}

/// Site-specific tracking settings
#[derive(Debug, Clone)]
pub struct SiteTrackingSettings {
    /// Enable for this site
    pub enabled: bool,
    /// Protection level for this site
    pub level: Option<ProtectionLevel>,
}

impl Default for SiteTrackingSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            level: None,
        }
    }
}

/// Strip tracking parameters from URL
pub fn strip_tracking_params(url: &str, params: &[String]) -> String {
    // Find query string
    if let Some(query_start) = url.find('?') {
        let (base, query) = url.split_at(query_start);
        let query = &query[1..]; // Skip '?'

        // Parse and filter parameters
        let filtered: Vec<&str> = query
            .split('&')
            .filter(|param| {
                let key = param.split('=').next().unwrap_or("");
                !params
                    .iter()
                    .any(|p| key == p || key.starts_with(&alloc::format!("{}_", p)))
            })
            .collect();

        if filtered.is_empty() {
            base.to_string()
        } else {
            alloc::format!("{}?{}", base, filtered.join("&"))
        }
    } else {
        url.to_string()
    }
}

/// Generate tracking protection shield icon state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShieldState {
    /// Protection enabled, nothing blocked
    Enabled,
    /// Protection enabled, something blocked
    Active(u32),
    /// Protection disabled for this site
    Disabled,
    /// Protection off globally
    Off,
}
