use std::path::PathBuf;
use std::process::ExitCode;

/// All errors produced by kpio-test.
///
/// Variants are split into two categories:
/// - **Infrastructure errors** (exit code 2): missing binaries, bad config, I/O failures
/// - **Operational errors** (exit code 1): runtime failures the user can act on
#[derive(thiserror::Error, Debug)]
pub enum KpioTestError {
    // ── Infrastructure errors (exit code 2) ──────────────────────────

    #[error("QEMU binary not found: {hint}")]
    QemuNotFound { hint: String },

    #[error("OVMF firmware not found: {hint}")]
    OvmfNotFound { hint: String },

    #[error("Disk image not found: {path}")]
    ImageNotFound { path: PathBuf },

    #[error("Manifest parse error: {0}")]
    ManifestParseError(String),

    #[error("OCR engine not available: {hint}")]
    OcrNotAvailable { hint: String },

    #[error("Snapshot requires qcow2 format")]
    SnapshotRequiresQcow2,

    #[error("Shared directory required for file transfer")]
    SharedDirRequired,

    #[error("Network device required for port forwarding")]
    NetworkDeviceRequired,

    #[error("Unknown subcommand: {name}")]
    UnknownSubcommand { name: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // ── Operational errors (exit code 1) ─────────────────────────────

    #[error("Instance not found: {name}")]
    InstanceNotFound { name: String },

    #[error("Instance not running: {name}")]
    InstanceNotRunning { name: String },

    #[error("Instance name conflict: {name}")]
    NameConflict { name: String },

    #[error("QMP error: {desc}")]
    QmpError { desc: String },

    #[error("QMP timeout after {seconds}s")]
    QmpTimeout { seconds: u64 },

    #[error("Wait-for timeout: pattern not found within {seconds}s")]
    WaitForTimeout { seconds: u64 },

    #[error("Verification failed: {fail_count} checks failed")]
    VerificationFailed { fail_count: usize },

    #[error("Build failed: {message}")]
    BuildFailed { message: String },

    #[error("Snapshot not found: {tag}")]
    SnapshotNotFound { tag: String },

    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },
}

impl KpioTestError {
    /// Map each error variant to its process exit code.
    ///
    /// - `2` — infrastructure error (missing prerequisite, bad config, I/O)
    /// - `1` — operational failure (user-actionable runtime error)
    pub fn exit_code(&self) -> ExitCode {
        match self {
            // Infrastructure errors → 2
            Self::QemuNotFound { .. }
            | Self::OvmfNotFound { .. }
            | Self::ImageNotFound { .. }
            | Self::ManifestParseError(_)
            | Self::OcrNotAvailable { .. }
            | Self::SnapshotRequiresQcow2
            | Self::SharedDirRequired
            | Self::NetworkDeviceRequired
            | Self::UnknownSubcommand { .. }
            | Self::Io(_)
            | Self::Json(_) => ExitCode::from(2),

            // Operational errors → 1
            Self::InstanceNotFound { .. }
            | Self::InstanceNotRunning { .. }
            | Self::NameConflict { .. }
            | Self::QmpError { .. }
            | Self::QmpTimeout { .. }
            | Self::WaitForTimeout { .. }
            | Self::VerificationFailed { .. }
            | Self::BuildFailed { .. }
            | Self::SnapshotNotFound { .. }
            | Self::FileNotFound { .. } => ExitCode::from(1),
        }
    }
}
