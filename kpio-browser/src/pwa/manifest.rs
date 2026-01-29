//! Web App Manifest Parser
//!
//! Parses and validates Web App Manifest files according to W3C spec.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::{DisplayMode, AppIcon, IconPurpose, PwaError};

/// Web App Manifest
#[derive(Debug, Clone)]
pub struct WebAppManifest {
    /// Application name
    pub name: Option<String>,
    /// Short name (for icons)
    pub short_name: Option<String>,
    /// Start URL
    pub start_url: String,
    /// Scope
    pub scope: Option<String>,
    /// Display mode
    pub display: DisplayMode,
    /// Orientation
    pub orientation: Orientation,
    /// Theme color
    pub theme_color: Option<String>,
    /// Background color
    pub background_color: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Icons
    pub icons: Vec<ManifestIcon>,
    /// Screenshots
    pub screenshots: Vec<ManifestScreenshot>,
    /// Shortcuts
    pub shortcuts: Vec<ManifestShortcut>,
    /// Related applications
    pub related_applications: Vec<RelatedApplication>,
    /// Prefer related applications
    pub prefer_related_applications: bool,
    /// Categories
    pub categories: Vec<String>,
    /// IARC rating ID
    pub iarc_rating_id: Option<String>,
    /// Dir (text direction)
    pub dir: TextDirection,
    /// Language
    pub lang: Option<String>,
    /// Share target
    pub share_target: Option<ShareTarget>,
    /// Protocol handlers
    pub protocol_handlers: Vec<ProtocolHandler>,
    /// File handlers
    pub file_handlers: Vec<FileHandler>,
    /// Display override
    pub display_override: Vec<DisplayMode>,
    /// ID
    pub id: Option<String>,
}

impl Default for WebAppManifest {
    fn default() -> Self {
        Self {
            name: None,
            short_name: None,
            start_url: "/".to_string(),
            scope: None,
            display: DisplayMode::Browser,
            orientation: Orientation::Any,
            theme_color: None,
            background_color: None,
            description: None,
            icons: Vec::new(),
            screenshots: Vec::new(),
            shortcuts: Vec::new(),
            related_applications: Vec::new(),
            prefer_related_applications: false,
            categories: Vec::new(),
            iarc_rating_id: None,
            dir: TextDirection::Auto,
            lang: None,
            share_target: None,
            protocol_handlers: Vec::new(),
            file_handlers: Vec::new(),
            display_override: Vec::new(),
            id: None,
        }
    }
}

impl WebAppManifest {
    /// Create new manifest
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse from JSON string
    pub fn parse(json: &str) -> Result<Self, PwaError> {
        // Simple JSON parser for manifest
        // In production, would use a proper JSON parser
        let mut manifest = Self::default();

        // Very basic parsing - just extracts simple string fields
        // Real implementation would use serde_json or similar

        if let Some(name) = extract_string_field(json, "name") {
            manifest.name = Some(name);
        }

        if let Some(short_name) = extract_string_field(json, "short_name") {
            manifest.short_name = Some(short_name);
        }

        if let Some(start_url) = extract_string_field(json, "start_url") {
            manifest.start_url = start_url;
        }

        if let Some(scope) = extract_string_field(json, "scope") {
            manifest.scope = Some(scope);
        }

        if let Some(display) = extract_string_field(json, "display") {
            manifest.display = DisplayMode::from_str(&display);
        }

        if let Some(theme_color) = extract_string_field(json, "theme_color") {
            manifest.theme_color = Some(theme_color);
        }

        if let Some(background_color) = extract_string_field(json, "background_color") {
            manifest.background_color = Some(background_color);
        }

        if let Some(description) = extract_string_field(json, "description") {
            manifest.description = Some(description);
        }

        if let Some(id) = extract_string_field(json, "id") {
            manifest.id = Some(id);
        }

        Ok(manifest)
    }

    /// Get effective start URL
    pub fn effective_start_url(&self, manifest_url: &str) -> String {
        if self.start_url.starts_with("http://") || self.start_url.starts_with("https://") {
            self.start_url.clone()
        } else {
            // Resolve relative to manifest URL
            resolve_url(manifest_url, &self.start_url)
        }
    }

