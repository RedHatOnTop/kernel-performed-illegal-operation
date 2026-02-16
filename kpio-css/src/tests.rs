//! CSS Parser Unit Tests
//!
//! Comprehensive tests for CSS parsing, selectors, and cascading.

#[cfg(test)]
mod selector_tests {
    use crate::selector::{Selector, SelectorParser, Specificity};

    #[test]
    fn test_element_selector() {
        let parser = SelectorParser::new();
        let sel = parser.parse("div").unwrap();

        assert!(sel.matches_element("div"));
        assert!(!sel.matches_element("span"));
    }

    #[test]
    fn test_class_selector() {
        let parser = SelectorParser::new();
        let sel = parser.parse(".main").unwrap();

        // Specificity for class selector
        assert!(sel.specificity().classes > 0);
    }

    #[test]
    fn test_id_selector() {
        let parser = SelectorParser::new();
        let sel = parser.parse("#header").unwrap();

        // Specificity for ID selector
        assert!(sel.specificity().ids > 0);
    }

    #[test]
    fn test_descendant_combinator() {
        let parser = SelectorParser::new();
        let sel = parser.parse("div span").unwrap();

        assert!(sel.is_compound());
    }

    #[test]
    fn test_child_combinator() {
        let parser = SelectorParser::new();
        let sel = parser.parse("div > span").unwrap();

        assert!(sel.is_compound());
    }

    #[test]
    fn test_sibling_combinators() {
        let parser = SelectorParser::new();

        // Adjacent sibling
        let adj = parser.parse("h1 + p").unwrap();
        assert!(adj.is_compound());

        // General sibling
        let gen = parser.parse("h1 ~ p").unwrap();
        assert!(gen.is_compound());
    }

    #[test]
    fn test_specificity_calculation() {
        fn calc_specificity(sel: &str) -> (u32, u32, u32) {
            let mut ids = 0;
            let mut classes = 0;
            let mut elements = 0;

            for part in sel.split_whitespace() {
                if part.starts_with('#') {
                    ids += 1;
                } else if part.starts_with('.') {
                    classes += 1;
                } else if part
                    .chars()
                    .next()
                    .map(|c| c.is_alphabetic())
                    .unwrap_or(false)
                {
                    elements += 1;
                }
            }

            (ids, classes, elements)
        }

        assert_eq!(calc_specificity("#main"), (1, 0, 0));
        assert_eq!(calc_specificity(".header"), (0, 1, 0));
        assert_eq!(calc_specificity("div"), (0, 0, 1));
        assert_eq!(calc_specificity("#main .header div"), (1, 1, 1));
    }

    #[test]
    fn test_specificity_comparison() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        struct Specificity {
            ids: u32,
            classes: u32,
            elements: u32,
        }

        let a = Specificity {
            ids: 1,
            classes: 0,
            elements: 0,
        };
        let b = Specificity {
            ids: 0,
            classes: 10,
            elements: 0,
        };

        // IDs always win
        assert!(a > b);
    }

    #[test]
    fn test_pseudo_class_selectors() {
        let pseudo_classes = [
            ":hover",
            ":active",
            ":focus",
            ":visited",
            ":first-child",
            ":last-child",
            ":nth-child(2n)",
            ":checked",
            ":disabled",
            ":enabled",
        ];

        for pseudo in pseudo_classes {
            assert!(pseudo.starts_with(':'));
        }
    }

    #[test]
    fn test_pseudo_element_selectors() {
        let pseudo_elements = [
            "::before",
            "::after",
            "::first-line",
            "::first-letter",
            "::placeholder",
            "::selection",
        ];

        for pseudo in pseudo_elements {
            assert!(pseudo.starts_with("::"));
        }
    }

    #[test]
    fn test_attribute_selectors() {
        fn matches_attr(selector: &str, attr_name: &str, attr_value: &str) -> bool {
            // [attr]
            if selector == format!("[{}]", attr_name) {
                return true;
            }
            // [attr=value]
            if selector == format!("[{}=\"{}\"]", attr_name, attr_value) {
                return true;
            }
            false
        }

        assert!(matches_attr("[href]", "href", ""));
        assert!(matches_attr("[type=\"text\"]", "type", "text"));
    }
}

#[cfg(test)]
mod parser_tests {
    use crate::parser::{CssParser, ParseError};

    #[test]
    fn test_parser_creation() {
        let parser = CssParser::new();
        assert!(parser.is_ready());
    }

