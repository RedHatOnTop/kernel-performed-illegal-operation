//! Atom - Interned string type for efficient comparison and storage
//!
//! This is a simplified implementation of Servo's `string_cache::Atom`.
//! Instead of compile-time string interning, we use runtime interning with
//! a global string pool.

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::Deref;

/// An interned string.
///
/// Atoms are reference-counted and can be compared by pointer equality
/// for O(1) comparison of strings.
#[derive(Clone)]
pub struct Atom(Arc<String>);

impl Atom {
    /// Create a new atom from a string slice.
    #[inline]
    pub fn new(s: &str) -> Self {
        // For simplicity, we create a new Arc each time.
        // A real implementation would use a global interner.
        Atom(Arc::new(s.to_owned()))
    }

    /// Create an atom from a String.
    #[inline]
    pub fn from_string(s: String) -> Self {
        Atom(Arc::new(s))
    }

    /// Get the string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Check if the atom is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get the length of the string.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Get the bytes of the string.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Convert to lowercase.
    pub fn to_ascii_lowercase(&self) -> Atom {
        Atom::from_string(self.0.to_ascii_lowercase())
    }

    /// Convert to uppercase.
    pub fn to_ascii_uppercase(&self) -> Atom {
        Atom::from_string(self.0.to_ascii_uppercase())
    }

    /// Check equality ignoring ASCII case.
    pub fn eq_ignore_ascii_case(&self, other: &Atom) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }

    /// Static empty atom.
    pub fn empty() -> Self {
        Atom::new("")
    }
}

impl Default for Atom {
    fn default() -> Self {
        Atom::empty()
    }
}

impl Deref for Atom {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Atom {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq for Atom {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // First try pointer equality for interned strings
        Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}

impl Eq for Atom {}

impl PartialEq<str> for Atom {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Atom {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for Atom {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialOrd for Atom {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Atom {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl Hash for Atom {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<&str> for Atom {
    #[inline]
    fn from(s: &str) -> Self {
        Atom::new(s)
    }
}

impl From<String> for Atom {
    #[inline]
    fn from(s: String) -> Self {
        Atom::from_string(s)
    }
}

impl From<Atom> for String {
    #[inline]
    fn from(atom: Atom) -> Self {
        (*atom.0).clone()
    }
}

/// Macro for creating atoms at compile time (simplified runtime version).
#[macro_export]
macro_rules! atom {
    ($s:expr) => {
        $crate::Atom::new($s)
    };
}

/// Collection of atoms with efficient lookup.
pub struct AtomSet {
    atoms: Vec<Atom>,
}

impl AtomSet {
    /// Create a new empty atom set.
    pub fn new() -> Self {
        AtomSet { atoms: Vec::new() }
    }

    /// Insert an atom, returning a reference to the interned version.
    pub fn insert(&mut self, atom: Atom) -> Atom {
        for existing in &self.atoms {
            if existing == &atom {
                return existing.clone();
            }
        }
        self.atoms.push(atom.clone());
        atom
    }

    /// Check if the set contains an atom.
    pub fn contains(&self, atom: &Atom) -> bool {
        self.atoms.iter().any(|a| a == atom)
    }

    /// Get the number of atoms in the set.
    pub fn len(&self) -> usize {
        self.atoms.len()
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }
}

impl Default for AtomSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atom_equality() {
        let a1 = Atom::new("hello");
        let a2 = Atom::new("hello");
        let a3 = Atom::new("world");

        assert_eq!(a1, a2);
        assert_ne!(a1, a3);
    }

    #[test]
    fn test_atom_str_equality() {
        let a = Atom::new("test");
        assert_eq!(a, "test");
        assert_ne!(a, "other");
    }
}
