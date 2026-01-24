//! Browser Integration Test - Tests HTML parsing, CSS styling, and DOM traversal
//!
//! This example demonstrates the complete HTML→DOM→CSS pipeline for KPIO OS.

use kpio_dom::document::parse_html;
use kpio_dom::Document;
use kpio_dom::traversal::{NodeIterator, show};
use kpio_dom::node::Node;
use kpio_css::prelude::*;
use kpio_css::computed::ComputedStyle;

fn main() {
    println!("=== KPIO Browser Engine Integration Test ===\n");

    // Test 1: Basic HTML parsing
    test_basic_html_parsing();

    // Test 2: HTML with attributes
    test_html_with_attributes();

    // Test 3: Nested structure
    test_nested_structure();

    // Test 4: CSS styling
    test_css_styling();

    // Test 5: DOM traversal
    test_dom_traversal();

    // Test 6: Full page rendering simulation
    test_full_page();

    println!("\n=== All tests completed successfully! ===");
}

fn test_basic_html_parsing() {
    println!("Test 1: Basic HTML Parsing");
    println!("--------------------------");

    let html = "<html><head><title>Test Page</title></head><body><p>Hello, KPIO!</p></body></html>";
    let doc = parse_html(html);

    println!("  Document nodes: {}", doc.len());

    let p_elements = doc.get_elements_by_tag_name("p");
    assert_eq!(p_elements.len(), 1, "Should have one <p> element");

    let p_text = doc.text_content(p_elements[0]);
    assert_eq!(p_text, "Hello, KPIO!", "Paragraph text should match");

    println!("  ✓ Parsed HTML correctly");
    println!("  ✓ Found <p> element with text: \"{}\"", p_text);
    println!();
}

fn test_html_with_attributes() {
    println!("Test 2: HTML with Attributes");
    println!("-----------------------------");

    let html = r#"<html>
        <body>
            <div id="main" class="container">
                <a href="https://example.com" target="_blank">Link</a>
            </div>
        </body>
    </html>"#;

    let doc = parse_html(html);

    // Find element by ID
    let main_id = doc.get_element_by_id("main");
    assert!(main_id.is_some(), "Should find element with id='main'");
    println!("  ✓ Found element by ID: main");

    // Check class
    let containers = doc.get_elements_by_class_name("container");
    assert_eq!(containers.len(), 1, "Should have one element with class 'container'");
    println!("  ✓ Found element by class: container");

    // Check link attributes
    let links = doc.get_elements_by_tag_name("a");
    assert_eq!(links.len(), 1, "Should have one <a> element");
    
    if let Some(link) = doc.get(links[0]) {
        let href = link.get_attribute("href");
        assert_eq!(href, Some("https://example.com"));
        println!("  ✓ Link href: {:?}", href);
    }
    println!();
}

fn test_nested_structure() {
    println!("Test 3: Nested Structure");
    println!("------------------------");

    let html = r#"<html>
        <body>
            <div class="level1">
                <div class="level2">
                    <div class="level3">
                        <span>Deep content</span>
                    </div>
                </div>
            </div>
        </body>
    </html>"#;

    let doc = parse_html(html);

    // Count divs
    let divs = doc.get_elements_by_tag_name("div");
    println!("  ✓ Found {} <div> elements", divs.len());

    // Check nesting depth
    let span_elements = doc.get_elements_by_tag_name("span");
    if !span_elements.is_empty() {
        let depth = doc.depth(span_elements[0]);
        println!("  ✓ <span> element depth: {}", depth);
    }

    // Check ancestors
    if let Some(&span_id) = span_elements.first() {
        let ancestors = doc.ancestors(span_id);
        println!("  ✓ <span> has {} ancestor(s)", ancestors.len());
    }
    println!();
}

fn test_css_styling() {
    println!("Test 4: CSS Styling");
    println!("-------------------");

    // Parse CSS
    let css = r#"
        body {
            margin: 0;
            padding: 20px;
            background-color: #f0f0f0;
        }
        
        .container {
            max-width: 800px;
            margin: 0 auto;
            background: white;
        }
        
        h1 {
            color: #333;
            font-size: 24px;
        }
        
        p {
            color: #666;
            line-height: 1.5;
        }
        
        .highlight {
            background-color: yellow;
            font-weight: bold;
        }
    "#;

    let parser = CssParser::new(css);
    match parser.parse_stylesheet() {
        Ok(stylesheet) => {
            println!("  ✓ Parsed {} CSS rules", stylesheet.rules.len());
            
            for (i, rule) in stylesheet.rules.iter().enumerate() {
                match rule {
                    kpio_css::stylesheet::Rule::Style(style_rule) => {
                        println!("    Rule {}: {} declarations", 
                            i + 1, 
                            style_rule.declarations.len());
                    }
                    _ => {}
                }
            }
        }
        Err(e) => {
            println!("  ✗ CSS parse error: {:?}", e);
        }
    }
    println!();
}

