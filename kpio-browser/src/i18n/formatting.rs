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
}

impl DateFormatter {
    /// Create new formatter
    pub const fn new() -> Self {
        Self {
            date_format: DateFormat::Short,
            time_format: TimeFormat::Short,
            first_day_of_week: 0,
        }
    }

    /// Format date components
    pub fn format_date(&self, year: i32, month: u8, day: u8) -> String {
        match self.date_format {
            DateFormat::Short => alloc::format!("{:04}-{:02}-{:02}", year, month, day),
            DateFormat::Medium => {
                let month_name = self.month_abbrev(month);
                alloc::format!("{} {}, {}", month_name, day, year)
            }
            DateFormat::Long => {
                let month_name = self.month_name(month);
                alloc::format!("{} {}, {}", month_name, day, year)
            }
            DateFormat::Full => {
                let month_name = self.month_name(month);
                // Would need day of week calculation
                alloc::format!("{} {}, {}", month_name, day, year)
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

    /// Format relative time
    pub fn format_relative(&self, seconds_ago: i64) -> String {
        let abs_seconds = seconds_ago.abs();
        let future = seconds_ago < 0;
        
        let (value, unit) = if abs_seconds < 60 {
            (abs_seconds, "second")
        } else if abs_seconds < 3600 {
            (abs_seconds / 60, "minute")
        } else if abs_seconds < 86400 {
            (abs_seconds / 3600, "hour")
        } else if abs_seconds < 2592000 {
            (abs_seconds / 86400, "day")
        } else if abs_seconds < 31536000 {
            (abs_seconds / 2592000, "month")
        } else {
            (abs_seconds / 31536000, "year")
        };

        let plural = if value != 1 { "s" } else { "" };
        
        if future {
            alloc::format!("in {} {}{}", value, unit, plural)
        } else {
            alloc::format!("{} {}{} ago", value, unit, plural)
        }
    }

    /// Get month abbreviation
    fn month_abbrev(&self, month: u8) -> &'static str {
        match month {
            1 => "Jan", 2 => "Feb", 3 => "Mar", 4 => "Apr",
            5 => "May", 6 => "Jun", 7 => "Jul", 8 => "Aug",
            9 => "Sep", 10 => "Oct", 11 => "Nov", 12 => "Dec",
            _ => "???",
        }
    }

    /// Get full month name
    fn month_name(&self, month: u8) -> &'static str {
        match month {
            1 => "January", 2 => "February", 3 => "March", 4 => "April",
            5 => "May", 6 => "June", 7 => "July", 8 => "August",
            9 => "September", 10 => "October", 11 => "November", 12 => "December",
            _ => "Unknown",
        }
    }
}

impl Default for DateFormatter {
    fn default() -> Self {
        Self::new()
    }
}
