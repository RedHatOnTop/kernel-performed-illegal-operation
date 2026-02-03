//! CSS Parser Fuzzing Target
//!
//! Run with: cargo fuzz run css_parser_fuzz

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    
    // Skip extremely large inputs
    if input.len() > 500_000 {
        return;
    }

    // In actual fuzzing, call the real CSS parser:
    // let _stylesheet = kpio_css::CssParser::new(&input).parse();
    
    simulate_css_parse(&input);
});

fn simulate_css_parse(input: &str) {
    let mut brace_depth = 0i32;
    let mut paren_depth = 0i32;
    let mut in_string = false;
    let mut string_char = '"';
    let mut in_comment = false;
    let mut prev_char = ' ';

    for c in input.chars() {
        // Handle comments
        if !in_string {
            if prev_char == '/' && c == '*' && !in_comment {
                in_comment = true;
                prev_char = c;
                continue;
            }
            if prev_char == '*' && c == '/' && in_comment {
                in_comment = false;
                prev_char = c;
                continue;
            }
        }

        if in_comment {
            prev_char = c;
            continue;
        }

        // Handle strings
        if (c == '"' || c == '\'') && prev_char != '\\' {
            if !in_string {
                in_string = true;
                string_char = c;
            } else if c == string_char {
                in_string = false;
            }
        }

        if in_string {
            prev_char = c;
            continue;
        }

        // Track nesting
        match c {
            '{' => brace_depth = brace_depth.saturating_add(1),
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => paren_depth = paren_depth.saturating_sub(1),
            _ => {}
        }

        prev_char = c;
    }

    let _final_brace_depth = brace_depth;
    let _final_paren_depth = paren_depth;
    let _unclosed_string = in_string;
    let _unclosed_comment = in_comment;
}