fn test_dom_traversal() {
    println!("Test 5: DOM Traversal");
    println!("---------------------");

    let html = r#"<html>
        <head><title>Traversal Test</title></head>
        <body>
            <header><h1>Title</h1></header>
            <main>
                <article>
                    <p>Paragraph 1</p>
                    <p>Paragraph 2</p>
                </article>
            </main>
            <footer>Footer</footer>
        </body>
    </html>"#;

    let doc = parse_html(html);

    // Get document element
    if let Some(html_elem) = doc.document_element() {
        println!("  ✓ Document element: <{}>", html_elem.tag_name().unwrap_or("?"));
    }

    // Count all elements
    if let Some(root_id) = doc.document_element_id() {
        let all_elements = doc.element_descendants(root_id);
        println!("  ✓ Total elements: {}", all_elements.len());
    }

    // Find specific elements
    let headers = doc.get_elements_by_tag_name("header");
    let mains = doc.get_elements_by_tag_name("main");
    let footers = doc.get_elements_by_tag_name("footer");
    
    println!("  ✓ Structure: {} header(s), {} main(s), {} footer(s)",
        headers.len(), mains.len(), footers.len());

    // Get children of body
    if let Some(body) = doc.body() {
        let body_children = doc.child_elements(body.id);
        println!("  ✓ Body has {} direct children", body_children.len());
    }
    println!();
}

fn test_full_page() {
    println!("Test 6: Full Page Rendering Simulation");
    println!("--------------------------------------");

    let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>KPIO OS - Welcome</title>
</head>
<body>
    <header class="site-header">
        <nav>
            <a href="/" class="logo">KPIO</a>
            <ul class="menu">
                <li><a href="/about">About</a></li>
                <li><a href="/docs">Documentation</a></li>
                <li><a href="/download">Download</a></li>
            </ul>
        </nav>
    </header>
    
    <main id="content">
        <section class="hero">
            <h1>Welcome to KPIO OS</h1>
            <p class="tagline">A modern operating system written in Rust</p>
            <button class="cta">Get Started</button>
        </section>
        
        <section class="features">
            <div class="feature">
                <h3>Safe</h3>
                <p>Memory-safe kernel written entirely in Rust</p>
            </div>
            <div class="feature">
                <h3>Fast</h3>
                <p>Optimized for modern hardware</p>
            </div>
            <div class="feature">
                <h3>Open</h3>
                <p>Fully open source under MIT license</p>
            </div>
        </section>
    </main>
    
    <footer>
        <p>&copy; 2024 KPIO Project</p>
    </footer>
</body>
</html>"#;

    let css = r#"
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: system-ui, sans-serif; }
        .site-header { background: #1a1a2e; color: white; padding: 1rem; }
        .logo { font-size: 1.5rem; font-weight: bold; }
        .hero { text-align: center; padding: 4rem 2rem; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); }
        .hero h1 { font-size: 3rem; color: white; }
        .tagline { font-size: 1.25rem; color: rgba(255,255,255,0.9); }
        .cta { padding: 1rem 2rem; background: white; border: none; border-radius: 4px; }
        .features { display: flex; justify-content: center; gap: 2rem; padding: 4rem 2rem; }
        .feature { flex: 1; max-width: 300px; text-align: center; }
        footer { background: #1a1a2e; color: white; padding: 2rem; text-align: center; }
    "#;

    // Parse HTML
    let doc = parse_html(html);
    println!("  ✓ Parsed HTML document ({} nodes)", doc.len());

    // Parse CSS
    let stylesheet = CssParser::new(css).parse_stylesheet().unwrap();
    println!("  ✓ Parsed CSS stylesheet ({} rules)", stylesheet.rules.len());

    // Apply styles
    let mut resolver = doc.create_style_resolver();
    resolver.add_stylesheet(stylesheet);
    
    if let Some(styled_tree) = resolver.resolve() {
        println!("  ✓ Applied styles to DOM");
        println!("  ✓ Root styled node: {:?}", styled_tree.node_id);
        println!("  ✓ Children count: {}", styled_tree.children.len());
    }

    // Verify structure
    let nav = doc.get_elements_by_tag_name("nav");
    let sections = doc.get_elements_by_tag_name("section");
    let features = doc.get_elements_by_class_name("feature");
    
    println!("\n  Page structure:");
    println!("    - Navigation: {} element(s)", nav.len());
    println!("    - Sections: {} element(s)", sections.len());
    println!("    - Features: {} element(s)", features.len());

    // Get page title
    let titles = doc.get_elements_by_tag_name("title");
    if let Some(&title_id) = titles.first() {
        let title_text = doc.text_content(title_id);
        println!("    - Title: \"{}\"", title_text);
    }

    // Count links
    let links = doc.get_elements_by_tag_name("a");
    println!("    - Links: {} element(s)", links.len());

    println!("\n  ✓ Full page simulation complete!");
}
