//! Servo-compatible fundamental types for KPIO OS
//!
//! This crate provides string interning and namespace types compatible with
//! Servo's `string_cache` and `markup5ever` crates, but implemented for
//! no_std environments.

#![no_std]

extern crate alloc;

pub mod atom;
pub mod local_name;
pub mod namespace;
pub mod prefix;
pub mod qualname;

pub use atom::Atom;
pub use local_name::LocalName;
pub use namespace::Namespace;
pub use prefix::Prefix;
pub use qualname::QualName;

/// Prelude for common imports
pub mod prelude {
    pub use crate::local_name::local_name;
    pub use crate::namespace::{
        ns, HTML_NAMESPACE, MATHML_NAMESPACE, SVG_NAMESPACE, XLINK_NAMESPACE, XMLNS_NAMESPACE,
        XML_NAMESPACE,
    };
    pub use crate::{Atom, LocalName, Namespace, Prefix, QualName};
}
