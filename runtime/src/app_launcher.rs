//! WASM app launcher — orchestrates loading, instantiating, and running a
//! `.kpioapp` package or a stand-alone `.wasm` binary.
//!
//! # Lifecycle
//!
//! 1. **Install** (optional): `KpioAppPackage::from_bytes()` → extract manifest,
//!    validate WASM, store in VFS.
//! 2. **Launch**: Parse WASM → create `WasiCtx` → register host functions →
//!    instantiate → call `_start`.
//! 3. **Terminate**: Capture exit code / trap → clean up instance.

use alloc::string::String;
use alloc::vec::Vec;

use crate::executor::{ExecutorContext, HostFunction};
use crate::host;
use crate::instance::Imports;
use crate::module::Module;
use crate::package::{AppManifest, KpioAppPackage};
use crate::wasi::WasiCtx;
use crate::RuntimeError;

// ---------------------------------------------------------------------------
// Launch configuration
// ---------------------------------------------------------------------------

/// Configuration for launching a WASM app.
#[derive(Debug, Clone)]
pub struct LaunchConfig {
    /// Command-line arguments visible to the WASM app.
    pub args: Vec<String>,
    /// Environment variables (`KEY=VALUE` pairs).
    pub env: Vec<String>,
    /// Preopened directories: `(guest_path, host_path)`.
    pub preopened_dirs: Vec<(String, String)>,
    /// Fuel limit (None = unlimited).
    pub fuel: Option<u64>,
    /// Whether to enable WASI support.
    pub enable_wasi: bool,
    /// Whether to register kpio host functions.
    pub enable_kpio_host: bool,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        LaunchConfig {
            args: Vec::new(),
            env: Vec::new(),
            preopened_dirs: Vec::new(),
            fuel: Some(100_000_000),
            enable_wasi: true,
            enable_kpio_host: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Launch result
// ---------------------------------------------------------------------------

/// Result of a WASM app execution.
#[derive(Debug, Clone)]
pub struct LaunchResult {
    /// Exit code (0 = success, non-zero = failure).
    pub exit_code: i32,
    /// Captured stdout output.
    pub stdout: Vec<u8>,
    /// Captured stderr output.
    pub stderr: Vec<u8>,
    /// Whether execution terminated due to a trap.
    pub trapped: bool,
    /// Trap message, if any.
    pub trap_message: Option<String>,
}

// ---------------------------------------------------------------------------
// Launcher
// ---------------------------------------------------------------------------

/// Launch a WASM binary with the given configuration.
///
/// This is the primary entry point for running `.wasm` files directly.
pub fn launch_wasm(wasm_bytes: &[u8], config: &LaunchConfig) -> Result<LaunchResult, RuntimeError> {
    // 1. Parse & validate module
    let module = Module::from_bytes(wasm_bytes)?;

    // 2. Build host functions
    let host_fns = build_host_functions(&module, config);

    // 3. Create executor context
    let mut ctx = ExecutorContext::new_with_host_functions(module, host_fns)
        .map_err(|e| RuntimeError::InstantiationError(alloc::format!("{}", e)))?;

    // 4. Configure fuel
    ctx.fuel = config.fuel;

    // 5. Set up WASI context
    if config.enable_wasi {
        let mut wasi = WasiCtx::new();
        if !config.args.is_empty() {
            wasi.set_args(config.args.clone());
        }
        for env_var in &config.env {
            if let Some(eq_pos) = env_var.find('=') {
                let key = &env_var[..eq_pos];
                let val = &env_var[eq_pos + 1..];
                wasi.set_env(key, val);
            }
        }
        for (guest, _host) in &config.preopened_dirs {
            wasi.preopen_dir(guest.as_str());
        }
        ctx.wasi_ctx = Some(wasi);
    }

    // 6. Execute _start (or main entry)
    let entry = find_entry_export(&ctx);
    match crate::executor::execute_export(&mut ctx, &entry, &[]) {
        Ok(_) => {
            let exit_code = ctx.exit_code.unwrap_or(0);
            Ok(LaunchResult {
                exit_code,
                stdout: ctx.stdout.clone(),
                stderr: ctx.stderr.clone(),
                trapped: false,
                trap_message: None,
            })
        }
        Err(trap) => {
            let exit_code = ctx.exit_code.unwrap_or(1);
            // ProcessExit is not really a trap — it's the normal exit path
            let is_exit = matches!(trap, crate::interpreter::TrapError::ProcessExit(_));
            Ok(LaunchResult {
                exit_code,
                stdout: ctx.stdout.clone(),
                stderr: ctx.stderr.clone(),
                trapped: !is_exit,
                trap_message: if is_exit {
                    None
                } else {
                    Some(alloc::format!("{}", trap))
                },
            })
        }
    }
}

/// Launch a `.kpioapp` package.
pub fn launch_kpioapp(
    package_bytes: &[u8],
    extra_config: Option<&LaunchConfig>,
) -> Result<LaunchResult, RuntimeError> {
    let pkg = KpioAppPackage::from_bytes(package_bytes)
        .map_err(|e| RuntimeError::InvalidBinary(alloc::format!("{:?}", e)))?;

    let mut config = extra_config.cloned().unwrap_or_default();

    // Inject app name as first arg if no args provided
    if config.args.is_empty() {
        config.args.push(pkg.manifest.name.clone());
    }

    // Add KPIO_APP_ID env var
    config
        .env
        .push(alloc::format!("KPIO_APP_ID={}", pkg.manifest.id));

    // Set permissions-based flags
    config.enable_kpio_host = pkg.manifest.permissions.gui
        || pkg.manifest.permissions.network
        || pkg.manifest.permissions.clipboard
        || pkg.manifest.permissions.notifications;

    // Initialize data files into WASI VFS if any
    launch_wasm(&pkg.wasm_bytes, &config)
}

/// Install a `.kpioapp` package. Returns the parsed manifest for registration.
pub fn install_kpioapp(package_bytes: &[u8]) -> Result<AppManifest, RuntimeError> {
    let pkg = KpioAppPackage::from_bytes(package_bytes)
        .map_err(|e| RuntimeError::InvalidBinary(alloc::format!("{:?}", e)))?;
    Ok(pkg.manifest)
}

/// Uninstall a WASM app by ID (cleanup marker — actual file removal is
/// handled by the kernel VFS layer).
pub fn uninstall_kpioapp(app_id: &str) -> Result<(), RuntimeError> {
    // In a full implementation this would call kernel syscalls to:
    // 1. Terminate running instance
    // 2. Remove /apps/data/{app_id}/
    // 3. Deregister from app registry
    // For now we just validate the ID is non-empty.
    if app_id.is_empty() {
        return Err(RuntimeError::InstantiationError(String::from(
            "empty app_id",
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// App Update Mechanism
// ---------------------------------------------------------------------------

/// Action resulting from an update check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateAction {
    /// The installed version is current.
    UpToDate,
    /// A newer version is available.
    UpdateAvailable {
        /// The new version string.
        new_version: String,
    },
    /// The candidate version is older than the installed version.
    OlderVersion {
        /// The candidate version string.
        candidate_version: String,
    },
    /// The candidate and installed app IDs do not match.
    IncompatibleApp,
}

/// Check whether a candidate package is an update for the installed app.
///
/// Compares the app IDs (must match) and version strings using semver-like
/// ordering (major.minor.patch numeric comparison).
pub fn check_update(
    installed: &AppManifest,
    candidate_bytes: &[u8],
) -> Result<UpdateAction, RuntimeError> {
    let candidate_pkg = KpioAppPackage::from_bytes(candidate_bytes)
        .map_err(|e| RuntimeError::InvalidBinary(alloc::format!("{:?}", e)))?;
    let candidate = &candidate_pkg.manifest;

    // App IDs must match
    if installed.id != candidate.id {
        return Ok(UpdateAction::IncompatibleApp);
    }

    // Compare versions
    let installed_v = parse_semver(&installed.version);
    let candidate_v = parse_semver(&candidate.version);

    match compare_semver(&installed_v, &candidate_v) {
        core::cmp::Ordering::Less => Ok(UpdateAction::UpdateAvailable {
            new_version: candidate.version.clone(),
        }),
        core::cmp::Ordering::Equal => Ok(UpdateAction::UpToDate),
        core::cmp::Ordering::Greater => Ok(UpdateAction::OlderVersion {
            candidate_version: candidate.version.clone(),
        }),
    }
}

/// Check whether a candidate manifest is an update (without needing package bytes).
pub fn check_update_manifest(
    installed: &AppManifest,
    candidate: &AppManifest,
) -> UpdateAction {
    if installed.id != candidate.id {
        return UpdateAction::IncompatibleApp;
    }

    let installed_v = parse_semver(&installed.version);
    let candidate_v = parse_semver(&candidate.version);

    match compare_semver(&installed_v, &candidate_v) {
        core::cmp::Ordering::Less => UpdateAction::UpdateAvailable {
            new_version: candidate.version.clone(),
        },
        core::cmp::Ordering::Equal => UpdateAction::UpToDate,
        core::cmp::Ordering::Greater => UpdateAction::OlderVersion {
            candidate_version: candidate.version.clone(),
        },
    }
}

/// Parse a "major.minor.patch" version string into a tuple of u32s.
/// Non-numeric or missing parts default to 0.
fn parse_semver(version: &str) -> (u32, u32, u32) {
    let mut parts = version.split('.');
    let major = parts
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let minor = parts
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let patch = parts
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (major, minor, patch)
}

/// Compare two semver tuples.
fn compare_semver(a: &(u32, u32, u32), b: &(u32, u32, u32)) -> core::cmp::Ordering {
    a.0.cmp(&b.0)
        .then(a.1.cmp(&b.1))
        .then(a.2.cmp(&b.2))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build host function list based on the module's imports and the launch config.
fn build_host_functions(module: &Module, config: &LaunchConfig) -> Vec<HostFunction> {
    let mut imports = Imports::new();
    if config.enable_wasi {
        host::register_all(&mut imports);
    }
    imports.to_exec_host_functions()
}

/// Find the entry export function name.
fn find_entry_export(ctx: &ExecutorContext) -> String {
    // Check common WASI/WASM entry points in priority order
    for name in &["_start", "main", "_initialize", "run"] {
        if ctx.module.find_export(name).is_some() {
            return String::from(*name);
        }
    }
    // Fallback: use the first exported function
    for export in &ctx.module.exports {
        if export.kind == crate::module::ExportKind::Function {
            return export.name.clone();
        }
    }
    String::from("_start")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{FsPermission, ManifestPermissions};

    fn make_manifest(id: &str, version: &str) -> AppManifest {
        AppManifest {
            id: String::from(id),
            name: String::from("Test App"),
            version: String::from(version),
            description: None,
            author: None,
            icon: None,
            entry: String::from("app.wasm"),
            permissions: ManifestPermissions::default(),
            min_kpio_version: None,
        }
    }

    #[test]
    fn test_launch_config_default() {
        let cfg = LaunchConfig::default();
        assert!(cfg.args.is_empty());
        assert!(cfg.enable_wasi);
        assert!(cfg.enable_kpio_host);
    }

    #[test]
    fn test_find_entry_fails_gracefully() {
        // Minimal WASM module (magic + version only, 8 bytes)
        let wasm = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        let module = Module::from_bytes(&wasm).unwrap();
        let ctx = ExecutorContext::new(module).unwrap();
        let entry = find_entry_export(&ctx);
        assert_eq!(entry, "_start");
    }

    #[test]
    fn test_uninstall_empty_id_error() {
        let result = uninstall_kpioapp("");
        assert!(result.is_err());
    }

    // ── Update mechanism tests ──────────────────────────────────────

    #[test]
    fn test_check_update_same_version() {
        let installed = make_manifest("com.test.app", "1.0.0");
        let candidate = make_manifest("com.test.app", "1.0.0");
        let action = check_update_manifest(&installed, &candidate);
        assert_eq!(action, UpdateAction::UpToDate);
    }

    #[test]
    fn test_check_update_newer_version() {
        let installed = make_manifest("com.test.app", "1.0.0");
        let candidate = make_manifest("com.test.app", "2.0.0");
        let action = check_update_manifest(&installed, &candidate);
        assert_eq!(
            action,
            UpdateAction::UpdateAvailable {
                new_version: String::from("2.0.0")
            }
        );
    }

    #[test]
    fn test_check_update_older_version() {
        let installed = make_manifest("com.test.app", "3.0.0");
        let candidate = make_manifest("com.test.app", "1.0.0");
        let action = check_update_manifest(&installed, &candidate);
        assert_eq!(
            action,
            UpdateAction::OlderVersion {
                candidate_version: String::from("1.0.0")
            }
        );
    }

    #[test]
    fn test_check_update_incompatible_app() {
        let installed = make_manifest("com.test.app1", "1.0.0");
        let candidate = make_manifest("com.test.app2", "1.0.0");
        let action = check_update_manifest(&installed, &candidate);
        assert_eq!(action, UpdateAction::IncompatibleApp);
    }

    #[test]
    fn test_check_update_minor_bump() {
        let installed = make_manifest("com.test.app", "1.2.3");
        let candidate = make_manifest("com.test.app", "1.3.0");
        let action = check_update_manifest(&installed, &candidate);
        assert_eq!(
            action,
            UpdateAction::UpdateAvailable {
                new_version: String::from("1.3.0")
            }
        );
    }

    #[test]
    fn test_check_update_patch_bump() {
        let installed = make_manifest("com.test.app", "1.0.0");
        let candidate = make_manifest("com.test.app", "1.0.1");
        let action = check_update_manifest(&installed, &candidate);
        assert_eq!(
            action,
            UpdateAction::UpdateAvailable {
                new_version: String::from("1.0.1")
            }
        );
    }

    // ── SemVer parsing tests ────────────────────────────────────────

    #[test]
    fn test_parse_semver_full() {
        assert_eq!(parse_semver("1.2.3"), (1, 2, 3));
    }

    #[test]
    fn test_parse_semver_partial() {
        assert_eq!(parse_semver("1.2"), (1, 2, 0));
        assert_eq!(parse_semver("1"), (1, 0, 0));
    }

    #[test]
    fn test_parse_semver_empty() {
        assert_eq!(parse_semver(""), (0, 0, 0));
    }

    #[test]
    fn test_compare_semver() {
        use core::cmp::Ordering;
        assert_eq!(
            compare_semver(&(1, 0, 0), &(1, 0, 0)),
            Ordering::Equal
        );
        assert_eq!(
            compare_semver(&(1, 0, 0), &(2, 0, 0)),
            Ordering::Less
        );
        assert_eq!(
            compare_semver(&(2, 0, 0), &(1, 0, 0)),
            Ordering::Greater
        );
        assert_eq!(
            compare_semver(&(1, 1, 0), &(1, 2, 0)),
            Ordering::Less
        );
    }
}
