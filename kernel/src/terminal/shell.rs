//! Shell Engine
//!
//! Line-oriented shell with command parsing, environment variables,
//! history, piping (`|`), redirection (`>`, `>>`, `<`), chaining
//! (`;`, `&&`, `||`), quoting, and glob expansion.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use super::fs;

// ────────────────────────── Types ──────────────────────────

/// A parsed single command segment.
#[derive(Debug, Clone)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
}

/// How commands are chained.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Chain {
    /// Always run next
    Sequence, // ;
    /// Run next only if prev succeeded
    And, // &&
    /// Run next only if prev failed
    Or, // ||
    /// Pipe stdout → stdin
    Pipe, // |
}

/// Redirection target.
#[derive(Debug, Clone)]
pub enum Redirect {
    /// > file
    StdoutOverwrite(String),
    /// >> file
    StdoutAppend(String),
    /// < file
    StdinFrom(String),
}

/// A complete parsed pipeline segment (command + redirections).
#[derive(Debug, Clone)]
pub struct Segment {
    pub command: Command,
    pub redirects: Vec<Redirect>,
}

/// A full parsed command line: segments with chain operators.
#[derive(Debug, Clone)]
pub struct Pipeline {
    pub segments: Vec<(Segment, Option<Chain>)>,
}

// ────────────────────────── Shell State ──────────────────────────

pub struct ShellState {
    pub env: BTreeMap<String, String>,
    pub cwd: String,
    pub history: Vec<String>,
    pub history_max: usize,
    pub aliases: BTreeMap<String, String>,
    pub last_exit_code: i32,
}

static SHELL: Mutex<Option<ShellState>> = Mutex::new(None);

pub fn init() {
    let mut env = BTreeMap::new();
    env.insert(String::from("HOME"), String::from("/home/root"));
    env.insert(String::from("USER"), String::from("root"));
    env.insert(String::from("HOSTNAME"), String::from("kpio"));
    env.insert(String::from("PATH"), String::from("/usr/bin:/bin:/sbin"));
    env.insert(String::from("PWD"), String::from("/home/root"));
    env.insert(String::from("SHELL"), String::from("/bin/sh"));
    env.insert(String::from("TERM"), String::from("kpio-term"));
    env.insert(String::from("LANG"), String::from("en_US.UTF-8"));
    env.insert(String::from("OLDPWD"), String::from("/"));

    let mut aliases = BTreeMap::new();
    aliases.insert(String::from("ll"), String::from("ls -la"));
    aliases.insert(String::from("la"), String::from("ls -a"));
    aliases.insert(String::from("l"), String::from("ls -CF"));
    aliases.insert(String::from(".."), String::from("cd .."));
    aliases.insert(String::from("..."), String::from("cd ../.."));
    aliases.insert(String::from("cls"), String::from("clear"));

    *SHELL.lock() = Some(ShellState {
        env,
        cwd: String::from("/home/root"),
        history: Vec::new(),
        history_max: 200,
        aliases,
        last_exit_code: 0,
    });
}

/// Run a closure with the global shell state.
pub fn with_shell<F, R>(f: F) -> R
where
    F: FnOnce(&mut ShellState) -> R,
{
    let mut guard = SHELL.lock();
    let shell = guard
        .as_mut()
        .expect("Shell not initialised — call terminal::shell::init()");
    f(shell)
}

impl ShellState {
    // ── Prompt ──────────────────────────────────────────────

    pub fn prompt(&self) -> String {
        let user = self.env.get("USER").map(|s| s.as_str()).unwrap_or("user");
        let host = self
            .env
            .get("HOSTNAME")
            .map(|s| s.as_str())
            .unwrap_or("kpio");
        let home = self
            .env
            .get("HOME")
            .map(|s| s.as_str())
            .unwrap_or("/home/root");

        let display_cwd = if self.cwd == home {
            String::from("~")
        } else if self.cwd.starts_with(home) {
            alloc::format!("~{}", &self.cwd[home.len()..])
        } else {
            self.cwd.clone()
        };

        let sigil = if user == "root" { '#' } else { '$' };
        alloc::format!("{}@{}:{}{} ", user, host, display_cwd, sigil)
    }

    // ── History ─────────────────────────────────────────────

    pub fn add_history(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        // Don't duplicate consecutive entries
        if self.history.last().map(|s| s.as_str()) == Some(line) {
            return;
        }
        if self.history.len() >= self.history_max {
            self.history.remove(0);
        }
        self.history.push(String::from(line));
    }

    // ── Path resolution ─────────────────────────────────────

