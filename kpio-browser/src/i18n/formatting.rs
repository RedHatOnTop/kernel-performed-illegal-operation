//! Number and Date Formatting
//!
//! Locale-aware formatting for numbers, dates, and currencies.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Simple power of 10 for no_std
fn pow10(exp: usize) -> i64 {
    match exp {
        0 => 1,
        1 => 10,
        2 => 100,
        3 => 1000,
        4 => 10000,
        5 => 100000,
        6 => 1000000,
        _ => {
            let mut result = 1i64;
            for _ in 0..exp {
                result *= 10;
            }
            result
        }
    }
}

/// Number formatter
#[derive(Debug, Clone)]
pub struct NumberFormatter {
    /// Decimal separator
    pub decimal_separator: char,
    /// Thousands separator
    pub thousands_separator: char,
    /// Grouping size
    pub grouping_size: usize,
}

impl NumberFormatter {
    /// Create new formatter
    pub const fn new() -> Self {
        Self {
            decimal_separator: '.',
            thousands_separator: ',',
            grouping_size: 3,
        }
    }

    /// Create for locale
    pub fn for_locale(locale: &str) -> Self {
        match locale {
            "de-DE" | "de" | "fr-FR" | "fr" | "es-ES" | "es" | "it-IT" | "it" => {
                Self {
                    decimal_separator: ',',
                    thousands_separator: '.',
                    grouping_size: 3,
                }
            }
            "fr-CA" => {
                Self {
                    decimal_separator: ',',
                    thousands_separator: ' ',
                    grouping_size: 3,
                }
            }
            "hi-IN" | "en-IN" => {
                Self {
                    decimal_separator: '.',
                    thousands_separator: ',',
                    grouping_size: 2, // Indian numbering system
                }
            }
            _ => Self::new(),
        }
    }

    /// Format integer
    pub fn format_int(&self, value: i64) -> String {
        let negative = value < 0;
        let abs_value = value.abs() as u64;
        let mut result = self.format_uint(abs_value);
        
        if negative {
            result.insert(0, '-');
        }
        
        result
    }

    /// Format unsigned integer
    pub fn format_uint(&self, value: u64) -> String {
        let s = alloc::format!("{}", value);
        let chars: Vec<char> = s.chars().collect();
        
        if chars.len() <= self.grouping_size {
            return s;
        }

        let mut result = String::new();
        let mut count = 0;

        for c in chars.iter().rev() {
            if count > 0 && count % self.grouping_size == 0 {
                result.insert(0, self.thousands_separator);
            }
            result.insert(0, *c);
            count += 1;
        }

        result
    }

    /// Format float with precision
    pub fn format_float(&self, value: f64, precision: usize) -> String {
        // Simple implementation for no_std
        let multiplier = pow10(precision);
        let rounded = ((value * multiplier as f64) + 0.5) as i64;
        let int_part = rounded / multiplier as i64;
        let frac_part = (rounded % multiplier as i64).abs() as u64;

        let int_str = self.format_int(int_part);
        
        if precision == 0 {
            return int_str;
        }

        let frac_str = alloc::format!("{:0>width$}", frac_part, width = precision);
        
        alloc::format!("{}{}{}", int_str, self.decimal_separator, frac_str)
    }

    /// Format percentage
    pub fn format_percent(&self, value: f64, precision: usize) -> String {
        alloc::format!("{}%", self.format_float(value * 100.0, precision))
    }

    /// Parse number from string
    pub fn parse(&self, s: &str) -> Option<f64> {
        let cleaned: String = s.chars()
            .filter(|&c| c != self.thousands_separator)
            .map(|c| if c == self.decimal_separator { '.' } else { c })
            .collect();
        
        cleaned.parse().ok()
    }
}

impl Default for NumberFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Currency formatter
#[derive(Debug, Clone)]
pub struct CurrencyFormatter {
    /// Number formatter
    number: NumberFormatter,
    /// Currency code
    currency_code: String,
    /// Currency symbol
    symbol: String,
    /// Symbol position
    symbol_before: bool,
    /// Space between symbol and number
    space: bool,
    /// Decimal places
    decimals: usize,
}

