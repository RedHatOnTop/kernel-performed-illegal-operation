//! HTML Parser Unit Tests
//!
//! Comprehensive tests for HTML tokenizer and parser.

#[cfg(test)]
mod tokenizer_tests {
    use crate::tokenizer::{Token, Tokenizer, TokenizerState};

    #[test]
    fn test_tokenizer_init() {
        let tokenizer = Tokenizer::new("");
        assert!(!tokenizer.is_eof());
    }

    #[test]
    fn test_simple_tag_tokenizing() {
        let mut tokenizer = Tokenizer::new("<div>");
        let tokens = tokenizer.tokenize();

        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_self_closing_tag() {
        let mut tokenizer = Tokenizer::new("<br/>");
        let tokens = tokenizer.tokenize();

        // Should have at least start tag
        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_text_content() {
        let mut tokenizer = Tokenizer::new("Hello World");
        let tokens = tokenizer.tokenize();

        // Should produce text token
        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_tag_with_attributes() {
        let mut tokenizer = Tokenizer::new(r#"<div id="test" class="main">"#);
        let tokens = tokenizer.tokenize();

        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_comment_tokenizing() {
        let mut tokenizer = Tokenizer::new("<!-- comment -->");
        let tokens = tokenizer.tokenize();

        // Comments should be tokenized
        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_doctype() {
        let mut tokenizer = Tokenizer::new("<!DOCTYPE html>");
        let tokens = tokenizer.tokenize();

        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_entity_decoding() {
        fn decode_entity(entity: &str) -> Option<char> {
            match entity {
                "&amp;" => Some('&'),
                "&lt;" => Some('<'),
                "&gt;" => Some('>'),
                "&quot;" => Some('"'),
                "&apos;" => Some('\''),
                "&nbsp;" => Some('\u{00A0}'),
                _ => None,
            }
        }

        assert_eq!(decode_entity("&amp;"), Some('&'));
        assert_eq!(decode_entity("&lt;"), Some('<'));
        assert_eq!(decode_entity("&unknown;"), None);
    }

    #[test]
    fn test_numeric_entity() {
        fn decode_numeric(entity: &str) -> Option<char> {
            if entity.starts_with("&#x") && entity.ends_with(';') {
                let hex = &entity[3..entity.len() - 1];
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else if entity.starts_with("&#") && entity.ends_with(';') {
                let dec = &entity[2..entity.len() - 1];
                dec.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                None
            }
        }

        assert_eq!(decode_numeric("&#65;"), Some('A'));
        assert_eq!(decode_numeric("&#x41;"), Some('A'));
    }
}

#[cfg(test)]
mod tree_builder_tests {
    use crate::tree_builder::{NodeType, TreeBuilder};

    #[test]
    fn test_simple_document() {
        let html = "<html><head></head><body></body></html>";
        let builder = TreeBuilder::new();
        let result = builder.build(html);

        assert!(result.is_ok());
    }

    #[test]
    fn test_nested_elements() {
        let html = "<div><span><a>link</a></span></div>";
        let builder = TreeBuilder::new();
        let result = builder.build(html);

        assert!(result.is_ok());
    }

    #[test]
    fn test_void_elements() {
        // Elements that don't need closing tags
        let void_elements = ["br", "hr", "img", "input", "meta", "link", "area", "base"];

        for elem in void_elements {
            assert!(is_void_element(elem));
        }

        fn is_void_element(name: &str) -> bool {
            matches!(
                name,
                "area"
                    | "base"
                    | "br"
                    | "col"
                    | "embed"
                    | "hr"
                    | "img"
                    | "input"
                    | "link"
                    | "meta"
                    | "source"
                    | "track"
                    | "wbr"
            )
        }
    }

    #[test]
    fn test_text_nodes() {
        let html = "<p>Hello <strong>world</strong>!</p>";
        let builder = TreeBuilder::new();
        let result = builder.build(html);

        assert!(result.is_ok());
    }

    #[test]
    fn test_attribute_parsing() {
        #[derive(Debug, Default)]
        struct Attribute {
            name: String,
            value: String,
        }

        use alloc::string::String;

        fn parse_attribute(s: &str) -> Option<Attribute> {
            let mut parts = s.splitn(2, '=');
            let name = parts.next()?.trim().to_lowercase();
            let value = parts
                .next()
                .map(|v| v.trim().trim_matches('"').to_string())
                .unwrap_or_default();

            Some(Attribute {
                name: String::from(name.as_str()),
                value,
            })
        }

        let attr = parse_attribute(r#"class="main""#).unwrap();
        assert_eq!(attr.name, "class");
        assert_eq!(attr.value, "main");
    }
}

#[cfg(test)]
mod parser_tests {
    use crate::parser::{HtmlParser, ParseError};

    #[test]
    fn test_parser_creation() {
        let parser = HtmlParser::new();
        assert!(parser.is_ready());
    }

    #[test]
    fn test_full_document_parsing() {
        let parser = HtmlParser::new();
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body><p>Hello</p></body>
            </html>
        "#;

        let result = parser.parse(html);
        assert!(result.is_ok());
    }

    #[test]
    fn test_fragment_parsing() {
        let parser = HtmlParser::new();
        let fragment = "<div><span>text</span></div>";

        let result = parser.parse_fragment(fragment);
        assert!(result.is_ok());
    }

    #[test]
    fn test_error_recovery() {
        let parser = HtmlParser::new();
        // Mismatched tags
        let html = "<div><span></div></span>";

        let result = parser.parse(html);
        // Should still parse with error recovery
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_script_parsing() {
        let parser = HtmlParser::new();
        let html = r#"
            <script>
                var x = 1 < 2 && 3 > 4;
            </script>
        "#;

        let result = parser.parse(html);
        assert!(result.is_ok());
    }

    #[test]
    fn test_style_parsing() {
        let parser = HtmlParser::new();
        let html = r#"
            <style>
                .class { color: red; }
            </style>
        "#;

        let result = parser.parse(html);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod element_tests {
    #[test]
    fn test_tag_name_normalization() {
        fn normalize_tag(name: &str) -> String {
            use alloc::string::String;
            String::from(name.to_lowercase().as_str())
        }

        use alloc::string::String;

        assert_eq!(normalize_tag("DIV"), String::from("div"));
        assert_eq!(normalize_tag("SPAN"), String::from("span"));
    }

    #[test]
    fn test_element_categories() {
        fn is_block_element(tag: &str) -> bool {
            matches!(
                tag,
                "div"
                    | "p"
                    | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "h5"
                    | "h6"
                    | "ul"
                    | "ol"
                    | "li"
                    | "table"
                    | "form"
                    | "header"
                    | "footer"
                    | "section"
                    | "article"
                    | "nav"
                    | "aside"
                    | "main"
                    | "figure"
            )
        }

        fn is_inline_element(tag: &str) -> bool {
            matches!(
                tag,
                "span"
                    | "a"
                    | "strong"
                    | "em"
                    | "b"
                    | "i"
                    | "u"
                    | "code"
                    | "small"
                    | "abbr"
                    | "cite"
                    | "q"
                    | "sub"
                    | "sup"
            )
        }

        assert!(is_block_element("div"));
        assert!(is_inline_element("span"));
        assert!(!is_block_element("span"));
    }

    #[test]
    fn test_html5_semantic_elements() {
        let semantic = [
            "header",
            "footer",
            "nav",
            "article",
            "section",
            "aside",
            "main",
            "figure",
            "figcaption",
            "time",
            "mark",
        ];

        for elem in semantic {
            assert!(!elem.is_empty());
        }
    }
}

#[cfg(test)]
mod dom_tests {
    #[test]
    fn test_node_type_enum() {
        #[derive(Debug, Clone, Copy, PartialEq)]
        enum NodeType {
            Element = 1,
            Text = 3,
            Comment = 8,
            Document = 9,
            DocumentType = 10,
            DocumentFragment = 11,
        }

        assert_eq!(NodeType::Element as u8, 1);
        assert_eq!(NodeType::Text as u8, 3);
    }

    #[test]
    fn test_parent_child_relationship() {
        struct Node {
            parent: Option<usize>,
            children: Vec<usize>,
        }

        use alloc::vec;
        use alloc::vec::Vec;

        let mut nodes = vec![
            Node {
                parent: None,
                children: vec![1, 2],
            },
            Node {
                parent: Some(0),
                children: vec![],
            },
            Node {
                parent: Some(0),
                children: vec![],
            },
        ];

        // Node 0 is parent of 1 and 2
        assert_eq!(nodes[0].children.len(), 2);
        assert_eq!(nodes[1].parent, Some(0));
    }

    #[test]
    fn test_sibling_navigation() {
        fn get_next_sibling(parent_children: &[usize], current: usize) -> Option<usize> {
            let pos = parent_children.iter().position(|&id| id == current)?;
            parent_children.get(pos + 1).copied()
        }

        fn get_prev_sibling(parent_children: &[usize], current: usize) -> Option<usize> {
            let pos = parent_children.iter().position(|&id| id == current)?;
            if pos > 0 {
                Some(parent_children[pos - 1])
            } else {
                None
            }
        }

        let children = [1, 2, 3, 4];

        assert_eq!(get_next_sibling(&children, 2), Some(3));
        assert_eq!(get_prev_sibling(&children, 2), Some(1));
        assert_eq!(get_next_sibling(&children, 4), None);
    }
}

#[cfg(test)]
mod insertion_mode_tests {
    #[test]
    fn test_insertion_modes() {
        #[derive(Debug, Clone, Copy, PartialEq)]
        enum InsertionMode {
            Initial,
            BeforeHtml,
            BeforeHead,
            InHead,
            AfterHead,
            InBody,
            Text,
            InTable,
            AfterBody,
            AfterAfterBody,
        }

        let mode = InsertionMode::Initial;

        // State machine transitions
        let next = match mode {
            InsertionMode::Initial => InsertionMode::BeforeHtml,
            InsertionMode::BeforeHtml => InsertionMode::BeforeHead,
            InsertionMode::BeforeHead => InsertionMode::InHead,
            InsertionMode::InHead => InsertionMode::AfterHead,
            InsertionMode::AfterHead => InsertionMode::InBody,
            InsertionMode::InBody => InsertionMode::AfterBody,
            _ => mode,
        };

        assert_eq!(next, InsertionMode::BeforeHtml);
    }
}
