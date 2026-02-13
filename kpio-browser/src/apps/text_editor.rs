//! Text Editor
//!
//! Basic text editing with syntax highlighting support.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Text editor instance
#[derive(Debug, Clone)]
pub struct TextEditor {
    /// Editor ID
    pub id: u64,
    /// Open documents
    pub documents: Vec<Document>,
    /// Active document index
    pub active_document: usize,
    /// Editor settings
    pub settings: EditorSettings,
    /// Find/replace state
    pub find_replace: Option<FindReplace>,
    /// Sidebar visible
    pub sidebar_visible: bool,
    /// Minimap visible
    pub minimap_visible: bool,
}

impl TextEditor {
    /// Create new text editor
    pub fn new(id: u64) -> Self {
        Self {
            id,
            documents: alloc::vec![Document::new_untitled()],
            active_document: 0,
            settings: EditorSettings::default(),
            find_replace: None,
            sidebar_visible: true,
            minimap_visible: true,
        }
    }

    /// Open file
    pub fn open(&mut self, path: &str, content: &str) {
        // Check if already open
        if let Some(idx) = self.documents.iter().position(|d| d.path.as_deref() == Some(path)) {
            self.active_document = idx;
            return;
        }

        let doc = Document::from_file(path, content);
        self.documents.push(doc);
        self.active_document = self.documents.len() - 1;
    }

    /// Create new document
    pub fn new_document(&mut self) {
        self.documents.push(Document::new_untitled());
        self.active_document = self.documents.len() - 1;
    }

    /// Close document - returns true if document needs save confirmation
    pub fn close(&mut self, index: usize) -> bool {
        if index >= self.documents.len() {
            return false;
        }

        if self.documents[index].modified {
            return true; // Caller should prompt to save
        }

        self.documents.remove(index);
        if self.active_document >= self.documents.len() && !self.documents.is_empty() {
            self.active_document = self.documents.len() - 1;
        }
        false
    }

    /// Get active document
    pub fn active(&mut self) -> Option<&mut Document> {
        self.documents.get_mut(self.active_document)
    }

    /// Save active document
    pub fn save(&mut self) -> Option<SaveRequest> {
        let doc = self.documents.get_mut(self.active_document)?;
        
        if let Some(path) = &doc.path {
            let request = SaveRequest {
                path: path.clone(),
                content: doc.content.clone(),
            };
            doc.modified = false;
            Some(request)
        } else {
            None // Need to prompt for filename
        }
    }

    /// Save document as new path
    pub fn save_as(&mut self, path: &str) -> Option<SaveRequest> {
        let doc = self.documents.get_mut(self.active_document)?;
        doc.path = Some(path.to_string());
        doc.name = path.split('/').last().unwrap_or("untitled").to_string();
        doc.detect_language();
        doc.modified = false;
        
        Some(SaveRequest {
            path: path.to_string(),
            content: doc.content.clone(),
        })
    }

    /// Toggle find/replace
    pub fn toggle_find(&mut self, with_replace: bool) {
        if self.find_replace.is_some() {
            self.find_replace = None;
        } else {
            self.find_replace = Some(FindReplace {
                query: String::new(),
                replacement: String::new(),
                case_sensitive: false,
                whole_word: false,
                regex: false,
                with_replace,
                results: Vec::new(),
                current_result: 0,
            });
        }
    }

    /// Find text
    pub fn find(&mut self, query: &str) {
        if let Some(find) = &mut self.find_replace {
            find.query = query.to_string();
            find.results.clear();
            find.current_result = 0;

            if let Some(doc) = self.documents.get(self.active_document) {
                let query_lower = if find.case_sensitive {
                    query.to_string()
                } else {
                    query.to_lowercase()
                };

                for (line_idx, line) in doc.lines.iter().enumerate() {
                    let search_line = if find.case_sensitive {
                        line.text.clone()
                    } else {
                        line.text.to_lowercase()
                    };

                    let mut start = 0;
                    while let Some(pos) = search_line[start..].find(&query_lower) {
                        find.results.push(FindResult {
                            line: line_idx,
                            start: start + pos,
                            end: start + pos + query.len(),
                        });
                        start += pos + 1;
                    }
                }
            }
        }
    }

    /// Find next
    pub fn find_next(&mut self) {
        if let Some(find) = &mut self.find_replace {
            if !find.results.is_empty() {
                find.current_result = (find.current_result + 1) % find.results.len();
            }
        }
    }

