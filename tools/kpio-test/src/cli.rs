use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::output::OutputFormat;

/// Stateless cross-platform CLI tool for managing QEMU boot testing of KPIO OS.
#[derive(Parser, Debug)]
#[command(name = "kpio-test", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Output format for all subcommands.
    #[arg(long, value_enum, default_value = "human", global = true)]
    pub output: OutputFormat,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a new QEMU instance as a background process.
    Create(CreateArgs),

    /// List all managed instances.
    List,

    /// Query the status of a specific instance.
    Status(NameArg),

    /// Read serial console output from an instance.
    Serial(SerialArgs),

    /// Search serial log for lines matching a regex pattern.
    SerialGrep(SerialGrepArgs),

    /// Block until a serial pattern appears or timeout elapses.
    WaitFor(WaitForArgs),

    /// Send a text command to the serial console.
    SendCommand(SendCommandArgs),

    /// Capture a screenshot of a running instance.
    Screenshot(ScreenshotArgs),

    /// Configure periodic screenshot capture.
    ScreenshotInterval(ScreenshotIntervalArgs),

    /// Compare a captured screenshot against a reference image.
    CompareScreenshot(CompareScreenshotArgs),

    /// Extract text from the guest display via OCR.
    ScreenOcr(ScreenOcrArgs),

    /// Send keyboard key press/release events to an instance.
    SendKey(SendKeyArgs),

    /// Type a text string as sequential key events.
    TypeText(TypeTextArgs),

    /// Send a mouse click at specific coordinates.
    MouseClick(MouseClickArgs),

    /// Move the mouse to specific coordinates.
    MouseMove(MouseMoveArgs),

    /// Save, restore, list, or delete VM state snapshots.
    Snapshot(SnapshotArgs),

    /// Query guest VM configuration and runtime information.
    GuestInfo(NameArg),

    /// Configure host-guest port forwarding.
    PortForward(PortForwardArgs),

    /// Copy a file from the host into the guest shared directory.
    CopyTo(CopyToArgs),

    /// Copy a file from the guest shared directory to the host.
    CopyFrom(CopyFromArgs),

    /// Retrieve QEMU process log output.
    Logs(LogsArgs),

    /// Build the kernel and create the UEFI disk image.
    Build(BuildArgs),

    /// Verify test results against a manifest.
    Verify(VerifyArgs),

    /// Run pre-flight health checks without creating an instance.
    Health,

    /// Destroy all managed instances.
    DestroyAll,

    /// Destroy a specific instance and clean up resources.
    Destroy(NameArg),

    /// Show help for a subcommand.
    #[command(name = "help-cmd", hide = true)]
    HelpCmd(HelpArgs),
}

// ── Shared argument structs ──────────────────────────────────────────

/// Argument struct for subcommands that only need an instance name.
#[derive(clap::Args, Debug)]
pub struct NameArg {
    /// Instance name.
    pub name: String,
}

// ── create ───────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct CreateArgs {
    /// Instance name (must be unique).
    pub name: String,

    /// Path to the disk image (overrides default build output).
    #[arg(long)]
    pub image: Option<PathBuf>,

    /// Launch with a VNC display backend instead of headless.
    #[arg(long, default_value_t = false)]
    pub gui: bool,

    /// Memory size (e.g. "512M", "1G").
    #[arg(long, default_value = "512M")]
    pub memory: String,

    /// Watchdog timeout in seconds.
    #[arg(long, default_value_t = 120)]
    pub timeout: u64,

    /// Attach a VirtIO network device with user-mode networking.
    #[arg(long, default_value_t = false)]
    pub virtio_net: bool,

    /// Attach a VirtIO block device with the specified disk image.
    #[arg(long)]
    pub virtio_blk: Option<PathBuf>,

    /// Map a host directory into the guest via VirtIO-9p.
    #[arg(long)]
    pub shared_dir: Option<PathBuf>,

    /// Additional QEMU arguments passed through verbatim.
    #[arg(long)]
    pub extra_args: Vec<String>,
}

// ── serial ───────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct SerialArgs {
    /// Instance name.
    pub name: String,

    /// Return only the last N lines.
    #[arg(long)]
    pub tail: Option<usize>,
}

// ── serial-grep ──────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct SerialGrepArgs {
    /// Instance name.
    pub name: String,

    /// Regex pattern to search for.
    #[arg(long)]
    pub pattern: String,

    /// Return only the match count.
    #[arg(long, default_value_t = false)]
    pub count: bool,

    /// Return only the first match.
    #[arg(long, default_value_t = false)]
    pub first: bool,

    /// Return only the last match.
    #[arg(long, default_value_t = false)]
    pub last: bool,
}

// ── wait-for ─────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct WaitForArgs {
    /// Instance name.
    pub name: String,

    /// Pattern to wait for (literal substring by default).
    #[arg(long)]
    pub pattern: String,

    /// Maximum seconds to wait (required, no indefinite wait).
    #[arg(long)]
    pub timeout: u64,

    /// Interpret the pattern as a regex.
    #[arg(long, default_value_t = false)]
    pub regex: bool,

    /// Poll interval in milliseconds.
    #[arg(long, default_value_t = 100)]
    pub interval: u64,
}

