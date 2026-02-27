//! Keyboard and mouse input injection via QMP.
//!
//! Provides handlers for `send-key`, `type-text`, `mouse-click`, and
//! `mouse-move` subcommands. All input is injected through QMP's
//! `input-send-event` command.

use serde::Serialize;

use crate::cli::{MouseClickArgs, MouseMoveArgs, SendKeyArgs, TypeTextArgs};
use crate::error::KpioTestError;
use crate::qmp::{InputEvent, QmpClient};
use crate::state::InstanceStatus;
use crate::{store, watchdog};

// ── Output structs ───────────────────────────────────────────────────

#[derive(Serialize)]
struct SendKeyOutput {
    name: String,
    keys_sent: Vec<String>,
}

#[derive(Serialize)]
struct TypeTextOutput {
    name: String,
    text: String,
    keys_sent: usize,
}

#[derive(Serialize)]
struct MouseClickOutput {
    name: String,
    x: u32,
    y: u32,
    button: String,
}

#[derive(Serialize)]
struct MouseMoveOutput {
    name: String,
    x: u32,
    y: u32,
}

// ── QMP key code mapping ─────────────────────────────────────────────

/// Map a human-readable key name to a QMP `qcode` string.
///
/// QMP uses `qcode` values like `"ret"`, `"spc"`, `"shift"`, etc.
/// This function normalises common aliases (e.g. `"enter"` → `"ret"`,
/// `"space"` → `"spc"`).
pub fn key_name_to_qcode(name: &str) -> Result<&'static str, KpioTestError> {
    let lower = name.to_lowercase();
    let qcode = match lower.as_str() {
        // Letters
        "a" => "a",
        "b" => "b",
        "c" => "c",
        "d" => "d",
        "e" => "e",
        "f" => "f",
        "g" => "g",
        "h" => "h",
        "i" => "i",
        "j" => "j",
        "k" => "k",
        "l" => "l",
        "m" => "m",
        "n" => "n",
        "o" => "o",
        "p" => "p",
        "q" => "q",
        "r" => "r",
        "s" => "s",
        "t" => "t",
        "u" => "u",
        "v" => "v",
        "w" => "w",
        "x" => "x",
        "y" => "y",
        "z" => "z",
        // Digits
        "0" => "0",
        "1" => "1",
        "2" => "2",
        "3" => "3",
        "4" => "4",
        "5" => "5",
        "6" => "6",
        "7" => "7",
        "8" => "8",
        "9" => "9",
        // Function keys
        "f1" => "f1",
        "f2" => "f2",
        "f3" => "f3",
        "f4" => "f4",
        "f5" => "f5",
        "f6" => "f6",
        "f7" => "f7",
        "f8" => "f8",
        "f9" => "f9",
        "f10" => "f10",
        "f11" => "f11",
        "f12" => "f12",
        // Special keys
        "ret" | "enter" | "return" => "ret",
        "spc" | "space" => "spc",
        "esc" | "escape" => "esc",
        "tab" => "tab",
        "backspace" | "bksp" => "backspace",
        "delete" | "del" => "delete",
        "insert" | "ins" => "insert",
        "home" => "home",
        "end" => "end",
        "pgup" | "pageup" | "page_up" => "pgup",
        "pgdn" | "pagedown" | "page_down" => "pgdn",
        "up" => "up",
        "down" => "down",
        "left" => "left",
        "right" => "right",
        // Modifiers
        "shift" | "shift_l" => "shift",
        "shift_r" => "shift_r",
        "ctrl" | "ctrl_l" | "control" => "ctrl",
        "ctrl_r" => "ctrl_r",
        "alt" | "alt_l" => "alt",
        "alt_r" | "altgr" => "alt_r",
        "meta_l" | "super" | "super_l" | "win" => "meta_l",
        "meta_r" | "super_r" => "meta_r",
        "caps_lock" | "capslock" => "caps_lock",
        "num_lock" | "numlock" => "num_lock",
        "scroll_lock" | "scrolllock" => "scroll_lock",
        // Punctuation / symbols
        "minus" | "-" => "minus",
        "equal" | "=" => "equal",
        "bracket_left" | "[" => "bracket_left",
        "bracket_right" | "]" => "bracket_right",
        "backslash" | "\\" => "backslash",
        "semicolon" | ";" => "semicolon",
        "apostrophe" | "'" => "apostrophe",
        "grave_accent" | "`" => "grave_accent",
        "comma" | "," => "comma",
        "dot" | "." => "dot",
        "slash" | "/" => "slash",
        // Print screen, pause
        "print" | "printscreen" | "sysrq" => "print",
        "pause" => "pause",
        _ => {
            return Err(KpioTestError::QmpError {
                desc: format!("unknown key name: {name}"),
            });
        }
    };
    Ok(qcode)
}

