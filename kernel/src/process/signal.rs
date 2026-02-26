//! Signal Handling Infrastructure
//!
//! Provides a minimal POSIX signal subsystem supporting SIGKILL, SIGTERM,
//! SIGCHLD, SIGSEGV, and SIGINT.  Signal delivery is checked on return
//! to user-space.

use alloc::vec::Vec;

// ─── Signal numbers (Linux x86_64) ──────────────────────────────────

pub const SIGHUP: u8 = 1;
pub const SIGINT: u8 = 2;
pub const SIGQUIT: u8 = 3;
pub const SIGILL: u8 = 4;
pub const SIGTRAP: u8 = 5;
pub const SIGABRT: u8 = 6;
pub const SIGBUS: u8 = 7;
pub const SIGFPE: u8 = 8;
pub const SIGKILL: u8 = 9;
pub const SIGUSR1: u8 = 10;
pub const SIGSEGV: u8 = 11;
pub const SIGUSR2: u8 = 12;
pub const SIGPIPE: u8 = 13;
pub const SIGALRM: u8 = 14;
pub const SIGTERM: u8 = 15;
pub const SIGCHLD: u8 = 17;
pub const SIGCONT: u8 = 18;
pub const SIGSTOP: u8 = 19;
pub const SIGTSTP: u8 = 20;

/// Maximum signal number we handle.
pub const NSIG: usize = 64;

// ─── sigprocmask how values ─────────────────────────────────────────

pub const SIG_BLOCK: i32 = 0;
pub const SIG_UNBLOCK: i32 = 1;
pub const SIG_SETMASK: i32 = 2;

// ─── Signal action disposition ──────────────────────────────────────

/// SIG_DFL sentinel
pub const SIG_DFL: u64 = 0;
/// SIG_IGN sentinel
pub const SIG_IGN: u64 = 1;

/// Per-signal action (matches `struct sigaction` layout conceptually).
#[derive(Debug, Clone, Copy)]
pub struct SignalAction {
    /// Handler address: SIG_DFL (0), SIG_IGN (1), or user function address.
    pub handler: u64,
    /// Signal mask to apply during handler execution.
    pub mask: u64,
    /// Flags (SA_RESTART, SA_SIGINFO, etc.) — mostly stubs for now.
    pub flags: u64,
    /// Restorer function pointer (used by user-space signal return trampoline).
    pub restorer: u64,
}

impl Default for SignalAction {
    fn default() -> Self {
        SignalAction {
            handler: SIG_DFL,
            mask: 0,
            flags: 0,
            restorer: 0,
        }
    }
}

/// Per-process signal state.
#[derive(Debug, Clone)]
pub struct SignalState {
    /// Bitmask of pending signals (bit N = signal N is pending).
    pub pending: u64,
    /// Bitmask of blocked signals (sigprocmask).
    pub blocked: u64,
    /// Per-signal disposition table (index = signal number).
    pub handlers: Vec<SignalAction>,
}

impl SignalState {
    /// Create a new signal state with all handlers set to SIG_DFL.
    pub fn new() -> Self {
        let mut handlers = Vec::with_capacity(NSIG);
        for _ in 0..NSIG {
            handlers.push(SignalAction::default());
        }
        SignalState {
            pending: 0,
            blocked: 0,
            handlers,
        }
    }

    /// Queue a signal for delivery.
    pub fn send_signal(&mut self, signum: u8) {
        if (signum as usize) < NSIG {
            self.pending |= 1u64 << signum;
        }
    }

    /// Check if a signal is pending and not blocked.
    pub fn has_deliverable_signal(&self) -> bool {
        let deliverable = self.pending & !self.blocked;
        deliverable != 0
    }

    /// Get the next deliverable signal (lowest numbered).
    ///
    /// Returns `Some(signum)` and clears it from pending, or `None`.
    pub fn dequeue_signal(&mut self) -> Option<u8> {
        let deliverable = self.pending & !self.blocked;
        if deliverable == 0 {
            return None;
        }
        // Find lowest set bit
        let signum = deliverable.trailing_zeros() as u8;
        self.pending &= !(1u64 << signum);
        Some(signum)
    }

    /// Get the action for a signal.
    pub fn get_action(&self, signum: u8) -> &SignalAction {
        if (signum as usize) < NSIG {
            &self.handlers[signum as usize]
        } else {
            &self.handlers[0] // fallback
        }
    }

    /// Set the action for a signal.
    ///
    /// Returns the old action.
    pub fn set_action(&mut self, signum: u8, action: SignalAction) -> SignalAction {
        if (signum as usize) < NSIG {
            let old = self.handlers[signum as usize];
            self.handlers[signum as usize] = action;
            old
        } else {
            SignalAction::default()
        }
    }

