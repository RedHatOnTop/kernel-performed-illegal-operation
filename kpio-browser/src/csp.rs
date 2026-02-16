//! Content Security Policy (CSP)
//!
//! Implements W3C Content Security Policy Level 3 for controlling
//! what resources a page can load and execute.
//!
//! # Features
//!
//! - CSP header parsing
//! - Directive enforcement (script-src, style-src, etc.)
//! - Inline script/style hash/nonce validation
//! - Report-only mode for debugging
//! - Violation reporting
//!
//! # Example CSP Header
//!
//! ```text
//! Content-Security-Policy: default-src 'self'; script-src 'self' 'nonce-abc123';
//!                          style-src 'self' 'unsafe-inline'; img-src *; report-uri /csp-report
//! ```

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// CSP policy.
#[derive(Debug, Clone)]
pub struct CspPolicy {
    /// Whether this is report-only mode.
    pub report_only: bool,
    /// Directives in this policy.
    pub directives: Vec<CspDirective>,
    /// Report URI for violations.
    pub report_uri: Option<String>,
}

impl CspPolicy {
    /// Create a new empty policy.
    pub fn new() -> Self {
        Self {
            report_only: false,
            directives: Vec::new(),
            report_uri: None,
        }
    }

    /// Parse CSP from header value.
    pub fn parse(header: &str, report_only: bool) -> Self {
        let mut policy = Self {
            report_only,
            directives: Vec::new(),
            report_uri: None,
        };

        // Split by semicolon
        for directive_str in header.split(';') {
            let directive_str = directive_str.trim();
            if directive_str.is_empty() {
                continue;
            }

            let mut parts = directive_str.split_whitespace();
            let name = match parts.next() {
                Some(n) => n,
                None => continue,
            };

            // Handle report-uri specially
            if name == "report-uri" {
                if let Some(uri) = parts.next() {
                    policy.report_uri = Some(uri.to_string());
                }
                continue;
            }

            // Parse directive
            let directive_type = CspDirectiveType::parse(name);
            let sources: Vec<CspSource> = parts.map(CspSource::parse).collect();

            policy.directives.push(CspDirective {
                directive_type,
                sources,
            });
        }

        policy
    }

    /// Get sources for a directive type.
    pub fn get_sources(&self, directive_type: &CspDirectiveType) -> Option<&[CspSource]> {
        // First, look for exact match
        for directive in &self.directives {
            if &directive.directive_type == directive_type {
                return Some(&directive.sources);
            }
        }

        // Fall back to default-src
        if directive_type != &CspDirectiveType::DefaultSrc {
            for directive in &self.directives {
                if directive.directive_type == CspDirectiveType::DefaultSrc {
                    return Some(&directive.sources);
                }
            }
        }

        None
    }

    /// Check if a resource URL is allowed.
    pub fn allows(
        &self,
        directive_type: &CspDirectiveType,
        url: &str,
        nonce: Option<&str>,
    ) -> CspCheck {
        let sources = match self.get_sources(directive_type) {
            Some(s) => s,
            None => return CspCheck::Allow, // No policy = allow
        };

        // Check each source
        for source in sources {
            if source.matches(url, nonce) {
                return CspCheck::Allow;
            }
        }

        if self.report_only {
            CspCheck::ReportOnly
        } else {
            CspCheck::Block
        }
    }

    /// Check if inline script is allowed.
    pub fn allows_inline_script(&self, nonce: Option<&str>, hash: Option<&str>) -> CspCheck {
        let sources = match self.get_sources(&CspDirectiveType::ScriptSrc) {
            Some(s) => s,
            None => return CspCheck::Allow,
        };

        for source in sources {
            match source {
                CspSource::UnsafeInline => return CspCheck::Allow,
                CspSource::Nonce(n) if Some(n.as_str()) == nonce => return CspCheck::Allow,
                CspSource::Hash(alg, h)
                    if Some(alloc::format!("{}-{}", alg, h).as_str()) == hash =>
                {
                    return CspCheck::Allow;
                }
                _ => {}
            }
        }

        if self.report_only {
            CspCheck::ReportOnly
        } else {
            CspCheck::Block
        }
    }

