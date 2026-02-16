//! Tree Builder - Builds DOM tree from tokens
//!
//! Implements a simplified HTML5 tree construction algorithm.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use servo_types::namespace::HTML_NAMESPACE;
use servo_types::{LocalName, Namespace, QualName};

use crate::tokenizer::{Attribute, TagKind, TagToken, Token};

/// Node ID in the tree.
pub type NodeId = usize;

/// A trait for receiving tree building events.
pub trait TreeSink {
    /// Get the document node.
    fn document(&self) -> NodeId;

    /// Create a new element.
    fn create_element(&mut self, name: QualName, attrs: Vec<Attribute>) -> NodeId;

    /// Create a text node.
    fn create_text(&mut self, text: String) -> NodeId;

    /// Create a comment node.
    fn create_comment(&mut self, text: String) -> NodeId;

    /// Append a child to a parent.
    fn append(&mut self, parent: NodeId, child: NodeId);

    /// Append text to a parent, merging with existing text if possible.
    fn append_text(&mut self, parent: NodeId, text: &str);

    /// Get the parent of a node.
    fn parent(&self, node: NodeId) -> Option<NodeId>;

    /// Get the tag name of an element.
    fn element_name(&self, node: NodeId) -> Option<QualName>;

    /// Set the document's quirks mode.
    fn set_quirks_mode(&mut self, quirks: QuirksMode);
}

/// Document quirks mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuirksMode {
    #[default]
    NoQuirks,
    Quirks,
    LimitedQuirks,
}

/// Tree builder insertion mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InsertionMode {
    Initial,
    BeforeHtml,
    BeforeHead,
    InHead,
    AfterHead,
    InBody,
    Text,
    InTable,
    InTableBody,
    InRow,
    InCell,
    InSelect,
    AfterBody,
    AfterAfterBody,
}

/// HTML tree builder.
pub struct TreeBuilder<S: TreeSink> {
    sink: S,
    mode: InsertionMode,
    original_mode: InsertionMode,
    open_elements: Vec<NodeId>,
    head_element: Option<NodeId>,
    form_element: Option<NodeId>,
    frameset_ok: bool,
    scripting: bool,
}

impl<S: TreeSink> TreeBuilder<S> {
    /// Create a new tree builder.
    pub fn new(sink: S) -> Self {
        TreeBuilder {
            sink,
            mode: InsertionMode::Initial,
            original_mode: InsertionMode::Initial,
            open_elements: Vec::new(),
            head_element: None,
            form_element: None,
            frameset_ok: true,
            scripting: false,
        }
    }

    /// Get a reference to the sink.
    pub fn sink(&self) -> &S {
        &self.sink
    }

    /// Get a mutable reference to the sink.
    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    /// Consume the builder and return the sink.
    pub fn into_sink(self) -> S {
        self.sink
    }

    /// Process a token.
    pub fn process_token(&mut self, token: Token) {
        match self.mode {
            InsertionMode::Initial => self.process_initial(token),
            InsertionMode::BeforeHtml => self.process_before_html(token),
            InsertionMode::BeforeHead => self.process_before_head(token),
            InsertionMode::InHead => self.process_in_head(token),
            InsertionMode::AfterHead => self.process_after_head(token),
            InsertionMode::InBody => self.process_in_body(token),
            InsertionMode::Text => self.process_text(token),
            InsertionMode::InTable => self.process_in_body(token), // Simplified
            InsertionMode::InTableBody => self.process_in_body(token), // Simplified
            InsertionMode::InRow => self.process_in_body(token),   // Simplified
            InsertionMode::InCell => self.process_in_body(token),  // Simplified
            InsertionMode::InSelect => self.process_in_body(token), // Simplified
            InsertionMode::AfterBody => self.process_after_body(token),
            InsertionMode::AfterAfterBody => self.process_after_after_body(token),
        }
    }

