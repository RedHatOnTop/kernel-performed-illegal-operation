//! KPIO HTML - HTML parsing for KPIO OS
//!
//! This crate provides HTML tokenization and parsing for the KPIO browser engine.
//! It implements a subset of the HTML5 parsing algorithm suitable for no_std environments.

#![no_std]
#![allow(dead_code)]

extern crate alloc;

pub mod parser;
pub mod tokenizer;
pub mod tree_builder;

#[cfg(test)]
mod tests;

pub use parser::{HtmlParser, ParseError};
pub use tokenizer::{Token, Tokenizer};
pub use tree_builder::{NodeId, TreeBuilder, TreeSink};

/// Prelude for common imports
pub mod prelude {
    pub use crate::tokenizer::{Attribute, TagKind, TagToken};
    pub use crate::{HtmlParser, NodeId, ParseError, Token, Tokenizer, TreeBuilder, TreeSink};
}
