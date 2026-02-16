//! HTML Tokenizer - Converts HTML source into tokens
//!
//! Implements a simplified HTML5 tokenizer.

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::iter::Peekable;
use core::str::Chars;

use servo_types::namespace::HTML_NAMESPACE;
use servo_types::{LocalName, Namespace, QualName};

/// An HTML token.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// DOCTYPE token
    Doctype(DoctypeToken),
    /// Start tag
    StartTag(TagToken),
    /// End tag
    EndTag(TagToken),
    /// Character data
    Character(char),
    /// Comment
    Comment(String),
    /// End of file
    Eof,
}

/// DOCTYPE token data.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DoctypeToken {
    pub name: Option<String>,
    pub public_id: Option<String>,
    pub system_id: Option<String>,
    pub force_quirks: bool,
}

/// Tag token data (for start and end tags).
#[derive(Debug, Clone, PartialEq)]
pub struct TagToken {
    pub kind: TagKind,
    pub name: LocalName,
    pub self_closing: bool,
    pub attributes: Vec<Attribute>,
}

impl TagToken {
    /// Create a new start tag.
    pub fn start(name: &str) -> Self {
        TagToken {
            kind: TagKind::Start,
            name: LocalName::new(name),
            self_closing: false,
            attributes: Vec::new(),
        }
    }

    /// Create a new end tag.
    pub fn end(name: &str) -> Self {
        TagToken {
            kind: TagKind::End,
            name: LocalName::new(name),
            self_closing: false,
            attributes: Vec::new(),
        }
    }

    /// Get the qualified name for this tag.
    pub fn qual_name(&self) -> QualName {
        QualName::html(self.name.clone())
    }

    /// Get an attribute by name.
    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|a| a.name.as_str() == name)
            .map(|a| a.value.as_str())
    }

    /// Check if this is a void element (self-closing by default).
    pub fn is_void_element(&self) -> bool {
        matches!(
            self.name.as_str(),
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
                | "param"
                | "source"
                | "track"
                | "wbr"
        )
    }
}

/// Tag kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagKind {
    Start,
    End,
}

/// An HTML attribute.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: LocalName,
    pub value: String,
}

impl Attribute {
    /// Create a new attribute.
    pub fn new(name: &str, value: &str) -> Self {
        Attribute {
            name: LocalName::new(name),
            value: value.to_string(),
        }
    }
}

/// HTML tokenizer state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Data,
    TagOpen,
    EndTagOpen,
    TagName,
    BeforeAttributeName,
    AttributeName,
    AfterAttributeName,
    BeforeAttributeValue,
    AttributeValueDoubleQuoted,
    AttributeValueSingleQuoted,
    AttributeValueUnquoted,
    AfterAttributeValueQuoted,
    SelfClosingStartTag,
    BogusComment,
    MarkupDeclarationOpen,
    CommentStart,
    CommentStartDash,
    Comment,
    CommentEndDash,
    CommentEnd,
    Doctype,
    BeforeDoctypeName,
    DoctypeName,
    AfterDoctypeName,
    CharacterReferenceInData,
    Rawtext,
    RawtextLessThanSign,
    RawtextEndTagOpen,
    RawtextEndTagName,
    ScriptData,
    ScriptDataLessThanSign,
    ScriptDataEndTagOpen,
    ScriptDataEndTagName,
}

/// HTML tokenizer.
pub struct Tokenizer<'a> {
    input: &'a str,
    pos: usize,
    state: State,
    return_state: State,
    current_tag: Option<TagToken>,
    current_attribute: Option<Attribute>,
    current_doctype: DoctypeToken,
    current_comment: String,
    temp_buffer: String,
    last_start_tag_name: Option<String>,
}

impl<'a> Tokenizer<'a> {
    /// Create a new tokenizer for the given input.
    pub fn new(input: &'a str) -> Self {
        Tokenizer {
            input,
            pos: 0,
            state: State::Data,
            return_state: State::Data,
            current_tag: None,
            current_attribute: None,
            current_doctype: DoctypeToken::default(),
            current_comment: String::new(),
            temp_buffer: String::new(),
            last_start_tag_name: None,
        }
    }