/// Map a printable ASCII character to its QMP `qcode` and whether shift
/// is required.
pub fn char_to_qcode(ch: char) -> Result<(&'static str, bool), KpioTestError> {
    let (qcode, shift) = match ch {
        'a'..='z' => {
            // 'a' maps to "a", etc.
            let codes = [
                "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
                "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
            ];
            (codes[(ch as u8 - b'a') as usize], false)
        }
        'A'..='Z' => {
            let codes = [
                "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
                "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
            ];
            (codes[(ch as u8 - b'A') as usize], true)
        }
        '0'..='9' => {
            let codes = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
            (codes[(ch as u8 - b'0') as usize], false)
        }
        ' ' => ("spc", false),
        '\n' => ("ret", false),
        '\t' => ("tab", false),
        '!' => ("1", true),
        '@' => ("2", true),
        '#' => ("3", true),
        '$' => ("4", true),
        '%' => ("5", true),
        '^' => ("6", true),
        '&' => ("7", true),
        '*' => ("8", true),
        '(' => ("9", true),
        ')' => ("0", true),
        '-' => ("minus", false),
        '_' => ("minus", true),
        '=' => ("equal", false),
        '+' => ("equal", true),
        '[' => ("bracket_left", false),
        '{' => ("bracket_left", true),
        ']' => ("bracket_right", false),
        '}' => ("bracket_right", true),
        '\\' => ("backslash", false),
        '|' => ("backslash", true),
        ';' => ("semicolon", false),
        ':' => ("semicolon", true),
        '\'' => ("apostrophe", false),
        '"' => ("apostrophe", true),
        '`' => ("grave_accent", false),
        '~' => ("grave_accent", true),
        ',' => ("comma", false),
        '<' => ("comma", true),
        '.' => ("dot", false),
        '>' => ("dot", true),
        '/' => ("slash", false),
        '?' => ("slash", true),
        _ => {
            return Err(KpioTestError::QmpError {
                desc: format!("unsupported character for type-text: {:?}", ch),
            });
        }
    };
    Ok((qcode, shift))
}

// ── Event builders ───────────────────────────────────────────────────

/// Build a QMP key press event.
pub fn key_event(qcode: &str, down: bool) -> InputEvent {
    InputEvent {
        event_type: "key".to_string(),
        data: serde_json::json!({
            "key": { "type": "qcode", "data": qcode },
            "down": down,
        }),
    }
}

/// Build a pair of absolute-axis events for (x, y).
pub fn mouse_move_events(x: u32, y: u32) -> Vec<InputEvent> {
    vec![
        InputEvent {
            event_type: "abs".to_string(),
            data: serde_json::json!({ "axis": "x", "value": x }),
        },
        InputEvent {
            event_type: "abs".to_string(),
            data: serde_json::json!({ "axis": "y", "value": y }),
        },
    ]
}

/// Map a button name to a QMP button index.
pub fn button_to_index(button: &str) -> Result<u32, KpioTestError> {
    match button.to_lowercase().as_str() {
        "left" => Ok(0),
        "middle" => Ok(1),
        "right" => Ok(2),
        other => Err(KpioTestError::QmpError {
            desc: format!("unknown mouse button: {other}"),
        }),
    }
}

/// Build a QMP mouse button event.
pub fn mouse_button_event(button_index: u32, down: bool) -> InputEvent {
    InputEvent {
        event_type: "btn".to_string(),
        data: serde_json::json!({
            "button": match button_index {
                0 => "left",
                1 => "middle",
                2 => "right",
                _ => "left",
            },
            "down": down,
        }),
    }
}

