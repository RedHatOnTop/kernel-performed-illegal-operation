//! Servo-compatible fundamental types for KPIO OS
//!
//! This crate provides string interning and namespace types compatible with
//! Servo's `string_cache` and `markup5ever` crates, but implemented for
//! no_std environments.

#![no_std]
#![allow(dead_code)]

extern crate alloc;

pub mod atom;
pub mod namespace;
pub mod local_name;
pub mod prefix;
pub mod qualname;

pub use atom::Atom;
pub use namespace::Namespace;
pub use local_name::LocalName;
pub use prefix::Prefix;
pub use qualname::QualName;

/// Prelude for common imports
pub mod prelude {
    pub use crate::{Atom, Namespace, LocalName, Prefix, QualName};
    pub use crate::namespace::{ns, HTML_NAMESPACE, SVG_NAMESPACE, MATHML_NAMESPACE, XML_NAMESPACE, XMLNS_NAMESPACE, XLINK_NAMESPACE};
    pub use crate::local_name::local_name;
}
