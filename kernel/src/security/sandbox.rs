//! Sandbox Management
//!
//! This module implements process sandboxing using capability-based
//! security and system call filtering.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use spin::{Mutex, RwLock};

use super::policy::DomainId;
use crate::process::ProcessId;

/// Sandbox identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SandboxId(pub u32);

/// Sandbox state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxState {
    /// Sandbox is being configured.
    Configuring,
    /// Sandbox is active.
    Active,
    /// Sandbox is suspended.
    Suspended,
    /// Sandbox is being terminated.
    Terminating,
    /// Sandbox has exited.
    Exited,
}

/// System call filter action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallAction {
    /// Allow the syscall.
    Allow,
    /// Deny with error code.
    Deny(i32),
    /// Log and allow.
    Audit,
    /// Log and deny.
    AuditDeny(i32),
    /// Trap to handler.
    Trap,
}

/// System call filter rule.
#[derive(Debug, Clone)]
pub struct SyscallRule {
    /// Syscall number.
    pub syscall_nr: u64,
    /// Action to take.
    pub action: SyscallAction,
    /// Argument constraints (optional).
    pub arg_constraints: Option<Vec<ArgConstraint>>,
}

/// Argument constraint for syscall filtering.
#[derive(Debug, Clone)]
pub struct ArgConstraint {
    /// Argument index (0-5).
    pub arg_index: u8,
    /// Comparison operation.
    pub op: ConstraintOp,
    /// Value to compare.
    pub value: u64,
}

/// Constraint comparison operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintOp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Ge,
    /// Bitwise AND with mask is non-zero.
    MaskedEq(u64),
}

/// Sandbox configuration.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Sandbox name.
    pub name: String,

    /// Associated security domain.
    pub domain: DomainId,

    /// Syscall filter rules.
    pub syscall_rules: Vec<SyscallRule>,

    /// Default action for unmatched syscalls.
    pub default_action: SyscallAction,

    /// Allow new user namespaces.
    pub allow_user_ns: bool,

    /// Allow network namespaces.
    pub allow_net_ns: bool,

    /// Allow PID namespaces.
    pub allow_pid_ns: bool,

    /// Allow mount namespaces.
    pub allow_mount_ns: bool,

    /// Allowed environment variables.
    pub allowed_env: Vec<String>,

    /// Read-only root filesystem.
    pub readonly_root: bool,

    /// Private /tmp.
    pub private_tmp: bool,

    /// No new privileges.
    pub no_new_privs: bool,
}

impl SandboxConfig {
    /// Create a minimal sandbox config.
    pub fn minimal(name: &str) -> Self {
        SandboxConfig {
            name: String::from(name),
            domain: DomainId::RENDERER,
            syscall_rules: Vec::new(),
            default_action: SyscallAction::Deny(-1),
            allow_user_ns: false,
            allow_net_ns: false,
            allow_pid_ns: false,
            allow_mount_ns: false,
            allowed_env: Vec::new(),
            readonly_root: true,
            private_tmp: true,
            no_new_privs: true,
        }
    }

    /// Create a renderer sandbox config.
    pub fn renderer(name: &str) -> Self {
        let mut config = Self::minimal(name);

        // Allow basic syscalls for renderers
        config.allow_syscall(0); // read
        config.allow_syscall(1); // write
        config.allow_syscall(3); // close
        config.allow_syscall(9); // mmap
        config.allow_syscall(10); // mprotect
        config.allow_syscall(11); // munmap
        config.allow_syscall(12); // brk
        config.allow_syscall(60); // exit
        config.allow_syscall(231); // exit_group

        // IPC syscalls (with restrictions)
        config.allow_syscall_with_args(100, vec![]); // Custom IPC send
        config.allow_syscall_with_args(101, vec![]); // Custom IPC recv

        config
    }