// ── Subcommand handlers ──────────────────────────────────────────────

/// `send-key <name> <key> [<key2> ...]` — send key press/release events.
pub fn send_key(args: SendKeyArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    if state.status != InstanceStatus::Running {
        return Err(KpioTestError::InstanceNotRunning {
            name: args.name.clone(),
        });
    }

    let mut qmp = QmpClient::connect(&state.qmp_socket)?;

    // Resolve all key names to qcodes first so we fail fast on bad names.
    let qcodes: Vec<&str> = args
        .keys
        .iter()
        .map(|k| key_name_to_qcode(k))
        .collect::<Result<Vec<_>, _>>()?;

    // Send press events for all keys, then release in reverse order.
    let mut events: Vec<InputEvent> = Vec::new();
    for &qcode in &qcodes {
        events.push(key_event(qcode, true));
    }
    for &qcode in qcodes.iter().rev() {
        events.push(key_event(qcode, false));
    }
    qmp.input_send_event(&events)?;

    let output = SendKeyOutput {
        name: args.name,
        keys_sent: args.keys,
    };
    Ok(serde_json::to_value(output)?)
}

/// `type-text <name> <text>` — type a string as sequential key events.
pub fn type_text(args: TypeTextArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    if state.status != InstanceStatus::Running {
        return Err(KpioTestError::InstanceNotRunning {
            name: args.name.clone(),
        });
    }

    let mut qmp = QmpClient::connect(&state.qmp_socket)?;
    let mut keys_sent: usize = 0;

    for ch in args.text.chars() {
        let (qcode, needs_shift) = char_to_qcode(ch)?;
        let mut events = Vec::new();

        if needs_shift {
            events.push(key_event("shift", true));
        }
        events.push(key_event(qcode, true));
        events.push(key_event(qcode, false));
        if needs_shift {
            events.push(key_event("shift", false));
        }

        qmp.input_send_event(&events)?;
        keys_sent += 1;
    }

    let output = TypeTextOutput {
        name: args.name,
        text: args.text,
        keys_sent,
    };
    Ok(serde_json::to_value(output)?)
}

/// `mouse-click <name> --x <X> --y <Y> [--button <btn>]` — click at coords.
pub fn mouse_click(args: MouseClickArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    if state.status != InstanceStatus::Running {
        return Err(KpioTestError::InstanceNotRunning {
            name: args.name.clone(),
        });
    }

    let btn_index = button_to_index(&args.button)?;
    let mut qmp = QmpClient::connect(&state.qmp_socket)?;

    // Move to position
    let move_events = mouse_move_events(args.x, args.y);
    qmp.input_send_event(&move_events)?;

    // Press and release button
    let click_events = vec![
        mouse_button_event(btn_index, true),
        mouse_button_event(btn_index, false),
    ];
    qmp.input_send_event(&click_events)?;

    let output = MouseClickOutput {
        name: args.name,
        x: args.x,
        y: args.y,
        button: args.button,
    };
    Ok(serde_json::to_value(output)?)
}

/// `mouse-move <name> --x <X> --y <Y>` — move mouse to coordinates.
pub fn mouse_move(args: MouseMoveArgs) -> Result<serde_json::Value, KpioTestError> {
    let mut state = store::read_state(&args.name)?;
    watchdog::enforce(&mut state)?;

    if state.status != InstanceStatus::Running {
        return Err(KpioTestError::InstanceNotRunning {
            name: args.name.clone(),
        });
    }

    let mut qmp = QmpClient::connect(&state.qmp_socket)?;

    let events = mouse_move_events(args.x, args.y);
    qmp.input_send_event(&events)?;

    let output = MouseMoveOutput {
        name: args.name,
        x: args.x,
        y: args.y,
    };
    Ok(serde_json::to_value(output)?)
}

