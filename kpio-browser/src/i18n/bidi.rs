//! Bidirectional Text Support
//!
//! Unicode BiDi algorithm implementation for mixed LTR/RTL text.

use alloc::string::String;
use alloc::vec::Vec;

/// BiDi direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiDirection {
    /// Left-to-right
    Ltr,
    /// Right-to-left
    Rtl,
    /// Neutral
    Neutral,
}

/// BiDi class (simplified)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiClass {
    /// Left-to-right (L)
    L,
    /// Left-to-right embedding (LRE)
    LRE,
    /// Left-to-right override (LRO)
    LRO,
    /// Right-to-left (R)
    R,
    /// Arabic letter (AL)
    AL,
    /// Right-to-left embedding (RLE)
    RLE,
    /// Right-to-left override (RLO)
    RLO,
    /// Pop directional format (PDF)
    PDF,
    /// European number (EN)
    EN,
    /// European separator (ES)
    ES,
    /// European terminator (ET)
    ET,
    /// Arabic number (AN)
    AN,
    /// Common separator (CS)
    CS,
    /// Nonspacing mark (NSM)
    NSM,
    /// Boundary neutral (BN)
    BN,
    /// Paragraph separator (B)
    B,
    /// Segment separator (S)
    S,
    /// Whitespace (WS)
    WS,
    /// Other neutral (ON)
    ON,
    /// Left-to-right isolate (LRI)
    LRI,
    /// Right-to-left isolate (RLI)
    RLI,
    /// First strong isolate (FSI)
    FSI,
    /// Pop directional isolate (PDI)
    PDI,
}

impl BidiClass {
    /// Get class for character
    pub fn for_char(c: char) -> Self {
        match c {
            // ASCII letters
            'A'..='Z' | 'a'..='z' => Self::L,
            
            // Digits
            '0'..='9' => Self::EN,
            
            // Common punctuation
            '.' | ',' => Self::CS,
            '+' | '-' => Self::ES,
            '#' | '$' | '%' => Self::ET,
            
            // Whitespace
            ' ' | '\t' => Self::WS,
            '\n' | '\r' => Self::B,
            
            // Hebrew
            '\u{0590}'..='\u{05FF}' => Self::R,
            
            // Arabic
            '\u{0600}'..='\u{06FF}' => Self::AL,
            '\u{0750}'..='\u{077F}' => Self::AL,
            '\u{08A0}'..='\u{08FF}' => Self::AL,
            '\u{FB50}'..='\u{FDFF}' => Self::AL,
            '\u{FE70}'..='\u{FEFF}' => Self::AL,
            
            // Arabic-Indic digits
            '\u{0660}'..='\u{0669}' => Self::AN,
            '\u{06F0}'..='\u{06F9}' => Self::AN,
            
            // CJK characters — all classified as L (strong left-to-right)
            // Hangul Jamo
            '\u{1100}'..='\u{11FF}' => Self::L,
            // CJK Radicals Supplement + Kangxi Radicals
            '\u{2E80}'..='\u{2FFF}' => Self::L,
            // CJK Unified Ideographs Extension A + CJK Unified Ideographs
            '\u{3400}'..='\u{9FFF}' => Self::L,
            // Hiragana
            '\u{3040}'..='\u{309F}' => Self::L,
            // Katakana
            '\u{30A0}'..='\u{30FF}' => Self::L,
            // Bopomofo
            '\u{3100}'..='\u{312F}' => Self::L,
            // Hangul Compatibility Jamo
            '\u{3130}'..='\u{318F}' => Self::L,
            // CJK Compatibility
            '\u{3300}'..='\u{33FF}' => Self::L,
            // Hangul Syllables
            '\u{AC00}'..='\u{D7AF}' => Self::L,
            // Hangul Jamo Extended
            '\u{D7B0}'..='\u{D7FF}' => Self::L,
            // CJK Compatibility Ideographs
            '\u{F900}'..='\u{FAFF}' => Self::L,
            // Halfwidth and Fullwidth Forms (fullwidth Latin + halfwidth Katakana)
            '\u{FF00}'..='\u{FFEF}' => Self::L,
            // CJK Unified Ideographs Extension B+
            '\u{20000}'..='\u{2FA1F}' => Self::L,
            // Latin Extended (covers letters from European languages beyond ASCII)
            '\u{00C0}'..='\u{024F}' => Self::L,
            // Cyrillic
            '\u{0400}'..='\u{04FF}' => Self::L,
            // Greek
            '\u{0370}'..='\u{03FF}' => Self::L,
            // Thai
            '\u{0E00}'..='\u{0E7F}' => Self::L,
            // Devanagari
            '\u{0900}'..='\u{097F}' => Self::L,
            
            // Explicit directional marks
            '\u{200E}' => Self::L,  // LRM
            '\u{200F}' => Self::R,  // RLM
            '\u{202A}' => Self::LRE,
            '\u{202B}' => Self::RLE,
            '\u{202C}' => Self::PDF,
            '\u{202D}' => Self::LRO,
            '\u{202E}' => Self::RLO,
            '\u{2066}' => Self::LRI,
            '\u{2067}' => Self::RLI,
            '\u{2068}' => Self::FSI,
            '\u{2069}' => Self::PDI,
            
            // Other neutral
            _ => Self::ON,
        }
    }

    /// Is strong type
    pub fn is_strong(self) -> bool {
        matches!(self, Self::L | Self::R | Self::AL)
    }