    /// Check if inline style is allowed.
    pub fn allows_inline_style(&self, nonce: Option<&str>, hash: Option<&str>) -> CspCheck {
        let sources = match self.get_sources(&CspDirectiveType::StyleSrc) {
            Some(s) => s,
            None => return CspCheck::Allow,
        };

        for source in sources {
            match source {
                CspSource::UnsafeInline => return CspCheck::Allow,
                CspSource::Nonce(n) if Some(n.as_str()) == nonce => return CspCheck::Allow,
                CspSource::Hash(alg, h)
                    if Some(alloc::format!("{}-{}", alg, h).as_str()) == hash =>
                {
                    return CspCheck::Allow;
                }
                _ => {}
            }
        }

        if self.report_only {
            CspCheck::ReportOnly
        } else {
            CspCheck::Block
        }
    }

    /// Check if eval is allowed.
    pub fn allows_eval(&self) -> CspCheck {
        let sources = match self.get_sources(&CspDirectiveType::ScriptSrc) {
            Some(s) => s,
            None => return CspCheck::Allow,
        };

        for source in sources {
            if matches!(source, CspSource::UnsafeEval) {
                return CspCheck::Allow;
            }
        }

        if self.report_only {
            CspCheck::ReportOnly
        } else {
            CspCheck::Block
        }
    }
}

impl Default for CspPolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// CSP check result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CspCheck {
    /// Resource is allowed.
    Allow,
    /// Resource is blocked.
    Block,
    /// Would be blocked, but report-only mode.
    ReportOnly,
}

/// CSP directive.
#[derive(Debug, Clone)]
pub struct CspDirective {
    /// Directive type.
    pub directive_type: CspDirectiveType,
    /// Sources allowed by this directive.
    pub sources: Vec<CspSource>,
}

/// CSP directive types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CspDirectiveType {
    /// Fallback for all resource types.
    DefaultSrc,
    /// Scripts.
    ScriptSrc,
    /// Stylesheets.
    StyleSrc,
    /// Images.
    ImgSrc,
    /// Media (audio/video).
    MediaSrc,
    /// Fonts.
    FontSrc,
    /// XMLHttpRequest/fetch.
    ConnectSrc,
    /// Objects (embed, object, applet).
    ObjectSrc,
    /// Workers and shared workers.
    WorkerSrc,
    /// Frame sources.
    FrameSrc,
    /// Which documents can frame this page.
    FrameAncestors,
    /// Form action destinations.
    FormAction,
    /// Base URIs.
    BaseUri,
    /// Plugin types.
    PluginTypes,
    /// Manifest file.
    ManifestSrc,
    /// WebSocket connections.
    WsSrc,
    /// Unknown directive.
    Unknown(String),
}

impl CspDirectiveType {
    /// Parse directive type from string.
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "default-src" => Self::DefaultSrc,
            "script-src" => Self::ScriptSrc,
            "style-src" => Self::StyleSrc,
            "img-src" => Self::ImgSrc,
            "media-src" => Self::MediaSrc,
            "font-src" => Self::FontSrc,
            "connect-src" => Self::ConnectSrc,
            "object-src" => Self::ObjectSrc,
            "worker-src" => Self::WorkerSrc,
            "frame-src" => Self::FrameSrc,
            "frame-ancestors" => Self::FrameAncestors,
            "form-action" => Self::FormAction,
            "base-uri" => Self::BaseUri,
            "plugin-types" => Self::PluginTypes,
            "manifest-src" => Self::ManifestSrc,
            _ => Self::Unknown(s.to_string()),
        }
    }
}

/// CSP source expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CspSource {
    /// Match self origin.
    Self_,
    /// Match all sources.
    Star,
    /// Match nothing.
    None,
    /// Allow unsafe inline scripts/styles.
    UnsafeInline,
    /// Allow eval().
    UnsafeEval,
    /// Allow WebAssembly eval.
    WasmUnsafeEval,
    /// Strict dynamic (propagates trust to dynamically loaded scripts).
    StrictDynamic,
    /// Nonce-based source.
    Nonce(String),
    /// Hash-based source.
    Hash(String, String), // (algorithm, hash)
    /// Scheme source (e.g., "https:").
    Scheme(String),
    /// Host source (e.g., "*.example.com").
    Host {
        scheme: Option<String>,
        host: String,
        port: Option<String>,
        path: Option<String>,
    },
    /// Data URI.
    Data,
    /// Blob URI.
    Blob,
    /// Mediastream URI.
    Mediastream,
    /// Filesystem URI.
    Filesystem,
}