    /// Get effective scope
    pub fn effective_scope(&self, manifest_url: &str) -> String {
        if let Some(ref scope) = self.scope {
            if scope.starts_with("http://") || scope.starts_with("https://") {
                scope.clone()
            } else {
                resolve_url(manifest_url, scope)
            }
        } else {
            // Default scope is directory of start_url
            let start = self.effective_start_url(manifest_url);
            if let Some(pos) = start.rfind('/') {
                start[..=pos].to_string()
            } else {
                start
            }
        }
    }

    /// Get display name
    pub fn display_name(&self) -> &str {
        self.short_name.as_deref()
            .or(self.name.as_deref())
            .unwrap_or("Web App")
    }

    /// Get best icon for size
    pub fn best_icon(&self, size: u32) -> Option<&ManifestIcon> {
        // Find exact match first
        let exact = self.icons.iter().find(|icon| {
            icon.sizes.split_whitespace().any(|s| {
                if let Some((w, _h)) = s.split_once('x') {
                    w.parse::<u32>().ok() == Some(size)
                } else {
                    false
                }
            })
        });

        if exact.is_some() {
            return exact;
        }

        // Find closest larger icon
        self.icons.iter()
            .filter(|icon| {
                icon.sizes.split_whitespace().any(|s| {
                    if let Some((w, _h)) = s.split_once('x') {
                        w.parse::<u32>().ok().map(|w| w >= size).unwrap_or(false)
                    } else {
                        false
                    }
                })
            })
            .next()
            .or_else(|| self.icons.first())
    }

    /// Check if manifest is valid for installation
    pub fn is_valid_for_install(&self) -> bool {
        self.name.is_some() || self.short_name.is_some()
    }

    /// Convert theme color string to u32
    pub fn theme_color_u32(&self) -> Option<u32> {
        self.theme_color.as_ref().and_then(|s| parse_color(s))
    }

    /// Convert background color string to u32
    pub fn background_color_u32(&self) -> Option<u32> {
        self.background_color.as_ref().and_then(|s| parse_color(s))
    }
}

/// Manifest icon
#[derive(Debug, Clone)]
pub struct ManifestIcon {
    /// Source URL
    pub src: String,
    /// Sizes (e.g., "192x192 512x512")
    pub sizes: String,
    /// MIME type
    pub icon_type: Option<String>,
    /// Purpose
    pub purpose: Vec<IconPurpose>,
}

impl ManifestIcon {
    /// Convert to AppIcon
    pub fn to_app_icon(&self) -> AppIcon {
        AppIcon {
            src: self.src.clone(),
            sizes: self.sizes.clone(),
            icon_type: self.icon_type.clone().unwrap_or_default(),
            purpose: self.purpose.first().copied().unwrap_or_default(),
        }
    }
}

/// Manifest screenshot
#[derive(Debug, Clone)]
pub struct ManifestScreenshot {
    /// Source URL
    pub src: String,
    /// Sizes
    pub sizes: Option<String>,
    /// MIME type
    pub screenshot_type: Option<String>,
    /// Label
    pub label: Option<String>,
    /// Form factor
    pub form_factor: Option<FormFactor>,
}

/// Form factor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormFactor {
    /// Wide (desktop)
    Wide,
    /// Narrow (mobile)
    Narrow,
}

/// Manifest shortcut
#[derive(Debug, Clone)]
pub struct ManifestShortcut {
    /// Name
    pub name: String,
    /// Short name
    pub short_name: Option<String>,
    /// Description
    pub description: Option<String>,
    /// URL
    pub url: String,
    /// Icons
    pub icons: Vec<ManifestIcon>,
}

/// Related application
#[derive(Debug, Clone)]
pub struct RelatedApplication {
    /// Platform
    pub platform: String,
    /// URL
    pub url: Option<String>,
    /// ID
    pub id: Option<String>,
}

/// Text direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDirection {
    /// Auto
    Auto,
    /// Left to right
    Ltr,
    /// Right to left
    Rtl,
}

impl Default for TextDirection {
    fn default() -> Self {
        Self::Auto
    }
}

/// Orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Any orientation
    Any,
    /// Natural
    Natural,
    /// Landscape
    Landscape,
    /// Portrait
    Portrait,
    /// Portrait primary
    PortraitPrimary,
    /// Portrait secondary
    PortraitSecondary,
    /// Landscape primary
    LandscapePrimary,
    /// Landscape secondary
    LandscapeSecondary,
}

impl Default for Orientation {
    fn default() -> Self {
        Self::Any
    }
}

