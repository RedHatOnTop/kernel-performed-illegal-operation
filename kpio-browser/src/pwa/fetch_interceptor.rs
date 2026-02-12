//! Fetch Interceptor
//!
//! Intercepts network requests from web apps and routes them through the
//! Service Worker pipeline. When no SW is active (or the SW doesn't
//! respond), falls back to direct network request or cache-only mode.
//!
//! ## Cache-Only Mode
//!
//! When the JS engine cannot execute SW scripts, the interceptor falls
//! back to a URL-pattern based cache strategy defined in
//! `sw_cache_config.json`.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::cache_storage::{CacheStorage, CachedResponse};
use super::sw_bridge::{SwState, SW_BRIDGE};

// ── Types ───────────────────────────────────────────────────

/// Fetch strategy for cache-only mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStrategy {
    /// Try cache first; on miss, fetch from network and cache the result.
    CacheFirst,
    /// Try network first; on failure, fall back to cache.
    NetworkFirst,
    /// Only serve from cache; never go to network.
    CacheOnly,
    /// Only fetch from network; never check cache.
    NetworkOnly,
    /// Serve stale from cache immediately, update cache in background.
    StaleWhileRevalidate,
}

/// A URL pattern rule for cache-only mode.
#[derive(Debug, Clone)]
pub struct CacheRule {
    /// Glob-like URL pattern (e.g., `"/**/*.css"`, `"/api/*"`).
    pub pattern: String,
    /// The caching strategy to use for matching URLs.
    pub strategy: CacheStrategy,
    /// Optional cache name to use (defaults to `"default"`).
    pub cache_name: String,
}

/// Result of intercepting a fetch request.
#[derive(Debug, Clone)]
pub enum FetchResult {
    /// Response found (from cache or network).
    Response(FetchResponse),
    /// No match — caller should proceed with normal network fetch.
    Passthrough,
    /// Error during fetch.
    Error(String),
}

/// A simplified HTTP response returned by the interceptor.
#[derive(Debug, Clone)]
pub struct FetchResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Where this response came from.
    pub source: FetchSource,
}

/// Indicates where a fetch response originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchSource {
    /// From the Cache Storage API.
    Cache,
    /// From the network.
    Network,
    /// From the Service Worker's `respondWith()`.
    ServiceWorker,
}

/// The fetch interceptor.
pub struct FetchInterceptor {
    /// Cache-only mode rules (used when SW JS execution is not available).
    rules: Vec<CacheRule>,
    /// Whether to log fetch events for debugging.
    pub debug_logging: bool,
}

/// Global fetch interceptor instance.
pub static FETCH_INTERCEPTOR: spin::RwLock<FetchInterceptor> =
    spin::RwLock::new(FetchInterceptor::new());

// ── Implementation ──────────────────────────────────────────

impl FetchInterceptor {
    pub const fn new() -> Self {
        Self {
            rules: Vec::new(),
            debug_logging: false,
        }
    }

    /// Add a cache-only mode rule.
    pub fn add_rule(&mut self, pattern: &str, strategy: CacheStrategy, cache_name: &str) {
        self.rules.push(CacheRule {
            pattern: String::from(pattern),
            strategy,
            cache_name: String::from(cache_name),
        });
    }

    /// Clear all cache-only rules.
    pub fn clear_rules(&mut self) {
        self.rules.clear();
    }

    /// Load rules from a JSON config string.
    ///
    /// Expected format:
    /// ```json
    /// { "patterns": [
    ///   { "url": "/**/*.css", "strategy": "cache-first", "cache": "static" }
    /// ]}
    /// ```
    pub fn load_rules_from_json(&mut self, json: &str) {
        // Minimal hand-rolled parser
        self.rules.clear();

        // Find each pattern object
        let mut pos = 0;
        while let Some(start) = json[pos..].find('{') {
            let abs_start = pos + start;
            if let Some(end) = json[abs_start..].find('}') {
                let obj = &json[abs_start..abs_start + end + 1];
                if let Some(rule) = Self::parse_rule(obj) {
                    self.rules.push(rule);
                }
                pos = abs_start + end + 1;
            } else {
                break;
            }
        }
    }

    /// Intercept a fetch request for the given URL and app.
    ///
    /// 1. Check if an active SW controls this URL's scope.
    /// 2. If SW available → dispatch FetchEvent (currently placeholder).
    /// 3. If no SW → check cache-only rules.
    /// 4. Apply the matching strategy.
    pub fn intercept(&self, url: &str, cache_storage: &CacheStorage) -> FetchResult {
        // Step 1: Check for active Service Worker
        let sw_bridge = SW_BRIDGE.read();
        let active_sw = sw_bridge.match_scope(url);

        if let Some(sw) = active_sw {
            if sw.state == SwState::Activated {
                // Step 2: In a full implementation, we'd dispatch a FetchEvent
                // to the SW's JS runtime. For now, fall through to cache-only
                // mode since we don't have a JS executor yet.
                //
                // TODO: dispatch FetchEvent to SW runtime
                // let response = sw_runtime.dispatch_fetch(sw.id, url, timeout_5s);
                // if response.is_some() { return FetchResult::Response(response); }
            }
        }
        drop(sw_bridge);

        // Step 3: Cache-only mode — match URL against rules
        if let Some(rule) = self.match_rule(url) {
            return self.apply_strategy(url, &rule, cache_storage);
        }

        // No match → passthrough to normal network
        FetchResult::Passthrough
    }