    fn process_initial(&mut self, token: Token) {
        match token {
            Token::Character(c) if c.is_whitespace() => {
                // Ignore whitespace
            }
            Token::Comment(text) => {
                let comment = self.sink.create_comment(text);
                let doc = self.sink.document();
                self.sink.append(doc, comment);
            }
            Token::Doctype(doctype) => {
                if doctype.force_quirks {
                    self.sink.set_quirks_mode(QuirksMode::Quirks);
                }
                self.mode = InsertionMode::BeforeHtml;
            }
            _ => {
                self.sink.set_quirks_mode(QuirksMode::Quirks);
                self.mode = InsertionMode::BeforeHtml;
                self.process_token(token);
            }
        }
    }

    fn process_before_html(&mut self, token: Token) {
        match token {
            Token::Doctype(_) => {
                // Ignore
            }
            Token::Comment(text) => {
                let comment = self.sink.create_comment(text);
                let doc = self.sink.document();
                self.sink.append(doc, comment);
            }
            Token::Character(c) if c.is_whitespace() => {
                // Ignore
            }
            Token::StartTag(ref tag) if tag.name.as_str() == "html" => {
                let elem = self.create_element_for_token(&token);
                let doc = self.sink.document();
                self.sink.append(doc, elem);
                self.open_elements.push(elem);
                self.mode = InsertionMode::BeforeHead;
            }
            Token::EndTag(ref tag) => {
                match tag.name.as_str() {
                    "head" | "body" | "html" | "br" => {
                        self.insert_html_element();
                        self.mode = InsertionMode::BeforeHead;
                        self.process_token(token);
                    }
                    _ => {
                        // Ignore
                    }
                }
            }
            _ => {
                self.insert_html_element();
                self.mode = InsertionMode::BeforeHead;
                self.process_token(token);
            }
        }
    }

    fn process_before_head(&mut self, token: Token) {
        match token {
            Token::Character(c) if c.is_whitespace() => {
                // Ignore
            }
            Token::Comment(text) => {
                self.insert_comment(text);
            }
            Token::Doctype(_) => {
                // Ignore
            }
            Token::StartTag(ref tag) if tag.name.as_str() == "html" => {
                self.process_in_body(token);
            }
            Token::StartTag(ref tag) if tag.name.as_str() == "head" => {
                let elem = self.create_element_for_token(&token);
                self.insert_element(elem);
                self.head_element = Some(elem);
                self.mode = InsertionMode::InHead;
            }
            Token::EndTag(ref tag) => {
                match tag.name.as_str() {
                    "head" | "body" | "html" | "br" => {
                        self.insert_head_element();
                        self.mode = InsertionMode::InHead;
                        self.process_token(token);
                    }
                    _ => {
                        // Ignore
                    }
                }
            }
            _ => {
                self.insert_head_element();
                self.mode = InsertionMode::InHead;
                self.process_token(token);
            }
        }
    }

    fn process_in_head(&mut self, token: Token) {
        match token {
            Token::Character(c) if c.is_whitespace() => {
                self.insert_character(c);
            }
            Token::Comment(text) => {
                self.insert_comment(text);
            }
            Token::Doctype(_) => {
                // Ignore
            }
            Token::StartTag(ref tag) => {
                match tag.name.as_str() {
                    "html" => {
                        self.process_in_body(token);
                    }
                    "base" | "basefont" | "bgsound" | "link" | "meta" => {
                        let elem = self.create_element_for_token(&token);
                        self.insert_element(elem);
                        self.open_elements.pop();
                    }
                    "title" => {
                        self.parse_generic_rcdata(&token);
                    }
                    "style" | "noscript" => {
                        self.parse_generic_rawtext(&token);
                    }
                    "script" => {
                        let elem = self.create_element_for_token(&token);
                        self.insert_element(elem);
                        self.original_mode = self.mode;
                        self.mode = InsertionMode::Text;
                    }
                    "head" => {
                        // Ignore
                    }
                    _ => {
                        self.pop_until("head");
                        self.mode = InsertionMode::AfterHead;
                        self.process_token(token);
                    }
                }
            }
            Token::EndTag(ref tag) => {
                match tag.name.as_str() {
                    "head" => {
                        self.open_elements.pop();
                        self.mode = InsertionMode::AfterHead;
                    }
                    "body" | "html" | "br" => {
                        self.open_elements.pop();
                        self.mode = InsertionMode::AfterHead;
                        self.process_token(token);
                    }
                    _ => {
                        // Ignore
                    }
                }
            }
            _ => {
                self.pop_until("head");
                self.mode = InsertionMode::AfterHead;
                self.process_token(token);
            }
        }
    }

