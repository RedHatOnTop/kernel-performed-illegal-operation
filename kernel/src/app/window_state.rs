//! Window State Persistence
//!
//! Saves and restores window position/size per app so that re-launched
//! WebApp windows remember where the user left them.
//!
//! State is stored in VFS at `/apps/data/{app_id}/window_state.json`.

use alloc::format;
use alloc::string::String;

use super::error::AppError;
use super::registry::KernelAppId;

// ── Types ────────────────────────────────────────────────────

/// Saved geometry for a single app window.
#[derive(Debug, Clone, Copy)]
pub struct SavedWindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

// ── Public API ──────────────────────────────────────────────

/// Save the window geometry for the given app.
pub fn save(app_id: KernelAppId, x: i32, y: i32, width: u32, height: u32) -> Result<(), AppError> {
    let path = state_path(app_id);
    let json = format!(
        r#"{{"x":{},"y":{},"width":{},"height":{}}}"#,
        x, y, width, height,
    );
    crate::vfs::write_all(&path, json.as_bytes()).map_err(|_| AppError::IoError)
}

/// Load the last saved window geometry for the given app.
pub fn load(app_id: KernelAppId) -> Option<SavedWindowState> {
    let path = state_path(app_id);
    let data = crate::vfs::read_all(&path).ok()?;
    let s = core::str::from_utf8(&data).ok()?;
    parse_state(s)
}

// ── Helpers ─────────────────────────────────────────────────

fn state_path(app_id: KernelAppId) -> String {
    format!("/apps/data/{}/window_state.json", app_id.0)
}

/// Minimal hand-rolled JSON parser for `{"x":..,"y":..,"width":..,"height":..}`
fn parse_state(s: &str) -> Option<SavedWindowState> {
    let x = extract_i32(s, "\"x\":")?;
    let y = extract_i32(s, "\"y\":")?;
    let width = extract_u32(s, "\"width\":")?;
    let height = extract_u32(s, "\"height\":")?;
    Some(SavedWindowState {
        x,
        y,
        width,
        height,
    })
}

fn extract_i32(s: &str, key: &str) -> Option<i32> {
    let idx = s.find(key)? + key.len();
    let rest = s[idx..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '-')
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn extract_u32(s: &str, key: &str) -> Option<u32> {
    let idx = s.find(key)? + key.len();
    let rest = s[idx..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip() {
        let json = r#"{"x":100,"y":200,"width":800,"height":600}"#;
        let state = parse_state(json).expect("should parse");
        assert_eq!(state.x, 100);
        assert_eq!(state.y, 200);
        assert_eq!(state.width, 800);
        assert_eq!(state.height, 600);
    }

    #[test]
    fn parse_negative_coords() {
        let json = r#"{"x":-10,"y":-20,"width":640,"height":480}"#;
        let state = parse_state(json).expect("should parse");
        assert_eq!(state.x, -10);
        assert_eq!(state.y, -20);
    }

    #[test]
    fn parse_missing_field() {
        let json = r#"{"x":100,"y":200}"#;
        assert!(parse_state(json).is_none());
    }
}
