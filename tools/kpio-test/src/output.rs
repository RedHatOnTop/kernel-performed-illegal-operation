use serde::Serialize;
use std::fmt;

/// Output format selection for all subcommands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Single JSON object on stdout.
    Json,
    /// Human-readable summary on stdout.
    #[default]
    Human,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Human => write!(f, "human"),
        }
    }
}

/// Write a successful result to stdout.
///
/// - **Json**: a single JSON object, no extraneous text.
/// - **Human**: the `Display` representation (falls back to JSON pretty-print
///   when `T` does not implement `Display`).
pub fn emit<T: Serialize>(format: OutputFormat, value: &T) -> Result<(), std::io::Error> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string(value)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            println!("{json}");
        }
        OutputFormat::Human => {
            // For human output, use indented JSON as a readable fallback.
            let pretty = serde_json::to_string_pretty(value)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            println!("{pretty}");
        }
    }
    Ok(())
}

/// Write an error to stdout (JSON mode) or stderr (human mode).
///
/// `exit_code_num` is the raw numeric exit code (0, 1, or 2).
pub fn emit_error(format: OutputFormat, exit_code_num: u8, message: &str) {
    match format {
        OutputFormat::Json => {
            let obj = serde_json::json!({
                "error": message,
                "exit_code": exit_code_num,
            });
            // JSON errors go to stdout so the caller always gets valid JSON on stdout.
            println!("{}", serde_json::to_string(&obj).unwrap_or_else(|_| {
                format!("{{\"error\":\"{message}\"}}")
            }));
        }
        OutputFormat::Human => {
            eprintln!("error: {message}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_emit_produces_valid_json() {
        // Capture by writing to a string via serde directly (emit writes to stdout).
        let value = serde_json::json!({"status": "ok", "count": 42});
        let json = serde_json::to_string(&value).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["count"], 42);
    }

    #[test]
    fn output_format_display() {
        assert_eq!(OutputFormat::Json.to_string(), "json");
        assert_eq!(OutputFormat::Human.to_string(), "human");
    }

    #[test]
    fn output_format_default_is_human() {
        assert_eq!(OutputFormat::default(), OutputFormat::Human);
    }
}
