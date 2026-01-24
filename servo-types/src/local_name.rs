//! LocalName - Element and attribute local names
//!
//! Compatible with Servo's LocalName in markup5ever.

use crate::Atom;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::Deref;

/// A local name (the name of an element or attribute without namespace prefix).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalName(pub Atom);

impl LocalName {
    /// Create a new local name from a string.
    #[inline]
    pub fn new(s: &str) -> Self {
        LocalName(Atom::new(s))
    }

    /// Get the local name as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Check if the local name is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get the length.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Convert to lowercase.
    pub fn to_ascii_lowercase(&self) -> LocalName {
        LocalName(self.0.to_ascii_lowercase())
    }

    /// Check equality ignoring ASCII case.
    pub fn eq_ignore_ascii_case(&self, other: &LocalName) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl Default for LocalName {
    fn default() -> Self {
        LocalName(Atom::empty())
    }
}

impl Deref for LocalName {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for LocalName {
    #[inline]
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for LocalName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LocalName({:?})", self.0)
    }
}

impl fmt::Display for LocalName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for LocalName {
    fn from(s: &str) -> Self {
        LocalName::new(s)
    }
}

impl From<Atom> for LocalName {
    fn from(atom: Atom) -> Self {
        LocalName(atom)
    }
}

impl PartialEq<str> for LocalName {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for LocalName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

/// Create a local name.
#[inline]
pub fn local_name(s: &str) -> LocalName {
    LocalName::new(s)
}

/// Macro for local name literals.
#[macro_export]
macro_rules! local_name {
    ($s:expr) => {
        $crate::LocalName::new($s)
    };
}

// ============================================================================
// Well-known HTML element names
// ============================================================================

pub mod html {
    use super::LocalName;

    macro_rules! define_local_names {
        ($($name:ident => $value:expr),* $(,)?) => {
            $(
                #[inline]
                pub fn $name() -> LocalName {
                    LocalName::new($value)
                }
            )*
        };
    }

    define_local_names! {
        // Document structure
        html => "html",
        head => "head",
        body => "body",
        title => "title",
        meta => "meta",
        link => "link",
        style => "style",
        script => "script",

        // Sections
        header => "header",
        footer => "footer",
        main => "main",
        nav => "nav",
        section => "section",
        article => "article",
        aside => "aside",

        // Headings
        h1 => "h1",
        h2 => "h2",
        h3 => "h3",
        h4 => "h4",
        h5 => "h5",
        h6 => "h6",

        // Text content
        div => "div",
        p => "p",
        span => "span",
        br => "br",
        hr => "hr",
        pre => "pre",
        blockquote => "blockquote",

        // Lists
        ul => "ul",
        ol => "ol",
        li => "li",
        dl => "dl",
        dt => "dt",
        dd => "dd",

        // Tables
        table => "table",
        thead => "thead",
        tbody => "tbody",
        tfoot => "tfoot",
        tr => "tr",
        th => "th",
        td => "td",
        caption => "caption",
        colgroup => "colgroup",
        col => "col",

        // Forms
        form => "form",
        input => "input",
        button => "button",
        select => "select",
        option => "option",
        optgroup => "optgroup",
        textarea => "textarea",
        label => "label",
        fieldset => "fieldset",
        legend => "legend",

        // Media
        img => "img",
        audio => "audio",
        video => "video",
        source => "source",
        track => "track",
        canvas => "canvas",

        // Embedded content
        iframe => "iframe",
        embed => "embed",
        object => "object",
        param => "param",

        // Interactive
        a => "a",
        details => "details",
        summary => "summary",
        dialog => "dialog",

        // Semantic text
        strong => "strong",
        em => "em",
        b => "b",
        i => "i",
        u => "u",
        s => "s",
        small => "small",
        mark => "mark",
        del => "del",
        ins => "ins",
        sub => "sub",
        sup => "sup",
        code => "code",
        kbd => "kbd",
        samp => "samp",
        var => "var",
        abbr => "abbr",
        cite => "cite",
        q => "q",
        dfn => "dfn",
        time => "time",
        data => "data",
        address => "address",

        // Ruby
        ruby => "ruby",
        rt => "rt",
        rp => "rp",
        rb => "rb",
        rtc => "rtc",

        // Other
        template => "template",
        slot => "slot",
        noscript => "noscript",
        base => "base",
        area => "area",
        map => "map",
        picture => "picture",
        figure => "figure",
        figcaption => "figcaption",
        wbr => "wbr",
        bdi => "bdi",
        bdo => "bdo",
        output => "output",
        progress => "progress",
        meter => "meter",
        datalist => "datalist",
    }
}

// ============================================================================
// Well-known attribute names
// ============================================================================

pub mod attr {
    use super::LocalName;