    fn process_after_head(&mut self, token: Token) {
        match token {
            Token::Character(c) if c.is_whitespace() => {
                self.insert_character(c);
            }
            Token::Comment(text) => {
                self.insert_comment(text);
            }
            Token::Doctype(_) => {
                // Ignore
            }
            Token::StartTag(ref tag) => {
                match tag.name.as_str() {
                    "html" => {
                        self.process_in_body(token);
                    }
                    "body" => {
                        let elem = self.create_element_for_token(&token);
                        self.insert_element(elem);
                        self.frameset_ok = false;
                        self.mode = InsertionMode::InBody;
                    }
                    "frameset" => {
                        let elem = self.create_element_for_token(&token);
                        self.insert_element(elem);
                        self.mode = InsertionMode::InBody; // Simplified
                    }
                    "base" | "basefont" | "bgsound" | "link" | "meta" | "noframes" | "script"
                    | "style" | "template" | "title" => {
                        if let Some(head) = self.head_element {
                            self.open_elements.push(head);
                            self.process_in_head(token);
                            self.open_elements.retain(|&e| e != head);
                        }
                    }
                    "head" => {
                        // Ignore
                    }
                    _ => {
                        self.insert_body_element();
                        self.mode = InsertionMode::InBody;
                        self.process_token(token);
                    }
                }
            }
            Token::EndTag(ref tag) => {
                match tag.name.as_str() {
                    "template" => {
                        self.process_in_head(token);
                    }
                    "body" | "html" | "br" => {
                        self.insert_body_element();
                        self.mode = InsertionMode::InBody;
                        self.process_token(token);
                    }
                    _ => {
                        // Ignore
                    }
                }
            }
            _ => {
                self.insert_body_element();
                self.mode = InsertionMode::InBody;
                self.process_token(token);
            }
        }
    }

    fn process_in_body(&mut self, token: Token) {
        match token {
            Token::Character('\0') => {
                // Ignore null characters
            }
            Token::Character(c) => {
                self.reconstruct_active_formatting_elements();
                self.insert_character(c);
                if !c.is_whitespace() {
                    self.frameset_ok = false;
                }
            }
            Token::Comment(text) => {
                self.insert_comment(text);
            }
            Token::Doctype(_) => {
                // Ignore
            }
            Token::StartTag(ref tag) => {
                self.process_start_tag_in_body(tag.clone());
            }
            Token::EndTag(ref tag) => {
                self.process_end_tag_in_body(tag.clone());
            }
            Token::Eof => {
                // End
            }
        }
    }

