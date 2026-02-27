//! Serial output retrieval, search, and pattern waiting.
//!
//! Handlers for the `serial`, `serial-grep`, and `wait-for` subcommands.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use regex::Regex;
use serde::Serialize;

use crate::cli::{SendCommandArgs, SerialArgs, SerialGrepArgs, WaitForArgs};
use crate::error::KpioTestError;
use crate::qmp::QmpClient;
use crate::state::InstanceStatus;
use crate::store;
use crate::watchdog;

// ── Output types ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SerialOutput {
    pub name: String,
    pub lines: Vec<String>,
    pub line_count: usize,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SerialGrepOutput {
    pub name: String,
    pub matches: Vec<String>,
    pub match_count: usize,
}

#[derive(Debug, Serialize)]
pub struct WaitForOutput {
    pub name: String,
    pub matched: bool,
    pub line: Option<String>,
    pub elapsed_seconds: f64,
}

#[derive(Debug, Serialize)]
pub struct SendCommandOutput {
    pub name: String,
    pub text: String,
    pub sent: bool,
}

// ── Handlers ─────────────────────────────────────────────────────────

/// `serial <name> [--tail N]` — read serial log contents.
pub fn read(args: SerialArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    let log_path = store::serial_log_path(&args.name);
    let content = read_serial_log(&log_path);

    let all_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    let lines = match args.tail {
        Some(n) => tail_lines(&all_lines, n),
        None => all_lines,
    };

    let message = if lines.is_empty() {
        Some("No serial output captured".to_string())
    } else {
        None
    };

    let output = SerialOutput {
        name: args.name,
        line_count: lines.len(),
        lines,
        message,
    };
    Ok(serde_json::to_value(output)?)
}

/// `serial-grep <name> --pattern <regex> [--count] [--first] [--last]`
pub fn grep(args: SerialGrepArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    let log_path = store::serial_log_path(&args.name);
    let content = read_serial_log(&log_path);
    let all_lines: Vec<&str> = content.lines().collect();

    let re = Regex::new(&args.pattern).map_err(|e| {
        KpioTestError::ManifestParseError(format!("invalid regex: {e}"))
    })?;

    let matching: Vec<String> = all_lines
        .iter()
        .filter(|line| re.is_match(line))
        .map(|s| s.to_string())
        .collect();

    let matches = if args.count {
        // --count: return empty matches vec, only the count matters
        Vec::new()
    } else if args.first {
        matching.first().cloned().into_iter().collect()
    } else if args.last {
        matching.last().cloned().into_iter().collect()
    } else {
        matching.clone()
    };

    let output = SerialGrepOutput {
        name: args.name,
        match_count: matching.len(),
        matches,
    };
    Ok(serde_json::to_value(output)?)
}

/// `wait-for <name> --pattern <text> --timeout <s> [--regex] [--interval <ms>]`
pub fn wait_for(args: WaitForArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    let log_path = store::serial_log_path(&args.name);
    let interval = Duration::from_millis(args.interval);
    let deadline = Duration::from_secs(args.timeout);
    let start = Instant::now();

    loop {
        let content = read_serial_log(&log_path);
        if let Some(line) = find_match(&content, &args.pattern, args.regex)? {
            let elapsed = start.elapsed().as_secs_f64();
            let output = WaitForOutput {
                name: args.name,
                matched: true,
                line: Some(line),
                elapsed_seconds: elapsed,
            };
            return Ok(serde_json::to_value(output)?);
        }

        if start.elapsed() >= deadline {
            return Err(KpioTestError::WaitForTimeout {
                seconds: args.timeout,
            });
        }

        thread::sleep(interval);
    }
}

/// `send-command <name> <text>` — write text + newline to serial input via QMP.
pub fn send_command(args: SendCommandArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    if state.status != InstanceStatus::Running {
        return Err(KpioTestError::InstanceNotRunning {
            name: args.name.clone(),
        });
    }

    let mut qmp = QmpClient::connect(&state.qmp_socket)?;

    // Encode text + newline as base64 for the ringbuf-write QMP command.
    let data = format!("{}\n", args.text);
    let encoded = base64_encode(data.as_bytes());

    qmp.execute_void(
        "ringbuf-write",
        Some(serde_json::json!({
            "device": "serial0",
            "data": encoded,
            "format": "base64"
        })),
    )?;

    let output = SendCommandOutput {
        name: args.name,
        text: args.text,
        sent: true,
    };
    Ok(serde_json::to_value(output)?)
}

// ── Internal helpers ─────────────────────────────────────────────────

/// Read the serial log file, returning an empty string if it doesn't exist.
fn read_serial_log(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// Encode bytes as standard base64 (RFC 4648) without pulling in an extra crate.
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Return the last `n` lines from a slice, preserving order.
pub fn tail_lines(lines: &[String], n: usize) -> Vec<String> {
    let start = lines.len().saturating_sub(n);
    lines[start..].to_vec()
}

/// Search content for a matching line. Returns the first match.
///
/// When `is_regex` is true, the pattern is compiled as a regex.
/// Otherwise it is treated as a literal substring.
pub fn find_match(content: &str, pattern: &str, is_regex: bool) -> Result<Option<String>, KpioTestError> {
    if is_regex {
        let re = Regex::new(pattern).map_err(|e| {
            KpioTestError::ManifestParseError(format!("invalid regex: {e}"))
        })?;
        Ok(content.lines().find(|line| re.is_match(line)).map(|s| s.to_string()))
    } else {
        Ok(content.lines().find(|line| line.contains(pattern)).map(|s| s.to_string()))
    }
}

/// Filter lines matching a pattern (regex or literal). Used by property tests.
pub fn grep_lines(content: &str, pattern: &str, is_regex: bool) -> Result<Vec<String>, KpioTestError> {
    if is_regex {
        let re = Regex::new(pattern).map_err(|e| {
            KpioTestError::ManifestParseError(format!("invalid regex: {e}"))
        })?;
        Ok(content.lines().filter(|line| re.is_match(line)).map(|s| s.to_string()).collect())
    } else {
        Ok(content.lines().filter(|line| line.contains(pattern)).map(|s| s.to_string()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encode_empty() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn base64_encode_single_byte() {
        // 'f' -> "Zg=="
        assert_eq!(base64_encode(b"f"), "Zg==");
    }

    #[test]
    fn base64_encode_two_bytes() {
        // 'fo' -> "Zm8="
        assert_eq!(base64_encode(b"fo"), "Zm8=");
    }

    #[test]
    fn base64_encode_three_bytes() {
        // 'foo' -> "Zm9v"
        assert_eq!(base64_encode(b"foo"), "Zm9v");
    }

    #[test]
    fn base64_encode_with_newline() {
        // "ls\n" -> "bHMK"
        assert_eq!(base64_encode(b"ls\n"), "bHMK");
    }

    #[test]
    fn send_command_output_serializes() {
        let output = SendCommandOutput {
            name: "test-vm".to_string(),
            text: "ls -la".to_string(),
            sent: true,
        };
        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(json["name"], "test-vm");
        assert_eq!(json["text"], "ls -la");
        assert_eq!(json["sent"], true);
    }
}