    /// Get the next token.
    pub fn next_token(&mut self) -> Token {
        loop {
            match self.state {
                State::Data => match self.consume_char() {
                    Some('<') => self.state = State::TagOpen,
                    Some('&') => {
                        self.return_state = State::Data;
                        self.state = State::CharacterReferenceInData;
                    }
                    Some(c) => return Token::Character(c),
                    None => return Token::Eof,
                },

                State::TagOpen => match self.peek_char() {
                    Some('!') => {
                        self.consume_char();
                        self.state = State::MarkupDeclarationOpen;
                    }
                    Some('/') => {
                        self.consume_char();
                        self.state = State::EndTagOpen;
                    }
                    Some(c) if c.is_ascii_alphabetic() => {
                        self.current_tag = Some(TagToken {
                            kind: TagKind::Start,
                            name: LocalName::new(""),
                            self_closing: false,
                            attributes: Vec::new(),
                        });
                        self.state = State::TagName;
                    }
                    Some('?') => {
                        self.current_comment = String::new();
                        self.state = State::BogusComment;
                    }
                    _ => {
                        self.state = State::Data;
                        return Token::Character('<');
                    }
                },

                State::EndTagOpen => match self.peek_char() {
                    Some(c) if c.is_ascii_alphabetic() => {
                        self.current_tag = Some(TagToken {
                            kind: TagKind::End,
                            name: LocalName::new(""),
                            self_closing: false,
                            attributes: Vec::new(),
                        });
                        self.state = State::TagName;
                    }
                    Some('>') => {
                        self.consume_char();
                        self.state = State::Data;
                    }
                    None => {
                        self.state = State::Data;
                        return Token::Character('<');
                    }
                    _ => {
                        self.current_comment = String::new();
                        self.state = State::BogusComment;
                    }
                },

                State::TagName => {
                    match self.consume_char() {
                        Some(c) if c.is_whitespace() => {
                            self.state = State::BeforeAttributeName;
                        }
                        Some('/') => {
                            self.state = State::SelfClosingStartTag;
                        }
                        Some('>') => {
                            self.state = State::Data;
                            let tag = self.current_tag.take().unwrap();
                            if tag.kind == TagKind::Start {
                                self.last_start_tag_name = Some(tag.name.to_string());

                                // Switch to rawtext/script state for special elements
                                match tag.name.as_str() {
                                    "script" => self.state = State::ScriptData,
                                    "style" | "textarea" | "title" => self.state = State::Rawtext,
                                    _ => {}
                                }
                                return Token::StartTag(tag);
                            } else {
                                return Token::EndTag(tag);
                            }
                        }
                        Some(c) => {
                            if let Some(ref mut tag) = self.current_tag {
                                let mut name = tag.name.to_string();
                                name.push(c.to_ascii_lowercase());
                                tag.name = LocalName::new(&name);
                            }
                        }
                        None => return Token::Eof,
                    }
                }

                State::BeforeAttributeName => {
                    match self.peek_char() {
                        Some(c) if c.is_whitespace() => {
                            self.consume_char();
                        }
                        Some('/') | Some('>') | None => {
                            self.state = State::AfterAttributeName;
                        }
                        Some('=') => {
                            // Start a new attribute with empty name
                            self.current_attribute = Some(Attribute::new("", ""));
                            self.state = State::AttributeName;
                        }
                        _ => {
                            self.current_attribute = Some(Attribute::new("", ""));
                            self.state = State::AttributeName;
                        }
                    }
                }

                State::AttributeName => match self.peek_char() {
                    Some(c) if c.is_whitespace() => {
                        self.consume_char();
                        self.finish_attribute();
                        self.state = State::AfterAttributeName;
                    }
                    Some('/') => {
                        self.finish_attribute();
                        self.state = State::SelfClosingStartTag;
                    }
                    Some('=') => {
                        self.consume_char();
                        self.state = State::BeforeAttributeValue;
                    }
                    Some('>') => {
                        self.finish_attribute();
                        self.state = State::AfterAttributeName;
                    }
                    Some(c) => {
                        self.consume_char();
                        if let Some(ref mut attr) = self.current_attribute {
                            let mut name = attr.name.to_string();
                            name.push(c.to_ascii_lowercase());
                            attr.name = LocalName::new(&name);
                        }
                    }
                    None => return Token::Eof,
                },

                State::AfterAttributeName => match self.peek_char() {
                    Some(c) if c.is_whitespace() => {
                        self.consume_char();
                    }
                    Some('/') => {
                        self.consume_char();
                        self.state = State::SelfClosingStartTag;
                    }
                    Some('=') => {
                        self.consume_char();
                        self.state = State::BeforeAttributeValue;
                    }
                    Some('>') => {
                        self.consume_char();
                        self.state = State::Data;
                        let tag = self.current_tag.take().unwrap();
                        if tag.kind == TagKind::Start {
                            self.last_start_tag_name = Some(tag.name.to_string());
                            match tag.name.as_str() {
                                "script" => self.state = State::ScriptData,
                                "style" | "textarea" | "title" => self.state = State::Rawtext,
                                _ => {}
                            }
                            return Token::StartTag(tag);
                        } else {
                            return Token::EndTag(tag);
                        }
                    }
                    _ => {
                        self.current_attribute = Some(Attribute::new("", ""));
                        self.state = State::AttributeName;
                    }
                },

                State::BeforeAttributeValue => match self.peek_char() {
                    Some(c) if c.is_whitespace() => {
                        self.consume_char();
                    }
                    Some('"') => {
                        self.consume_char();
                        self.state = State::AttributeValueDoubleQuoted;
                    }
                    Some('\'') => {
                        self.consume_char();
                        self.state = State::AttributeValueSingleQuoted;
                    }
                    Some('>') => {
                        self.finish_attribute();
                        self.state = State::AfterAttributeName;
                    }
                    _ => {
                        self.state = State::AttributeValueUnquoted;
                    }
                },

                State::AttributeValueDoubleQuoted => match self.consume_char() {
                    Some('"') => {
                        self.finish_attribute();
                        self.state = State::AfterAttributeValueQuoted;
                    }
                    Some(c) => {
                        if let Some(ref mut attr) = self.current_attribute {
                            attr.value.push(c);
                        }
                    }
                    None => return Token::Eof,
                },

                State::AttributeValueSingleQuoted => match self.consume_char() {
                    Some('\'') => {
                        self.finish_attribute();
                        self.state = State::AfterAttributeValueQuoted;
                    }
                    Some(c) => {
                        if let Some(ref mut attr) = self.current_attribute {
                            attr.value.push(c);
                        }
                    }
                    None => return Token::Eof,
                },

                State::AttributeValueUnquoted => match self.peek_char() {
                    Some(c) if c.is_whitespace() => {
                        self.consume_char();
                        self.finish_attribute();
                        self.state = State::BeforeAttributeName;
                    }
                    Some('>') => {
                        self.finish_attribute();
                        self.state = State::AfterAttributeName;
                    }
                    Some(c) => {
                        self.consume_char();
                        if let Some(ref mut attr) = self.current_attribute {
                            attr.value.push(c);
                        }
                    }
                    None => return Token::Eof,
                },

                State::AfterAttributeValueQuoted => match self.peek_char() {
                    Some(c) if c.is_whitespace() => {
                        self.consume_char();
                        self.state = State::BeforeAttributeName;
                    }
                    Some('/') => {
                        self.consume_char();
                        self.state = State::SelfClosingStartTag;
                    }
                    Some('>') => {
                        self.consume_char();
                        self.state = State::Data;
                        let tag = self.current_tag.take().unwrap();
                        if tag.kind == TagKind::Start {
                            self.last_start_tag_name = Some(tag.name.to_string());
                            match tag.name.as_str() {
                                "script" => self.state = State::ScriptData,
                                "style" | "textarea" | "title" => self.state = State::Rawtext,
                                _ => {}
                            }
                            return Token::StartTag(tag);
                        } else {
                            return Token::EndTag(tag);
                        }
                    }
                    _ => {
                        self.state = State::BeforeAttributeName;
                    }
                },

                State::SelfClosingStartTag => match self.consume_char() {
                    Some('>') => {
                        self.state = State::Data;
                        if let Some(mut tag) = self.current_tag.take() {
                            tag.self_closing = true;
                            if tag.kind == TagKind::Start {
                                return Token::StartTag(tag);
                            } else {
                                return Token::EndTag(tag);
                            }
                        }
                    }
                    _ => {
                        self.state = State::BeforeAttributeName;
                    }
                },

                State::MarkupDeclarationOpen => {
                    if self.starts_with("--") {
                        self.consume_chars(2);
                        self.current_comment = String::new();
                        self.state = State::CommentStart;
                    } else if self.starts_with_ignore_case("DOCTYPE") {
                        self.consume_chars(7);
                        self.state = State::Doctype;
                    } else {
                        self.current_comment = String::new();
                        self.state = State::BogusComment;
                    }
                }

                State::CommentStart => match self.peek_char() {
                    Some('-') => {
                        self.consume_char();
                        self.state = State::CommentStartDash;
                    }
                    Some('>') => {
                        self.consume_char();
                        self.state = State::Data;
                        let comment = core::mem::take(&mut self.current_comment);
                        return Token::Comment(comment);
                    }
                    _ => {
                        self.state = State::Comment;
                    }
                },

                State::CommentStartDash => match self.peek_char() {
                    Some('-') => {
                        self.consume_char();
                        self.state = State::CommentEnd;
                    }
                    Some('>') => {
                        self.consume_char();
                        self.state = State::Data;
                        let comment = core::mem::take(&mut self.current_comment);
                        return Token::Comment(comment);
                    }
                    None => {
                        self.state = State::Data;
                        let comment = core::mem::take(&mut self.current_comment);
                        return Token::Comment(comment);
                    }
                    _ => {
                        self.current_comment.push('-');
                        self.state = State::Comment;
                    }
                },

                State::Comment => match self.consume_char() {
                    Some('-') => {
                        self.state = State::CommentEndDash;
                    }
                    Some(c) => {
                        self.current_comment.push(c);
                    }
                    None => {
                        self.state = State::Data;
                        let comment = core::mem::take(&mut self.current_comment);
                        return Token::Comment(comment);
                    }
                },

                State::CommentEndDash => match self.peek_char() {
                    Some('-') => {
                        self.consume_char();
                        self.state = State::CommentEnd;
                    }
                    None => {
                        self.state = State::Data;
                        let comment = core::mem::take(&mut self.current_comment);
                        return Token::Comment(comment);
                    }
                    _ => {
                        self.current_comment.push('-');
                        self.state = State::Comment;
                    }
                },

                State::CommentEnd => match self.peek_char() {
                    Some('>') => {
                        self.consume_char();
                        self.state = State::Data;
                        let comment = core::mem::take(&mut self.current_comment);
                        return Token::Comment(comment);
                    }
                    Some('-') => {
                        self.consume_char();
                        self.current_comment.push('-');
                    }
                    None => {
                        self.state = State::Data;
                        let comment = core::mem::take(&mut self.current_comment);
                        return Token::Comment(comment);
                    }
                    _ => {
                        self.current_comment.push_str("--");
                        self.state = State::Comment;
                    }
                },

                State::Doctype => match self.peek_char() {
                    Some(c) if c.is_whitespace() => {
                        self.consume_char();
                        self.state = State::BeforeDoctypeName;
                    }
                    Some('>') => {
                        self.state = State::BeforeDoctypeName;
                    }
                    None => {
                        self.current_doctype.force_quirks = true;
                        self.state = State::Data;
                        return Token::Doctype(core::mem::take(&mut self.current_doctype));
                    }
                    _ => {
                        self.state = State::BeforeDoctypeName;
                    }
                },

                State::BeforeDoctypeName => match self.peek_char() {
                    Some(c) if c.is_whitespace() => {
                        self.consume_char();
                    }
                    Some('>') => {
                        self.consume_char();
                        self.current_doctype.force_quirks = true;
                        self.state = State::Data;
                        return Token::Doctype(core::mem::take(&mut self.current_doctype));
                    }
                    Some(c) => {
                        self.consume_char();
                        self.current_doctype.name = Some(c.to_ascii_lowercase().to_string());
                        self.state = State::DoctypeName;
                    }
                    None => {
                        self.current_doctype.force_quirks = true;
                        self.state = State::Data;
                        return Token::Doctype(core::mem::take(&mut self.current_doctype));
                    }
                },

                State::DoctypeName => match self.peek_char() {
                    Some(c) if c.is_whitespace() => {
                        self.consume_char();
                        self.state = State::AfterDoctypeName;
                    }
                    Some('>') => {
                        self.consume_char();
                        self.state = State::Data;
                        return Token::Doctype(core::mem::take(&mut self.current_doctype));
                    }
                    Some(c) => {
                        self.consume_char();
                        if let Some(ref mut name) = self.current_doctype.name {
                            name.push(c.to_ascii_lowercase());
                        }
                    }
                    None => {
                        self.current_doctype.force_quirks = true;
                        self.state = State::Data;
                        return Token::Doctype(core::mem::take(&mut self.current_doctype));
                    }
                },

                State::AfterDoctypeName => {
                    match self.peek_char() {
                        Some(c) if c.is_whitespace() => {
                            self.consume_char();
                        }
                        Some('>') => {
                            self.consume_char();
                            self.state = State::Data;
                            return Token::Doctype(core::mem::take(&mut self.current_doctype));
                        }
                        None => {
                            self.current_doctype.force_quirks = true;
                            self.state = State::Data;
                            return Token::Doctype(core::mem::take(&mut self.current_doctype));
                        }
                        _ => {
                            // Simplified: skip PUBLIC/SYSTEM identifiers
                            while let Some(c) = self.peek_char() {
                                if c == '>' {
                                    break;
                                }
                                self.consume_char();
                            }
                        }
                    }
                }

                State::CharacterReferenceInData => {
                    // Simplified: just emit the &
                    self.state = self.return_state;
                    return Token::Character('&');
                }

                State::BogusComment => match self.consume_char() {
                    Some('>') => {
                        self.state = State::Data;
                        let comment = core::mem::take(&mut self.current_comment);
                        return Token::Comment(comment);
                    }
                    Some(c) => {
                        self.current_comment.push(c);
                    }
                    None => {
                        self.state = State::Data;
                        let comment = core::mem::take(&mut self.current_comment);
                        return Token::Comment(comment);
                    }
                },

                State::Rawtext => match self.consume_char() {
                    Some('<') => {
                        self.state = State::RawtextLessThanSign;
                    }
                    Some(c) => return Token::Character(c),
                    None => return Token::Eof,
                },

                State::RawtextLessThanSign => match self.peek_char() {
                    Some('/') => {
                        self.consume_char();
                        self.temp_buffer.clear();
                        self.state = State::RawtextEndTagOpen;
                    }
                    _ => {
                        self.state = State::Rawtext;
                        return Token::Character('<');
                    }
                },

                State::RawtextEndTagOpen => match self.peek_char() {
                    Some(c) if c.is_ascii_alphabetic() => {
                        self.current_tag = Some(TagToken {
                            kind: TagKind::End,
                            name: LocalName::new(""),
                            self_closing: false,
                            attributes: Vec::new(),
                        });
                        self.state = State::RawtextEndTagName;
                    }
                    _ => {
                        self.state = State::Rawtext;
                        return Token::Character('<');
                    }
                },

                State::RawtextEndTagName => match self.peek_char() {
                    Some(c) if c.is_whitespace() => {
                        if self.is_appropriate_end_tag() {
                            self.consume_char();
                            self.state = State::BeforeAttributeName;
                        } else {
                            self.state = State::Rawtext;
                        }
                    }
                    Some('/') => {
                        if self.is_appropriate_end_tag() {
                            self.consume_char();
                            self.state = State::SelfClosingStartTag;
                        } else {
                            self.state = State::Rawtext;
                        }
                    }
                    Some('>') => {
                        if self.is_appropriate_end_tag() {
                            self.consume_char();
                            self.state = State::Data;
                            return Token::EndTag(self.current_tag.take().unwrap());
                        } else {
                            self.state = State::Rawtext;
                        }
                    }
                    Some(c) if c.is_ascii_alphabetic() => {
                        self.consume_char();
                        if let Some(ref mut tag) = self.current_tag {
                            let mut name = tag.name.to_string();
                            name.push(c.to_ascii_lowercase());
                            tag.name = LocalName::new(&name);
                        }
                        self.temp_buffer.push(c);
                    }
                    _ => {
                        self.state = State::Rawtext;
                    }
                },

                State::ScriptData => match self.consume_char() {
                    Some('<') => {
                        self.state = State::ScriptDataLessThanSign;
                    }
                    Some(c) => return Token::Character(c),
                    None => return Token::Eof,
                },

                State::ScriptDataLessThanSign => match self.peek_char() {
                    Some('/') => {
                        self.consume_char();
                        self.temp_buffer.clear();
                        self.state = State::ScriptDataEndTagOpen;
                    }
                    _ => {
                        self.state = State::ScriptData;
                        return Token::Character('<');
                    }
                },

                State::ScriptDataEndTagOpen => match self.peek_char() {
                    Some(c) if c.is_ascii_alphabetic() => {
                        self.current_tag = Some(TagToken {
                            kind: TagKind::End,
                            name: LocalName::new(""),
                            self_closing: false,
                            attributes: Vec::new(),
                        });
                        self.state = State::ScriptDataEndTagName;
                    }
                    _ => {
                        self.state = State::ScriptData;
                        return Token::Character('<');
                    }
                },

                State::ScriptDataEndTagName => match self.peek_char() {
                    Some(c) if c.is_whitespace() => {
                        if self.is_appropriate_end_tag() {
                            self.consume_char();
                            self.state = State::BeforeAttributeName;
                        } else {
                            self.state = State::ScriptData;
                        }
                    }
                    Some('/') => {
                        if self.is_appropriate_end_tag() {
                            self.consume_char();
                            self.state = State::SelfClosingStartTag;
                        } else {
                            self.state = State::ScriptData;
                        }
                    }
                    Some('>') => {
                        if self.is_appropriate_end_tag() {
                            self.consume_char();
                            self.state = State::Data;
                            return Token::EndTag(self.current_tag.take().unwrap());
                        } else {
                            self.state = State::ScriptData;
                        }
                    }
                    Some(c) if c.is_ascii_alphabetic() => {
                        self.consume_char();
                        if let Some(ref mut tag) = self.current_tag {
                            let mut name = tag.name.to_string();
                            name.push(c.to_ascii_lowercase());
                            tag.name = LocalName::new(&name);
                        }
                        self.temp_buffer.push(c);
                    }
                    _ => {
                        self.state = State::ScriptData;
                    }
                },
            }
        }
    }