impl CspSource {
    /// Parse a source expression.
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "'self'" => Self::Self_,
            "*" => Self::Star,
            "'none'" => Self::None,
            "'unsafe-inline'" => Self::UnsafeInline,
            "'unsafe-eval'" => Self::UnsafeEval,
            "'wasm-unsafe-eval'" => Self::WasmUnsafeEval,
            "'strict-dynamic'" => Self::StrictDynamic,
            "data:" => Self::Data,
            "blob:" => Self::Blob,
            "mediastream:" => Self::Mediastream,
            "filesystem:" => Self::Filesystem,
            _ => {
                // Check for nonce
                if let Some(stripped) = s.strip_prefix("'nonce-") {
                    if let Some(nonce) = stripped.strip_suffix("'") {
                        return Self::Nonce(nonce.to_string());
                    }
                }

                // Check for hash
                for alg in &["sha256", "sha384", "sha512"] {
                    let prefix = alloc::format!("'{}-", alg);
                    if let Some(stripped) = s.strip_prefix(&prefix) {
                        if let Some(hash) = stripped.strip_suffix("'") {
                            return Self::Hash(alg.to_string(), hash.to_string());
                        }
                    }
                }

                // Check for scheme
                if s.ends_with(':') && !s.contains('/') {
                    return Self::Scheme(s.trim_end_matches(':').to_string());
                }

                // Parse as host source
                Self::parse_host(s)
            }
        }
    }

    /// Parse host source.
    fn parse_host(s: &str) -> Self {
        let mut scheme = None;
        let mut rest = s;

        // Extract scheme
        if let Some((sch, r)) = s.split_once("://") {
            scheme = Some(sch.to_string());
            rest = r;
        }

        // Extract path
        let (host_port, path) = if let Some((hp, p)) = rest.split_once('/') {
            (hp, Some(alloc::format!("/{}", p)))
        } else {
            (rest, None)
        };

        // Extract port
        let (host, port) = if let Some((h, p)) = host_port.rsplit_once(':') {
            // Check if it's IPv6 address
            if h.contains(':') && !h.ends_with(']') {
                (host_port.to_string(), None)
            } else {
                (h.to_string(), Some(p.to_string()))
            }
        } else {
            (host_port.to_string(), None)
        };

        Self::Host {
            scheme,
            host,
            port,
            path,
        }
    }

    /// Check if source matches a URL.
    pub fn matches(&self, url: &str, nonce: Option<&str>) -> bool {
        match self {
            Self::Star => true,
            Self::None => false,
            Self::Self_ => {
                // Would need page origin to check properly
                // For now, assume relative URLs match
                !url.contains("://")
            }
            Self::Nonce(n) => nonce == Some(n.as_str()),
            Self::Data => url.starts_with("data:"),
            Self::Blob => url.starts_with("blob:"),
            Self::Scheme(s) => url.starts_with(&alloc::format!("{}:", s)),
            Self::Host {
                scheme,
                host,
                port,
                path,
            } => self.matches_host(
                url,
                scheme.as_deref(),
                host,
                port.as_deref(),
                path.as_deref(),
            ),
            _ => false,
        }
    }

    /// Check if host source matches URL.
    fn matches_host(
        &self,
        url: &str,
        scheme: Option<&str>,
        host: &str,
        port: Option<&str>,
        path: Option<&str>,
    ) -> bool {
        // Parse URL
        let (url_scheme, rest) = match url.split_once("://") {
            Some((s, r)) => (s, r),
            None => return false,
        };

        // Check scheme
        if let Some(s) = scheme {
            if url_scheme != s {
                return false;
            }
        }

        // Extract host from URL
        let (url_host_port, url_path) = rest.split_once('/').unwrap_or((rest, ""));
        let (url_host, url_port) = if let Some((h, p)) = url_host_port.rsplit_once(':') {
            (h, Some(p))
        } else {
            (url_host_port, None)
        };

        // Check host (support wildcards)
        if host.starts_with("*.") {
            let suffix = &host[1..];
            if !url_host.ends_with(suffix) && url_host != &host[2..] {
                return false;
            }
        } else if url_host != host {
            return false;
        }

        // Check port
        if let Some(p) = port {
            if p != "*" && url_port != Some(p) {
                return false;
            }
        }

        // Check path
        if let Some(p) = path {
            let check_path = alloc::format!("/{}", url_path);
            if !check_path.starts_with(p) {
                return false;
            }
        }

        true
    }
}

/// CSP violation report.
#[derive(Debug, Clone)]
pub struct CspViolation {
    /// Document URI where violation occurred.
    pub document_uri: String,
    /// Violated directive.
    pub violated_directive: String,
    /// Effective directive.
    pub effective_directive: String,
    /// Original policy.
    pub original_policy: String,
    /// Blocked URI.
    pub blocked_uri: String,
    /// Source file.
    pub source_file: Option<String>,
    /// Line number.
    pub line_number: Option<u32>,
    /// Column number.
    pub column_number: Option<u32>,
    /// Status code.
    pub status_code: u16,
}

