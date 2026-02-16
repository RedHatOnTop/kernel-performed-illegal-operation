//! WIT (WebAssembly Interface Types) support.
//!
//! Provides a minimal WIT parser and type system for the KPIO
//! Component Model integration.  This module handles:
//!
//! * Parsing `.wit` text definitions into an AST
//! * Representing WIT types (records, enums, variants, flags, etc.)
//! * Resolving interface/world declarations
//!
//! The implementation targets the WASI Preview 2 component model subset
//! that KPIO applications require.

pub mod parser;
pub mod types;

pub use parser::parse_wit;
pub use types::*;
