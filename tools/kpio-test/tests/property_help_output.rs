//! Property 23: Help JSON output validity and completeness
//!
//! For any valid subcommand name, help JSON contains name, description,
//! parameters (with name/type/required/default/description), exit_codes,
//! and examples.
//!
//! **Validates: Requirements 29.5, 29.6**

use kpio_test::cli::HelpArgs;
use kpio_test::help;
use proptest::prelude::*;

// ── Strategies ───────────────────────────────────────────────────────

fn arb_subcommand_name() -> impl Strategy<Value = String> {
    let names = help::subcommand_names();
    (0..names.len()).prop_map(move |i| names[i].clone())
}

// ── Property tests ───────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// For any valid subcommand name, help returns valid JSON with required
    /// fields: name, description, parameters, exit_codes, examples.
    #[test]
    fn help_subcommand_returns_valid_json(name in arb_subcommand_name()) {
        let args = HelpArgs { subcommand: Some(name.clone()) };
        let result = help::show(args);
        prop_assert!(result.is_ok(), "help should succeed for subcommand '{}'", name);

        let value = result.unwrap();
        prop_assert!(value.is_object(), "help output should be a JSON object");

        let obj = value.as_object().unwrap();
        prop_assert!(obj.contains_key("name"), "missing 'name' field");
        prop_assert!(obj.contains_key("description"), "missing 'description' field");
        prop_assert!(obj.contains_key("parameters"), "missing 'parameters' field");
        prop_assert!(obj.contains_key("exit_codes"), "missing 'exit_codes' field");
        prop_assert!(obj.contains_key("examples"), "missing 'examples' field");

        // Name matches the requested subcommand
        prop_assert_eq!(obj["name"].as_str().unwrap(), name.as_str());

        // Description is non-empty
        prop_assert!(!obj["description"].as_str().unwrap().is_empty());

        // Parameters is an array
        prop_assert!(obj["parameters"].is_array());

        // Exit codes is a non-empty array
        let exit_codes = obj["exit_codes"].as_array().unwrap();
        prop_assert!(!exit_codes.is_empty(), "exit_codes should not be empty");

        // Each exit code has code and meaning
        for ec in exit_codes {
            prop_assert!(ec.get("code").is_some(), "exit code missing 'code'");
            prop_assert!(ec.get("meaning").is_some(), "exit code missing 'meaning'");
        }
    }

    /// Help overview (no subcommand) returns all subcommand names.
    #[test]
    fn help_overview_lists_all_subcommands(_dummy in 0..1u32) {
        let args = HelpArgs { subcommand: None };
        let result = help::show(args).unwrap();
        let subs = result["subcommands"].as_array().unwrap();

        // Should have 27 subcommands
        prop_assert!(subs.len() >= 27, "expected at least 27 subcommands, got {}", subs.len());

        // Each entry has name and description
        for sub in subs {
            prop_assert!(sub.get("name").is_some());
            prop_assert!(sub.get("description").is_some());
            prop_assert!(!sub["name"].as_str().unwrap().is_empty());
            prop_assert!(!sub["description"].as_str().unwrap().is_empty());
        }
    }

    /// For any unknown subcommand name, help returns an error.
    #[test]
    fn help_unknown_subcommand_returns_error(name in "[a-z]{10,20}") {
        // Filter out names that happen to match real subcommands
        let known = help::subcommand_names();
        if known.contains(&name) {
            return Ok(());
        }
        let args = HelpArgs { subcommand: Some(name) };
        let result = help::show(args);
        prop_assert!(result.is_err(), "help should fail for unknown subcommand");
    }
}