// ── send-command ─────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct SendCommandArgs {
    /// Instance name.
    pub name: String,

    /// Text to send to the serial console (newline appended automatically).
    pub text: String,
}

// ── screenshot ───────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct ScreenshotArgs {
    /// Instance name.
    pub name: String,

    /// Directory to save the screenshot.
    #[arg(long)]
    pub output: PathBuf,
}

// ── screenshot-interval ──────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct ScreenshotIntervalArgs {
    /// Instance name.
    pub name: String,

    /// Capture interval in milliseconds (0 to disable).
    #[arg(long)]
    pub interval: u64,

    /// Directory to save periodic screenshots.
    #[arg(long)]
    pub output: PathBuf,
}

// ── compare-screenshot ───────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct CompareScreenshotArgs {
    /// Path to the captured screenshot.
    pub captured: PathBuf,

    /// Path to the reference image.
    pub reference: PathBuf,

    /// Similarity threshold (0.0–1.0).
    #[arg(long, default_value_t = 0.95)]
    pub threshold: f64,

    /// Comparison mode.
    #[arg(long, value_enum, default_value = "perceptual")]
    pub mode: CompareMode,

    /// Crop region before comparison: x,y,width,height.
    #[arg(long)]
    pub region: Option<String>,

    /// Copy captured image to reference path instead of comparing.
    #[arg(long, default_value_t = false)]
    pub update_reference: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum CompareMode {
    Perceptual,
    PixelExact,
}

// ── screen-ocr ───────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct ScreenOcrArgs {
    /// Instance name.
    pub name: String,

    /// Crop region for OCR: x,y,width,height.
    #[arg(long)]
    pub region: Option<String>,
}

// ── send-key ─────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct SendKeyArgs {
    /// Instance name.
    pub name: String,

    /// Key names to send (e.g. "ret", "ctrl-c").
    pub keys: Vec<String>,
}

// ── type-text ────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct TypeTextArgs {
    /// Instance name.
    pub name: String,

    /// Text string to type as sequential key events.
    pub text: String,
}

// ── mouse-click ──────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct MouseClickArgs {
    /// Instance name.
    pub name: String,

    /// X coordinate.
    #[arg(long)]
    pub x: u32,

    /// Y coordinate.
    #[arg(long)]
    pub y: u32,

    /// Mouse button (default: left).
    #[arg(long, default_value = "left")]
    pub button: String,
}

// ── mouse-move ───────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct MouseMoveArgs {
    /// Instance name.
    pub name: String,

    /// X coordinate.
    #[arg(long)]
    pub x: u32,

    /// Y coordinate.
    #[arg(long)]
    pub y: u32,
}

// ── snapshot ─────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct SnapshotArgs {
    /// Instance name.
    pub name: String,

    /// Save a snapshot with the given tag.
    #[arg(long)]
    pub save: Option<String>,

    /// Restore a snapshot by tag.
    #[arg(long)]
    pub restore: Option<String>,

    /// List all snapshots.
    #[arg(long, default_value_t = false)]
    pub list: bool,

    /// Delete a snapshot by tag.
    #[arg(long)]
    pub delete: Option<String>,
}

// ── port-forward ─────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct PortForwardArgs {
    /// Instance name.
    pub name: String,

    /// Host port to forward.
    #[arg(long)]
    pub host: Option<u16>,

    /// Guest port to forward to.
    #[arg(long)]
    pub guest: Option<u16>,

    /// Protocol (tcp or udp).
    #[arg(long, default_value = "tcp")]
    pub protocol: String,

    /// List active port forwarding rules.
    #[arg(long, default_value_t = false)]
    pub list: bool,

    /// Remove a port forwarding rule by host port.
    #[arg(long, default_value_t = false)]
    pub remove: bool,
}

// ── copy-to ──────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct CopyToArgs {
    /// Instance name.
    pub name: String,

    /// Source file path on the host.
    #[arg(long)]
    pub src: PathBuf,

    /// Destination path in the guest shared directory.
    #[arg(long)]
    pub dest: PathBuf,
}

// ── copy-from ────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct CopyFromArgs {
    /// Instance name.
    pub name: String,

    /// Source path in the guest shared directory.
    #[arg(long)]
    pub src: PathBuf,

    /// Destination file path on the host.
    #[arg(long)]
    pub dest: PathBuf,
}

// ── logs ─────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct LogsArgs {
    /// Instance name.
    pub name: String,

    /// Return only the last N lines.
    #[arg(long)]
    pub tail: Option<usize>,
}

// ── build ────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct BuildArgs {
    /// Build with the release profile.
    #[arg(long, default_value_t = false)]
    pub release: bool,
}

// ── verify ───────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct VerifyArgs {
    /// Instance name.
    pub name: String,

    /// Path to the TOML test manifest.
    #[arg(long)]
    pub manifest: PathBuf,

    /// Run only the specified test suite from the manifest.
    #[arg(long)]
    pub mode: Option<String>,
}

// ── help ─────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct HelpArgs {
    /// Subcommand to show help for (omit for overview).
    pub subcommand: Option<String>,
}
