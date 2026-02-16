//! Browser UI Module
//!
//! User interface components for the KPIO browser.

#![allow(dead_code)]

pub mod bookmarks;
pub mod downloads;
pub mod history;
pub mod print;
pub mod settings;
pub mod tabs;

use alloc::string::String;

/// UI Theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// Light theme.
    #[default]
    Light,
    /// Dark theme.
    Dark,
    /// System preference.
    System,
}

/// UI Color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create a new color.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create opaque color.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// White color.
    pub const fn white() -> Self {
        Self::rgb(255, 255, 255)
    }

    /// Black color.
    pub const fn black() -> Self {
        Self::rgb(0, 0, 0)
    }

    /// Transparent.
    pub const fn transparent() -> Self {
        Self::new(0, 0, 0, 0)
    }
}

/// Rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// Create a new rectangle.
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if point is inside.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width as i32
            && y >= self.y
            && y < self.y + self.height as i32
    }
}

/// UI Event.
#[derive(Debug, Clone)]
pub enum UiEvent {
    /// Mouse click.
    Click { x: i32, y: i32, button: MouseButton },
    /// Mouse move.
    MouseMove { x: i32, y: i32 },
    /// Key press.
    KeyPress {
        key: KeyCode,
        modifiers: KeyModifiers,
    },
    /// Key release.
    KeyRelease {
        key: KeyCode,
        modifiers: KeyModifiers,
    },
    /// Text input.
    TextInput { text: String },
    /// Window resize.
    Resize { width: u32, height: u32 },
    /// Focus gained.
    FocusIn,
    /// Focus lost.
    FocusOut,
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Back,
    Forward,
}

/// Key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    // Numbers
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    // Special keys
    Escape,
    Tab,
    CapsLock,
    Shift,
    Control,
    Alt,
    Meta,
    Space,
    Enter,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Left,
    Right,
    Up,
    Down,
    // Punctuation
    Comma,
    Period,
    Slash,
    Semicolon,
    Quote,
    BracketLeft,
    BracketRight,
    Backslash,
    Minus,
    Equal,
    Grave,
}

/// Key modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl KeyModifiers {
    /// No modifiers.
    pub const fn none() -> Self {
        Self {
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        }
    }

    /// Ctrl modifier.
    pub const fn ctrl() -> Self {
        Self {
            shift: false,
            ctrl: true,
            alt: false,
            meta: false,
        }
    }

    /// Check if Ctrl is pressed.
    pub fn has_ctrl(&self) -> bool {
        self.ctrl
    }
}