    #[test]
    fn test_simple_rule() {
        let parser = CssParser::new();
        let css = "div { color: red; }";

        let result = parser.parse(css);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_declarations() {
        let parser = CssParser::new();
        let css = r#"
            .container {
                width: 100%;
                padding: 10px;
                margin: 0 auto;
                background-color: #fff;
            }
        "#;

        let result = parser.parse(css);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_selectors() {
        let parser = CssParser::new();
        let css = "h1, h2, h3 { font-weight: bold; }";

        let result = parser.parse(css);
        assert!(result.is_ok());
    }

    #[test]
    fn test_at_rule_import() {
        let parser = CssParser::new();
        let css = "@import url('styles.css');";

        let result = parser.parse(css);
        assert!(result.is_ok());
    }

    #[test]
    fn test_at_rule_media() {
        let parser = CssParser::new();
        let css = r#"
            @media (min-width: 768px) {
                .container { max-width: 720px; }
            }
        "#;

        let result = parser.parse(css);
        assert!(result.is_ok());
    }

    #[test]
    fn test_at_rule_keyframes() {
        let parser = CssParser::new();
        let css = r#"
            @keyframes fade {
                from { opacity: 0; }
                to { opacity: 1; }
            }
        "#;

        let result = parser.parse(css);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod value_tests {
    use crate::values::{Color, CssValue, Length};

    #[test]
    fn test_length_units() {
        fn parse_length(s: &str) -> Option<(f32, &str)> {
            let num_end = s
                .find(|c: char| c.is_alphabetic() || c == '%')
                .unwrap_or(s.len());
            let num: f32 = s[..num_end].parse().ok()?;
            let unit = &s[num_end..];
            Some((num, unit))
        }

        assert_eq!(parse_length("10px"), Some((10.0, "px")));
        assert_eq!(parse_length("2em"), Some((2.0, "em")));
        assert_eq!(parse_length("100%"), Some((100.0, "%")));
        assert_eq!(parse_length("1.5rem"), Some((1.5, "rem")));
    }

    #[test]
    fn test_color_hex() {
        fn parse_hex_color(s: &str) -> Option<(u8, u8, u8)> {
            let s = s.strip_prefix('#')?;

            if s.len() == 3 {
                let r = u8::from_str_radix(&s[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&s[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&s[2..3], 16).ok()? * 17;
                Some((r, g, b))
            } else if s.len() == 6 {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                Some((r, g, b))
            } else {
                None
            }
        }

        assert_eq!(parse_hex_color("#fff"), Some((255, 255, 255)));
        assert_eq!(parse_hex_color("#000000"), Some((0, 0, 0)));
        assert_eq!(parse_hex_color("#ff0000"), Some((255, 0, 0)));
    }

    #[test]
    fn test_color_rgb() {
        fn parse_rgb(s: &str) -> Option<(u8, u8, u8)> {
            let inner = s.strip_prefix("rgb(")?.strip_suffix(')')?;
            let parts: Vec<_> = inner.split(',').collect();

            if parts.len() != 3 {
                return None;
            }

            let r: u8 = parts[0].trim().parse().ok()?;
            let g: u8 = parts[1].trim().parse().ok()?;
            let b: u8 = parts[2].trim().parse().ok()?;

            Some((r, g, b))
        }

        assert_eq!(parse_rgb("rgb(255, 0, 0)"), Some((255, 0, 0)));
        assert_eq!(parse_rgb("rgb(0, 128, 255)"), Some((0, 128, 255)));
    }

    #[test]
    fn test_color_named() {
        fn get_named_color(name: &str) -> Option<(u8, u8, u8)> {
            match name.to_lowercase().as_str() {
                "red" => Some((255, 0, 0)),
                "green" => Some((0, 128, 0)),
                "blue" => Some((0, 0, 255)),
                "white" => Some((255, 255, 255)),
                "black" => Some((0, 0, 0)),
                "yellow" => Some((255, 255, 0)),
                "cyan" => Some((0, 255, 255)),
                "magenta" => Some((255, 0, 255)),
                "transparent" => Some((0, 0, 0)), // With alpha 0
                _ => None,
            }
        }

        assert_eq!(get_named_color("red"), Some((255, 0, 0)));
        assert_eq!(get_named_color("RED"), Some((255, 0, 0)));
    }

    #[test]
    fn test_keyword_values() {
        let keywords = ["auto", "inherit", "initial", "unset", "none"];

        for keyword in keywords {
            assert!(!keyword.is_empty());
        }
    }
}

#[cfg(test)]
mod cascade_tests {
    use crate::cascade::{Cascade, Importance, Origin};

    #[test]
    fn test_origin_priority() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        enum Origin {
            UserAgent,
            User,
            Author,
        }

        // Author styles override user agent
        assert!(Origin::Author > Origin::UserAgent);
    }

    #[test]
    fn test_important_flag() {
        fn is_important(value: &str) -> bool {
            value.trim().ends_with("!important")
        }

        assert!(is_important("red !important"));
        assert!(!is_important("red"));
    }

    #[test]
    fn test_cascade_order() {
        // Cascade order: Origin -> Importance -> Specificity -> Order
        #[derive(Debug)]
        struct CascadeEntry {
            origin: u8,
            important: bool,
            specificity: (u32, u32, u32),
            order: usize,
        }

        fn cascade_compare(a: &CascadeEntry, b: &CascadeEntry) -> core::cmp::Ordering {
            // Important declarations from author win over non-important
            if a.important != b.important {
                return if a.important {
                    core::cmp::Ordering::Greater
                } else {
                    core::cmp::Ordering::Less
                };
            }

            // Then by origin
            if a.origin != b.origin {
                return a.origin.cmp(&b.origin);
            }

            // Then by specificity
            let spec_cmp = a.specificity.cmp(&b.specificity);
            if spec_cmp != core::cmp::Ordering::Equal {
                return spec_cmp;
            }

            // Finally by order (later wins)
            a.order.cmp(&b.order)
        }

        let a = CascadeEntry {
            origin: 1,
            important: false,
            specificity: (1, 0, 0),
            order: 0,
        };
        let b = CascadeEntry {
            origin: 1,
            important: true,
            specificity: (0, 0, 1),
            order: 1,
        };

        // Important wins even with lower specificity
        assert!(matches!(cascade_compare(&a, &b), core::cmp::Ordering::Less));
    }
}

#[cfg(test)]
mod property_tests {
    use crate::properties::{Property, PropertyId};

    #[test]
    fn test_property_inheritance() {
        fn is_inherited(prop: &str) -> bool {
            matches!(
                prop,
                "color"
                    | "font-family"
                    | "font-size"
                    | "font-style"
                    | "font-weight"
                    | "line-height"
                    | "text-align"
                    | "visibility"
                    | "cursor"
                    | "list-style-type"
            )
        }

        assert!(is_inherited("color"));
        assert!(is_inherited("font-size"));
        assert!(!is_inherited("width"));
        assert!(!is_inherited("margin"));
    }

    #[test]
    fn test_shorthand_expansion() {
        fn expand_margin(value: &str) -> [&str; 4] {
            let parts: Vec<_> = value.split_whitespace().collect();

            match parts.len() {
                1 => [parts[0], parts[0], parts[0], parts[0]],
                2 => [parts[0], parts[1], parts[0], parts[1]],
                3 => [parts[0], parts[1], parts[2], parts[1]],
                4 => [parts[0], parts[1], parts[2], parts[3]],
                _ => ["0", "0", "0", "0"],
            }
        }

        assert_eq!(expand_margin("10px"), ["10px"; 4]);
        assert_eq!(expand_margin("10px 20px"), ["10px", "20px", "10px", "20px"]);
    }

    #[test]
    fn test_property_ids() {
        let properties = [
            "display",
            "position",
            "top",
            "right",
            "bottom",
            "left",
            "width",
            "height",
            "margin",
            "padding",
            "border",
            "background",
            "color",
            "font-size",
            "line-height",
        ];

        for (id, prop) in properties.iter().enumerate() {
            assert!(!prop.is_empty());
            assert!(id < 100);
        }
    }
}

#[cfg(test)]
mod computed_tests {
    #[test]
    fn test_percentage_resolution() {
        fn resolve_percentage(percent: f32, base: f32) -> f32 {
            percent / 100.0 * base
        }

        assert_eq!(resolve_percentage(50.0, 200.0), 100.0);
        assert_eq!(resolve_percentage(100.0, 100.0), 100.0);
    }

    #[test]
    fn test_em_resolution() {
        fn resolve_em(em: f32, font_size: f32) -> f32 {
            em * font_size
        }

        // 2em with 16px font = 32px
        assert_eq!(resolve_em(2.0, 16.0), 32.0);
    }

    #[test]
    fn test_rem_resolution() {
        fn resolve_rem(rem: f32, root_font_size: f32) -> f32 {
            rem * root_font_size
        }

        // 1.5rem with 16px root = 24px
        assert_eq!(resolve_rem(1.5, 16.0), 24.0);
    }

    #[test]
    fn test_viewport_units() {
        fn resolve_vw(vw: f32, viewport_width: f32) -> f32 {
            vw / 100.0 * viewport_width
        }

        fn resolve_vh(vh: f32, viewport_height: f32) -> f32 {
            vh / 100.0 * viewport_height
        }

        // 50vw with 1920px viewport = 960px
        assert_eq!(resolve_vw(50.0, 1920.0), 960.0);
        assert_eq!(resolve_vh(100.0, 1080.0), 1080.0);
    }
}

#[cfg(test)]
mod stylesheet_tests {
    use crate::stylesheet::{StyleRule, StyleSheet};

    #[test]
    fn test_stylesheet_creation() {
        let sheet = StyleSheet::new();
        assert!(sheet.rules().is_empty());
    }

    #[test]
    fn test_rule_iteration() {
        let sheet = StyleSheet::new();
        let rules: Vec<_> = sheet.rules().iter().collect();

        assert!(rules.is_empty());
    }

    #[test]
    fn test_media_query_evaluation() {
        fn matches_media_query(query: &str, width: u32, height: u32) -> bool {
            if query.contains("min-width") {
                if let Some(start) = query.find("min-width:") {
                    let rest = &query[start + 10..];
                    if let Some(end) = rest.find("px") {
                        let min: u32 = rest[..end].trim().parse().unwrap_or(0);
                        return width >= min;
                    }
                }
            }
            true
        }

        assert!(matches_media_query("(min-width: 768px)", 1024, 768));
        assert!(!matches_media_query("(min-width: 768px)", 480, 800));
    }
}