    /// Find the first rule whose pattern matches the URL.
    fn match_rule(&self, url: &str) -> Option<&CacheRule> {
        for rule in &self.rules {
            if glob_match(&rule.pattern, url) {
                return Some(rule);
            }
        }
        None
    }

    /// Apply a caching strategy for the given URL.
    fn apply_strategy(
        &self,
        url: &str,
        rule: &CacheRule,
        cache_storage: &CacheStorage,
    ) -> FetchResult {
        match rule.strategy {
            CacheStrategy::CacheFirst => {
                // Try cache first
                if let Some(resp) = cache_storage.match_in(&rule.cache_name, url) {
                    return FetchResult::Response(cached_to_fetch(resp, FetchSource::Cache));
                }
                // Cache miss → network would go here, but we return passthrough
                FetchResult::Passthrough
            }
            CacheStrategy::NetworkFirst => {
                // Network would be tried first; on failure fall back to cache
                // Since we can't do real network here, check cache as fallback
                if let Some(resp) = cache_storage.match_in(&rule.cache_name, url) {
                    FetchResult::Response(cached_to_fetch(resp, FetchSource::Cache))
                } else {
                    FetchResult::Passthrough
                }
            }
            CacheStrategy::CacheOnly => {
                if let Some(resp) = cache_storage.match_in(&rule.cache_name, url) {
                    FetchResult::Response(cached_to_fetch(resp, FetchSource::Cache))
                } else {
                    FetchResult::Error(String::from("cache miss (cache-only strategy)"))
                }
            }
            CacheStrategy::NetworkOnly => {
                FetchResult::Passthrough
            }
            CacheStrategy::StaleWhileRevalidate => {
                // Serve from cache immediately (if available)
                if let Some(resp) = cache_storage.match_in(&rule.cache_name, url) {
                    // In a real implementation, we'd also trigger a background
                    // network fetch to update the cache
                    FetchResult::Response(cached_to_fetch(resp, FetchSource::Cache))
                } else {
                    FetchResult::Passthrough
                }
            }
        }
    }

    /// Parse a single rule JSON object.
    fn parse_rule(obj: &str) -> Option<CacheRule> {
        let url = extract_json_string(obj, "url")?;
        let strategy_str = extract_json_string(obj, "strategy").unwrap_or_default();
        let cache = extract_json_string(obj, "cache")
            .unwrap_or_else(|| String::from("default"));

        let strategy = match strategy_str.as_str() {
            "cache-first" => CacheStrategy::CacheFirst,
            "network-first" => CacheStrategy::NetworkFirst,
            "cache-only" => CacheStrategy::CacheOnly,
            "network-only" => CacheStrategy::NetworkOnly,
            "stale-while-revalidate" => CacheStrategy::StaleWhileRevalidate,
            _ => CacheStrategy::CacheFirst, // default
        };

        Some(CacheRule {
            pattern: url,
            strategy,
            cache_name: cache,
        })
    }
}

// ── Helpers ─────────────────────────────────────────────────

/// Convert a `CachedResponse` to a `FetchResponse`.
fn cached_to_fetch(cached: &CachedResponse, source: FetchSource) -> FetchResponse {
    FetchResponse {
        status: cached.status,
        headers: cached
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        body: cached.body.clone(),
        source,
    }
}

/// Simple glob matching supporting `*` (single segment) and `**` (any depth).
///
/// Only handles trailing `*` / `**` patterns common in SW cache configs.
fn glob_match(pattern: &str, input: &str) -> bool {
    if pattern == "**" || pattern == "/**" {
        return true;
    }

    // Handle `/**/*.ext` patterns
    if pattern.starts_with("/**/") {
        let suffix = &pattern[4..]; // e.g., "*.css"
        if suffix.starts_with('*') {
            let ext = &suffix[1..]; // e.g., ".css"
            return input.ends_with(ext);
        }
    }

    // Handle `/path/*` patterns
    if pattern.ends_with("/*") {
        let prefix = &pattern[..pattern.len() - 1];
        return input.starts_with(prefix);
    }

    // Handle exact match
    pattern == input
}

