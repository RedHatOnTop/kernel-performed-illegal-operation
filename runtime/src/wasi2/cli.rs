// WASI Preview 2 — CLI (wasi:cli)
//
// Provides stdin/stdout/stderr handle access, environment variables,
// and exit functionality for WASI P2 components.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

/// CLI environment configuration.
pub struct CliEnvironment {
    /// Environment variable key-value pairs.
    vars: Vec<(String, String)>,
    /// Command-line arguments.
    args: Vec<String>,
}

impl CliEnvironment {
    /// Create an empty environment.
    pub fn new() -> Self {
        Self {
            vars: Vec::new(),
            args: Vec::new(),
        }
    }

    /// Set environment variables.
    pub fn set_vars(&mut self, vars: Vec<(String, String)>) {
        self.vars = vars;
    }

    /// Set command-line arguments.
    pub fn set_args(&mut self, args: Vec<String>) {
        self.args = args;
    }

    /// Get environment variables (wasi:cli/environment.get-environment).
    pub fn get_environment(&self) -> &[(String, String)] {
        &self.vars
    }

    /// Get command-line arguments (wasi:cli/environment.get-arguments).
    pub fn get_arguments(&self) -> &[String] {
        &self.args
    }

    /// Get initial working directory (wasi:cli/environment.initial-cwd).
    pub fn initial_cwd(&self) -> Option<&str> {
        // In our kernel environment, we always start at "/"
        Some("/")
    }
}

// ---------------------------------------------------------------------------
// Stdin / Stdout / Stderr
// ---------------------------------------------------------------------------

/// Standard stream identifiers for the CLI interface.
///
/// These map to pre-created resource handles in `Wasi2Ctx`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdioStream {
    Stdin,
    Stdout,
    Stderr,
}

/// Get the resource handle index for stdin (wasi:cli/stdin.get-stdin).
///
/// Returns 0 — the first pre-allocated handle in Wasi2Ctx.
pub fn get_stdin_handle() -> u32 {
    0
}

/// Get the resource handle index for stdout (wasi:cli/stdout.get-stdout).
///
/// Returns 1 — the second pre-allocated handle in Wasi2Ctx.
pub fn get_stdout_handle() -> u32 {
    1
}

/// Get the resource handle index for stderr (wasi:cli/stderr.get-stderr).
///
/// Returns 2 — the third pre-allocated handle in Wasi2Ctx.
pub fn get_stderr_handle() -> u32 {
    2
}

// ---------------------------------------------------------------------------
// Exit
// ---------------------------------------------------------------------------

/// Exit status for wasi:cli/exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitStatus {
    /// Normal exit with code.
    Code(u32),
    /// Panic / trap exit.
    Trap,
}

/// Request to exit the program (wasi:cli/exit.exit).
///
/// Returns the exit status — the caller is responsible for halting
/// execution based on this value.
pub fn exit(status: u32) -> ExitStatus {
    ExitStatus::Code(status)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_environment() {
        let env = CliEnvironment::new();
        assert!(env.get_environment().is_empty());
        assert!(env.get_arguments().is_empty());
    }

    #[test]
    fn set_and_get_vars() {
        let mut env = CliEnvironment::new();
        env.set_vars(alloc::vec![
            (String::from("HOME"), String::from("/")),
            (String::from("PATH"), String::from("/bin")),
        ]);
        let vars = env.get_environment();
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].0, "HOME");
        assert_eq!(vars[0].1, "/");
        assert_eq!(vars[1].0, "PATH");
        assert_eq!(vars[1].1, "/bin");
    }

    #[test]
    fn set_and_get_args() {
        let mut env = CliEnvironment::new();
        env.set_args(alloc::vec![
            String::from("myapp"),
            String::from("--verbose"),
        ]);
        let args = env.get_arguments();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "myapp");
        assert_eq!(args[1], "--verbose");
    }

    #[test]
    fn initial_cwd_is_root() {
        let env = CliEnvironment::new();
        assert_eq!(env.initial_cwd(), Some("/"));
    }

    #[test]
    fn stdio_handles() {
        assert_eq!(get_stdin_handle(), 0);
        assert_eq!(get_stdout_handle(), 1);
        assert_eq!(get_stderr_handle(), 2);
    }

    #[test]
    fn exit_code() {
        assert_eq!(exit(0), ExitStatus::Code(0));
        assert_eq!(exit(1), ExitStatus::Code(1));
    }

    #[test]
    fn exit_status_equality() {
        assert_ne!(ExitStatus::Code(0), ExitStatus::Trap);
        assert_ne!(ExitStatus::Code(0), ExitStatus::Code(1));
    }
}