    /// Find previous
    pub fn find_prev(&mut self) {
        if let Some(find) = &mut self.find_replace {
            if !find.results.is_empty() {
                if find.current_result == 0 {
                    find.current_result = find.results.len() - 1;
                } else {
                    find.current_result -= 1;
                }
            }
        }
    }

    /// Replace current
    pub fn replace(&mut self) {
        if let Some(find) = &self.find_replace.clone() {
            if let Some(result) = find.results.get(find.current_result) {
                if let Some(doc) = self.documents.get_mut(self.active_document) {
                    if let Some(line) = doc.lines.get_mut(result.line) {
                        let mut new_text = line.text[..result.start].to_string();
                        new_text.push_str(&find.replacement);
                        new_text.push_str(&line.text[result.end..]);
                        line.text = new_text;
                        doc.modified = true;
                    }
                }
            }
            // Re-find after replace
            let query = find.query.clone();
            self.find(&query);
        }
    }

    /// Replace all
    pub fn replace_all(&mut self) {
        if let Some(find) = &self.find_replace {
            if let Some(doc) = self.documents.get_mut(self.active_document) {
                let query = &find.query;
                let replacement = &find.replacement;
                
                for line in &mut doc.lines {
                    if find.case_sensitive {
                        line.text = line.text.replace(query, replacement);
                    } else {
                        // Case insensitive replace is more complex
                        let mut result = String::new();
                        let mut remaining = line.text.as_str();
                        let query_lower = query.to_lowercase();
                        
                        while !remaining.is_empty() {
                            let lower = remaining.to_lowercase();
                            if let Some(pos) = lower.find(&query_lower) {
                                result.push_str(&remaining[..pos]);
                                result.push_str(replacement);
                                remaining = &remaining[pos + query.len()..];
                            } else {
                                result.push_str(remaining);
                                break;
                            }
                        }
                        line.text = result;
                    }
                }
                doc.modified = true;
                doc.update_content();
            }
        }
    }
}