    fn process_start_tag_in_body(&mut self, tag: TagToken) {
        match tag.name.as_str() {
            "html" => {
                // Merge attributes
            }
            "base" | "basefont" | "bgsound" | "link" | "meta" | "noframes" | "script" | "style"
            | "template" | "title" => {
                self.process_in_head(Token::StartTag(tag));
            }
            "body" => {
                // Merge attributes, ignore
            }
            "frameset" => {
                // Ignore in most cases
            }
            "address" | "article" | "aside" | "blockquote" | "center" | "details" | "dialog"
            | "dir" | "div" | "dl" | "fieldset" | "figcaption" | "figure" | "footer" | "header"
            | "hgroup" | "main" | "menu" | "nav" | "ol" | "p" | "section" | "summary" | "ul" => {
                self.close_p_element_if_in_scope();
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.close_p_element_if_in_scope();
                // Pop heading if on stack
                if let Some(&current) = self.open_elements.last() {
                    if let Some(name) = self.sink.element_name(current) {
                        if matches!(name.local.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
                            self.open_elements.pop();
                        }
                    }
                }
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
            }
            "pre" | "listing" => {
                self.close_p_element_if_in_scope();
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
                self.frameset_ok = false;
            }
            "form" => {
                if self.form_element.is_some() {
                    // Ignore
                } else {
                    self.close_p_element_if_in_scope();
                    let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                    self.insert_element(elem);
                    self.form_element = Some(elem);
                }
            }
            "li" => {
                self.frameset_ok = false;
                self.close_p_element_if_in_scope();
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
            }
            "dd" | "dt" => {
                self.frameset_ok = false;
                self.close_p_element_if_in_scope();
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
            }
            "plaintext" => {
                self.close_p_element_if_in_scope();
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
            }
            "button" => {
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
                self.frameset_ok = false;
            }
            "a" => {
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
            }
            "b" | "big" | "code" | "em" | "font" | "i" | "s" | "small" | "strike" | "strong"
            | "tt" | "u" => {
                self.reconstruct_active_formatting_elements();
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
            }
            "nobr" => {
                self.reconstruct_active_formatting_elements();
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
            }
            "area" | "br" | "embed" | "img" | "keygen" | "wbr" => {
                self.reconstruct_active_formatting_elements();
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
                self.open_elements.pop();
                self.frameset_ok = false;
            }
            "input" => {
                self.reconstruct_active_formatting_elements();
                let elem = self
                    .sink
                    .create_element(tag.qual_name(), tag.attributes.clone());
                self.insert_element(elem);
                self.open_elements.pop();
                if tag
                    .get_attribute("type")
                    .map(|t| t.eq_ignore_ascii_case("hidden"))
                    != Some(true)
                {
                    self.frameset_ok = false;
                }
            }
            "param" | "source" | "track" => {
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
                self.open_elements.pop();
            }
            "hr" => {
                self.close_p_element_if_in_scope();
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
                self.open_elements.pop();
                self.frameset_ok = false;
            }
            "image" => {
                // Treat as "img"
                let mut tag = tag;
                tag.name = LocalName::new("img");
                self.process_start_tag_in_body(tag);
            }
            "textarea" => {
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
                self.frameset_ok = false;
                self.original_mode = self.mode;
                self.mode = InsertionMode::Text;
            }
            "iframe" | "noembed" | "noframes" => {
                self.parse_generic_rawtext(&Token::StartTag(tag));
            }
            "select" => {
                self.reconstruct_active_formatting_elements();
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
                self.frameset_ok = false;
                self.mode = InsertionMode::InSelect;
            }
            "optgroup" | "option" => {
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
            }
            "table" => {
                self.close_p_element_if_in_scope();
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
                self.frameset_ok = false;
                self.mode = InsertionMode::InTable;
            }
            "caption" | "colgroup" | "col" | "tbody" | "td" | "tfoot" | "th" | "thead" | "tr" => {
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
            }
            "span" => {
                self.reconstruct_active_formatting_elements();
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
            }
            _ => {
                // Generic element
                self.reconstruct_active_formatting_elements();
                let elem = self.sink.create_element(tag.qual_name(), tag.attributes);
                self.insert_element(elem);
            }
        }
    }