    macro_rules! define_attr_names {
        ($($name:ident => $value:expr),* $(,)?) => {
            $(
                #[inline]
                pub fn $name() -> LocalName {
                    LocalName::new($value)
                }
            )*
        };
    }

    define_attr_names! {
        // Global attributes
        id => "id",
        class => "class",
        style => "style",
        title => "title",
        lang => "lang",
        dir => "dir",
        hidden => "hidden",
        tabindex => "tabindex",
        accesskey => "accesskey",
        contenteditable => "contenteditable",
        draggable => "draggable",
        spellcheck => "spellcheck",
        translate => "translate",

        // Data attributes (prefix)
        data => "data",

        // Event handlers
        onclick => "onclick",
        onload => "onload",
        onerror => "onerror",
        onsubmit => "onsubmit",
        onchange => "onchange",
        oninput => "oninput",
        onfocus => "onfocus",
        onblur => "onblur",
        onkeydown => "onkeydown",
        onkeyup => "onkeyup",
        onmousedown => "onmousedown",
        onmouseup => "onmouseup",
        onmouseover => "onmouseover",
        onmouseout => "onmouseout",

        // Link attributes
        href => "href",
        src => "src",
        rel => "rel",
        target => "target",
        download => "download",
        hreflang => "hreflang",
        type_ => "type",
        media => "media",

        // Form attributes
        name => "name",
        value => "value",
        placeholder => "placeholder",
        required => "required",
        disabled => "disabled",
        readonly => "readonly",
        checked => "checked",
        selected => "selected",
        multiple => "multiple",
        maxlength => "maxlength",
        minlength => "minlength",
        pattern => "pattern",
        min => "min",
        max => "max",
        step => "step",
        autocomplete => "autocomplete",
        autofocus => "autofocus",
        form => "form",
        formaction => "formaction",
        formmethod => "formmethod",

        // Image/Media attributes
        alt => "alt",
        width => "width",
        height => "height",
        loading => "loading",
        decoding => "decoding",
        crossorigin => "crossorigin",
        srcset => "srcset",
        sizes => "sizes",
        autoplay => "autoplay",
        controls => "controls",
        loop_ => "loop",
        muted => "muted",
        preload => "preload",
        poster => "poster",

        // Table attributes
        colspan => "colspan",
        rowspan => "rowspan",
        scope => "scope",
        headers => "headers",

        // Meta attributes
        charset => "charset",
        content => "content",
        http_equiv => "http-equiv",

        // ARIA attributes
        role => "role",
        aria_label => "aria-label",
        aria_labelledby => "aria-labelledby",
        aria_describedby => "aria-describedby",
        aria_hidden => "aria-hidden",
        aria_expanded => "aria-expanded",
        aria_controls => "aria-controls",
        aria_live => "aria-live",

        // Other
        action => "action",
        method => "method",
        enctype => "enctype",
        accept => "accept",
        accept_charset => "accept-charset",
        for_ => "for",
        list => "list",
        label => "label",
        open => "open",
        datetime => "datetime",
        cite => "cite",
        async_ => "async",
        defer => "defer",
        integrity => "integrity",
        nonce => "nonce",
        referrerpolicy => "referrerpolicy",
        sandbox => "sandbox",
        allow => "allow",
        allowfullscreen => "allowfullscreen",
        coords => "coords",
        shape => "shape",
        usemap => "usemap",
        ismap => "ismap",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_name_equality() {
        let name1 = LocalName::new("div");
        let name2 = html::div();
        assert_eq!(name1, name2);
    }

    #[test]
    fn test_case_insensitive() {
        let name1 = LocalName::new("DIV");
        let name2 = LocalName::new("div");
        assert!(name1.eq_ignore_ascii_case(&name2));
    }
}
