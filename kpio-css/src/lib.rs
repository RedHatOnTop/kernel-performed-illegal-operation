//! KPIO CSS - CSS parsing and styling for KPIO OS
//!
//! This crate provides CSS parsing, selector matching, and style computation
//! for the KPIO browser engine. It's designed to work in no_std environments.

#![no_std]
#![allow(dead_code)]

extern crate alloc;

pub mod parser;
pub mod selector;
pub mod properties;
pub mod values;
pub mod stylesheet;
pub mod cascade;
pub mod computed;

pub use parser::{CssParser, ParseError};
pub use selector::{Selector, SelectorList, Specificity};
pub use properties::{PropertyId, PropertyDeclaration};
pub use values::{CssValue, Length, Color, Display};
pub use stylesheet::{Stylesheet, Rule, StyleRule};
pub use cascade::CascadedValues;
pub use computed::ComputedStyle;

/// Prelude for common imports
pub mod prelude {
    pub use crate::{
        CssParser, ParseError,
        Selector, SelectorList, Specificity,
        PropertyId, PropertyDeclaration,
        CssValue, Length, Color, Display,
        Stylesheet, Rule, StyleRule,
        CascadedValues, ComputedStyle,
    };
}
