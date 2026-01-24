//! QualName - Fully qualified name (namespace + prefix + local name)
//!
//! Compatible with Servo's QualName in markup5ever.

use crate::{LocalName, Namespace, Prefix};
use crate::namespace::{HTML_NAMESPACE, SVG_NAMESPACE, MATHML_NAMESPACE};
use core::fmt;
use core::hash::{Hash, Hasher};

/// A fully qualified name, containing namespace, prefix, and local name.
///
/// This is the primary type used to identify elements and attributes
/// in the DOM tree.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct QualName {
    /// The namespace prefix (e.g., "svg" in `svg:rect`).
    pub prefix: Option<Prefix>,
    /// The namespace URI.
    pub ns: Namespace,
    /// The local name (e.g., "rect" in `svg:rect` or just "div").
    pub local: LocalName,
}

impl QualName {
    /// Create a new qualified name.
    #[inline]
    pub fn new(prefix: Option<Prefix>, ns: Namespace, local: LocalName) -> Self {
        QualName { prefix, ns, local }
    }

    /// Create a qualified name with no prefix.
    #[inline]
    pub fn with_ns(ns: Namespace, local: LocalName) -> Self {
        QualName {
            prefix: None,
            ns,
            local,
        }
    }

    /// Create a qualified name in the null namespace.
    #[inline]
    pub fn local(local: LocalName) -> Self {
        QualName {
            prefix: None,
            ns: Namespace::default(),
            local,
        }
    }

    /// Create a qualified name in the HTML namespace.
    #[inline]
    pub fn html(local: LocalName) -> Self {
        QualName {
            prefix: None,
            ns: Namespace::new(HTML_NAMESPACE),
            local,
        }
    }

    /// Create a qualified name in the SVG namespace.
    #[inline]
    pub fn svg(local: LocalName) -> Self {
        QualName {
            prefix: Some(Prefix::new("svg")),
            ns: Namespace::new(SVG_NAMESPACE),
            local,
        }
    }

    /// Create a qualified name in the MathML namespace.
    #[inline]
    pub fn mathml(local: LocalName) -> Self {
        QualName {
            prefix: Some(Prefix::new("math")),
            ns: Namespace::new(MATHML_NAMESPACE),
            local,
        }
    }

    /// Check if this is in the HTML namespace.
    #[inline]
    pub fn is_html(&self) -> bool {
        self.ns.as_str() == HTML_NAMESPACE
    }

    /// Check if this is in the SVG namespace.
    #[inline]
    pub fn is_svg(&self) -> bool {
        self.ns.as_str() == SVG_NAMESPACE
    }

    /// Check if this is in the MathML namespace.
    #[inline]
    pub fn is_mathml(&self) -> bool {
        self.ns.as_str() == MATHML_NAMESPACE
    }

    /// Check if the namespace is empty (null namespace).
    #[inline]
    pub fn is_null_namespace(&self) -> bool {
        self.ns.is_empty()
    }

    /// Get the local name as a string.
    #[inline]
    pub fn local_name(&self) -> &str {
        self.local.as_str()
    }

    /// Get the namespace as a string.
    #[inline]
    pub fn namespace(&self) -> &str {
        self.ns.as_str()
    }

    /// Check if local names match (case-sensitive).
    #[inline]
    pub fn local_name_eq(&self, other: &str) -> bool {
        self.local.as_str() == other
    }

    /// Check if local names match (case-insensitive for HTML).
    pub fn local_name_eq_ignore_case(&self, other: &str) -> bool {
        self.local.as_str().eq_ignore_ascii_case(other)
    }

    /// Expanded name representation: `{namespace}localname`
    pub fn expanded_name(&self) -> alloc::string::String {
        use alloc::string::ToString;
        if self.ns.is_empty() {
            self.local.to_string()
        } else {
            alloc::format!("{{{}}}{}", self.ns, self.local)
        }
    }
}

impl fmt::Debug for QualName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref prefix) = self.prefix {
            write!(f, "{}:", prefix)?;
        }
        if !self.ns.is_empty() {
            write!(f, "{{{}}}", self.ns)?;
        }
        write!(f, "{}", self.local)
    }
}

impl fmt::Display for QualName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref prefix) = self.prefix {
            write!(f, "{}:{}", prefix, self.local)
        } else {
            write!(f, "{}", self.local)
        }
    }
}

/// Macro for creating qualified names.
#[macro_export]
macro_rules! qualname {
    // Just local name (null namespace)
    ($local:expr) => {
        $crate::QualName::local($crate::LocalName::new($local))
    };
    // Namespace and local name
    ($ns:expr, $local:expr) => {
        $crate::QualName::with_ns(
            $crate::Namespace::new($ns),
            $crate::LocalName::new($local),
        )
    };
    // Prefix, namespace, and local name
    ($prefix:expr, $ns:expr, $local:expr) => {
        $crate::QualName::new(
            Some($crate::Prefix::new($prefix)),
            $crate::Namespace::new($ns),
            $crate::LocalName::new($local),
        )
    };
}

// ============================================================================
// Convenience constructors for common HTML elements
// ============================================================================

pub mod html_elem {
    use super::*;
    use crate::local_name::html as html_local;

    macro_rules! define_html_elements {
        ($($name:ident),* $(,)?) => {
            $(
                #[inline]
                pub fn $name() -> QualName {
                    QualName::html(html_local::$name())
                }
            )*
        };
    }

    define_html_elements! {
        html, head, body, title, meta, link, style, script,
        header, footer, main, nav, section, article, aside,
        h1, h2, h3, h4, h5, h6,
        div, p, span, br, hr, pre, blockquote,
        ul, ol, li, dl, dt, dd,
        table, thead, tbody, tfoot, tr, th, td,
        form, input, button, select, option, textarea, label,
        img, audio, video, canvas,
        iframe,
        a, details, summary,
        strong, em, b, i, code,
        template,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qualname_html() {
        let name = QualName::html(LocalName::new("div"));
        assert!(name.is_html());
        assert_eq!(name.local_name(), "div");
    }

    #[test]
    fn test_qualname_expanded() {
        let name = QualName::html(LocalName::new("div"));
        assert_eq!(
            name.expanded_name(),
            "{http://www.w3.org/1999/xhtml}div"
        );
    }

    #[test]
    fn test_qualname_macro() {
        let name = qualname!("div");
        assert!(name.is_null_namespace());
        assert_eq!(name.local_name(), "div");
    }
}