    fn process_end_tag_in_body(&mut self, tag: TagToken) {
        match tag.name.as_str() {
            "template" => {
                self.process_in_head(Token::EndTag(tag));
            }
            "body" => {
                if self.has_element_in_scope("body") {
                    self.mode = InsertionMode::AfterBody;
                }
            }
            "html" => {
                if self.has_element_in_scope("body") {
                    self.mode = InsertionMode::AfterBody;
                    self.process_token(Token::EndTag(tag));
                }
            }
            "address" | "article" | "aside" | "blockquote" | "button" | "center" | "details"
            | "dialog" | "dir" | "div" | "dl" | "fieldset" | "figcaption" | "figure" | "footer"
            | "header" | "hgroup" | "listing" | "main" | "menu" | "nav" | "ol" | "pre"
            | "section" | "summary" | "ul" => {
                if self.has_element_in_scope(tag.name.as_str()) {
                    self.generate_implied_end_tags();
                    self.pop_until(tag.name.as_str());
                }
            }
            "form" => {
                self.form_element = None;
                if self.has_element_in_scope("form") {
                    self.generate_implied_end_tags();
                    self.pop_until("form");
                }
            }
            "p" => {
                if !self.has_element_in_button_scope("p") {
                    let elem = self
                        .sink
                        .create_element(QualName::html(LocalName::new("p")), Vec::new());
                    self.insert_element(elem);
                }
                self.close_p_element();
            }
            "li" => {
                if self.has_element_in_list_scope("li") {
                    self.generate_implied_end_tags_except("li");
                    self.pop_until("li");
                }
            }
            "dd" | "dt" => {
                if self.has_element_in_scope(tag.name.as_str()) {
                    self.generate_implied_end_tags_except(tag.name.as_str());
                    self.pop_until(tag.name.as_str());
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                if self.has_any_heading_in_scope() {
                    self.generate_implied_end_tags();
                    self.pop_until_heading();
                }
            }
            "a" | "b" | "big" | "code" | "em" | "font" | "i" | "nobr" | "s" | "small"
            | "strike" | "strong" | "tt" | "u" => {
                // Adoption agency algorithm (simplified)
                self.pop_until(tag.name.as_str());
            }
            "br" => {
                // Parse error, treat as <br>
                self.process_start_tag_in_body(TagToken::start("br"));
            }
            _ => {
                // Any other end tag
                self.pop_until(tag.name.as_str());
            }
        }
    }

    fn process_text(&mut self, token: Token) {
        match token {
            Token::Character(c) => {
                self.insert_character(c);
            }
            Token::Eof => {
                self.open_elements.pop();
                self.mode = self.original_mode;
            }
            Token::EndTag(_) => {
                self.open_elements.pop();
                self.mode = self.original_mode;
            }
            _ => {}
        }
    }

    fn process_after_body(&mut self, token: Token) {
        match token {
            Token::Character(c) if c.is_whitespace() => {
                self.process_in_body(token);
            }
            Token::Comment(text) => {
                // Append to html element
                if let Some(&html) = self.open_elements.first() {
                    let comment = self.sink.create_comment(text);
                    self.sink.append(html, comment);
                }
            }
            Token::Doctype(_) => {
                // Ignore
            }
            Token::StartTag(ref tag) if tag.name.as_str() == "html" => {
                self.process_in_body(token);
            }
            Token::EndTag(ref tag) if tag.name.as_str() == "html" => {
                self.mode = InsertionMode::AfterAfterBody;
            }
            Token::Eof => {
                // Stop parsing
            }
            _ => {
                self.mode = InsertionMode::InBody;
                self.process_token(token);
            }
        }
    }

    fn process_after_after_body(&mut self, token: Token) {
        match token {
            Token::Comment(text) => {
                let comment = self.sink.create_comment(text);
                let doc = self.sink.document();
                self.sink.append(doc, comment);
            }
            Token::Doctype(_) | Token::Character(_) => {
                self.process_in_body(token);
            }
            Token::StartTag(ref tag) if tag.name.as_str() == "html" => {
                self.process_in_body(token);
            }
            Token::Eof => {
                // Stop parsing
            }
            _ => {
                self.mode = InsertionMode::InBody;
                self.process_token(token);
            }
        }
    }

    // Helper methods

    fn create_element_for_token(&mut self, token: &Token) -> NodeId {
        match token {
            Token::StartTag(tag) | Token::EndTag(tag) => self
                .sink
                .create_element(tag.qual_name(), tag.attributes.clone()),
            _ => panic!("Expected tag token"),
        }
    }

    fn insert_element(&mut self, elem: NodeId) {
        if let Some(&parent) = self.open_elements.last() {
            self.sink.append(parent, elem);
        } else {
            let doc = self.sink.document();
            self.sink.append(doc, elem);
        }
        self.open_elements.push(elem);
    }

    fn insert_character(&mut self, c: char) {
        if let Some(&parent) = self.open_elements.last() {
            let mut s = String::new();
            s.push(c);
            self.sink.append_text(parent, &s);
        }
    }

    fn insert_comment(&mut self, text: String) {
        let comment = self.sink.create_comment(text);
        if let Some(&parent) = self.open_elements.last() {
            self.sink.append(parent, comment);
        }
    }

    fn insert_html_element(&mut self) {
        let elem = self
            .sink
            .create_element(QualName::html(LocalName::new("html")), Vec::new());
        let doc = self.sink.document();
        self.sink.append(doc, elem);
        self.open_elements.push(elem);
    }

    fn insert_head_element(&mut self) {
        let elem = self
            .sink
            .create_element(QualName::html(LocalName::new("head")), Vec::new());
        self.insert_element(elem);
        self.head_element = Some(elem);
    }

    fn insert_body_element(&mut self) {
        let elem = self
            .sink
            .create_element(QualName::html(LocalName::new("body")), Vec::new());
        self.insert_element(elem);
    }

    fn parse_generic_rcdata(&mut self, token: &Token) {
        let elem = self.create_element_for_token(token);
        self.insert_element(elem);
        self.original_mode = self.mode;
        self.mode = InsertionMode::Text;
    }

    fn parse_generic_rawtext(&mut self, token: &Token) {
        let elem = self.create_element_for_token(token);
        self.insert_element(elem);
        self.original_mode = self.mode;
        self.mode = InsertionMode::Text;
    }

    fn reconstruct_active_formatting_elements(&mut self) {
        // Simplified: no-op for now
    }

    fn close_p_element_if_in_scope(&mut self) {
        if self.has_element_in_button_scope("p") {
            self.close_p_element();
        }
    }

    fn close_p_element(&mut self) {
        self.generate_implied_end_tags_except("p");
        self.pop_until("p");
    }

    fn pop_until(&mut self, name: &str) {
        while let Some(&elem) = self.open_elements.last() {
            if let Some(elem_name) = self.sink.element_name(elem) {
                if elem_name.local.as_str() == name {
                    self.open_elements.pop();
                    break;
                }
            }
            self.open_elements.pop();
        }
    }

    fn pop_until_heading(&mut self) {
        while let Some(&elem) = self.open_elements.last() {
            if let Some(elem_name) = self.sink.element_name(elem) {
                if matches!(
                    elem_name.local.as_str(),
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                ) {
                    self.open_elements.pop();
                    break;
                }
            }
            self.open_elements.pop();
        }
    }

    fn generate_implied_end_tags(&mut self) {
        self.generate_implied_end_tags_except("");
    }

    fn generate_implied_end_tags_except(&mut self, except: &str) {
        loop {
            if let Some(&elem) = self.open_elements.last() {
                if let Some(name) = self.sink.element_name(elem) {
                    let tag_name = name.local.as_str();
                    if tag_name != except
                        && matches!(
                            tag_name,
                            "dd" | "dt"
                                | "li"
                                | "optgroup"
                                | "option"
                                | "p"
                                | "rb"
                                | "rp"
                                | "rt"
                                | "rtc"
                        )
                    {
                        self.open_elements.pop();
                        continue;
                    }
                }
            }
            break;
        }
    }

    fn has_element_in_scope(&self, name: &str) -> bool {
        self.has_element_in_scope_impl(
            name,
            &[
                "applet", "caption", "html", "table", "td", "th", "marquee", "object", "template",
            ],
        )
    }

    fn has_element_in_button_scope(&self, name: &str) -> bool {
        self.has_element_in_scope_impl(
            name,
            &[
                "applet", "caption", "html", "table", "td", "th", "marquee", "object", "template",
                "button",
            ],
        )
    }

    fn has_element_in_list_scope(&self, name: &str) -> bool {
        self.has_element_in_scope_impl(
            name,
            &[
                "applet", "caption", "html", "table", "td", "th", "marquee", "object", "template",
                "ol", "ul",
            ],
        )
    }

    fn has_element_in_scope_impl(&self, name: &str, scope: &[&str]) -> bool {
        for &elem in self.open_elements.iter().rev() {
            if let Some(elem_name) = self.sink.element_name(elem) {
                if elem_name.local.as_str() == name {
                    return true;
                }
                if scope.contains(&elem_name.local.as_str()) {
                    return false;
                }
            }
        }
        false
    }

    fn has_any_heading_in_scope(&self) -> bool {
        self.has_element_in_scope("h1")
            || self.has_element_in_scope("h2")
            || self.has_element_in_scope("h3")
            || self.has_element_in_scope("h4")
            || self.has_element_in_scope("h5")
            || self.has_element_in_scope("h6")
    }
}
