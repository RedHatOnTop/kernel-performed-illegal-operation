use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;

use crate::cli::BuildArgs;
use crate::error::KpioTestError;

/// Output returned on a successful build.
#[derive(Debug, Serialize)]
pub struct BuildOutput {
    /// Path to the built kernel binary.
    pub kernel_path: PathBuf,
    /// Path to the UEFI disk image.
    pub image_path: PathBuf,
}

/// Build the kernel and create the UEFI disk image.
///
/// 1. Runs `cargo build -p kpio-kernel` (with `--release` if requested).
/// 2. Runs the image builder (`tools/boot`) passing the kernel binary path.
/// 3. Returns paths to the built artifacts.
pub fn build(args: BuildArgs) -> Result<serde_json::Value, KpioTestError> {
    let profile = if args.release { "release" } else { "debug" };

    // ── Step 1: Build the kernel ─────────────────────────────────────
    let mut cargo_cmd = Command::new("cargo");
    cargo_cmd.args(["build", "-p", "kpio-kernel"]);
    if args.release {
        cargo_cmd.arg("--release");
    }

    let cargo_output = cargo_cmd.output()?;

    if !cargo_output.status.success() {
        let stderr = String::from_utf8_lossy(&cargo_output.stderr);
        return Err(KpioTestError::BuildFailed {
            message: format!("cargo build failed:\n{stderr}"),
        });
    }

    // ── Step 2: Determine artifact paths ─────────────────────────────
    let target_dir = PathBuf::from(format!("target/x86_64-unknown-none/{profile}"));
    let kernel_path = target_dir.join("kpio-kernel");
    let image_path = target_dir.join("kpio-uefi.img");

    // ── Step 3: Run the image builder ────────────────────────────────
    let mut boot_cmd = Command::new("cargo");
    boot_cmd
        .args(["run", "--release", "--manifest-path", "tools/boot/Cargo.toml", "--"])
        .arg(&kernel_path);

    let boot_output = boot_cmd.output()?;

    if !boot_output.status.success() {
        let stderr = String::from_utf8_lossy(&boot_output.stderr);
        return Err(KpioTestError::BuildFailed {
            message: format!("image builder failed:\n{stderr}"),
        });
    }

    // ── Step 4: Return artifact paths ────────────────────────────────
    let output = BuildOutput {
        kernel_path,
        image_path,
    };

    serde_json::to_value(&output).map_err(KpioTestError::Json)
}