// ── Unit tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_name_mapping_letters() {
        assert_eq!(key_name_to_qcode("a").unwrap(), "a");
        assert_eq!(key_name_to_qcode("Z").unwrap(), "z");
    }

    #[test]
    fn key_name_mapping_aliases() {
        assert_eq!(key_name_to_qcode("enter").unwrap(), "ret");
        assert_eq!(key_name_to_qcode("return").unwrap(), "ret");
        assert_eq!(key_name_to_qcode("space").unwrap(), "spc");
        assert_eq!(key_name_to_qcode("escape").unwrap(), "esc");
        assert_eq!(key_name_to_qcode("backspace").unwrap(), "backspace");
        assert_eq!(key_name_to_qcode("pageup").unwrap(), "pgup");
    }

    #[test]
    fn key_name_unknown_fails() {
        assert!(key_name_to_qcode("nonexistent_key").is_err());
    }

    #[test]
    fn char_mapping_lowercase() {
        let (qcode, shift) = char_to_qcode('a').unwrap();
        assert_eq!(qcode, "a");
        assert!(!shift);
    }

    #[test]
    fn char_mapping_uppercase() {
        let (qcode, shift) = char_to_qcode('A').unwrap();
        assert_eq!(qcode, "a");
        assert!(shift);
    }

    #[test]
    fn char_mapping_digit() {
        let (qcode, shift) = char_to_qcode('5').unwrap();
        assert_eq!(qcode, "5");
        assert!(!shift);
    }

    #[test]
    fn char_mapping_shifted_symbols() {
        let (qcode, shift) = char_to_qcode('!').unwrap();
        assert_eq!(qcode, "1");
        assert!(shift);

        let (qcode, shift) = char_to_qcode('{').unwrap();
        assert_eq!(qcode, "bracket_left");
        assert!(shift);

        let (qcode, shift) = char_to_qcode('~').unwrap();
        assert_eq!(qcode, "grave_accent");
        assert!(shift);
    }

    #[test]
    fn char_mapping_unshifted_symbols() {
        let (qcode, shift) = char_to_qcode('-').unwrap();
        assert_eq!(qcode, "minus");
        assert!(!shift);

        let (qcode, shift) = char_to_qcode('.').unwrap();
        assert_eq!(qcode, "dot");
        assert!(!shift);
    }

    #[test]
    fn char_mapping_space_and_newline() {
        let (qcode, shift) = char_to_qcode(' ').unwrap();
        assert_eq!(qcode, "spc");
        assert!(!shift);

        let (qcode, shift) = char_to_qcode('\n').unwrap();
        assert_eq!(qcode, "ret");
        assert!(!shift);
    }

    #[test]
    fn char_mapping_unsupported_fails() {
        assert!(char_to_qcode('\x01').is_err());
        assert!(char_to_qcode('é').is_err());
    }

    #[test]
    fn key_event_structure() {
        let evt = key_event("ret", true);
        assert_eq!(evt.event_type, "key");
        assert_eq!(evt.data["key"]["type"], "qcode");
        assert_eq!(evt.data["key"]["data"], "ret");
        assert_eq!(evt.data["down"], true);
    }

    #[test]
    fn mouse_move_events_structure() {
        let evts = mouse_move_events(100, 200);
        assert_eq!(evts.len(), 2);
        assert_eq!(evts[0].event_type, "abs");
        assert_eq!(evts[0].data["axis"], "x");
        assert_eq!(evts[0].data["value"], 100);
        assert_eq!(evts[1].event_type, "abs");
        assert_eq!(evts[1].data["axis"], "y");
        assert_eq!(evts[1].data["value"], 200);
    }

    #[test]
    fn mouse_button_event_structure() {
        let evt = mouse_button_event(0, true);
        assert_eq!(evt.event_type, "btn");
        assert_eq!(evt.data["button"], "left");
        assert_eq!(evt.data["down"], true);

        let evt = mouse_button_event(2, false);
        assert_eq!(evt.data["button"], "right");
        assert_eq!(evt.data["down"], false);
    }

    #[test]
    fn button_name_mapping() {
        assert_eq!(button_to_index("left").unwrap(), 0);
        assert_eq!(button_to_index("middle").unwrap(), 1);
        assert_eq!(button_to_index("right").unwrap(), 2);
        assert_eq!(button_to_index("Left").unwrap(), 0); // case-insensitive
        assert!(button_to_index("unknown").is_err());
    }
}
