//! HTML Parser Fuzzing Target
//!
//! Run with: cargo fuzz run html_parser_fuzz

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Convert to string (lossy for invalid UTF-8)
    let input = String::from_utf8_lossy(data);
    
    // Skip extremely large inputs
    if input.len() > 1_000_000 {
        return;
    }

    // In actual fuzzing, call the real HTML parser:
    // let _doc = kpio_html::HtmlParser::new(&input).parse();
    
    // Simulate parsing for testing
    simulate_html_parse(&input);
});

fn simulate_html_parse(input: &str) {
    let mut depth = 0i32;
    let mut in_tag = false;
    let mut in_comment = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '<' if !in_comment => {
                // Check for comment start
                if chars.clone().take(3).collect::<String>() == "!--" {
                    in_comment = true;
                    chars.next(); chars.next(); chars.next();
                    continue;
                }
                
                in_tag = true;
                
                // Check for closing tag
                if chars.peek() == Some(&'/') {
                    depth = depth.saturating_sub(1);
                } else if chars.peek() != Some(&'!') && chars.peek() != Some(&'?') {
                    depth = depth.saturating_add(1);
                }
            }
            '>' if !in_comment => {
                in_tag = false;
            }
            '-' if in_comment => {
                // Check for comment end
                if chars.clone().take(2).collect::<String>() == "->" {
                    in_comment = false;
                    chars.next(); chars.next();
                }
            }
            _ => {}
        }
    }
    
    // Track some metrics (could be used for coverage)
    let _final_depth = depth;
    let _unclosed = in_tag;
    let _in_comment = in_comment;
}
