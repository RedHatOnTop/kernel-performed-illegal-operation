//! KPIO HTML - HTML parsing for KPIO OS
//!
//! This crate provides HTML tokenization and parsing for the KPIO browser engine.
//! It implements a subset of the HTML5 parsing algorithm suitable for no_std environments.

#![no_std]
#![allow(dead_code)]

extern crate alloc;

pub mod tokenizer;
pub mod tree_builder;
pub mod parser;

#[cfg(test)]
mod tests;

pub use tokenizer::{Tokenizer, Token};
pub use tree_builder::{TreeBuilder, TreeSink, NodeId};
pub use parser::{HtmlParser, ParseError};

/// Prelude for common imports
pub mod prelude {
    pub use crate::{Tokenizer, Token, TreeBuilder, TreeSink, NodeId, HtmlParser, ParseError};
    pub use crate::tokenizer::{TagToken, Attribute, TagKind};
}
