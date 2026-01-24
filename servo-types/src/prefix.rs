//! Prefix - Namespace prefix handling
//!
//! Compatible with Servo's Prefix in markup5ever.

use crate::Atom;
use core::fmt;
use core::hash::Hash;
use core::ops::Deref;

/// A namespace prefix (the part before the colon in `prefix:localname`).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Prefix(pub Atom);

impl Prefix {
    /// Create a new prefix from a string.
    #[inline]
    pub fn new(s: &str) -> Self {
        Prefix(Atom::new(s))
    }

    /// Get the prefix as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Check if the prefix is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get the length.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl Default for Prefix {
    fn default() -> Self {
        Prefix(Atom::empty())
    }
}

impl Deref for Prefix {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for Prefix {
    #[inline]
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "Prefix(none)")
        } else {
            write!(f, "Prefix({:?})", self.0)
        }
    }
}

impl fmt::Display for Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Prefix {
    fn from(s: &str) -> Self {
        Prefix::new(s)
    }
}

impl From<Atom> for Prefix {
    fn from(atom: Atom) -> Self {
        Prefix(atom)
    }
}

impl PartialEq<str> for Prefix {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

/// Create a prefix.
#[inline]
pub fn prefix(s: &str) -> Prefix {
    Prefix::new(s)
}

/// Well-known prefixes.
pub mod known {
    use super::Prefix;

    /// The `xml` prefix.
    #[inline]
    pub fn xml() -> Prefix {
        Prefix::new("xml")
    }

    /// The `xmlns` prefix.
    #[inline]
    pub fn xmlns() -> Prefix {
        Prefix::new("xmlns")
    }

    /// The `xlink` prefix.
    #[inline]
    pub fn xlink() -> Prefix {
        Prefix::new("xlink")
    }

    /// The `svg` prefix (commonly used for SVG in HTML).
    #[inline]
    pub fn svg() -> Prefix {
        Prefix::new("svg")
    }

    /// The `math` prefix (for MathML).
    #[inline]
    pub fn math() -> Prefix {
        Prefix::new("math")
    }
}

/// Macro for prefix literals.
#[macro_export]
macro_rules! prefix {
    () => {
        $crate::Prefix::default()
    };
    ($s:expr) => {
        $crate::Prefix::new($s)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_equality() {
        let p1 = Prefix::new("xml");
        let p2 = known::xml();
        assert_eq!(p1, p2);
    }

    #[test]
    fn test_empty_prefix() {
        let p = Prefix::default();
        assert!(p.is_empty());
    }
}