/// Share target
#[derive(Debug, Clone)]
pub struct ShareTarget {
    /// Action URL
    pub action: String,
    /// HTTP method
    pub method: ShareMethod,
    /// Encoding type
    pub enctype: ShareEnctype,
    /// Parameters
    pub params: ShareParams,
}

/// Share method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareMethod {
    /// GET
    Get,
    /// POST
    Post,
}

impl Default for ShareMethod {
    fn default() -> Self {
        Self::Get
    }
}

/// Share encoding type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareEnctype {
    /// URL encoded
    UrlEncoded,
    /// Multipart form data
    Multipart,
}

impl Default for ShareEnctype {
    fn default() -> Self {
        Self::UrlEncoded
    }
}

/// Share parameters
#[derive(Debug, Clone, Default)]
pub struct ShareParams {
    /// Title parameter name
    pub title: Option<String>,
    /// Text parameter name
    pub text: Option<String>,
    /// URL parameter name
    pub url: Option<String>,
    /// Files
    pub files: Vec<ShareFile>,
}

/// Share file configuration
#[derive(Debug, Clone)]
pub struct ShareFile {
    /// Parameter name
    pub name: String,
    /// Accepted MIME types
    pub accept: Vec<String>,
}

/// Protocol handler
#[derive(Debug, Clone)]
pub struct ProtocolHandler {
    /// Protocol
    pub protocol: String,
    /// URL
    pub url: String,
}

/// File handler
#[derive(Debug, Clone)]
pub struct FileHandler {
    /// Action URL
    pub action: String,
    /// Accept
    pub accept: BTreeMap<String, Vec<String>>,
    /// Launch type
    pub launch_type: LaunchType,
}

/// File handler launch type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchType {
    /// Single client
    SingleClient,
    /// Multiple clients
    MultipleClients,
}

impl Default for LaunchType {
    fn default() -> Self {
        Self::SingleClient
    }
}

// Helper functions

/// Extract a simple string field from JSON
fn extract_string_field(json: &str, field: &str) -> Option<String> {
    let pattern = alloc::format!("\"{}\"", field);
    let pos = json.find(&pattern)?;
    let rest = &json[pos + pattern.len()..];
    
    // Skip whitespace and colon
    let rest = rest.trim_start();
    if !rest.starts_with(':') {
        return None;
    }
    let rest = rest[1..].trim_start();
    
    // Extract string value
    if !rest.starts_with('"') {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Resolve a relative URL against a base
fn resolve_url(base: &str, relative: &str) -> String {
    if relative.starts_with("http://") || relative.starts_with("https://") {
        return relative.to_string();
    }

    if relative.starts_with('/') {
        // Get origin from base
        if let Some(pos) = base.find("://") {
            if let Some(path_start) = base[pos + 3..].find('/') {
                let origin = &base[..pos + 3 + path_start];
                return alloc::format!("{}{}", origin, relative);
            }
        }
        return relative.to_string();
    }

    // Relative to base path
    if let Some(pos) = base.rfind('/') {
        alloc::format!("{}/{}", &base[..pos], relative)
    } else {
        relative.to_string()
    }
}

/// Parse color string to u32 (ARGB)
fn parse_color(s: &str) -> Option<u32> {
    let s = s.trim();
    
    // Handle #RGB or #RRGGBB
    if s.starts_with('#') {
        let hex = &s[1..];
        let val = u32::from_str_radix(hex, 16).ok()?;
        
        if hex.len() == 3 {
            // #RGB -> #RRGGBB
            let r = ((val >> 8) & 0xF) * 0x11;
            let g = ((val >> 4) & 0xF) * 0x11;
            let b = (val & 0xF) * 0x11;
            return Some(0xFF000000 | (r << 16) | (g << 8) | b);
        } else if hex.len() == 6 {
            return Some(0xFF000000 | val);
        } else if hex.len() == 8 {
            // #RRGGBBAA -> AARRGGBB
            let a = val & 0xFF;
            let rgb = val >> 8;
            return Some((a << 24) | rgb);
        }
    }

    // Could add rgb(), rgba() parsing here

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color() {
        assert_eq!(parse_color("#000"), Some(0xFF000000));
        assert_eq!(parse_color("#fff"), Some(0xFFFFFFFF));
        assert_eq!(parse_color("#ff0000"), Some(0xFFFF0000));
        assert_eq!(parse_color("#00ff00"), Some(0xFF00FF00));
    }
}
