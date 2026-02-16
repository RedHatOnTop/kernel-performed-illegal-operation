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
}
