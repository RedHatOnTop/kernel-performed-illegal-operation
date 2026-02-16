//! Namespace - XML/HTML namespace handling
//!
//! Compatible with Servo's namespace handling in markup5ever.

use crate::Atom;
use core::fmt;
use core::hash::{Hash, Hasher};

/// An XML namespace.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Namespace(pub Atom);

impl Namespace {
    /// Create a new namespace from a string.
    #[inline]
    pub fn new(s: &str) -> Self {
        Namespace(Atom::new(s))
    }

    /// Get the namespace as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Check if this is the empty/null namespace.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Default for Namespace {
    fn default() -> Self {
        Namespace(Atom::empty())
    }
}

impl fmt::Debug for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Namespace({:?})", self.0)
    }
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Namespace {
    fn from(s: &str) -> Self {
        Namespace::new(s)
    }
}

impl From<Atom> for Namespace {
    fn from(atom: Atom) -> Self {
        Namespace(atom)
    }
}

// ============================================================================
// Well-known namespaces
// ============================================================================

/// The HTML namespace: `http://www.w3.org/1999/xhtml`
pub const HTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";

/// The SVG namespace: `http://www.w3.org/2000/svg`
pub const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

/// The MathML namespace: `http://www.w3.org/1998/Math/MathML`
pub const MATHML_NAMESPACE: &str = "http://www.w3.org/1998/Math/MathML";

/// The XML namespace: `http://www.w3.org/XML/1998/namespace`
pub const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

/// The XMLNS namespace: `http://www.w3.org/2000/xmlns/`
pub const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";

/// The XLink namespace: `http://www.w3.org/1999/xlink`
pub const XLINK_NAMESPACE: &str = "http://www.w3.org/1999/xlink";

/// Create a namespace from a well-known constant or custom string.
#[inline]
pub fn ns(s: &str) -> Namespace {
    Namespace::new(s)
}

/// Well-known namespace constants as Namespace objects.
pub mod known {
    use super::*;

    lazy_static_namespace!(HTML, HTML_NAMESPACE);
    lazy_static_namespace!(SVG, SVG_NAMESPACE);
    lazy_static_namespace!(MATHML, MATHML_NAMESPACE);
    lazy_static_namespace!(XML, XML_NAMESPACE);
    lazy_static_namespace!(XMLNS, XMLNS_NAMESPACE);
    lazy_static_namespace!(XLINK, XLINK_NAMESPACE);

    /// The empty/null namespace.
    #[inline]
    pub fn empty() -> Namespace {
        Namespace::default()
    }
}

/// Helper macro for creating namespace constants.
/// In no_std, we create them on demand rather than as true statics.
macro_rules! lazy_static_namespace {
    ($name:ident, $value:expr) => {
        #[inline]
        pub fn $name() -> Namespace {
            Namespace::new($value)
        }
    };
}
use lazy_static_namespace;

/// Macro for namespace literals.
#[macro_export]
macro_rules! ns {
    () => {
        $crate::Namespace::default()
    };
    (html) => {
        $crate::namespace::ns($crate::namespace::HTML_NAMESPACE)
    };
    (svg) => {
        $crate::namespace::ns($crate::namespace::SVG_NAMESPACE)
    };
    (mathml) => {
        $crate::namespace::ns($crate::namespace::MATHML_NAMESPACE)
    };
    (xml) => {
        $crate::namespace::ns($crate::namespace::XML_NAMESPACE)
    };
    (xmlns) => {
        $crate::namespace::ns($crate::namespace::XMLNS_NAMESPACE)
    };
    (xlink) => {
        $crate::namespace::ns($crate::namespace::XLINK_NAMESPACE)
    };
    ($s:expr) => {
        $crate::namespace::ns($s)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_equality() {
        let ns1 = Namespace::new(HTML_NAMESPACE);
        let ns2 = known::HTML();
        assert_eq!(ns1, ns2);
    }

    #[test]
    fn test_empty_namespace() {
        let ns = Namespace::default();
        assert!(ns.is_empty());
    }
}