    /// Get direction
    pub fn direction(self) -> Option<BidiDirection> {
        match self {
            Self::L | Self::LRE | Self::LRO | Self::LRI => Some(BidiDirection::Ltr),
            Self::R | Self::AL | Self::RLE | Self::RLO | Self::RLI => Some(BidiDirection::Rtl),
            _ => None,
        }
    }
}

/// BiDi run
#[derive(Debug, Clone)]
pub struct BidiRun {
    /// Start index
    pub start: usize,
    /// End index (exclusive)
    pub end: usize,
    /// Direction
    pub direction: BidiDirection,
    /// Embedding level
    pub level: u8,
}

/// BiDi paragraph
pub struct BidiParagraph {
    /// Text
    text: String,
    /// Base direction
    base_direction: BidiDirection,
    /// Runs
    runs: Vec<BidiRun>,
}

impl BidiParagraph {
    /// Create new paragraph
    pub fn new(text: &str, base_direction: Option<BidiDirection>) -> Self {
        let base = base_direction.unwrap_or_else(|| Self::detect_direction(text));
        let runs = Self::compute_runs(text, base);

        Self {
            text: text.into(),
            base_direction: base,
            runs,
        }
    }

    /// Detect paragraph direction
    fn detect_direction(text: &str) -> BidiDirection {
        // Find first strong character
        for c in text.chars() {
            match BidiClass::for_char(c) {
                BidiClass::L | BidiClass::LRE | BidiClass::LRO => return BidiDirection::Ltr,
                BidiClass::R | BidiClass::AL | BidiClass::RLE | BidiClass::RLO => return BidiDirection::Rtl,
                _ => continue,
            }
        }
        BidiDirection::Ltr // Default
    }

    /// Compute BiDi runs (simplified)
    fn compute_runs(text: &str, base: BidiDirection) -> Vec<BidiRun> {
        let mut runs = Vec::new();
        let mut current_start = 0;
        let mut current_direction = base;
        let chars: Vec<char> = text.chars().collect();
        
        if chars.is_empty() {
            return runs;
        }

        for (i, &c) in chars.iter().enumerate() {
            let class = BidiClass::for_char(c);
            let direction = class.direction().unwrap_or(BidiDirection::Neutral);
            
            if direction != BidiDirection::Neutral && direction != current_direction {
                if i > current_start {
                    runs.push(BidiRun {
                        start: current_start,
                        end: i,
                        direction: current_direction,
                        level: if current_direction == BidiDirection::Rtl { 1 } else { 0 },
                    });
                }
                current_start = i;
                current_direction = direction;
            }
        }

        // Final run
        if current_start < chars.len() {
            runs.push(BidiRun {
                start: current_start,
                end: chars.len(),
                direction: current_direction,
                level: if current_direction == BidiDirection::Rtl { 1 } else { 0 },
            });
        }

        runs
    }

    /// Get base direction
    pub fn base_direction(&self) -> BidiDirection {
        self.base_direction
    }

    /// Get runs
    pub fn runs(&self) -> &[BidiRun] {
        &self.runs
    }

    /// Get text
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Reorder for display
    pub fn reorder(&self) -> String {
        if self.runs.is_empty() {
            return self.text.clone();
        }

        let chars: Vec<char> = self.text.chars().collect();
        let mut result = String::new();

        // For LTR base, display runs in order, reversing RTL runs
        // For RTL base, reverse order of runs, reversing each LTR run
        
        if self.base_direction == BidiDirection::Ltr {
            for run in &self.runs {
                let segment: String = chars[run.start..run.end].iter().collect();
                if run.direction == BidiDirection::Rtl {
                    result.extend(segment.chars().rev());
                } else {
                    result.push_str(&segment);
                }
            }
        } else {
            for run in self.runs.iter().rev() {
                let segment: String = chars[run.start..run.end].iter().collect();
                if run.direction == BidiDirection::Ltr {
                    result.extend(segment.chars().rev());
                } else {
                    result.push_str(&segment);
                }
            }
        }

        result
    }
}

/// Unicode marks
pub struct UnicodeMark;

impl UnicodeMark {
    /// Left-to-right mark
    pub const LRM: char = '\u{200E}';
    /// Right-to-left mark
    pub const RLM: char = '\u{200F}';
    /// Left-to-right embedding
    pub const LRE: char = '\u{202A}';
    /// Right-to-left embedding
    pub const RLE: char = '\u{202B}';
    /// Pop directional formatting
    pub const PDF: char = '\u{202C}';
    /// Left-to-right override
    pub const LRO: char = '\u{202D}';
    /// Right-to-left override
    pub const RLO: char = '\u{202E}';
    /// Left-to-right isolate
    pub const LRI: char = '\u{2066}';
    /// Right-to-left isolate
    pub const RLI: char = '\u{2067}';
    /// First strong isolate
    pub const FSI: char = '\u{2068}';
    /// Pop directional isolate
    pub const PDI: char = '\u{2069}';
    /// Word joiner
    pub const WJ: char = '\u{2060}';
    /// Zero-width non-joiner
    pub const ZWNJ: char = '\u{200C}';
    /// Zero-width joiner
    pub const ZWJ: char = '\u{200D}';
}

/// Check if character is RTL
pub fn is_rtl_char(c: char) -> bool {
    matches!(
        BidiClass::for_char(c),
        BidiClass::R | BidiClass::AL | BidiClass::RLE | BidiClass::RLO | BidiClass::RLI
    )
}

/// Check if string contains RTL
pub fn contains_rtl(s: &str) -> bool {
    s.chars().any(is_rtl_char)
}