impl CspViolation {
    /// Create a new violation report.
    pub fn new(
        document_uri: String,
        violated_directive: String,
        effective_directive: String,
        original_policy: String,
        blocked_uri: String,
    ) -> Self {
        Self {
            document_uri,
            violated_directive,
            effective_directive,
            original_policy,
            blocked_uri,
            source_file: None,
            line_number: None,
            column_number: None,
            status_code: 0,
        }
    }
}

/// CSP enforcement context for a document.
pub struct CspContext {
    /// Policies for this document.
    policies: Vec<CspPolicy>,
    /// Document URI.
    document_uri: String,
    /// Violations collected.
    violations: Vec<CspViolation>,
}

impl CspContext {
    /// Create a new CSP context.
    pub fn new(document_uri: String) -> Self {
        Self {
            policies: Vec::new(),
            document_uri,
            violations: Vec::new(),
        }
    }

    /// Add a policy.
    pub fn add_policy(&mut self, policy: CspPolicy) {
        self.policies.push(policy);
    }

    /// Check if resource is allowed (checks all policies).
    pub fn allows(
        &mut self,
        directive_type: &CspDirectiveType,
        url: &str,
        nonce: Option<&str>,
    ) -> bool {
        let mut allowed = true;
        let mut violations = Vec::new();

        for policy in &self.policies {
            match policy.allows(directive_type, url, nonce) {
                CspCheck::Allow => {}
                CspCheck::Block => {
                    allowed = false;
                    violations.push((directive_type.clone(), url.to_string()));
                }
                CspCheck::ReportOnly => {
                    violations.push((directive_type.clone(), url.to_string()));
                }
            }
        }

        // Record violations after iteration
        for (directive, blocked_uri) in violations {
            self.record_violation_simple(&directive, &blocked_uri);
        }

        allowed
    }

    /// Check if inline script is allowed.
    pub fn allows_inline_script(&mut self, nonce: Option<&str>, hash: Option<&str>) -> bool {
        let mut allowed = true;
        let mut has_violation = false;

        for policy in &self.policies {
            match policy.allows_inline_script(nonce, hash) {
                CspCheck::Allow => {}
                CspCheck::Block => {
                    allowed = false;
                    has_violation = true;
                }
                CspCheck::ReportOnly => {
                    has_violation = true;
                }
            }
        }

        if has_violation {
            self.record_violation_simple(&CspDirectiveType::ScriptSrc, "inline");
        }

        allowed
    }

    /// Record a violation (simple version).
    fn record_violation_simple(&mut self, directive: &CspDirectiveType, blocked_uri: &str) {
        let directive_name = match directive {
            CspDirectiveType::DefaultSrc => "default-src",
            CspDirectiveType::ScriptSrc => "script-src",
            CspDirectiveType::StyleSrc => "style-src",
            CspDirectiveType::ImgSrc => "img-src",
            _ => "unknown",
        };

        self.violations.push(CspViolation::new(
            self.document_uri.clone(),
            directive_name.to_string(),
            directive_name.to_string(),
            String::new(), // Would serialize policy here
            blocked_uri.to_string(),
        ));
    }

    /// Get collected violations.
    pub fn violations(&self) -> &[CspViolation] {
        &self.violations
    }

    /// Clear violations.
    pub fn clear_violations(&mut self) {
        self.violations.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_policy() {
        let policy = CspPolicy::parse(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; img-src *",
            false,
        );

        assert_eq!(policy.directives.len(), 3);
    }

    #[test]
    fn test_source_matching() {
        let source = CspSource::Host {
            scheme: Some("https".to_string()),
            host: "*.example.com".to_string(),
            port: None,
            path: None,
        };

        assert!(source.matches("https://sub.example.com/path", None));
        assert!(source.matches("https://example.com/path", None));
        assert!(!source.matches("http://example.com/path", None));
        assert!(!source.matches("https://other.com/path", None));
    }

    #[test]
    fn test_nonce() {
        let policy = CspPolicy::parse("script-src 'nonce-abc123'", false);

        assert_eq!(
            policy.allows_inline_script(Some("abc123"), None),
            CspCheck::Allow
        );
        assert_eq!(
            policy.allows_inline_script(Some("wrong"), None),
            CspCheck::Block
        );
        assert_eq!(policy.allows_inline_script(None, None), CspCheck::Block);
    }
}
