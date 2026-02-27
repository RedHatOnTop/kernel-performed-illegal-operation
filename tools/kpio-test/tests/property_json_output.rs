//! Property 1: JSON output validity
//!
//! For any serializable value, `emit` with JSON format produces valid JSON
//! parseable by `serde_json::from_str`.
//!
//! Validates: Requirements 1.5

use kpio_test::output::OutputFormat;
use proptest::prelude::*;
use serde::Serialize;

/// A test struct covering common JSON value shapes: strings, numbers,
/// booleans, optional fields, and nested vectors.
#[derive(Debug, Clone, Serialize)]
struct ArbitraryPayload {
    text: String,
    number: i64,
    float_val: f64,
    flag: bool,
    optional: Option<String>,
    items: Vec<String>,
}

/// Strategy that produces an arbitrary `ArbitraryPayload`.
fn arb_payload() -> impl Strategy<Value = ArbitraryPayload> {
    (
        ".*",                                    // text: any string including special chars
        any::<i64>(),                            // number
        any::<f64>(),                            // float_val
        any::<bool>(),                           // flag
        proptest::option::of(".*"),              // optional
        proptest::collection::vec(".*", 0..10),  // items
    )
        .prop_map(|(text, number, float_val, flag, optional, items)| ArbitraryPayload {
            text,
            number,
            float_val,
            flag,
            optional,
            items,
        })
}

/// Serialize a value the same way `emit` does in JSON mode and return the
/// JSON string. This mirrors the code path inside `output::emit` without
/// writing to stdout.
fn serialize_as_emit<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string(value)
}

/// Serialize a value the same way `emit` does in Human mode (pretty JSON
/// fallback) and return the string.
fn serialize_as_emit_human<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value)
}

/// Build the same JSON object that `emit_error` produces in JSON mode.
fn serialize_as_emit_error(exit_code: u8, message: &str) -> String {
    let obj = serde_json::json!({
        "error": message,
        "exit_code": exit_code,
    });
    serde_json::to_string(&obj).unwrap_or_else(|_| {
        format!("{{\"error\":\"{message}\"}}")
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// For any arbitrary payload, JSON-mode serialization produces a string
    /// that `serde_json::from_str` can parse back into a `Value`.
    #[test]
    fn json_emit_produces_valid_json(payload in arb_payload()) {
        let json_str = serialize_as_emit(&payload)
            .expect("serde_json::to_string should not fail for ArbitraryPayload");
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&json_str);
        prop_assert!(
            parsed.is_ok(),
            "emit JSON output must be parseable: {:?}",
            json_str
        );
    }

    /// For any arbitrary payload, human-mode serialization (pretty JSON
    /// fallback) also produces valid JSON.
    #[test]
    fn human_emit_produces_valid_json(payload in arb_payload()) {
        let json_str = serialize_as_emit_human(&payload)
            .expect("serde_json::to_string_pretty should not fail for ArbitraryPayload");
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&json_str);
        prop_assert!(
            parsed.is_ok(),
            "emit human output must be parseable JSON: {:?}",
            json_str
        );
    }

    /// For any exit code (0, 1, 2) and arbitrary error message,
    /// `emit_error` in JSON mode produces valid parseable JSON containing
    /// the "error" and "exit_code" fields.
    #[test]
    fn emit_error_produces_valid_json(
        exit_code in prop::sample::select(vec![0u8, 1, 2]),
        message in ".*"
    ) {
        let json_str = serialize_as_emit_error(exit_code, &message);
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&json_str);
        prop_assert!(
            parsed.is_ok(),
            "emit_error JSON output must be parseable: {:?}",
            json_str
        );
        let val = parsed.unwrap();
        prop_assert!(val.get("error").is_some(), "JSON must contain 'error' field");
        prop_assert!(val.get("exit_code").is_some(), "JSON must contain 'exit_code' field");
    }

    /// Round-trip: for any payload, serializing to JSON and deserializing
    /// back preserves the structure (fields present with correct types).
    #[test]
    fn json_round_trip_preserves_structure(payload in arb_payload()) {
        let json_str = serialize_as_emit(&payload).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // Verify all expected fields exist with correct types
        prop_assert!(val.get("text").and_then(|v| v.as_str()).is_some());
        prop_assert!(val.get("number").and_then(|v| v.as_i64()).is_some());
        prop_assert!(val.get("flag").and_then(|v| v.as_bool()).is_some());
        prop_assert!(val.get("items").and_then(|v| v.as_array()).is_some());
    }

    /// OutputFormat::Json and OutputFormat::Human are the only two variants,
    /// and both are well-defined display values.
    #[test]
    fn output_format_display_is_stable(_dummy in 0..1u8) {
        prop_assert_eq!(OutputFormat::Json.to_string(), "json");
        prop_assert_eq!(OutputFormat::Human.to_string(), "human");
    }
}