    /// Allow a syscall.
    pub fn allow_syscall(&mut self, nr: u64) {
        self.syscall_rules.push(SyscallRule {
            syscall_nr: nr,
            action: SyscallAction::Allow,
            arg_constraints: None,
        });
    }

    /// Allow syscall with argument constraints.
    pub fn allow_syscall_with_args(&mut self, nr: u64, constraints: Vec<ArgConstraint>) {
        self.syscall_rules.push(SyscallRule {
            syscall_nr: nr,
            action: SyscallAction::Allow,
            arg_constraints: if constraints.is_empty() {
                None
            } else {
                Some(constraints)
            },
        });
    }

    /// Deny a syscall.
    pub fn deny_syscall(&mut self, nr: u64, errno: i32) {
        self.syscall_rules.push(SyscallRule {
            syscall_nr: nr,
            action: SyscallAction::Deny(errno),
            arg_constraints: None,
        });
    }

    /// Audit a syscall.
    pub fn audit_syscall(&mut self, nr: u64) {
        self.syscall_rules.push(SyscallRule {
            syscall_nr: nr,
            action: SyscallAction::Audit,
            arg_constraints: None,
        });
    }
}

/// A sandbox instance.
pub struct Sandbox {
    /// Sandbox ID.
    pub id: SandboxId,

    /// Configuration.
    pub config: SandboxConfig,

    /// Current state.
    pub state: SandboxState,

    /// Processes in this sandbox.
    pub processes: BTreeSet<ProcessId>,

    /// Parent sandbox (if nested).
    pub parent: Option<SandboxId>,

    /// Child sandboxes.
    pub children: Vec<SandboxId>,

    /// Syscall count by number.
    pub syscall_counts: BTreeMap<u64, u64>,

    /// Denied syscall count.
    pub denied_count: u64,

    /// Creation timestamp.
    pub created_at: u64,
}

impl Sandbox {
    /// Create new sandbox.
    pub fn new(id: SandboxId, config: SandboxConfig) -> Self {
        Sandbox {
            id,
            config,
            state: SandboxState::Configuring,
            processes: BTreeSet::new(),
            parent: None,
            children: Vec::new(),
            syscall_counts: BTreeMap::new(),
            denied_count: 0,
            created_at: 0,
        }
    }

    /// Activate the sandbox.
    pub fn activate(&mut self) -> Result<(), SandboxError> {
        if self.state != SandboxState::Configuring {
            return Err(SandboxError::InvalidState);
        }
        self.state = SandboxState::Active;
        Ok(())
    }

    /// Add a process to the sandbox.
    pub fn add_process(&mut self, pid: ProcessId) -> Result<(), SandboxError> {
        if self.state != SandboxState::Active {
            return Err(SandboxError::InvalidState);
        }
        self.processes.insert(pid);
        Ok(())
    }

    /// Remove a process from the sandbox.
    pub fn remove_process(&mut self, pid: ProcessId) {
        self.processes.remove(&pid);
    }

    /// Check if process is in sandbox.
    pub fn contains_process(&self, pid: ProcessId) -> bool {
        self.processes.contains(&pid)
    }

    /// Filter a syscall.
    pub fn filter_syscall(&mut self, nr: u64, args: &[u64; 6]) -> SyscallAction {
        // Track syscall
        *self.syscall_counts.entry(nr).or_insert(0) += 1;

        // Find matching rule
        for rule in &self.config.syscall_rules {
            if rule.syscall_nr == nr {
                // Check argument constraints
                if let Some(constraints) = &rule.arg_constraints {
                    if !Self::check_constraints(args, constraints) {
                        continue;
                    }
                }

                if matches!(
                    rule.action,
                    SyscallAction::Deny(_) | SyscallAction::AuditDeny(_)
                ) {
                    self.denied_count += 1;
                }

                return rule.action;
            }
        }

        // Use default action
        if matches!(self.config.default_action, SyscallAction::Deny(_)) {
            self.denied_count += 1;
        }

        self.config.default_action
    }

