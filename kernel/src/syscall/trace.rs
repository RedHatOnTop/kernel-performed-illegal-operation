//! Linux Syscall Tracing and Debugging
//!
//! Provides comprehensive syscall tracing for debugging Linux binary
//! compatibility. When enabled, logs every syscall invocation with
//! its number, human-readable name, arguments, and return value.
//!
//! # Features
//!
//! - **Syscall tracing**: Log every syscall entry/exit with args and return value
//! - **Unknown syscall reporting**: Human-readable names for first encounter
//! - **Syscall statistics**: Per-syscall invocation counts for profiling
//! - **Configurable**: Enable/disable via `--linux-trace` boot flag or runtime toggle
//!
//! # Phase 7-4.6 (Job 20)

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// ═══════════════════════════════════════════════════════════════════════
// Trace configuration
// ═══════════════════════════════════════════════════════════════════════

/// Global flag controlling whether syscall tracing is active.
static TRACE_ENABLED: AtomicBool = AtomicBool::new(false);

/// Global flag controlling whether syscall statistics collection is active.
static STATS_ENABLED: AtomicBool = AtomicBool::new(true);

/// Enable syscall tracing.
pub fn enable_trace() {
    TRACE_ENABLED.store(true, Ordering::SeqCst);
    crate::serial_println!("[KPIO/Linux/Trace] Syscall tracing ENABLED");
}

/// Disable syscall tracing.
pub fn disable_trace() {
    TRACE_ENABLED.store(false, Ordering::SeqCst);
    crate::serial_println!("[KPIO/Linux/Trace] Syscall tracing DISABLED");
}

/// Check if tracing is enabled.
pub fn is_trace_enabled() -> bool {
    TRACE_ENABLED.load(Ordering::Relaxed)
}

/// Enable statistics collection.
pub fn enable_stats() {
    STATS_ENABLED.store(true, Ordering::SeqCst);
}

/// Disable statistics collection.
pub fn disable_stats() {
    STATS_ENABLED.store(false, Ordering::SeqCst);
}

/// Check if stats are enabled.
pub fn is_stats_enabled() -> bool {
    STATS_ENABLED.load(Ordering::Relaxed)
}

// ═══════════════════════════════════════════════════════════════════════
// Syscall name lookup table
// ═══════════════════════════════════════════════════════════════════════

/// Maximum syscall number we track (Linux x86_64 has ~450 syscalls;
/// we only need names for the ones we might encounter).
const MAX_TRACKED_SYSCALL: usize = 335;