impl CurrencyFormatter {
    /// Create for currency
    pub fn new(currency_code: &str) -> Self {
        let (symbol, decimals) = match currency_code {
            "USD" => ("$", 2),
            "EUR" => ("€", 2),
            "GBP" => ("£", 2),
            "JPY" => ("¥", 0),
            "KRW" => ("₩", 0),
            "CNY" => ("¥", 2),
            _ => (currency_code, 2),
        };

        Self {
            number: NumberFormatter::new(),
            currency_code: currency_code.to_string(),
            symbol: symbol.to_string(),
            symbol_before: true,
            space: false,
            decimals,
        }
    }

    /// Format currency
    pub fn format(&self, value: f64) -> String {
        let num_str = self.number.format_float(value, self.decimals);
        
        let space = if self.space { " " } else { "" };
        
        if self.symbol_before {
            alloc::format!("{}{}{}", self.symbol, space, num_str)
        } else {
            alloc::format!("{}{}{}", num_str, space, self.symbol)
        }
    }
}

/// Date format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    /// Short (e.g., 2024-01-15)
    Short,
    /// Medium (e.g., Jan 15, 2024)
    Medium,
    /// Long (e.g., January 15, 2024)
    Long,
    /// Full (e.g., Monday, January 15, 2024)
    Full,
}

impl Default for DateFormat {
    fn default() -> Self {
        Self::Short
    }
}

/// Time format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFormat {
    /// Short (e.g., 14:30)
    Short,
    /// Medium (e.g., 14:30:00)
    Medium,
    /// Long (e.g., 14:30:00 KST)
    Long,
}

impl Default for TimeFormat {
    fn default() -> Self {
        Self::Short
    }
}

/// Date formatter
#[derive(Debug, Clone)]
pub struct DateFormatter {
    /// Date format
    pub date_format: DateFormat,
    /// Time format
    pub time_format: TimeFormat,
    /// First day of week (0 = Sunday)
    pub first_day_of_week: u8,
    /// Locale code for localized names
    pub locale: String,
}

impl DateFormatter {
    /// Create new formatter
    pub const fn new() -> Self {
        Self {
            date_format: DateFormat::Short,
            time_format: TimeFormat::Short,
            first_day_of_week: 0,
            locale: String::new(),
        }
    }

    /// Create for locale
    pub fn for_locale(locale: &str) -> Self {
        let first_day = match locale {
            "ko" | "ko-KR" | "ja" | "ja-JP" | "zh-CN" | "zh" => 1, // Monday
            "de" | "de-DE" | "fr" | "fr-FR" | "es" | "es-ES" => 1,
            _ => 0, // Sunday
        };
        Self {
            date_format: DateFormat::Short,
            time_format: TimeFormat::Short,
            first_day_of_week: first_day,
            locale: locale.into(),
        }
    }

    /// Format date components
    pub fn format_date(&self, year: i32, month: u8, day: u8) -> String {
        let loc = self.locale.as_str();
        match self.date_format {
            DateFormat::Short => {
                match loc {
                    "ko" | "ko-KR" | "ja" | "ja-JP" | "zh-CN" | "zh" => {
                        alloc::format!("{:04}-{:02}-{:02}", year, month, day)
                    }
                    "de" | "de-DE" | "fr" | "fr-FR" => {
                        alloc::format!("{:02}.{:02}.{:04}", day, month, year)
                    }
                    _ => alloc::format!("{:04}-{:02}-{:02}", year, month, day),
                }
            }
            DateFormat::Medium => {
                let month_name = self.month_abbrev(month);
                match loc {
                    "ko" | "ko-KR" => alloc::format!("{}\u{b144} {}\u{c6d4} {}\u{c77c}", year, month, day),
                    "ja" | "ja-JP" => alloc::format!("{}\u{5e74}{}\u{6708}{}\u{65e5}", year, month, day),
                    "zh-CN" | "zh" => alloc::format!("{}\u{5e74}{}\u{6708}{}\u{65e5}", year, month, day),
                    "de" | "de-DE" => alloc::format!("{:02}. {} {}", day, month_name, year),
                    _ => alloc::format!("{} {}, {}", month_name, day, year),
                }
            }
            DateFormat::Long => {
                let month_name = self.month_name(month);
                match loc {
                    "ko" | "ko-KR" => alloc::format!("{}\u{b144} {}\u{c6d4} {}\u{c77c}", year, month, day),
                    "ja" | "ja-JP" => alloc::format!("{}\u{5e74}{}\u{6708}{}\u{65e5}", year, month, day),
                    "zh-CN" | "zh" => alloc::format!("{}\u{5e74}{}\u{6708}{}\u{65e5}", year, month, day),
                    "de" | "de-DE" => alloc::format!("{:02}. {} {}", day, month_name, year),
                    _ => alloc::format!("{} {}, {}", month_name, day, year),
                }
            }
            DateFormat::Full => {
                let month_name = self.month_name(month);
                let dow = day_of_week(year, month, day);
                let day_name = self.day_name(dow);
                match loc {
                    "ko" | "ko-KR" => alloc::format!("{}\u{b144} {}\u{c6d4} {}\u{c77c} {}", year, month, day, day_name),
                    "ja" | "ja-JP" => alloc::format!("{}\u{5e74}{}\u{6708}{}\u{65e5} {}", year, month, day, day_name),
                    "zh-CN" | "zh" => alloc::format!("{}\u{5e74}{}\u{6708}{}\u{65e5} {}", year, month, day, day_name),
                    "de" | "de-DE" => alloc::format!("{}, {:02}. {} {}", day_name, day, month_name, year),
                    "es" | "es-ES" => alloc::format!("{}, {} de {} de {}", day_name, day, month_name, year),
                    _ => alloc::format!("{}, {} {}, {}", day_name, month_name, day, year),
                }
            }
        }
    }

