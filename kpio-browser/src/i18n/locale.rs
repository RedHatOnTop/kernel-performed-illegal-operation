//! Locale Management
//!
//! Locale detection, negotiation, and preferences.

use super::Direction;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Locale
#[derive(Debug, Clone)]
pub struct Locale {
    /// Language code (ISO 639-1)
    pub language: String,
    /// Country/region code (ISO 3166-1)
    pub region: Option<String>,
    /// Script code (ISO 15924)
    pub script: Option<String>,
    /// Variant
    pub variant: Option<String>,
}

impl Locale {
    /// Create new locale
    pub fn new(language: &str) -> Self {
        Self {
            language: language.to_string(),
            region: None,
            script: None,
            variant: None,
        }
    }

    /// Parse locale from string
    pub fn parse(tag: &str) -> Option<Self> {
        let parts: Vec<&str> = tag.split(|c| c == '-' || c == '_').collect();

        if parts.is_empty() {
            return None;
        }

        let mut locale = Self::new(parts[0]);

        // Parse remaining parts
        for part in parts.iter().skip(1) {
            if part.len() == 2 && part.chars().all(|c| c.is_ascii_uppercase()) {
                locale.region = Some(part.to_string());
            } else if part.len() == 4
                && part
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_uppercase())
                    .unwrap_or(false)
            {
                locale.script = Some(part.to_string());
            } else {
                locale.variant = Some(part.to_string());
            }
        }

        Some(locale)
    }

    /// Convert to BCP 47 tag
    pub fn to_tag(&self) -> String {
        let mut tag = self.language.clone();

        if let Some(ref script) = self.script {
            tag.push('-');
            tag.push_str(script);
        }

        if let Some(ref region) = self.region {
            tag.push('-');
            tag.push_str(region);
        }

        if let Some(ref variant) = self.variant {
            tag.push('-');
            tag.push_str(variant);
        }

        tag
    }

    /// Get direction for this locale
    pub fn direction(&self) -> Direction {
        match self.language.as_str() {
            "ar" | "he" | "fa" | "ur" | "yi" | "ps" | "sd" | "ug" => Direction::Rtl,
            _ => Direction::Ltr,
        }
    }

    /// Is RTL
    pub fn is_rtl(&self) -> bool {
        self.direction() == Direction::Rtl
    }

    /// Check if matches other locale
    pub fn matches(&self, other: &Locale) -> bool {
        if self.language != other.language {
            return false;
        }

        // If regions specified, they must match
        if let (Some(ref r1), Some(ref r2)) = (&self.region, &other.region) {
            if r1 != r2 {
                return false;
            }
        }

        // If scripts specified, they must match
        if let (Some(ref s1), Some(ref s2)) = (&self.script, &other.script) {
            if s1 != s2 {
                return false;
            }
        }

        true
    }
}

/// Locale negotiation
pub struct LocaleNegotiator {
    /// Available locales
    available: Vec<Locale>,
    /// Default locale
    default: Locale,
}

impl LocaleNegotiator {
    /// Create new negotiator
    pub fn new(available: Vec<Locale>, default: Locale) -> Self {
        Self { available, default }
    }

    /// Negotiate best matching locale
    pub fn negotiate(&self, requested: &[Locale]) -> Locale {
        // Try exact match first
        for req in requested {
            for avail in &self.available {
                if req.to_tag() == avail.to_tag() {
                    return avail.clone();
                }
            }
        }

        // Try language + region match
        for req in requested {
            for avail in &self.available {
                if req.language == avail.language && req.region == avail.region {
                    return avail.clone();
                }
            }
        }

        // Try language-only match
        for req in requested {
            for avail in &self.available {
                if req.language == avail.language {
                    return avail.clone();
                }
            }
        }

        // Return default
        self.default.clone()
    }

    /// Negotiate from Accept-Language header
    pub fn negotiate_from_header(&self, header: &str) -> Locale {
        let requested = self.parse_accept_language(header);
        self.negotiate(&requested)
    }

    /// Parse Accept-Language header
    fn parse_accept_language(&self, header: &str) -> Vec<Locale> {
        let mut locales: Vec<(Locale, f32)> = Vec::new();

        for part in header.split(',') {
            let part = part.trim();
            let (tag, q) = if let Some(pos) = part.find(";q=") {
                let q_str = &part[pos + 3..];
                let q = q_str.parse::<f32>().unwrap_or(1.0);
                (&part[..pos], q)
            } else {
                (part, 1.0)
            };

            if let Some(locale) = Locale::parse(tag) {
                locales.push((locale, q));
            }
        }

        // Sort by quality
        locales.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));

        locales.into_iter().map(|(l, _)| l).collect()
    }
}

/// Locale preferences
#[derive(Debug, Clone)]
pub struct LocalePreferences {
    /// Preferred locale
    pub preferred: Locale,
    /// Alternative locales
    pub alternatives: Vec<Locale>,
    /// Override system locale
    pub override_system: bool,
    /// Use 24-hour time
    pub use_24_hour: bool,
    /// First day of week (0 = Sunday, 1 = Monday)
    pub first_day_of_week: u8,
    /// Temperature unit
    pub temperature_unit: TemperatureUnit,
    /// Distance unit
    pub distance_unit: DistanceUnit,
}

/// Temperature unit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemperatureUnit {
    /// Celsius
    Celsius,
    /// Fahrenheit
    Fahrenheit,
}

impl Default for TemperatureUnit {
    fn default() -> Self {
        Self::Celsius
    }
}

/// Distance unit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceUnit {
    /// Metric (km, m)
    Metric,
    /// Imperial (mi, ft)
    Imperial,
}

impl Default for DistanceUnit {
    fn default() -> Self {
        Self::Metric
    }
}

impl Default for LocalePreferences {
    fn default() -> Self {
        Self {
            preferred: Locale::new("en"),
            alternatives: Vec::new(),
            override_system: false,
            use_24_hour: true,
            first_day_of_week: 1, // Monday
            temperature_unit: TemperatureUnit::Celsius,
            distance_unit: DistanceUnit::Metric,
        }
    }
}