    /// Resolve a path relative to cwd. Returns absolute path.
    pub fn resolve_path(&self, path: &str) -> String {
        if path.starts_with('/') {
            normalise_path(path)
        } else if path.starts_with("~/") {
            let home = self
                .env
                .get("HOME")
                .map(|s| s.as_str())
                .unwrap_or("/home/root");
            normalise_path(&alloc::format!("{}/{}", home, &path[2..]))
        } else if path == "~" {
            self.env
                .get("HOME")
                .cloned()
                .unwrap_or_else(|| String::from("/home/root"))
        } else {
            normalise_path(&alloc::format!("{}/{}", self.cwd, path))
        }
    }

    // ── cd ──────────────────────────────────────────────────

    pub fn change_dir(&mut self, path: &str) -> Result<(), String> {
        let target = if path == "-" {
            self.env
                .get("OLDPWD")
                .cloned()
                .unwrap_or_else(|| String::from("/"))
        } else {
            self.resolve_path(path)
        };

        // Verify target is a directory
        let exists = fs::with_fs(|fs| {
            if let Some(ino) = fs.resolve(&target) {
                fs.get(ino).map(|n| n.mode.is_dir()).unwrap_or(false)
            } else {
                false
            }
        });

        if !exists {
            return Err(alloc::format!("cd: {}: No such file or directory", path));
        }

        let old = self.cwd.clone();
        self.cwd = target.clone();
        self.env.insert(String::from("PWD"), target);
        self.env.insert(String::from("OLDPWD"), old);
        Ok(())
    }

    // ── Variable expansion ──────────────────────────────────

    pub fn expand_variables(&self, input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '$' && i + 1 < chars.len() {
                i += 1;
                if chars[i] == '?' {
                    // $? → last exit code
                    result.push_str(&alloc::format!("{}", self.last_exit_code));
                    i += 1;
                } else if chars[i] == '{' {
                    // ${VAR}
                    i += 1;
                    let start = i;
                    while i < chars.len() && chars[i] != '}' {
                        i += 1;
                    }
                    let name: String = chars[start..i].iter().collect();
                    if let Some(val) = self.env.get(&name) {
                        result.push_str(val);
                    }
                    if i < chars.len() {
                        i += 1;
                    } // skip '}'
                } else {
                    // $VAR
                    let start = i;
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        i += 1;
                    }
                    let name: String = chars[start..i].iter().collect();
                    if let Some(val) = self.env.get(&name) {
                        result.push_str(val);
                    }
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }
        result
    }

    // ── Alias expansion ─────────────────────────────────────

    pub fn expand_alias(&self, line: &str) -> String {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return String::from(trimmed);
        }

        // Only expand the first word
        let first_space = trimmed.find(' ');
        let first_word = if let Some(sp) = first_space {
            &trimmed[..sp]
        } else {
            trimmed
        };
        let rest = if let Some(sp) = first_space {
            &trimmed[sp..]
        } else {
            ""
        };

        if let Some(expansion) = self.aliases.get(first_word) {
            alloc::format!("{}{}", expansion, rest)
        } else {
            String::from(trimmed)
        }
    }
}

// ────────────────────────── Parser ──────────────────────────