/// Human-readable name for a Linux x86_64 syscall number.
///
/// Returns `None` for unknown/unrecognized numbers.
pub fn syscall_name(nr: u64) -> Option<&'static str> {
    match nr {
        0 => Some("read"),
        1 => Some("write"),
        2 => Some("open"),
        3 => Some("close"),
        4 => Some("stat"),
        5 => Some("fstat"),
        6 => Some("lstat"),
        7 => Some("poll"),
        8 => Some("lseek"),
        9 => Some("mmap"),
        10 => Some("mprotect"),
        11 => Some("munmap"),
        12 => Some("brk"),
        13 => Some("rt_sigaction"),
        14 => Some("rt_sigprocmask"),
        15 => Some("rt_sigreturn"),
        16 => Some("ioctl"),
        17 => Some("pread64"),
        18 => Some("pwrite64"),
        19 => Some("readv"),
        20 => Some("writev"),
        21 => Some("access"),
        22 => Some("pipe"),
        23 => Some("select"),
        24 => Some("sched_yield"),
        25 => Some("mremap"),
        26 => Some("msync"),
        27 => Some("mincore"),
        28 => Some("madvise"),
        29 => Some("shmget"),
        30 => Some("shmat"),
        31 => Some("shmctl"),
        32 => Some("dup"),
        33 => Some("dup2"),
        34 => Some("pause"),
        35 => Some("nanosleep"),
        36 => Some("getitimer"),
        37 => Some("alarm"),
        38 => Some("setitimer"),
        39 => Some("getpid"),
        40 => Some("sendfile"),
        41 => Some("socket"),
        42 => Some("connect"),
        43 => Some("accept"),
        44 => Some("sendto"),
        45 => Some("recvfrom"),
        46 => Some("sendmsg"),
        47 => Some("recvmsg"),
        48 => Some("shutdown"),
        49 => Some("bind"),
        50 => Some("listen"),
        51 => Some("getsockname"),
        52 => Some("getpeername"),
        53 => Some("socketpair"),
        54 => Some("setsockopt"),
        55 => Some("getsockopt"),
        56 => Some("clone"),
        57 => Some("fork"),
        58 => Some("vfork"),
        59 => Some("execve"),
        60 => Some("exit"),
        61 => Some("wait4"),
        62 => Some("kill"),
        63 => Some("uname"),
        64 => Some("semget"),
        65 => Some("semop"),
        66 => Some("semctl"),
        67 => Some("shmdt"),
        68 => Some("msgget"),
        69 => Some("msgsnd"),
        70 => Some("msgrcv"),
        71 => Some("msgctl"),
        72 => Some("fcntl"),
        73 => Some("flock"),
        74 => Some("fsync"),
        75 => Some("fdatasync"),
        76 => Some("truncate"),
        77 => Some("ftruncate"),
        78 => Some("getdents"),
        79 => Some("getcwd"),
        80 => Some("chdir"),
        81 => Some("fchdir"),
        82 => Some("rename"),
        83 => Some("mkdir"),
        84 => Some("rmdir"),
        85 => Some("creat"),
        86 => Some("link"),
        87 => Some("unlink"),
        88 => Some("symlink"),
        89 => Some("readlink"),
        90 => Some("chmod"),
        91 => Some("fchmod"),
        92 => Some("chown"),
        93 => Some("fchown"),
        94 => Some("lchown"),
        95 => Some("umask"),
        96 => Some("gettimeofday"),
        97 => Some("getrlimit"),
        98 => Some("getrusage"),
        99 => Some("sysinfo"),
        100 => Some("times"),
        101 => Some("ptrace"),
        102 => Some("getuid"),
        103 => Some("syslog"),
        104 => Some("getgid"),
        105 => Some("setuid"),
        106 => Some("setgid"),
        107 => Some("geteuid"),
        108 => Some("getegid"),
        109 => Some("setpgid"),
        110 => Some("getppid"),
        111 => Some("getpgrp"),
        112 => Some("setsid"),
        158 => Some("arch_prctl"),
        200 => Some("tkill"),
        201 => Some("time"),
        202 => Some("futex"),
        203 => Some("sched_setaffinity"),
        204 => Some("sched_getaffinity"),
        217 => Some("getdents64"),
        218 => Some("set_tid_address"),
        228 => Some("clock_gettime"),
        229 => Some("clock_getres"),
        230 => Some("clock_nanosleep"),
        231 => Some("exit_group"),
        232 => Some("epoll_wait"),
        233 => Some("epoll_ctl"),
        257 => Some("openat"),
        258 => Some("mkdirat"),
        259 => Some("mknodat"),
        260 => Some("fchownat"),
        262 => Some("newfstatat"),
        263 => Some("unlinkat"),
        264 => Some("renameat"),
        265 => Some("linkat"),
        266 => Some("symlinkat"),
        267 => Some("readlinkat"),
        268 => Some("fchmodat"),
        269 => Some("faccessat"),
        270 => Some("pselect6"),
        271 => Some("ppoll"),
        273 => Some("set_robust_list"),
        274 => Some("get_robust_list"),
        281 => Some("epoll_pwait"),
        284 => Some("eventfd"),
        288 => Some("accept4"),
        290 => Some("eventfd2"),
        291 => Some("epoll_create1"),
        292 => Some("dup3"),
        293 => Some("pipe2"),
        302 => Some("prlimit64"),
        318 => Some("getrandom"),
        332 => Some("statx"),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Syscall statistics
// ═══════════════════════════════════════════════════════════════════════

/// Per-syscall invocation counter.
///
/// We track counters for syscall numbers 0..MAX_TRACKED_SYSCALL.
/// Anything beyond that is counted in the "unknown" bucket.
struct SyscallStats {
    /// Counters for known syscall numbers [0..MAX_TRACKED_SYSCALL)
    counts: [AtomicU64; MAX_TRACKED_SYSCALL],
    /// Counter for unknown/out-of-range syscall numbers
    unknown_count: AtomicU64,
    /// Total syscall invocations
    total: AtomicU64,
}

/// We can't use `[AtomicU64::new(0); N]` in const context across all
/// Rust editions, so generate via a macro.
macro_rules! atomic_array {
    ($n:expr) => {{
        // Safety: AtomicU64 is repr(transparent) over u64, and 0 is valid.
        // This is the standard pattern for large const atomic arrays.
        unsafe {
            core::mem::transmute::<[u64; $n], [AtomicU64; $n]>([0u64; $n])
        }
    }};
}

static SYSCALL_STATS: SyscallStats = SyscallStats {
    counts: atomic_array!(MAX_TRACKED_SYSCALL),
    unknown_count: AtomicU64::new(0),
    total: AtomicU64::new(0),
};

/// Record a syscall invocation for statistics.
fn record_syscall(nr: u64) {
    if !is_stats_enabled() {
        return;
    }

    SYSCALL_STATS.total.fetch_add(1, Ordering::Relaxed);

    if (nr as usize) < MAX_TRACKED_SYSCALL {
        SYSCALL_STATS.counts[nr as usize].fetch_add(1, Ordering::Relaxed);
    } else {
        SYSCALL_STATS.unknown_count.fetch_add(1, Ordering::Relaxed);
    }
}

/// Get the invocation count for a specific syscall number.
pub fn get_syscall_count(nr: u64) -> u64 {
    if (nr as usize) < MAX_TRACKED_SYSCALL {
        SYSCALL_STATS.counts[nr as usize].load(Ordering::Relaxed)
    } else {
        0
    }
}

/// Get the total number of syscall invocations.
pub fn get_total_syscall_count() -> u64 {
    SYSCALL_STATS.total.load(Ordering::Relaxed)
}

/// Get the number of unknown syscall invocations.
pub fn get_unknown_syscall_count() -> u64 {
    SYSCALL_STATS.unknown_count.load(Ordering::Relaxed)
}

/// Reset all syscall statistics.
pub fn reset_stats() {
    for counter in SYSCALL_STATS.counts.iter() {
        counter.store(0, Ordering::Relaxed);
    }
    SYSCALL_STATS.unknown_count.store(0, Ordering::Relaxed);
    SYSCALL_STATS.total.store(0, Ordering::Relaxed);
}

// ═══════════════════════════════════════════════════════════════════════
// Trace entry/exit logging
// ═══════════════════════════════════════════════════════════════════════

/// Log a syscall entry (before execution).
///
/// Called from `linux_syscall_dispatch` when tracing is enabled.
pub fn trace_syscall_entry(nr: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) {
    let name = syscall_name(nr).unwrap_or("unknown");
    let pid = super::percpu::get_current_pid(0);

    crate::serial_println!(
        "[TRACE] pid={} {}({}) args=({:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x})",
        pid, name, nr, a1, a2, a3, a4, a5, a6
    );
}

/// Log a syscall exit (after execution).
///
/// Called from `linux_syscall_dispatch` when tracing is enabled.
pub fn trace_syscall_exit(nr: u64, result: i64) {
    let name = syscall_name(nr).unwrap_or("unknown");
    let pid = super::percpu::get_current_pid(0);

    if result < 0 {
        // Error return — show errno name
        let errno_name = errno_name(-result);
        crate::serial_println!(
            "[TRACE] pid={} {}({}) → {} ({})",
            pid, name, nr, result, errno_name
        );
    } else {
        crate::serial_println!(
            "[TRACE] pid={} {}({}) → {:#x} ({})",
            pid, name, nr, result, result
        );
    }
}

/// Log an unknown syscall on first encounter.
///
/// Prints a prominent warning with the human-readable name if available.
pub fn trace_unknown_syscall(nr: u64, a1: u64, a2: u64) {
    let name = syscall_name(nr).unwrap_or("???");
    let pid = super::percpu::get_current_pid(0);

    crate::serial_println!(
        "[KPIO/Linux] WARNING: Unimplemented syscall #{} ({}) from pid={} (a1={:#x}, a2={:#x})",
        nr, name, pid, a1, a2
    );
}

/// Combined trace + record function called from dispatch.
///
/// This is the main entry point for the tracing system.
pub fn on_syscall_entry(nr: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) {
    record_syscall(nr);

    if is_trace_enabled() {
        trace_syscall_entry(nr, a1, a2, a3, a4, a5, a6);
    }
}

/// Called after syscall dispatch with the result.
pub fn on_syscall_exit(nr: u64, result: i64) {
    if is_trace_enabled() {
        trace_syscall_exit(nr, result);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Errno name helper
// ═══════════════════════════════════════════════════════════════════════

/// Human-readable name for a Linux errno value.
fn errno_name(errno: i64) -> &'static str {
    match errno {
        1 => "EPERM",
        2 => "ENOENT",
        3 => "ESRCH",
        4 => "EINTR",
        5 => "EIO",
        6 => "ENXIO",
        7 => "E2BIG",
        8 => "ENOEXEC",
        9 => "EBADF",
        10 => "ECHILD",
        11 => "EAGAIN",
        12 => "ENOMEM",
        13 => "EACCES",
        14 => "EFAULT",
        17 => "EEXIST",
        19 => "ENODEV",
        20 => "ENOTDIR",
        21 => "EISDIR",
        22 => "EINVAL",
        23 => "ENFILE",
        24 => "EMFILE",
        25 => "ENOTTY",
        28 => "ENOSPC",
        29 => "ESPIPE",
        30 => "EROFS",
        32 => "EPIPE",
        34 => "ERANGE",
        36 => "ENAMETOOLONG",
        38 => "ENOSYS",
        39 => "ENOTEMPTY",
        _ => "E???",
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Statistics dump
// ═══════════════════════════════════════════════════════════════════════

/// Print a summary of syscall statistics to the serial console.
///
/// Shows all syscalls that have been invoked at least once, sorted by
/// invocation count (descending).
pub fn dump_stats() {
    crate::serial_println!("╔══════════════════════════════════════════════╗");
    crate::serial_println!("║      Linux Syscall Statistics Summary        ║");
    crate::serial_println!("╠══════════════════════════════════════════════╣");

    let total = get_total_syscall_count();
    let unknown = get_unknown_syscall_count();
    crate::serial_println!("║ Total invocations: {:<25} ║", total);
    crate::serial_println!("║ Unknown syscalls:  {:<25} ║", unknown);
    crate::serial_println!("╠══════════════════════════════════════════════╣");
    crate::serial_println!("║  # │ Name                │ Count            ║");
    crate::serial_println!("╠══════════════════════════════════════════════╣");

    // Collect non-zero entries
    let mut entries: alloc::vec::Vec<(u64, u64, &str)> = alloc::vec::Vec::new();
    for i in 0..MAX_TRACKED_SYSCALL {
        let count = SYSCALL_STATS.counts[i].load(Ordering::Relaxed);
        if count > 0 {
            let name = syscall_name(i as u64).unwrap_or("unknown");
            entries.push((i as u64, count, name));
        }
    }

    // Sort by count descending
    entries.sort_by(|a, b| b.1.cmp(&a.1));

    for (nr, count, name) in &entries {
        crate::serial_println!(
            "║ {:>3} │ {:<19} │ {:<16} ║",
            nr, name, count
        );
    }

    if unknown > 0 {
        crate::serial_println!(
            "║ ??? │ {:<19} │ {:<16} ║",
            "(unrecognized)", unknown
        );
    }

    crate::serial_println!("╚══════════════════════════════════════════════╝");
}

// ═══════════════════════════════════════════════════════════════════════
// Boot flag handling
// ═══════════════════════════════════════════════════════════════════════

/// Initialize the trace module, optionally enabling tracing.
///
/// Call this during kernel init. If the `--linux-trace` boot flag
/// is detected, tracing will be enabled automatically.
pub fn init(enable: bool) {
    if enable {
        enable_trace();
    }
    enable_stats();
    crate::serial_println!(
        "[KPIO/Linux/Trace] Initialized (trace={}, stats=enabled)",
        if enable { "enabled" } else { "disabled" }
    );
}

/// Check boot arguments for `--linux-trace` flag.
///
/// Returns `true` if the flag is present.
pub fn check_boot_flag(boot_args: &str) -> bool {
    boot_args.contains("--linux-trace")
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syscall_name_known() {
        assert_eq!(syscall_name(0), Some("read"));
        assert_eq!(syscall_name(1), Some("write"));
        assert_eq!(syscall_name(2), Some("open"));
        assert_eq!(syscall_name(3), Some("close"));
        assert_eq!(syscall_name(9), Some("mmap"));
        assert_eq!(syscall_name(12), Some("brk"));
        assert_eq!(syscall_name(39), Some("getpid"));
        assert_eq!(syscall_name(60), Some("exit"));
        assert_eq!(syscall_name(63), Some("uname"));
        assert_eq!(syscall_name(158), Some("arch_prctl"));
        assert_eq!(syscall_name(231), Some("exit_group"));
        assert_eq!(syscall_name(257), Some("openat"));
        assert_eq!(syscall_name(318), Some("getrandom"));
    }

    #[test]
    fn test_syscall_name_unknown() {
        assert_eq!(syscall_name(999), None);
        assert_eq!(syscall_name(500), None);
        assert_eq!(syscall_name(u64::MAX), None);
    }

    #[test]
    fn test_errno_name_known() {
        assert_eq!(errno_name(1), "EPERM");
        assert_eq!(errno_name(2), "ENOENT");
        assert_eq!(errno_name(9), "EBADF");
        assert_eq!(errno_name(12), "ENOMEM");
        assert_eq!(errno_name(14), "EFAULT");
        assert_eq!(errno_name(22), "EINVAL");
        assert_eq!(errno_name(38), "ENOSYS");
    }

    #[test]
    fn test_errno_name_unknown() {
        assert_eq!(errno_name(999), "E???");
    }

    #[test]
    fn test_trace_enable_disable() {
        // Start disabled
        disable_trace();
        assert!(!is_trace_enabled());

        // Enable
        enable_trace();
        assert!(is_trace_enabled());

        // Disable again
        disable_trace();
        assert!(!is_trace_enabled());
    }

    #[test]
    fn test_stats_enable_disable() {
        enable_stats();
        assert!(is_stats_enabled());

        disable_stats();
        assert!(!is_stats_enabled());

        enable_stats();
        assert!(is_stats_enabled());
    }

    #[test]
    fn test_record_and_get_syscall_count() {
        // Reset first
        reset_stats();
        enable_stats();

        // Record some syscalls
        record_syscall(0); // read
        record_syscall(0); // read
        record_syscall(1); // write
        record_syscall(60); // exit

        assert_eq!(get_syscall_count(0), 2);
        assert_eq!(get_syscall_count(1), 1);
        assert_eq!(get_syscall_count(60), 1);
        assert_eq!(get_syscall_count(99), 0);
        assert_eq!(get_total_syscall_count(), 4);
    }

    #[test]
    fn test_record_unknown_syscall() {
        reset_stats();
        enable_stats();

        record_syscall(999); // unknown / out of range
        assert_eq!(get_unknown_syscall_count(), 1);
        assert_eq!(get_total_syscall_count(), 1);
    }

    #[test]
    fn test_reset_stats() {
        reset_stats();
        enable_stats();

        record_syscall(0);
        record_syscall(1);
        assert!(get_total_syscall_count() > 0);

        reset_stats();
        assert_eq!(get_total_syscall_count(), 0);
        assert_eq!(get_syscall_count(0), 0);
        assert_eq!(get_syscall_count(1), 0);
        assert_eq!(get_unknown_syscall_count(), 0);
    }

    #[test]
    fn test_check_boot_flag() {
        assert!(check_boot_flag("--linux-trace"));
        assert!(check_boot_flag("quiet --linux-trace --debug"));
        assert!(!check_boot_flag(""));
        assert!(!check_boot_flag("--debug --verbose"));
    }

    #[test]
    fn test_stats_disabled_no_record() {
        reset_stats();
        disable_stats();

        record_syscall(0);
        record_syscall(1);

        assert_eq!(get_total_syscall_count(), 0);

        enable_stats(); // re-enable for other tests
    }

    #[test]
    fn test_all_tier_a_syscalls_have_names() {
        // All Tier A syscalls from the plan should have human-readable names
        let tier_a = [
            0, 1, 2, 3, 4, 5, 8, 9, 10, 11, 12, 13, 14, 16, 19, 20, 21, 22,
            28, 32, 33, 35, 39, 57, 59, 60, 62, 63, 72, 79, 80, 83, 87, 89,
            96, 102, 104, 107, 108, 158, 202, 217, 218, 228, 231, 257, 267,
            273, 293, 302, 318,
        ];

        for &nr in &tier_a {
            assert!(
                syscall_name(nr).is_some(),
                "Tier A syscall #{} should have a name",
                nr
            );
        }
    }

    #[test]
    fn test_on_syscall_entry_records_stats() {
        reset_stats();
        enable_stats();
        disable_trace(); // don't spam output

        on_syscall_entry(0, 0, 0, 0, 0, 0, 0);
        on_syscall_entry(1, 0, 0, 0, 0, 0, 0);
        on_syscall_entry(1, 0, 0, 0, 0, 0, 0);

        assert_eq!(get_syscall_count(0), 1);
        assert_eq!(get_syscall_count(1), 2);
        assert_eq!(get_total_syscall_count(), 3);
    }
}
