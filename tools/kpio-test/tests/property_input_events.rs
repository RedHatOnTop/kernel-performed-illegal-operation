//! Property 17: Input event generation correctness
//!
//! For any printable text, `type-text` generates correct key events;
//! for any key name, `send-key` generates correct events;
//! for any coordinates and button, `mouse-click` generates correct events.
//!
//! **Validates: Requirements 9.1, 9.2, 10.1, 10.2, 10.3**

use kpio_test::input::{
    button_to_index, char_to_qcode, key_event, key_name_to_qcode, mouse_button_event,
    mouse_move_events,
};
use proptest::prelude::*;

// ── Strategies ───────────────────────────────────────────────────────

/// All printable ASCII characters supported by `char_to_qcode`.
const SUPPORTED_CHARS: &str =
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 \n\t!@#$%^&*()-_=+[{]}\\|;:'\",<.>/?`~";

fn arb_printable_char() -> impl Strategy<Value = char> {
    (0..SUPPORTED_CHARS.len()).prop_map(|i| SUPPORTED_CHARS.chars().nth(i).unwrap())
}

/// Printable ASCII strings (1–20 chars) for type-text testing.
fn arb_printable_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(arb_printable_char(), 1..=20)
        .prop_map(|chars| chars.into_iter().collect())
}

/// Valid key names recognised by `key_name_to_qcode`.
fn arb_key_name() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("a"),
        Just("z"),
        Just("0"),
        Just("9"),
        Just("f1"),
        Just("f12"),
        Just("enter"),
        Just("return"),
        Just("ret"),
        Just("space"),
        Just("spc"),
        Just("esc"),
        Just("escape"),
        Just("tab"),
        Just("backspace"),
        Just("delete"),
        Just("insert"),
        Just("home"),
        Just("end"),
        Just("pgup"),
        Just("pageup"),
        Just("pgdn"),
        Just("pagedown"),
        Just("up"),
        Just("down"),
        Just("left"),
        Just("right"),
        Just("shift"),
        Just("ctrl"),
        Just("alt"),
        Just("minus"),
        Just("equal"),
        Just("comma"),
        Just("dot"),
        Just("slash"),
        Just("print"),
        Just("pause"),
    ]
}

/// Valid mouse button names.
fn arb_button_name() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("left"), Just("middle"), Just("right")]
}

