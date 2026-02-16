//! KPIO CSS - CSS parsing and styling for KPIO OS
//!
//! This crate provides CSS parsing, selector matching, and style computation
//! for the KPIO browser engine. It's designed to work in no_std environments.

#![no_std]
#![allow(dead_code)]

extern crate alloc;

pub mod cascade;
pub mod computed;
pub mod parser;
pub mod properties;
pub mod selector;
pub mod stylesheet;
pub mod values;

#[cfg(test)]
mod tests;

pub use cascade::CascadedValues;
pub use computed::ComputedStyle;
pub use parser::{CssParser, ParseError};
pub use properties::{PropertyDeclaration, PropertyId};
pub use selector::{Selector, SelectorList, Specificity};
pub use stylesheet::{Rule, StyleRule, Stylesheet};
pub use values::{Color, CssValue, Display, Length};

/// Prelude for common imports
pub mod prelude {
    pub use crate::{
        CascadedValues, Color, ComputedStyle, CssParser, CssValue, Display, Length, ParseError,
        PropertyDeclaration, PropertyId, Rule, Selector, SelectorList, Specificity, StyleRule,
        Stylesheet,
    };
}