    /// Format time components
    pub fn format_time(&self, hour: u8, minute: u8, second: u8) -> String {
        match self.time_format {
            TimeFormat::Short => alloc::format!("{:02}:{:02}", hour, minute),
            TimeFormat::Medium => alloc::format!("{:02}:{:02}:{:02}", hour, minute, second),
            TimeFormat::Long => alloc::format!("{:02}:{:02}:{:02}", hour, minute, second),
        }
    }

    /// Format relative time (locale-aware)
    pub fn format_relative(&self, seconds_ago: i64) -> String {
        let abs_seconds = seconds_ago.abs();
        let future = seconds_ago < 0;
        
        let (value, unit_idx) = if abs_seconds < 60 {
            (abs_seconds, 0) // second
        } else if abs_seconds < 3600 {
            (abs_seconds / 60, 1) // minute
        } else if abs_seconds < 86400 {
            (abs_seconds / 3600, 2) // hour
        } else if abs_seconds < 2592000 {
            (abs_seconds / 86400, 3) // day
        } else if abs_seconds < 31536000 {
            (abs_seconds / 2592000, 4) // month
        } else {
            (abs_seconds / 31536000, 5) // year
        };

        let loc = self.locale.as_str();
        match loc {
            "ko" | "ko-KR" => {
                let unit = match unit_idx {
                    0 => "\u{CD08}",
                    1 => "\u{BD84}",
                    2 => "\u{C2DC}\u{AC04}",
                    3 => "\u{C77C}",
                    4 => "\u{AC1C}\u{C6D4}",
                    _ => "\u{B144}",
                };
                if future {
                    alloc::format!("{}{}  \u{D6C4}", value, unit)
                } else {
                    alloc::format!("{}{}  \u{C804}", value, unit)
                }
            }
            "ja" | "ja-JP" => {
                let unit = match unit_idx {
                    0 => "\u{79D2}",
                    1 => "\u{5206}",
                    2 => "\u{6642}\u{9593}",
                    3 => "\u{65E5}",
                    4 => "\u{304B}\u{6708}",
                    _ => "\u{5E74}",
                };
                if future {
                    alloc::format!("{}{}  \u{5F8C}", value, unit)
                } else {
                    alloc::format!("{}{}  \u{524D}", value, unit)
                }
            }
            "zh-CN" | "zh" => {
                let unit = match unit_idx {
                    0 => "\u{79D2}",
                    1 => "\u{5206}\u{949F}",
                    2 => "\u{5C0F}\u{65F6}",
                    3 => "\u{5929}",
                    4 => "\u{4E2A}\u{6708}",
                    _ => "\u{5E74}",
                };
                if future {
                    alloc::format!("{}{}  \u{540E}", value, unit)
                } else {
                    alloc::format!("{}{}  \u{524D}", value, unit)
                }
            }
            "de" | "de-DE" => {
                let unit = match unit_idx {
                    0 => if value == 1 { "Sekunde" } else { "Sekunden" },
                    1 => if value == 1 { "Minute" } else { "Minuten" },
                    2 => if value == 1 { "Stunde" } else { "Stunden" },
                    3 => if value == 1 { "Tag" } else { "Tagen" },
                    4 => if value == 1 { "Monat" } else { "Monaten" },
                    _ => if value == 1 { "Jahr" } else { "Jahren" },
                };
                if future {
                    alloc::format!("in {} {}", value, unit)
                } else {
                    alloc::format!("vor {} {}", value, unit)
                }
            }
            "es" | "es-ES" => {
                let unit = match unit_idx {
                    0 => if value == 1 { "segundo" } else { "segundos" },
                    1 => if value == 1 { "minuto" } else { "minutos" },
                    2 => if value == 1 { "hora" } else { "horas" },
                    3 => if value == 1 { "d\u{ed}a" } else { "d\u{ed}as" },
                    4 => if value == 1 { "mes" } else { "meses" },
                    _ => if value == 1 { "a\u{f1}o" } else { "a\u{f1}os" },
                };
                if future {
                    alloc::format!("en {} {}", value, unit)
                } else {
                    alloc::format!("hace {} {}", value, unit)
                }
            }
            "fr" | "fr-FR" => {
                let unit = match unit_idx {
                    0 => if value == 1 { "seconde" } else { "secondes" },
                    1 => if value == 1 { "minute" } else { "minutes" },
                    2 => if value == 1 { "heure" } else { "heures" },
                    3 => if value == 1 { "jour" } else { "jours" },
                    4 => "mois",
                    _ => if value == 1 { "an" } else { "ans" },
                };
                if future {
                    alloc::format!("dans {} {}", value, unit)
                } else {
                    alloc::format!("il y a {} {}", value, unit)
                }
            }
            _ => {
                let unit = match unit_idx {
                    0 => "second",
                    1 => "minute",
                    2 => "hour",
                    3 => "day",
                    4 => "month",
                    _ => "year",
                };
                let plural = if value != 1 { "s" } else { "" };
                if future {
                    alloc::format!("in {} {}{}", value, unit, plural)
                } else {
                    alloc::format!("{} {}{} ago", value, unit, plural)
                }
            }
        }
    }

