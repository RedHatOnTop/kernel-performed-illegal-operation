//! Screen Reader Support
//!
//! Text-to-speech and screen reader integration.

use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use super::{A11yNode, Role};

/// Speech priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SpeechPriority {
    /// Low priority (can be interrupted)
    Low,
    /// Normal priority
    Normal,
    /// High priority (alerts)
    High,
    /// Critical (errors, urgent)
    Critical,
}

impl Default for SpeechPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Speech request
#[derive(Debug, Clone)]
pub struct SpeechRequest {
    /// Text to speak
    pub text: String,
    /// Priority
    pub priority: SpeechPriority,
    /// Language
    pub language: Option<String>,
    /// Rate (0.1 - 10.0)
    pub rate: f32,
    /// Pitch (0.0 - 2.0)
    pub pitch: f32,
    /// Volume (0.0 - 1.0)
    pub volume: f32,
    /// Voice ID
    pub voice: Option<String>,
    /// Interrupt current speech
    pub interrupt: bool,
}

impl SpeechRequest {
    /// Create new request
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            priority: SpeechPriority::Normal,
            language: None,
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
            voice: None,
            interrupt: false,
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: SpeechPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set interrupt
    pub fn with_interrupt(mut self, interrupt: bool) -> Self {
        self.interrupt = interrupt;
        self
    }

    /// Set rate
    pub fn with_rate(mut self, rate: f32) -> Self {
        self.rate = rate.clamp(0.1, 10.0);
        self
    }
}

/// Speech voice
#[derive(Debug, Clone)]
pub struct Voice {
    /// Voice ID
    pub id: String,
    /// Name
    pub name: String,
    /// Language
    pub language: String,
    /// Is default
    pub default: bool,
    /// Is local
    pub local: bool,
}

/// Screen reader
pub struct ScreenReader {
    /// Enabled
    enabled: bool,
    /// Speech queue
    queue: VecDeque<SpeechRequest>,
    /// Available voices
    voices: Vec<Voice>,
    /// Current voice
    current_voice: Option<String>,
    /// Speech rate
    rate: f32,
    /// Speech pitch
    pitch: f32,
    /// Speech volume
    volume: f32,
    /// Verbosity level
    verbosity: Verbosity,
    /// Reading cursor position
    cursor: Option<u64>,
    /// Keyboard echo
    keyboard_echo: KeyboardEcho,
}

/// Verbosity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    /// Minimal announcements
    Low,
    /// Normal announcements
    Normal,
    /// Detailed announcements
    High,
}

impl Default for Verbosity {
    fn default() -> Self {
        Self::Normal
    }
}

/// Keyboard echo mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardEcho {
    /// No echo
    None,
    /// Echo characters
    Characters,
    /// Echo words
    Words,
    /// Echo both
    Both,
}

impl Default for KeyboardEcho {
    fn default() -> Self {
        Self::Words
    }
}

impl ScreenReader {
    /// Create new screen reader
    pub const fn new() -> Self {
        Self {
            enabled: false,
            queue: VecDeque::new(),
            voices: Vec::new(),
            current_voice: None,
            rate: 1.0,
            pitch: 1.0,
            volume: 1.0,
            verbosity: Verbosity::Normal,
            cursor: None,
            keyboard_echo: KeyboardEcho::Words,
        }
    }