    // Helper methods

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn consume_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn consume_chars(&mut self, n: usize) {
        for _ in 0..n {
            self.consume_char();
        }
    }

    fn starts_with(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn starts_with_ignore_case(&self, s: &str) -> bool {
        let remaining = &self.input[self.pos..];
        if remaining.len() < s.len() {
            return false;
        }
        remaining[..s.len()].eq_ignore_ascii_case(s)
    }

    fn finish_attribute(&mut self) {
        if let Some(attr) = self.current_attribute.take() {
            if let Some(ref mut tag) = self.current_tag {
                // Don't add duplicate attributes
                if !tag.attributes.iter().any(|a| a.name == attr.name) {
                    tag.attributes.push(attr);
                }
            }
        }
    }

    fn is_appropriate_end_tag(&self) -> bool {
        if let (Some(ref tag), Some(ref last)) = (&self.current_tag, &self.last_start_tag_name) {
            tag.name.as_str() == last.as_str()
        } else {
            false
        }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.next_token();
        if token == Token::Eof {
            None
        } else {
            Some(token)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tag() {
        let mut tok = Tokenizer::new("<div>");
        assert!(matches!(tok.next_token(), Token::StartTag(t) if t.name.as_str() == "div"));
    }

    #[test]
    fn test_tag_with_attribute() {
        let mut tok = Tokenizer::new("<div class=\"foo\">");
        if let Token::StartTag(tag) = tok.next_token() {
            assert_eq!(tag.name.as_str(), "div");
            assert_eq!(tag.get_attribute("class"), Some("foo"));
        } else {
            panic!("Expected start tag");
        }
    }
}