    /// Check argument constraints.
    fn check_constraints(args: &[u64; 6], constraints: &[ArgConstraint]) -> bool {
        for constraint in constraints {
            let idx = constraint.arg_index as usize;
            if idx >= 6 {
                return false;
            }

            let arg = args[idx];
            let matches = match constraint.op {
                ConstraintOp::Eq => arg == constraint.value,
                ConstraintOp::Ne => arg != constraint.value,
                ConstraintOp::Lt => arg < constraint.value,
                ConstraintOp::Le => arg <= constraint.value,
                ConstraintOp::Gt => arg > constraint.value,
                ConstraintOp::Ge => arg >= constraint.value,
                ConstraintOp::MaskedEq(mask) => (arg & mask) == constraint.value,
            };

            if !matches {
                return false;
            }
        }

        true
    }

    /// Suspend the sandbox.
    pub fn suspend(&mut self) -> Result<(), SandboxError> {
        if self.state != SandboxState::Active {
            return Err(SandboxError::InvalidState);
        }
        self.state = SandboxState::Suspended;
        Ok(())
    }

    /// Resume the sandbox.
    pub fn resume(&mut self) -> Result<(), SandboxError> {
        if self.state != SandboxState::Suspended {
            return Err(SandboxError::InvalidState);
        }
        self.state = SandboxState::Active;
        Ok(())
    }

    /// Terminate the sandbox.
    pub fn terminate(&mut self) {
        self.state = SandboxState::Terminating;
        // TODO: Kill all processes
        self.state = SandboxState::Exited;
    }

    /// Get statistics.
    pub fn stats(&self) -> SandboxStats {
        SandboxStats {
            process_count: self.processes.len(),
            total_syscalls: self.syscall_counts.values().sum(),
            denied_syscalls: self.denied_count,
            unique_syscalls: self.syscall_counts.len(),
        }
    }
}

/// Sandbox statistics.
#[derive(Debug, Clone, Copy)]
pub struct SandboxStats {
    /// Number of processes.
    pub process_count: usize,
    /// Total syscalls.
    pub total_syscalls: u64,
    /// Denied syscalls.
    pub denied_syscalls: u64,
    /// Unique syscall numbers.
    pub unique_syscalls: usize,
}

/// Sandbox error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxError {
    /// Sandbox not found.
    NotFound,
    /// Invalid state for operation.
    InvalidState,
    /// Resource limit exceeded.
    ResourceLimit,
    /// Permission denied.
    PermissionDenied,
    /// Invalid configuration.
    InvalidConfig,
}

/// Sandbox manager.
pub struct SandboxManager {
    /// All sandboxes.
    sandboxes: BTreeMap<SandboxId, Mutex<Sandbox>>,
    /// Process to sandbox mapping.
    process_sandbox: BTreeMap<ProcessId, SandboxId>,
    /// Next sandbox ID.
    next_id: u32,
    /// Maximum sandboxes.
    max_sandboxes: usize,
}

impl SandboxManager {
    /// Create new sandbox manager.
    pub fn new() -> Self {
        SandboxManager {
            sandboxes: BTreeMap::new(),
            process_sandbox: BTreeMap::new(),
            next_id: 1,
            max_sandboxes: 256,
        }
    }

    /// Create a new sandbox.
    pub fn create(&mut self, config: SandboxConfig) -> Result<SandboxId, SandboxError> {
        if self.sandboxes.len() >= self.max_sandboxes {
            return Err(SandboxError::ResourceLimit);
        }

        let id = SandboxId(self.next_id);
        self.next_id += 1;

        let sandbox = Sandbox::new(id, config);

        crate::serial_println!(
            "[Sandbox] Created sandbox {} ({})",
            id.0,
            sandbox.config.name
        );

        self.sandboxes.insert(id, Mutex::new(sandbox));

        Ok(id)
    }