impl Default for TextEditor {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Document
#[derive(Debug, Clone)]
pub struct Document {
    /// Document name
    pub name: String,
    /// File path
    pub path: Option<String>,
    /// Content
    pub content: String,
    /// Lines
    pub lines: Vec<Line>,
    /// Modified flag
    pub modified: bool,
    /// Language
    pub language: Language,
    /// Cursor position
    pub cursor: CursorPosition,
    /// Selection
    pub selection: Option<Selection>,
    /// Scroll position
    pub scroll_line: usize,
    /// Undo stack
    pub undo_stack: Vec<Edit>,
    /// Redo stack
    pub redo_stack: Vec<Edit>,
}

impl Document {
    /// Create new untitled document
    pub fn new_untitled() -> Self {
        Self {
            name: String::from("Untitled"),
            path: None,
            content: String::new(),
            lines: alloc::vec![Line::new(String::new())],
            modified: false,
            language: Language::PlainText,
            cursor: CursorPosition { line: 0, column: 0 },
            selection: None,
            scroll_line: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Create from file
    pub fn from_file(path: &str, content: &str) -> Self {
        let name = path.split('/').last().unwrap_or("file").to_string();
        let lines: Vec<Line> = content
            .lines()
            .map(|l| Line::new(l.to_string()))
            .collect();
        
        let lines = if lines.is_empty() {
            alloc::vec![Line::new(String::new())]
        } else {
            lines
        };

        let mut doc = Self {
            name,
            path: Some(path.to_string()),
            content: content.to_string(),
            lines,
            modified: false,
            language: Language::PlainText,
            cursor: CursorPosition { line: 0, column: 0 },
            selection: None,
            scroll_line: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };
        doc.detect_language();
        doc
    }

    /// Detect language from extension
    pub fn detect_language(&mut self) {
        self.language = match self.path.as_ref().and_then(|p| p.rsplit('.').next()) {
            Some("rs") => Language::Rust,
            Some("js") => Language::JavaScript,
            Some("ts") => Language::TypeScript,
            Some("py") => Language::Python,
            Some("html") | Some("htm") => Language::Html,
            Some("css") => Language::Css,
            Some("json") => Language::Json,
            Some("md") => Language::Markdown,
            Some("c") | Some("h") => Language::C,
            Some("cpp") | Some("hpp") | Some("cc") => Language::Cpp,
            Some("java") => Language::Java,
            Some("go") => Language::Go,
            Some("sh") | Some("bash") => Language::Shell,
            Some("toml") => Language::Toml,
            Some("yaml") | Some("yml") => Language::Yaml,
            Some("xml") => Language::Xml,
            Some("sql") => Language::Sql,
            _ => Language::PlainText,
        };
    }

    /// Insert character
    pub fn insert_char(&mut self, c: char) {
        self.save_undo();
        
        if let Some(line) = self.lines.get_mut(self.cursor.line) {
            let col = self.cursor.column.min(line.text.len());
            line.text.insert(col, c);
            self.cursor.column = col + 1;
            self.modified = true;
        }
    }

    /// Insert text
    pub fn insert_text(&mut self, text: &str) {
        self.save_undo();
        
        for c in text.chars() {
            if c == '\n' {
                self.insert_newline_internal();
            } else {
                if let Some(line) = self.lines.get_mut(self.cursor.line) {
                    let col = self.cursor.column.min(line.text.len());
                    line.text.insert(col, c);
                    self.cursor.column = col + 1;
                }
            }
        }
        self.modified = true;
    }

    /// Insert newline
    pub fn insert_newline(&mut self) {
        self.save_undo();
        self.insert_newline_internal();
        self.modified = true;
    }

    fn insert_newline_internal(&mut self) {
        if let Some(line) = self.lines.get_mut(self.cursor.line) {
            let col = self.cursor.column.min(line.text.len());
            let remaining = line.text[col..].to_string();
            line.text.truncate(col);
            
            let new_line = Line::new(remaining);
            self.lines.insert(self.cursor.line + 1, new_line);
            self.cursor.line += 1;
            self.cursor.column = 0;
        }
    }

    /// Delete character (backspace)
    pub fn backspace(&mut self) {
        self.save_undo();
        
        if self.cursor.column > 0 {
            if let Some(line) = self.lines.get_mut(self.cursor.line) {
                self.cursor.column -= 1;
                line.text.remove(self.cursor.column);
                self.modified = true;
            }
        } else if self.cursor.line > 0 {
            let current_line = self.lines.remove(self.cursor.line);
            self.cursor.line -= 1;
            if let Some(prev_line) = self.lines.get_mut(self.cursor.line) {
                self.cursor.column = prev_line.text.len();
                prev_line.text.push_str(&current_line.text);
            }
            self.modified = true;
        }
    }

    /// Delete character at cursor
    pub fn delete(&mut self) {
        self.save_undo();
        
        let line_len = self.lines.get(self.cursor.line)
            .map(|l| l.text.len())
            .unwrap_or(0);
        
        if self.cursor.column < line_len {
            // Delete character at current position
            if let Some(line) = self.lines.get_mut(self.cursor.line) {
                line.text.remove(self.cursor.column);
                self.modified = true;
            }
        } else if self.cursor.line + 1 < self.lines.len() {
            // Join with next line
            let next_line = self.lines.remove(self.cursor.line + 1);
            if let Some(line) = self.lines.get_mut(self.cursor.line) {
                line.text.push_str(&next_line.text);
                self.modified = true;
            }
        }
    }

    /// Move cursor
    pub fn move_cursor(&mut self, direction: CursorMove, extend_selection: bool) {
        if extend_selection && self.selection.is_none() {
            self.selection = Some(Selection {
                start: self.cursor.clone(),
                end: self.cursor.clone(),
            });
        }

        match direction {
            CursorMove::Left => {
                if self.cursor.column > 0 {
                    self.cursor.column -= 1;
                } else if self.cursor.line > 0 {
                    self.cursor.line -= 1;
                    self.cursor.column = self.lines.get(self.cursor.line)
                        .map(|l| l.text.len())
                        .unwrap_or(0);
                }
            }
            CursorMove::Right => {
                let line_len = self.lines.get(self.cursor.line)
                    .map(|l| l.text.len())
                    .unwrap_or(0);
                if self.cursor.column < line_len {
                    self.cursor.column += 1;
                } else if self.cursor.line + 1 < self.lines.len() {
                    self.cursor.line += 1;
                    self.cursor.column = 0;
                }
            }
            CursorMove::Up => {
                if self.cursor.line > 0 {
                    self.cursor.line -= 1;
                    let line_len = self.lines.get(self.cursor.line)
                        .map(|l| l.text.len())
                        .unwrap_or(0);
                    self.cursor.column = self.cursor.column.min(line_len);
                }
            }
            CursorMove::Down => {
                if self.cursor.line + 1 < self.lines.len() {
                    self.cursor.line += 1;
                    let line_len = self.lines.get(self.cursor.line)
                        .map(|l| l.text.len())
                        .unwrap_or(0);
                    self.cursor.column = self.cursor.column.min(line_len);
                }
            }
            CursorMove::LineStart => {
                self.cursor.column = 0;
            }
            CursorMove::LineEnd => {
                self.cursor.column = self.lines.get(self.cursor.line)
                    .map(|l| l.text.len())
                    .unwrap_or(0);
            }
            CursorMove::DocumentStart => {
                self.cursor.line = 0;
                self.cursor.column = 0;
            }
            CursorMove::DocumentEnd => {
                self.cursor.line = self.lines.len().saturating_sub(1);
                self.cursor.column = self.lines.last()
                    .map(|l| l.text.len())
                    .unwrap_or(0);
            }
        }

        if extend_selection {
            if let Some(sel) = &mut self.selection {
                sel.end = self.cursor.clone();
            }
        } else {
            self.selection = None;
        }
    }

    /// Select all
    pub fn select_all(&mut self) {
        self.selection = Some(Selection {
            start: CursorPosition { line: 0, column: 0 },
            end: CursorPosition {
                line: self.lines.len().saturating_sub(1),
                column: self.lines.last().map(|l| l.text.len()).unwrap_or(0),
            },
        });
        self.cursor = self.selection.as_ref().unwrap().end.clone();
    }

    /// Get selected text
    pub fn selected_text(&self) -> Option<String> {
        let sel = self.selection.as_ref()?;
        let (start, end) = if (sel.start.line, sel.start.column) <= (sel.end.line, sel.end.column) {
            (&sel.start, &sel.end)
        } else {
            (&sel.end, &sel.start)
        };

        if start.line == end.line {
            self.lines.get(start.line).map(|l| {
                l.text[start.column..end.column].to_string()
            })
        } else {
            let mut result = String::new();
            for i in start.line..=end.line {
                if let Some(line) = self.lines.get(i) {
                    if i == start.line {
                        result.push_str(&line.text[start.column..]);
                    } else if i == end.line {
                        result.push_str(&line.text[..end.column]);
                    } else {
                        result.push_str(&line.text);
                    }
                    if i < end.line {
                        result.push('\n');
                    }
                }
            }
            Some(result)
        }
    }

    /// Delete selection
    pub fn delete_selection(&mut self) {
        if let Some(sel) = self.selection.take() {
            self.save_undo();
            
            let (start, end) = if (sel.start.line, sel.start.column) <= (sel.end.line, sel.end.column) {
                (sel.start, sel.end)
            } else {
                (sel.end, sel.start)
            };

            if start.line == end.line {
                if let Some(line) = self.lines.get_mut(start.line) {
                    line.text = alloc::format!(
                        "{}{}",
                        &line.text[..start.column],
                        &line.text[end.column..]
                    );
                }
            } else {
                if let (Some(start_line), Some(end_line)) = (
                    self.lines.get(start.line).map(|l| l.text[..start.column].to_string()),
                    self.lines.get(end.line).map(|l| l.text[end.column..].to_string()),
                ) {
                    // Remove lines in between
                    for _ in start.line..=end.line {
                        if start.line < self.lines.len() {
                            self.lines.remove(start.line);
                        }
                    }
                    // Insert merged line
                    self.lines.insert(start.line, Line::new(alloc::format!("{}{}", start_line, end_line)));
                }
            }

            self.cursor = start;
            self.modified = true;
        }
    }

    /// Copy
    pub fn copy(&self) -> Option<String> {
        self.selected_text()
    }

    /// Cut
    pub fn cut(&mut self) -> Option<String> {
        let text = self.selected_text();
        self.delete_selection();
        text
    }

    /// Paste
    pub fn paste(&mut self, text: &str) {
        if self.selection.is_some() {
            self.delete_selection();
        }
        self.insert_text(text);
    }

    /// Save undo state
    fn save_undo(&mut self) {
        self.undo_stack.push(Edit {
            content: self.content.clone(),
            cursor: self.cursor.clone(),
        });
        self.redo_stack.clear();
        
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    /// Undo
    pub fn undo(&mut self) {
        if let Some(edit) = self.undo_stack.pop() {
            self.redo_stack.push(Edit {
                content: self.content.clone(),
                cursor: self.cursor.clone(),
            });
            self.content = edit.content;
            self.cursor = edit.cursor;
            self.rebuild_lines();
        }
    }

    /// Redo
    pub fn redo(&mut self) {
        if let Some(edit) = self.redo_stack.pop() {
            self.undo_stack.push(Edit {
                content: self.content.clone(),
                cursor: self.cursor.clone(),
            });
            self.content = edit.content;
            self.cursor = edit.cursor;
            self.rebuild_lines();
        }
    }

    /// Rebuild lines from content
    fn rebuild_lines(&mut self) {
        self.lines = self.content
            .lines()
            .map(|l| Line::new(l.to_string()))
            .collect();
        if self.lines.is_empty() {
            self.lines.push(Line::new(String::new()));
        }
    }

    /// Update content from lines
    pub fn update_content(&mut self) {
        self.content = self.lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
    }

    /// Get line count
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

/// Line
#[derive(Debug, Clone)]
pub struct Line {
    /// Text content
    pub text: String,
    /// Syntax tokens (for highlighting)
    pub tokens: Vec<Token>,
}

impl Line {
    pub fn new(text: String) -> Self {
        Self {
            text,
            tokens: Vec::new(),
        }
    }
}

/// Syntax token
#[derive(Debug, Clone)]
pub struct Token {
    pub start: usize,
    pub end: usize,
    pub token_type: TokenType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    Keyword,
    String,
    Number,
    Comment,
    Operator,
    Punctuation,
    Function,
    Type,
    Variable,
    Constant,
    Attribute,
}

/// Cursor position
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorPosition {
    pub line: usize,
    pub column: usize,
}

/// Selection
#[derive(Debug, Clone)]
pub struct Selection {
    pub start: CursorPosition,
    pub end: CursorPosition,
}

/// Cursor movement
#[derive(Debug, Clone, Copy)]
pub enum CursorMove {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
    DocumentStart,
    DocumentEnd,
}

/// Edit for undo/redo
#[derive(Debug, Clone)]
pub struct Edit {
    pub content: String,
    pub cursor: CursorPosition,
}

/// Save request
#[derive(Debug, Clone)]
pub struct SaveRequest {
    pub path: String,
    pub content: String,
}

/// Find/replace state
#[derive(Debug, Clone)]
pub struct FindReplace {
    pub query: String,
    pub replacement: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
    pub with_replace: bool,
    pub results: Vec<FindResult>,
    pub current_result: usize,
}

/// Find result
#[derive(Debug, Clone)]
pub struct FindResult {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

/// Language
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    PlainText,
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Html,
    Css,
    Json,
    Markdown,
    C,
    Cpp,
    Java,
    Go,
    Shell,
    Toml,
    Yaml,
    Xml,
    Sql,
}

impl Language {
    pub fn name(&self) -> &'static str {
        match self {
            Self::PlainText => "Plain Text",
            Self::Rust => "Rust",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Python => "Python",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Json => "JSON",
            Self::Markdown => "Markdown",
            Self::C => "C",
            Self::Cpp => "C++",
            Self::Java => "Java",
            Self::Go => "Go",
            Self::Shell => "Shell",
            Self::Toml => "TOML",
            Self::Yaml => "YAML",
            Self::Xml => "XML",
            Self::Sql => "SQL",
        }
    }
}

/// Editor settings
#[derive(Debug, Clone)]
pub struct EditorSettings {
    /// Font family
    pub font_family: String,
    /// Font size
    pub font_size: u32,
    /// Tab size
    pub tab_size: u32,
    /// Insert spaces
    pub insert_spaces: bool,
    /// Word wrap
    pub word_wrap: bool,
    /// Line numbers
    pub line_numbers: bool,
    /// Highlight current line
    pub highlight_line: bool,
    /// Auto-indent
    pub auto_indent: bool,
    /// Auto-closing brackets
    pub auto_close_brackets: bool,
    /// Minimap
    pub minimap: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            font_family: String::from("monospace"),
            font_size: 14,
            tab_size: 4,
            insert_spaces: true,
            word_wrap: false,
            line_numbers: true,
            highlight_line: true,
            auto_indent: true,
            auto_close_brackets: true,
            minimap: true,
        }
    }
}