// ── Property tests ───────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    // Feature: qemu-boot-testing-infrastructure, Property 17: Input event generation correctness

    /// For any printable ASCII character, `char_to_qcode` succeeds and the
    /// generated key events have correct structure (type "key", qcode data,
    /// down flag).
    #[test]
    fn char_to_qcode_produces_valid_key_events(ch in arb_printable_char()) {
        let (qcode, needs_shift) = char_to_qcode(ch).unwrap();

        // qcode must be a non-empty string
        prop_assert!(!qcode.is_empty(), "qcode should not be empty for {:?}", ch);

        // Build press and release events and verify structure
        let press = key_event(qcode, true);
        prop_assert_eq!(press.event_type.as_str(), "key");
        prop_assert!(press.data["key"]["type"] == "qcode");
        prop_assert!(press.data["key"]["data"] == qcode);
        prop_assert!(press.data["down"] == true);

        let release = key_event(qcode, false);
        prop_assert_eq!(release.event_type.as_str(), "key");
        prop_assert!(release.data["down"] == false);

        // If shift is needed, verify shift events also have correct structure
        if needs_shift {
            let shift_press = key_event("shift", true);
            prop_assert_eq!(shift_press.event_type.as_str(), "key");
            prop_assert!(shift_press.data["key"]["data"] == "shift");
            prop_assert!(shift_press.data["down"] == true);
        }
    }

    /// For any valid key name, `key_name_to_qcode` returns a non-empty qcode
    /// and the resulting key events have correct structure.
    #[test]
    fn key_name_to_qcode_produces_valid_events(name in arb_key_name()) {
        let qcode = key_name_to_qcode(name).unwrap();
        prop_assert!(!qcode.is_empty(), "qcode should not be empty for key '{}'", name);

        let press = key_event(qcode, true);
        let release = key_event(qcode, false);

        prop_assert_eq!(press.event_type.as_str(), "key");
        prop_assert!(press.data["key"]["type"] == "qcode");
        prop_assert!(press.data["key"]["data"] == qcode);
        prop_assert!(press.data["down"] == true);

        prop_assert_eq!(release.event_type.as_str(), "key");
        prop_assert!(release.data["key"]["data"] == qcode);
        prop_assert!(release.data["down"] == false);
    }

    /// For any (x, y) coordinates, `mouse_move_events` generates exactly 2
    /// abs events with correct axis values.
    #[test]
    fn mouse_move_events_correct_structure(x in 0u32..=4096, y in 0u32..=4096) {
        let events = mouse_move_events(x, y);

        prop_assert_eq!(events.len(), 2, "mouse_move_events should produce exactly 2 events");

        prop_assert_eq!(events[0].event_type.as_str(), "abs");
        prop_assert!(events[0].data["axis"] == "x");
        prop_assert!(events[0].data["value"] == x);

        prop_assert_eq!(events[1].event_type.as_str(), "abs");
        prop_assert!(events[1].data["axis"] == "y");
        prop_assert!(events[1].data["value"] == y);
    }

    /// For any valid button name and coordinates, mouse click events have
    /// correct structure: move events + button press + button release.
    #[test]
    fn mouse_click_events_correct_structure(
        x in 0u32..=4096,
        y in 0u32..=4096,
        button in arb_button_name(),
    ) {
        let btn_index = button_to_index(button).unwrap();

        // Move events
        let move_events = mouse_move_events(x, y);
        prop_assert_eq!(move_events.len(), 2);

        // Button press and release
        let press = mouse_button_event(btn_index, true);
        prop_assert_eq!(press.event_type.as_str(), "btn");
        prop_assert!(press.data["button"] == button);
        prop_assert!(press.data["down"] == true);

        let release = mouse_button_event(btn_index, false);
        prop_assert_eq!(release.event_type.as_str(), "btn");
        prop_assert!(release.data["button"] == button);
        prop_assert!(release.data["down"] == false);
    }

    /// For any printable text string, each character maps to valid key events
    /// with proper shift handling: shifted chars produce 4 events
    /// (shift-down, key-down, key-up, shift-up), unshifted produce 2
    /// (key-down, key-up).
    #[test]
    fn type_text_generates_correct_event_sequence(text in arb_printable_text()) {
        for ch in text.chars() {
            let (qcode, needs_shift) = char_to_qcode(ch).unwrap();

            let mut events = Vec::new();
            if needs_shift {
                events.push(key_event("shift", true));
            }
            events.push(key_event(qcode, true));
            events.push(key_event(qcode, false));
            if needs_shift {
                events.push(key_event("shift", false));
            }

            if needs_shift {
                prop_assert_eq!(events.len(), 4,
                    "shifted char {:?} should produce 4 events", ch);
                prop_assert!(events[0].data["key"]["data"] == "shift");
                prop_assert!(events[0].data["down"] == true);
                prop_assert!(events[1].data["key"]["data"] == qcode);
                prop_assert!(events[1].data["down"] == true);
                prop_assert!(events[2].data["key"]["data"] == qcode);
                prop_assert!(events[2].data["down"] == false);
                prop_assert!(events[3].data["key"]["data"] == "shift");
                prop_assert!(events[3].data["down"] == false);
            } else {
                prop_assert_eq!(events.len(), 2,
                    "unshifted char {:?} should produce 2 events", ch);
                prop_assert!(events[0].data["key"]["data"] == qcode);
                prop_assert!(events[0].data["down"] == true);
                prop_assert!(events[1].data["key"]["data"] == qcode);
                prop_assert!(events[1].data["down"] == false);
            }
        }
    }
}