    /// Get month abbreviation (locale-aware)
    fn month_abbrev(&self, month: u8) -> &'static str {
        let loc = self.locale.as_str();
        match loc {
            "de" | "de-DE" => match month {
                1 => "Jan", 2 => "Feb", 3 => "M\u{e4}r", 4 => "Apr",
                5 => "Mai", 6 => "Jun", 7 => "Jul", 8 => "Aug",
                9 => "Sep", 10 => "Okt", 11 => "Nov", 12 => "Dez",
                _ => "???",
            },
            "es" | "es-ES" => match month {
                1 => "ene", 2 => "feb", 3 => "mar", 4 => "abr",
                5 => "may", 6 => "jun", 7 => "jul", 8 => "ago",
                9 => "sep", 10 => "oct", 11 => "nov", 12 => "dic",
                _ => "???",
            },
            "fr" | "fr-FR" => match month {
                1 => "janv", 2 => "f\u{e9}vr", 3 => "mars", 4 => "avr",
                5 => "mai", 6 => "juin", 7 => "juil", 8 => "ao\u{fb}t",
                9 => "sept", 10 => "oct", 11 => "nov", 12 => "d\u{e9}c",
                _ => "???",
            },
            _ => match month {
                1 => "Jan", 2 => "Feb", 3 => "Mar", 4 => "Apr",
                5 => "May", 6 => "Jun", 7 => "Jul", 8 => "Aug",
                9 => "Sep", 10 => "Oct", 11 => "Nov", 12 => "Dec",
                _ => "???",
            },
        }
    }

    /// Get full month name (locale-aware)
    fn month_name(&self, month: u8) -> &'static str {
        let loc = self.locale.as_str();
        match loc {
            "de" | "de-DE" => match month {
                1 => "Januar", 2 => "Februar", 3 => "M\u{e4}rz", 4 => "April",
                5 => "Mai", 6 => "Juni", 7 => "Juli", 8 => "August",
                9 => "September", 10 => "Oktober", 11 => "November", 12 => "Dezember",
                _ => "Unbekannt",
            },
            "es" | "es-ES" => match month {
                1 => "enero", 2 => "febrero", 3 => "marzo", 4 => "abril",
                5 => "mayo", 6 => "junio", 7 => "julio", 8 => "agosto",
                9 => "septiembre", 10 => "octubre", 11 => "noviembre", 12 => "diciembre",
                _ => "desconocido",
            },
            "fr" | "fr-FR" => match month {
                1 => "janvier", 2 => "f\u{e9}vrier", 3 => "mars", 4 => "avril",
                5 => "mai", 6 => "juin", 7 => "juillet", 8 => "ao\u{fb}t",
                9 => "septembre", 10 => "octobre", 11 => "novembre", 12 => "d\u{e9}cembre",
                _ => "inconnu",
            },
            _ => match month {
                1 => "January", 2 => "February", 3 => "March", 4 => "April",
                5 => "May", 6 => "June", 7 => "July", 8 => "August",
                9 => "September", 10 => "October", 11 => "November", 12 => "December",
                _ => "Unknown",
            },
        }
    }

    /// Get day of week name (locale-aware)
    /// dow: 0=Sunday, 1=Monday, ..., 6=Saturday
    fn day_name(&self, dow: u8) -> &'static str {
        let loc = self.locale.as_str();
        match loc {
            "ko" | "ko-KR" => match dow {
                0 => "\u{C77C}\u{C694}\u{C77C}",
                1 => "\u{C6D4}\u{C694}\u{C77C}",
                2 => "\u{D654}\u{C694}\u{C77C}",
                3 => "\u{C218}\u{C694}\u{C77C}",
                4 => "\u{BAA9}\u{C694}\u{C77C}",
                5 => "\u{AE08}\u{C694}\u{C77C}",
                6 => "\u{D1A0}\u{C694}\u{C77C}",
                _ => "",
            },
            "ja" | "ja-JP" => match dow {
                0 => "\u{65E5}\u{66DC}\u{65E5}",
                1 => "\u{6708}\u{66DC}\u{65E5}",
                2 => "\u{706B}\u{66DC}\u{65E5}",
                3 => "\u{6C34}\u{66DC}\u{65E5}",
                4 => "\u{6728}\u{66DC}\u{65E5}",
                5 => "\u{91D1}\u{66DC}\u{65E5}",
                6 => "\u{571F}\u{66DC}\u{65E5}",
                _ => "",
            },
            "zh-CN" | "zh" => match dow {
                0 => "\u{661F}\u{671F}\u{65E5}",
                1 => "\u{661F}\u{671F}\u{4E00}",
                2 => "\u{661F}\u{671F}\u{4E8C}",
                3 => "\u{661F}\u{671F}\u{4E09}",
                4 => "\u{661F}\u{671F}\u{56DB}",
                5 => "\u{661F}\u{671F}\u{4E94}",
                6 => "\u{661F}\u{671F}\u{516D}",
                _ => "",
            },
            "de" | "de-DE" => match dow {
                0 => "Sonntag", 1 => "Montag", 2 => "Dienstag", 3 => "Mittwoch",
                4 => "Donnerstag", 5 => "Freitag", 6 => "Samstag",
                _ => "",
            },
            "es" | "es-ES" => match dow {
                0 => "domingo", 1 => "lunes", 2 => "martes", 3 => "mi\u{e9}rcoles",
                4 => "jueves", 5 => "viernes", 6 => "s\u{e1}bado",
                _ => "",
            },
            "fr" | "fr-FR" => match dow {
                0 => "dimanche", 1 => "lundi", 2 => "mardi", 3 => "mercredi",
                4 => "jeudi", 5 => "vendredi", 6 => "samedi",
                _ => "",
            },
            _ => match dow {
                0 => "Sunday", 1 => "Monday", 2 => "Tuesday", 3 => "Wednesday",
                4 => "Thursday", 5 => "Friday", 6 => "Saturday",
                _ => "",
            },
        }
    }
}

/// Calculate day of week using Zeller-like formula
/// Returns 0=Sunday, 1=Monday, ..., 6=Saturday
fn day_of_week(year: i32, month: u8, day: u8) -> u8 {
    let m = month as i32;
    let d = day as i32;
    let (y, m) = if m < 3 { (year - 1, m + 12) } else { (year, m) };
    let dow = (d + (13 * (m + 1)) / 5 + y + y / 4 - y / 100 + y / 400) % 7;
    // Zeller: 0=Saturday, 1=Sunday, ..., 6=Friday → convert to 0=Sunday
    ((dow + 6) % 7) as u8
}

impl Default for DateFormatter {
    fn default() -> Self {
        Self::new()
    }
}