/// Parse a command line into a pipeline structure.
pub fn parse(input: &str) -> Pipeline {
    let tokens = tokenise(input);
    let mut segments: Vec<(Segment, Option<Chain>)> = Vec::new();
    let mut current_args: Vec<String> = Vec::new();
    let mut redirects: Vec<Redirect> = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        match tokens[i].as_str() {
            "|" => {
                if !current_args.is_empty() {
                    let seg = build_segment(&mut current_args, &mut redirects);
                    segments.push((seg, Some(Chain::Pipe)));
                }
                i += 1;
            }
            ";" => {
                if !current_args.is_empty() {
                    let seg = build_segment(&mut current_args, &mut redirects);
                    segments.push((seg, Some(Chain::Sequence)));
                }
                i += 1;
            }
            "&&" => {
                if !current_args.is_empty() {
                    let seg = build_segment(&mut current_args, &mut redirects);
                    segments.push((seg, Some(Chain::And)));
                }
                i += 1;
            }
            "||" => {
                if !current_args.is_empty() {
                    let seg = build_segment(&mut current_args, &mut redirects);
                    segments.push((seg, Some(Chain::Or)));
                }
                i += 1;
            }
            ">" => {
                if i + 1 < tokens.len() {
                    redirects.push(Redirect::StdoutOverwrite(tokens[i + 1].clone()));
                    i += 2;
                } else {
                    i += 1;
                }
            }
            ">>" => {
                if i + 1 < tokens.len() {
                    redirects.push(Redirect::StdoutAppend(tokens[i + 1].clone()));
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "<" => {
                if i + 1 < tokens.len() {
                    redirects.push(Redirect::StdinFrom(tokens[i + 1].clone()));
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => {
                current_args.push(tokens[i].clone());
                i += 1;
            }
        }
    }

    // Final segment
    if !current_args.is_empty() {
        let seg = build_segment(&mut current_args, &mut redirects);
        segments.push((seg, None));
    }

    Pipeline { segments }
}

fn build_segment(args: &mut Vec<String>, redirects: &mut Vec<Redirect>) -> Segment {
    let program = args.remove(0);
    let seg = Segment {
        command: Command {
            program,
            args: args.clone(),
        },
        redirects: redirects.clone(),
    };
    args.clear();
    redirects.clear();
    seg
}

/// Tokenise input handling quotes and special operators.
fn tokenise(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Skip whitespace
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }

        // Two-character operators
        if i + 1 < chars.len() {
            let two: String = chars[i..=i + 1].iter().collect();
            match two.as_str() {
                "&&" | "||" | ">>" => {
                    tokens.push(two);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }

        // Single-character operators
        match chars[i] {
            '|' | ';' | '>' | '<' => {
                tokens.push(String::from(chars[i]));
                i += 1;
                continue;
            }
            _ => {}
        }

        // Quoted string
        if chars[i] == '"' || chars[i] == '\'' {
            let quote = chars[i];
            i += 1;
            let mut s = String::new();
            while i < chars.len() && chars[i] != quote {
                if chars[i] == '\\' && quote == '"' && i + 1 < chars.len() {
                    i += 1;
                    match chars[i] {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        '\\' => s.push('\\'),
                        '"' => s.push('"'),
                        c => {
                            s.push('\\');
                            s.push(c);
                        }
                    }
                } else {
                    s.push(chars[i]);
                }
                i += 1;
            }
            if i < chars.len() {
                i += 1;
            } // closing quote
            tokens.push(s);
            continue;
        }

        // Regular word
        let mut word = String::new();
        while i < chars.len()
            && !chars[i].is_whitespace()
            && !matches!(chars[i], '|' | ';' | '>' | '<')
        {
            if chars[i] == '\\' && i + 1 < chars.len() {
                i += 1;
                word.push(chars[i]);
            } else {
                word.push(chars[i]);
            }
            i += 1;
        }
        if !word.is_empty() {
            tokens.push(word);
        }
    }

    tokens
}

// ────────────────────────── Execute ──────────────────────────

/// Execute a full command line. Returns output lines.
pub fn execute(line: &str) -> Vec<String> {
    // Save to history
    with_shell(|sh| sh.add_history(line));

    // Alias + variable expansion
    let expanded = with_shell(|sh| {
        let aliased = sh.expand_alias(line);
        sh.expand_variables(&aliased)
    });

    let pipeline = parse(&expanded);
    let mut output = Vec::new();
    let mut pipe_input: Option<Vec<String>> = None;
    let mut last_ok = true;

    for (i, (segment, chain)) in pipeline.segments.iter().enumerate() {
        // Check chain condition
        if i > 0 {
            if let Some((_, prev_chain)) = pipeline.segments.get(i - 1) {
                match prev_chain {
                    Some(Chain::And) if !last_ok => continue,
                    Some(Chain::Or) if last_ok => continue,
                    _ => {}
                }
            }
        }

        // Handle stdin redirect
        let stdin_content = segment.redirects.iter().find_map(|r| {
            if let Redirect::StdinFrom(path) = r {
                let abs = with_shell(|sh| sh.resolve_path(path));
                fs::with_fs(|fs| fs.resolve(&abs).and_then(|ino| fs.read_file(ino).ok())).map(
                    |data| {
                        String::from_utf8_lossy(&data)
                            .lines()
                            .map(String::from)
                            .collect::<Vec<_>>()
                    },
                )
            } else {
                None
            }
        });

        let input = stdin_content.or(pipe_input.take());

        // Execute the command
        let result = super::commands::execute_command(
            &segment.command.program,
            &segment.command.args,
            input.as_deref(),
        );

        last_ok = result.success;

        // Handle stdout redirections
        let mut redirected = false;
        for redir in &segment.redirects {
            match redir {
                Redirect::StdoutOverwrite(path) => {
                    let abs = with_shell(|sh| sh.resolve_path(path));
                    let data = result.output.join("\n");
                    let data_bytes = if data.is_empty() {
                        Vec::new()
                    } else {
                        let mut b = data.into_bytes();
                        b.push(b'\n');
                        b
                    };
                    fs::with_fs(|fs| {
                        if let Some((parent, name)) = fs.resolve_parent(&abs) {
                            let _ = fs.create_file(parent, &name, &data_bytes);
                        }
                    });
                    redirected = true;
                }
                Redirect::StdoutAppend(path) => {
                    let abs = with_shell(|sh| sh.resolve_path(path));
                    let data = result.output.join("\n");
                    let data_bytes = if data.is_empty() {
                        Vec::new()
                    } else {
                        let mut b = data.into_bytes();
                        b.push(b'\n');
                        b
                    };
                    fs::with_fs(|fs| {
                        if let Some(ino) = fs.resolve(&abs) {
                            let _ = fs.append_file(ino, &data_bytes);
                        } else if let Some((parent, name)) = fs.resolve_parent(&abs) {
                            let _ = fs.create_file(parent, &name, &data_bytes);
                        }
                    });
                    redirected = true;
                }
                _ => {}
            }
        }

        // If piped, pass output to next; otherwise collect
        match chain {
            Some(Chain::Pipe) => {
                pipe_input = Some(result.output);
            }
            _ => {
                if !redirected {
                    output.extend(result.output);
                }
            }
        }
    }

    with_shell(|sh| {
        sh.last_exit_code = if last_ok { 0 } else { 1 };
    });

    output
}

// ────────────────────────── Tab Completion ──────────────────────────

/// Generate tab-completion candidates for partial input.
pub fn complete(input: &str) -> Vec<String> {
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.is_empty() || (parts.len() == 1 && !input.ends_with(' ')) {
        // Complete command name
        let prefix = parts.first().copied().unwrap_or("");
        let mut candidates: Vec<String> = super::commands::COMMAND_LIST
            .iter()
            .filter(|c| c.starts_with(prefix))
            .map(|c| String::from(*c))
            .collect();
        // aliases
        with_shell(|sh| {
            for name in sh.aliases.keys() {
                if name.starts_with(prefix) {
                    candidates.push(name.clone());
                }
            }
        });
        candidates.sort();
        candidates.dedup();
        candidates
    } else {
        // Complete file/dir name
        let partial = if input.ends_with(' ') {
            ""
        } else {
            parts.last().unwrap_or(&"")
        };
        complete_path(partial)
    }
}

/// Try tab-completing the input line. Returns the completed line if exactly
/// one candidate matches, or `None` if zero or multiple.
pub fn tab_complete(input: &str) -> Option<String> {
    let candidates = complete(input);

    if candidates.len() == 1 {
        // Replace the last word with the completion
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() || (parts.len() == 1 && !input.ends_with(' ')) {
            // Command completion — just return the command + space
            Some(alloc::format!("{} ", candidates[0]))
        } else {
            // File/path completion — rebuild with completed token
            let last_start = input
                .rfind(parts.last().unwrap_or(&""))
                .unwrap_or(input.len());
            let prefix = &input[..last_start];
            Some(alloc::format!("{}{}", prefix, candidates[0]))
        }
    } else {
        None
    }
}

fn complete_path(partial: &str) -> Vec<String> {
    let (dir_path, prefix) = if partial.contains('/') {
        let idx = partial.rfind('/').unwrap();
        let dir = if idx == 0 {
            String::from("/")
        } else {
            String::from(&partial[..idx])
        };
        let pfx = &partial[idx + 1..];
        (dir, String::from(pfx))
    } else {
        (with_shell(|sh| sh.cwd.clone()), String::from(partial))
    };

    let abs_dir = with_shell(|sh| sh.resolve_path(&dir_path));

    fs::with_fs(|fs| {
        let ino = match fs.resolve(&abs_dir) {
            Some(i) => i,
            None => return Vec::new(),
        };
        let entries = match fs.readdir(ino) {
            Some(e) => e,
            None => return Vec::new(),
        };
        entries
            .iter()
            .filter(|(name, _)| name.starts_with(&prefix))
            .map(|(name, child_ino)| {
                let is_dir = fs.get(*child_ino).map(|n| n.mode.is_dir()).unwrap_or(false);
                if is_dir {
                    alloc::format!("{}/", name)
                } else {
                    name.clone()
                }
            })
            .collect()
    })
}

// ────────────────────────── Helpers ──────────────────────────

/// Normalise a path: collapse `/./`, `/../`, `//`.
pub fn normalise_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            c => parts.push(c),
        }
    }
    if parts.is_empty() {
        String::from("/")
    } else {
        let mut result = String::new();
        for p in &parts {
            result.push('/');
            result.push_str(p);
        }
        result
    }
}