    /// Reset all signal handlers to default (used by execve).
    pub fn reset_handlers(&mut self) {
        for action in &mut self.handlers {
            // Only reset non-ignored handlers; SIG_IGN is preserved across exec
            // for some signals, but for simplicity, reset all to DFL.
            *action = SignalAction::default();
        }
        self.pending = 0;
    }

    /// Apply sigprocmask operation.
    pub fn sigprocmask(&mut self, how: i32, set: u64) {
        match how {
            SIG_BLOCK => self.blocked |= set,
            SIG_UNBLOCK => self.blocked &= !set,
            SIG_SETMASK => self.blocked = set,
            _ => {}
        }
        // SIGKILL and SIGSTOP can never be blocked
        self.blocked &= !(1u64 << SIGKILL);
        self.blocked &= !(1u64 << SIGSTOP);
    }
}

impl Default for SignalState {
    fn default() -> Self {
        Self::new()
    }
}

/// Determine the default action for a signal.
///
/// Returns `true` if the default action is to terminate the process.
pub fn default_action_is_terminate(signum: u8) -> bool {
    matches!(
        signum,
        SIGHUP | SIGINT | SIGQUIT | SIGILL | SIGTRAP | SIGABRT | SIGBUS | SIGFPE | SIGKILL
            | SIGUSR1
            | SIGSEGV
            | SIGUSR2
            | SIGPIPE
            | SIGALRM
            | SIGTERM
    )
}

/// Determine if the default action is to ignore the signal.
pub fn default_action_is_ignore(signum: u8) -> bool {
    matches!(signum, SIGCHLD | SIGCONT)
}

/// Check if a signal can be caught or ignored.
pub fn is_catchable(signum: u8) -> bool {
    signum != SIGKILL && signum != SIGSTOP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_state_new() {
        let state = SignalState::new();
        assert_eq!(state.pending, 0);
        assert_eq!(state.blocked, 0);
        assert_eq!(state.handlers.len(), NSIG);
    }

    #[test]
    fn test_send_signal() {
        let mut state = SignalState::new();
        state.send_signal(SIGTERM);
        assert!(state.pending & (1u64 << SIGTERM) != 0);
    }

    #[test]
    fn test_dequeue_signal() {
        let mut state = SignalState::new();
        state.send_signal(SIGTERM);
        state.send_signal(SIGINT);
        // SIGINT (2) should dequeue first (lower number)
        let sig = state.dequeue_signal();
        assert_eq!(sig, Some(SIGINT));
        let sig = state.dequeue_signal();
        assert_eq!(sig, Some(SIGTERM));
        let sig = state.dequeue_signal();
        assert_eq!(sig, None);
    }

    #[test]
    fn test_blocked_signals() {
        let mut state = SignalState::new();
        state.send_signal(SIGTERM);
        state.blocked = 1u64 << SIGTERM;
        assert!(!state.has_deliverable_signal());
        assert_eq!(state.dequeue_signal(), None);
        // Unblock
        state.blocked = 0;
        assert!(state.has_deliverable_signal());
        assert_eq!(state.dequeue_signal(), Some(SIGTERM));
    }

    #[test]
    fn test_sigkill_cannot_be_blocked() {
        let mut state = SignalState::new();
        state.sigprocmask(SIG_BLOCK, 1u64 << SIGKILL);
        assert_eq!(state.blocked & (1u64 << SIGKILL), 0);
    }

    #[test]
    fn test_sigprocmask_operations() {
        let mut state = SignalState::new();
        state.sigprocmask(SIG_BLOCK, 1u64 << SIGUSR1);
        assert!(state.blocked & (1u64 << SIGUSR1) != 0);
        state.sigprocmask(SIG_UNBLOCK, 1u64 << SIGUSR1);
        assert_eq!(state.blocked & (1u64 << SIGUSR1), 0);
        state.sigprocmask(SIG_SETMASK, 0xFF00);
        assert_eq!(state.blocked, 0xFF00 & !(1u64 << SIGKILL));
    }

    #[test]
    fn test_catchable() {
        assert!(!is_catchable(SIGKILL));
        assert!(!is_catchable(SIGSTOP));
        assert!(is_catchable(SIGTERM));
        assert!(is_catchable(SIGINT));
    }

    #[test]
    fn test_default_actions() {
        assert!(default_action_is_terminate(SIGKILL));
        assert!(default_action_is_terminate(SIGTERM));
        assert!(default_action_is_terminate(SIGSEGV));
        assert!(default_action_is_ignore(SIGCHLD));
        assert!(!default_action_is_terminate(SIGCHLD));
    }
}