/// Extract a JSON string value for a given key from a simple JSON object.
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let search = alloc::format!("\"{}\"", key);
    let idx = json.find(&search)?;
    let after = &json[idx + search.len()..];

    // Skip `:` and whitespace
    let colon = after.find(':')?;
    let after_colon = after[colon + 1..].trim_start();

    if after_colon.starts_with('"') {
        let start = 1;
        let end = after_colon[start..].find('"')?;
        Some(String::from(&after_colon[start..start + end]))
    } else {
        None
    }
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;

    fn make_cache_storage() -> CacheStorage {
        let mut cs = CacheStorage::new(1);
        cs.put(
            "static",
            "/style.css",
            200,
            BTreeMap::new(),
            b"body{color:red}".to_vec(),
        )
        .unwrap();
        cs.put(
            "static",
            "/app.js",
            200,
            BTreeMap::new(),
            b"console.log('hi')".to_vec(),
        )
        .unwrap();
        cs.put(
            "default",
            "/index.html",
            200,
            BTreeMap::new(),
            b"<html>".to_vec(),
        )
        .unwrap();
        cs
    }

    #[test]
    fn glob_match_extension() {
        assert!(glob_match("/**/*.css", "/style.css"));
        assert!(glob_match("/**/*.css", "/deep/path/main.css"));
        assert!(!glob_match("/**/*.css", "/script.js"));
    }

    #[test]
    fn glob_match_prefix() {
        assert!(glob_match("/api/*", "/api/users"));
        assert!(glob_match("/api/*", "/api/data"));
        assert!(!glob_match("/api/*", "/other/path"));
    }

    #[test]
    fn glob_match_all() {
        assert!(glob_match("/**", "/anything/at/all"));
        assert!(glob_match("**", "anything"));
    }

    #[test]
    fn glob_match_exact() {
        assert!(glob_match("/index.html", "/index.html"));
        assert!(!glob_match("/index.html", "/other.html"));
    }

    #[test]
    fn cache_first_hit() {
        let cs = make_cache_storage();
        let mut interceptor = FetchInterceptor::new();
        interceptor.add_rule("/**/*.css", CacheStrategy::CacheFirst, "static");

        let result = interceptor.intercept("/style.css", &cs);
        match result {
            FetchResult::Response(resp) => {
                assert_eq!(resp.source, FetchSource::Cache);
                assert_eq!(resp.body, b"body{color:red}");
            }
            _ => panic!("expected cache hit"),
        }
    }

    #[test]
    fn cache_first_miss() {
        let cs = make_cache_storage();
        let mut interceptor = FetchInterceptor::new();
        interceptor.add_rule("/**/*.css", CacheStrategy::CacheFirst, "static");

        let result = interceptor.intercept("/missing.css", &cs);
        assert!(matches!(result, FetchResult::Passthrough));
    }

    #[test]
    fn cache_only_miss_returns_error() {
        let cs = make_cache_storage();
        let mut interceptor = FetchInterceptor::new();
        interceptor.add_rule("/**/*.png", CacheStrategy::CacheOnly, "static");

        let result = interceptor.intercept("/image.png", &cs);
        assert!(matches!(result, FetchResult::Error(_)));
    }

    #[test]
    fn network_only_passthrough() {
        let cs = make_cache_storage();
        let mut interceptor = FetchInterceptor::new();
        interceptor.add_rule("/api/*", CacheStrategy::NetworkOnly, "default");

        let result = interceptor.intercept("/api/data", &cs);
        assert!(matches!(result, FetchResult::Passthrough));
    }

    #[test]
    fn no_matching_rule_passthrough() {
        let cs = make_cache_storage();
        let interceptor = FetchInterceptor::new();
        let result = interceptor.intercept("/random/path", &cs);
        assert!(matches!(result, FetchResult::Passthrough));
    }

    #[test]
    fn load_rules_from_json() {
        let json = r#"{ "patterns": [
            { "url": "/**/*.css", "strategy": "cache-first", "cache": "static" },
            { "url": "/api/*", "strategy": "network-first", "cache": "api" }
        ]}"#;

        let mut interceptor = FetchInterceptor::new();
        interceptor.load_rules_from_json(json);
        assert_eq!(interceptor.rules.len(), 2);
        assert_eq!(interceptor.rules[0].pattern, "/**/*.css");
        assert_eq!(interceptor.rules[0].strategy, CacheStrategy::CacheFirst);
        assert_eq!(interceptor.rules[1].pattern, "/api/*");
        assert_eq!(interceptor.rules[1].strategy, CacheStrategy::NetworkFirst);
    }

    #[test]
    fn extract_json_string_works() {
        let obj = r#"{"url": "/**/*.css", "strategy": "cache-first"}"#;
        assert_eq!(
            extract_json_string(obj, "url"),
            Some(String::from("/**/*.css"))
        );
        assert_eq!(
            extract_json_string(obj, "strategy"),
            Some(String::from("cache-first"))
        );
        assert_eq!(extract_json_string(obj, "missing"), None);
    }
}