    /// Enable/disable
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            self.speak(SpeechRequest::new("Screen reader on")
                .with_priority(SpeechPriority::High));
        }
    }

    /// Is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Speak text
    pub fn speak(&mut self, request: SpeechRequest) {
        if !self.enabled {
            return;
        }

        if request.interrupt {
            // Clear lower priority items
            self.queue.retain(|r| r.priority >= request.priority);
        }

        // Insert based on priority
        let pos = self.queue.iter()
            .position(|r| r.priority < request.priority)
            .unwrap_or(self.queue.len());
        
        self.queue.insert(pos, request);
    }

    /// Speak node
    pub fn speak_node(&mut self, node: &A11yNode) {
        let text = self.describe_node(node);
        self.speak(SpeechRequest::new(text));
    }

    /// Describe node for speech
    fn describe_node(&self, node: &A11yNode) -> String {
        let mut parts = Vec::new();

        // Role description
        if self.verbosity != Verbosity::Low {
            parts.push(self.role_description(node.role));
        }

        // Name
        if !node.name.is_empty() {
            parts.push(node.name.clone());
        }

        // Value
        if !node.value.is_empty() {
            parts.push(node.value.clone());
        }

        // State
        if self.verbosity == Verbosity::High {
            if node.state.disabled {
                parts.push("disabled".to_string());
            }
            if let Some(checked) = node.state.checked {
                parts.push(if checked { "checked" } else { "not checked" }.to_string());
            }
            if let Some(expanded) = node.state.expanded {
                parts.push(if expanded { "expanded" } else { "collapsed" }.to_string());
            }
        }

        // Level for headings
        if node.role == Role::Heading {
            if let Some(level) = node.level {
                parts.push(alloc::format!("level {}", level));
            }
        }

        // Position in set
        if self.verbosity != Verbosity::Low {
            if let (Some(pos), Some(size)) = (node.pos_in_set, node.set_size) {
                parts.push(alloc::format!("{} of {}", pos, size));
            }
        }

        parts.join(", ")
    }

    /// Get role description
    fn role_description(&self, role: Role) -> String {
        match role {
            Role::Button => "button",
            Role::Link => "link",
            Role::Heading => "heading",
            Role::TextField => "text field",
            Role::CheckBox => "checkbox",
            Role::RadioButton => "radio button",
            Role::ComboBox => "combo box",
            Role::Menu => "menu",
            Role::MenuItem => "menu item",
            Role::List => "list",
            Role::ListItem => "list item",
            Role::Image => "image",
            Role::Dialog => "dialog",
            Role::Alert => "alert",
            Role::Tab => "tab",
            Role::TabPanel => "tab panel",
            Role::Table => "table",
            Role::Row => "row",
            Role::Cell => "cell",
            Role::ProgressBar => "progress bar",
            Role::Slider => "slider",
            Role::Tree => "tree",
            Role::TreeItem => "tree item",
            Role::Navigation => "navigation",
            Role::Main => "main content",
            Role::Banner => "banner",
            Role::Search => "search",
            _ => "",
        }.to_string()
    }

    /// Stop speaking
    pub fn stop(&mut self) {
        self.queue.clear();
    }

    /// Pause
    pub fn pause(&mut self) {
        // Would pause TTS engine
    }

    /// Resume
    pub fn resume(&mut self) {
        // Would resume TTS engine
    }

    /// Get next speech request
    pub fn next(&mut self) -> Option<SpeechRequest> {
        self.queue.pop_front()
    }

    /// Set verbosity
    pub fn set_verbosity(&mut self, level: Verbosity) {
        self.verbosity = level;
    }

    /// Set rate
    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate.clamp(0.1, 10.0);
    }

    /// Set pitch
    pub fn set_pitch(&mut self, pitch: f32) {
        self.pitch = pitch.clamp(0.0, 2.0);
    }

    /// Set volume
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Set keyboard echo
    pub fn set_keyboard_echo(&mut self, mode: KeyboardEcho) {
        self.keyboard_echo = mode;
    }

    /// Add voice
    pub fn add_voice(&mut self, voice: Voice) {
        self.voices.push(voice);
    }

    /// Get available voices
    pub fn voices(&self) -> &[Voice] {
        &self.voices
    }

    /// Set current voice
    pub fn set_voice(&mut self, voice_id: &str) {
        if self.voices.iter().any(|v| v.id == voice_id) {
            self.current_voice = Some(voice_id.to_string());
        }
    }
}

impl Default for ScreenReader {
    fn default() -> Self {
        Self::new()
    }
}

/// Live region
#[derive(Debug, Clone)]
pub struct LiveRegion {
    /// Node ID
    pub node_id: u64,
    /// Politeness
    pub politeness: LivePoliteness,
    /// Atomic (announce whole region)
    pub atomic: bool,
    /// Relevant changes
    pub relevant: LiveRelevant,
}

/// Live region politeness
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LivePoliteness {
    /// Off (don't announce)
    Off,
    /// Polite (announce when idle)
    Polite,
    /// Assertive (interrupt)
    Assertive,
}

/// Live region relevance
#[derive(Debug, Clone, Copy, Default)]
pub struct LiveRelevant {
    /// Additions
    pub additions: bool,
    /// Removals
    pub removals: bool,
    /// Text changes
    pub text: bool,
    /// All changes
    pub all: bool,
}