    /// Get sandbox.
    pub fn get(&self, id: SandboxId) -> Option<&Mutex<Sandbox>> {
        self.sandboxes.get(&id)
    }

    /// Activate sandbox.
    pub fn activate(&self, id: SandboxId) -> Result<(), SandboxError> {
        self.sandboxes
            .get(&id)
            .ok_or(SandboxError::NotFound)?
            .lock()
            .activate()
    }

    /// Add process to sandbox.
    pub fn add_process(&mut self, id: SandboxId, pid: ProcessId) -> Result<(), SandboxError> {
        // Add to sandbox
        self.sandboxes
            .get(&id)
            .ok_or(SandboxError::NotFound)?
            .lock()
            .add_process(pid)?;

        // Track mapping
        self.process_sandbox.insert(pid, id);

        Ok(())
    }

    /// Get sandbox for process.
    pub fn process_sandbox(&self, pid: ProcessId) -> Option<SandboxId> {
        self.process_sandbox.get(&pid).copied()
    }

    /// Filter syscall for process.
    pub fn filter_syscall(&self, pid: ProcessId, nr: u64, args: &[u64; 6]) -> SyscallAction {
        let sandbox_id = match self.process_sandbox.get(&pid) {
            Some(id) => *id,
            None => return SyscallAction::Allow, // No sandbox = allow
        };

        match self.sandboxes.get(&sandbox_id) {
            Some(sandbox) => sandbox.lock().filter_syscall(nr, args),
            None => SyscallAction::Allow,
        }
    }

    /// Remove process.
    pub fn remove_process(&mut self, pid: ProcessId) {
        if let Some(sandbox_id) = self.process_sandbox.remove(&pid) {
            if let Some(sandbox) = self.sandboxes.get(&sandbox_id) {
                sandbox.lock().remove_process(pid);
            }
        }
    }

    /// Destroy sandbox.
    pub fn destroy(&mut self, id: SandboxId) -> Result<(), SandboxError> {
        let sandbox = self.sandboxes.remove(&id).ok_or(SandboxError::NotFound)?;

        // Remove process mappings
        let processes: Vec<ProcessId> = sandbox.lock().processes.iter().copied().collect();
        for pid in processes {
            self.process_sandbox.remove(&pid);
        }

        crate::serial_println!("[Sandbox] Destroyed sandbox {}", id.0);

        Ok(())
    }
}

/// Global sandbox manager.
static SANDBOX_MANAGER: RwLock<Option<SandboxManager>> = RwLock::new(None);

/// Initialize sandbox manager.
pub fn init() {
    let mut mgr = SANDBOX_MANAGER.write();
    *mgr = Some(SandboxManager::new());
    crate::serial_println!("[Sandbox] Manager initialized");
}

/// Create sandbox.
pub fn create(config: SandboxConfig) -> Result<SandboxId, SandboxError> {
    SANDBOX_MANAGER
        .write()
        .as_mut()
        .ok_or(SandboxError::PermissionDenied)?
        .create(config)
}

/// Activate sandbox.
pub fn activate(id: SandboxId) -> Result<(), SandboxError> {
    SANDBOX_MANAGER
        .read()
        .as_ref()
        .ok_or(SandboxError::NotFound)?
        .activate(id)
}

/// Add process to sandbox.
pub fn add_process(id: SandboxId, pid: ProcessId) -> Result<(), SandboxError> {
    SANDBOX_MANAGER
        .write()
        .as_mut()
        .ok_or(SandboxError::PermissionDenied)?
        .add_process(id, pid)
}

/// Filter syscall.
pub fn filter_syscall(pid: ProcessId, nr: u64, args: &[u64; 6]) -> SyscallAction {
    SANDBOX_MANAGER
        .read()
        .as_ref()
        .map(|m| m.filter_syscall(pid, nr, args))
        .unwrap_or(SyscallAction::Allow)
}
